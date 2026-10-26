//! Post-exit version snapshot and known-good tag effects (Task 2.4).
//!
//! Moved verbatim from the Tauri stream finalizer
//! (`src-tauri/src/commands/launch/streaming.rs:286-424`): on a clean or
//! indeterminate exit the service records a version snapshot correlating the
//! installed Steam build with the trainer file, then independently tags the
//! latest config revision as known-good. Both effects are fail-open: every
//! error is traced and swallowed, and a disabled metadata store makes them
//! silent no-ops (its lookups return `None`/empty defaults).
//!
//! The success gate (`CleanExit | Indeterminate` plus a profile name) is the
//! finalizer's decision, not this module's; callers only invoke
//! [`record_success_effects`] when it holds.

use std::path::Path;

use crosshook_core::metadata::{
    compute_correlation_status, hash_trainer_file, MetadataStore, VersionCorrelationStatus,
};
use crosshook_core::steam::discover_steam_root_candidates;
use crosshook_core::steam::libraries::discover_steam_libraries;
use crosshook_core::steam::manifest::parse_manifest_full;

use super::super::run_blocking;
use super::types::SessionContext;

/// Records the version snapshot, then the known-good tag, for the session's
/// profile. All metadata and filesystem work runs inside one blocking task;
/// errors are traced and swallowed.
pub(super) async fn record_success_effects(ctx: &SessionContext) {
    let Some(profile_name) = ctx.profile_name.clone() else {
        return;
    };
    let metadata = ctx.metadata.clone();
    let steam_app_id = ctx.steam_app_id.clone();
    let steam_client_path = ctx.steam_client_path.clone();
    let trainer_host_path = ctx.trainer_host_path.clone();

    let result = run_blocking(move || {
        record_version_snapshot(
            &metadata,
            &profile_name,
            &steam_app_id,
            trainer_host_path.as_deref(),
            &steam_client_path,
        );
        record_known_good(&metadata, &profile_name);
        Ok(())
    })
    .await;
    if let Err(error) = result {
        tracing::warn!(%error, "version snapshot spawn_blocking join failed");
    }
}

/// Version snapshot (I-48), moved verbatim. An `UpdateInProgress` correlation
/// skips the snapshot; the known-good tag is a separate step and still runs.
fn record_version_snapshot(
    metadata: &MetadataStore,
    profile_name: &str,
    steam_app_id: &str,
    trainer_host_path: Option<&str>,
    steam_client_path: &str,
) {
    let profile_id = match metadata.lookup_profile_id(profile_name) {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::warn!(profile_name, "version snapshot skipped: profile not in metadata");
            return;
        }
        Err(error) => {
            tracing::warn!(%error, "version snapshot skipped: lookup_profile_id failed");
            return;
        }
    };

    let manifest_data = if !steam_app_id.trim().is_empty() {
        let mut diag = Vec::new();
        let roots = discover_steam_root_candidates(steam_client_path, &mut diag);
        let libraries = discover_steam_libraries(&roots, &mut diag);
        libraries.iter().find_map(|lib| {
            let path = lib
                .steamapps_path
                .join(format!("appmanifest_{}.acf", steam_app_id.trim()));
            if path.is_file() {
                parse_manifest_full(&path).ok()
            } else {
                None
            }
        })
    } else {
        None
    };

    let build_id = manifest_data.as_ref().map(|data| data.build_id.clone());
    let current_build_id = build_id.as_deref().unwrap_or("");
    let state_flags = manifest_data.as_ref().and_then(|data| data.state_flags);

    let trainer_file_hash = trainer_host_path.and_then(|path| {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            None
        } else {
            hash_trainer_file(Path::new(trimmed))
        }
    });

    let prior_snapshot = match metadata.lookup_latest_version_snapshot(&profile_id) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            tracing::warn!(%error, "version snapshot: prior lookup failed, continuing");
            None
        }
    };

    let snapshot_build_id = prior_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.steam_build_id.as_deref());
    let snapshot_trainer_hash = prior_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.trainer_file_hash.as_deref());

    let status = compute_correlation_status(
        current_build_id,
        snapshot_build_id,
        trainer_file_hash.as_deref(),
        snapshot_trainer_hash,
        state_flags,
    );

    if matches!(status, VersionCorrelationStatus::UpdateInProgress) {
        tracing::debug!(profile_name, "version snapshot skipped: game update in progress");
        return;
    }

    if let Err(error) = metadata.upsert_version_snapshot(
        &profile_id,
        steam_app_id.trim(),
        if current_build_id.is_empty() {
            None
        } else {
            Some(current_build_id)
        },
        None,
        trainer_file_hash.as_deref(),
        None,
        status.as_str(),
    ) {
        tracing::warn!(%error, "version snapshot upsert failed");
    }
}

/// Known-good tagging (I-49), moved verbatim. Independent of the snapshot: it
/// runs even when the snapshot was skipped (for example `UpdateInProgress`).
fn record_known_good(metadata: &MetadataStore, profile_name: &str) {
    let profile_id = match metadata.lookup_profile_id(profile_name) {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::debug!(
                profile_name,
                "known-good tagging skipped: profile not in metadata"
            );
            return;
        }
        Err(error) => {
            tracing::warn!(%error, "known-good tagging skipped: lookup_profile_id failed");
            return;
        }
    };

    let revisions = match metadata.list_config_revisions(&profile_id, Some(1)) {
        Ok(revisions) => revisions,
        Err(error) => {
            tracing::warn!(%error, "known-good tagging skipped: list_config_revisions failed");
            return;
        }
    };

    let latest = match revisions.into_iter().next() {
        Some(revision) => revision,
        None => {
            tracing::debug!(
                profile_name,
                "known-good tagging skipped: no config revisions exist"
            );
            return;
        }
    };

    if let Err(error) = metadata.set_known_good_revision(&profile_id, latest.id) {
        tracing::warn!(%error, "known-good tagging failed: set_known_good_revision failed");
    }
}
