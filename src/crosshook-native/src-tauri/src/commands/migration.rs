use std::path::{Path, PathBuf};

use crosshook_core::metadata::{MetadataStore, SyncSource};
use crosshook_core::profile::health::{check_profile_health, HealthStatus};
use crosshook_core::profile::migration::{
    apply_single_migration, scan_proton_migrations, ApplyMigrationRequest, BatchMigrationRequest,
    BatchMigrationResult, MigrationApplyResult, MigrationOutcome, MigrationScanResult,
};
use crosshook_core::profile::ProfileStore;
use crosshook_core::steam::discover_steam_root_candidates;
use tauri::State;

use super::shared::sanitize_display_path;
use super::steam::default_steam_client_install_path;

/// Scans all profiles for stale Proton paths and returns migration suggestions.
///
/// The `steam_client_install_path` argument is treated as advisory (A-5): if the
/// provided path does not contain a `steamapps` subdirectory, the command falls
/// back to `default_steam_client_install_path()`. All path strings in the result
/// are sanitized with `sanitize_display_path()` before returning (A-4).
///
/// This command is read-only — it performs no writes and requires no `MetadataStore`.
#[tauri::command]
pub fn check_proton_migrations(
    steam_client_install_path: Option<String>,
    store: State<'_, ProfileStore>,
) -> Result<MigrationScanResult, String> {
    // A-5: validate caller-supplied steam path; fall back to default if invalid.
    let resolved_path = steam_client_install_path
        .filter(|p| {
            let candidate = PathBuf::from(p.trim());
            !candidate.as_os_str().is_empty() && candidate.join("steamapps").is_dir()
        })
        .unwrap_or_else(default_steam_client_install_path);

    let mut diagnostics = Vec::new();
    let steam_root_candidates = discover_steam_root_candidates(resolved_path, &mut diagnostics);
    let mut result = scan_proton_migrations(&store, &steam_root_candidates, &mut diagnostics);

    // A-4: sanitize all IPC-bound path strings before returning.
    for suggestion in &mut result.suggestions {
        suggestion.old_path = sanitize_display_path(&suggestion.old_path);
        suggestion.new_path = sanitize_display_path(&suggestion.new_path);
    }
    for unmatched in &mut result.unmatched {
        unmatched.stale_path = sanitize_display_path(&unmatched.stale_path);
    }
    for install_info in &mut result.installed_proton_versions {
        install_info.path = sanitize_display_path(&install_info.path);
    }

    Ok(result)
}

/// Applies a single Proton path migration to the named profile.
///
/// Re-validates the replacement path immediately before writing (TOCTOU mitigation).
/// On success, updates the metadata store and refreshes the health snapshot — both
/// are fail-soft and do not cause the command to return an error. All returned path
/// strings are sanitized with `sanitize_display_path()` (A-4).
#[tauri::command]
pub fn apply_proton_migration(
    request: ApplyMigrationRequest,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<MigrationApplyResult, String> {
    // TOCTOU mitigation: re-verify replacement path immediately before write.
    match Path::new(&request.new_path).try_exists() {
        Ok(true) => {}
        Ok(false) => {
            return Err(format!(
                "Replacement path no longer exists: {}",
                request.new_path
            ));
        }
        Err(err) => {
            return Err(format!(
                "Could not verify replacement path {}: {err}",
                request.new_path
            ));
        }
    }

    let mut result = apply_single_migration(&store, &request);

    if result.outcome == MigrationOutcome::Applied {
        let profile_path = store
            .base_path
            .join(format!("{}.toml", request.profile_name));

        // Fail-soft: observe profile write in metadata store.
        if let Ok(updated_profile) = store.load(&request.profile_name) {
            if let Err(e) = metadata_store.observe_profile_write(
                &request.profile_name,
                &updated_profile,
                &profile_path,
                SyncSource::AppMigration,
                None,
            ) {
                tracing::warn!(
                    %e,
                    profile = %request.profile_name,
                    "metadata sync after migration failed"
                );
            }

            // Fail-soft: invalidate health snapshot so the next health check
            // reflects the updated path.
            let profile_id = metadata_store
                .lookup_profile_id(&request.profile_name)
                .ok()
                .flatten();
            if let Some(ref pid) = profile_id {
                let health_report = check_profile_health(&request.profile_name, &updated_profile);
                let status_str = match health_report.status {
                    HealthStatus::Healthy => "healthy",
                    HealthStatus::Stale => "stale",
                    HealthStatus::Broken => "broken",
                };
                if let Err(error) = metadata_store.upsert_health_snapshot(
                    pid,
                    status_str,
                    health_report.issues.len(),
                    &health_report.checked_at,
                ) {
                    tracing::warn!(
                        %error,
                        profile_id = %pid,
                        "failed to invalidate health snapshot after migration"
                    );
                }
            }
        }
    }

    // A-4: sanitize returned path strings.
    result.old_path = sanitize_display_path(&result.old_path);
    result.new_path = sanitize_display_path(&result.new_path);

    Ok(result)
}

/// Applies a batch of Proton path migrations.
///
/// Performs a W-4 pre-flight validation pass before any writes: verifies every
/// replacement path exists and every profile is loadable. If any check fails,
/// returns immediately with zero writes and the failure details.
///
/// After pre-flight passes, applies each migration independently via
/// `apply_single_migration()` (atomic temp+rename write per W-1). One failure
/// does not abort the remaining writes — results are collected per profile.
///
/// Post-write metadata sync and health snapshot invalidation are both fail-soft.
/// All returned path strings are sanitized with `sanitize_display_path()` (A-4).
#[tauri::command]
pub fn apply_batch_migration(
    request: BatchMigrationRequest,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<BatchMigrationResult, String> {
    // W-4: Pre-flight validation — verify all replacement paths and load all
    // profiles before any writes. If ANY check fails, abort with zero writes.
    for m in &request.migrations {
        match Path::new(&m.new_path).try_exists() {
            Ok(true) => {}
            Ok(false) => {
                return Err(format!(
                    "Pre-flight failed for '{}': replacement path does not exist: {}",
                    m.profile_name, m.new_path
                ));
            }
            Err(err) => {
                return Err(format!(
                    "Pre-flight failed for '{}': could not verify replacement path: {err}",
                    m.profile_name
                ));
            }
        }

        store.load(&m.profile_name).map_err(|e| {
            format!(
                "Pre-flight failed for '{}': could not load profile: {e}",
                m.profile_name
            )
        })?;
    }

    // Pre-flight passed — apply each migration with per-profile error isolation.
    let mut results = Vec::new();
    let mut applied_count = 0usize;
    let mut failed_count = 0usize;
    let mut skipped_count = 0usize;

    for m in &request.migrations {
        let mut result = apply_single_migration(&store, m);

        if result.outcome == MigrationOutcome::Applied {
            let profile_path = store.base_path.join(format!("{}.toml", m.profile_name));

            // Fail-soft: observe profile write in metadata store.
            if let Ok(updated_profile) = store.load(&m.profile_name) {
                if let Err(e) = metadata_store.observe_profile_write(
                    &m.profile_name,
                    &updated_profile,
                    &profile_path,
                    SyncSource::AppMigration,
                    None,
                ) {
                    tracing::warn!(
                        %e,
                        profile = %m.profile_name,
                        "metadata sync after batch migration failed"
                    );
                }

                // Fail-soft: invalidate health snapshot so the next check reflects
                // the updated path.
                let profile_id = metadata_store
                    .lookup_profile_id(&m.profile_name)
                    .ok()
                    .flatten();
                if let Some(ref pid) = profile_id {
                    let health_report = check_profile_health(&m.profile_name, &updated_profile);
                    let status_str = match health_report.status {
                        HealthStatus::Healthy => "healthy",
                        HealthStatus::Stale => "stale",
                        HealthStatus::Broken => "broken",
                    };
                    if let Err(error) = metadata_store.upsert_health_snapshot(
                        pid,
                        status_str,
                        health_report.issues.len(),
                        &health_report.checked_at,
                    ) {
                        tracing::warn!(
                            %error,
                            profile_id = %pid,
                            "failed to invalidate health snapshot after batch migration"
                        );
                    }
                }
            }

            applied_count += 1;
        } else if result.outcome == MigrationOutcome::AlreadyValid {
            skipped_count += 1;
        } else {
            failed_count += 1;
        }

        // A-4: sanitize returned path strings.
        result.old_path = sanitize_display_path(&result.old_path);
        result.new_path = sanitize_display_path(&result.new_path);

        results.push(result);
    }

    Ok(BatchMigrationResult {
        results,
        applied_count,
        failed_count,
        skipped_count,
    })
}
