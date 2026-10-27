use std::path::PathBuf;

use crosshook_app::services::profiles::ProfileCommit;
use crosshook_app::services::SteamService;
use crosshook_app::ProfileWriter;
use crosshook_core::profile::migration::{
    scan_proton_migrations, ApplyMigrationRequest, BatchMigrationRequest, BatchMigrationResult,
    MigrationApplyResult, MigrationScanResult,
};
use crosshook_core::profile::ProfileStore;
use crosshook_core::steam::discover_steam_root_candidates;
use tauri::State;

use super::shared::sanitize_display_path;

/// Scans all profiles for stale Proton paths and returns migration suggestions.
///
/// The `steam_client_install_path` argument is treated as advisory (A-5): if the
/// provided path does not contain a `steamapps` subdirectory, the command falls
/// back to `default_steam_client_install_path()`. All path strings in the result
/// are sanitized with `sanitize_display_path()` before returning (A-4).
///
/// This command is read-only — it performs no writes and requires no `MetadataStore`.
#[tauri::command]
pub async fn check_proton_migrations(
    steam_client_install_path: Option<String>,
    store: State<'_, ProfileStore>,
    steam: State<'_, SteamService>,
) -> Result<MigrationScanResult, String> {
    // A-5: validate caller-supplied steam path; fall back to default if invalid.
    let supplied_path = tauri::async_runtime::spawn_blocking(move || {
        steam_client_install_path.filter(|p| {
            let candidate = PathBuf::from(p.trim());
            !candidate.as_os_str().is_empty() && candidate.join("steamapps").is_dir()
        })
    })
    .await
    .map_err(|error| error.to_string())?;
    let resolved_path = match supplied_path {
        Some(path) => path,
        None => steam.default_client_install_path().await,
    };

    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
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

        result
    })
    .await
    .map_err(|error| error.to_string())
}

/// Applies a single Proton path migration to the named profile.
///
/// Delegates to [`ProfileWriter::apply_proton_migration`]: TOCTOU re-check of
/// the replacement path, write without MangoHud companion, fail-soft metadata
/// sync and health invalidation, sanitized result paths (A-4).
#[tauri::command]
pub async fn apply_proton_migration(
    request: ApplyMigrationRequest,
    writer: State<'_, ProfileWriter>,
) -> Result<MigrationApplyResult, String> {
    writer
        .apply_proton_migration(request)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}

/// Applies a batch of Proton path migrations.
///
/// Delegates to [`ProfileWriter::apply_batch_migration`]: W-4 pre-flight of
/// every path and profile before any write, per-profile error isolation,
/// fail-soft metadata sync and health invalidation, sanitized result paths (A-4).
#[tauri::command]
pub async fn apply_batch_migration(
    request: BatchMigrationRequest,
    writer: State<'_, ProfileWriter>,
) -> Result<BatchMigrationResult, String> {
    writer
        .apply_batch_migration(request)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}
