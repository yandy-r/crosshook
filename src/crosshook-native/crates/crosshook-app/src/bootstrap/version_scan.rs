//! Startup version scan: correlates installed Steam build IDs and trainer
//! file hashes against the latest persisted version snapshots.
//!
//! Moved from `src-tauri/src/startup.rs::run_version_scan` (deleted by the
//! shell slice). This variant is synchronous and toolkit-free: it returns a
//! [`VersionScanSummary`] instead of emitting a Tauri event; the startup job
//! publishes the result through the app event bus.

use std::collections::HashMap;

use crosshook_core::metadata::{
    compute_correlation_status, hash_trainer_file, MetadataStore, VersionCorrelationStatus,
};
use crosshook_core::profile::ProfileStore;
use crosshook_core::steam::discovery::discover_steam_root_candidates;
use crosshook_core::steam::libraries::discover_steam_libraries;
use crosshook_core::steam::manifest::parse_manifest_full;

use super::report::VersionScanSummary;

/// Runs the version scan over every profile with a Steam app id. Failures to
/// list or load individual profiles are logged and skipped, never fatal.
pub(crate) fn run(
    profile_store: &ProfileStore,
    metadata_store: &MetadataStore,
) -> VersionScanSummary {
    let profile_names = match profile_store.list() {
        Ok(names) => names,
        Err(error) => {
            tracing::warn!(%error, "version scan: failed to list profiles");
            return VersionScanSummary {
                scanned: 0,
                mismatches: 0,
            };
        }
    };

    let profile_id_map: HashMap<String, String> = metadata_store
        .query_profile_ids_for_names(&profile_names)
        .unwrap_or_default()
        .into_iter()
        .collect();

    // Steam library discovery reads the user's home; defer it until a profile
    // actually has a Steam app id to look up.
    let mut libraries = None;

    let mut scanned: u32 = 0;
    let mut mismatches: u32 = 0;

    for name in &profile_names {
        let profile = match profile_store.load(name) {
            Ok(profile) => profile,
            Err(error) => {
                tracing::warn!(%error, profile_name = %name, "version scan: failed to load profile");
                continue;
            }
        };

        let app_id = profile.steam.app_id.trim().to_string();
        if app_id.is_empty() {
            continue;
        }

        let libraries = libraries.get_or_insert_with(|| {
            let mut diagnostics = Vec::new();
            let steam_roots = discover_steam_root_candidates("", &mut diagnostics);
            discover_steam_libraries(&steam_roots, &mut diagnostics)
        });
        let manifest_path = libraries.iter().find_map(|lib| {
            let path = lib.steamapps_path.join(format!("appmanifest_{app_id}.acf"));
            path.exists().then_some(path)
        });

        let manifest_path = match manifest_path {
            Some(path) => path,
            None => continue,
        };

        let manifest = match parse_manifest_full(&manifest_path) {
            Ok(manifest) => manifest,
            Err(error) => {
                tracing::warn!(profile_name = %name, %error, "version scan: manifest parse failed");
                continue;
            }
        };

        // Skip only when StateFlags is present and not 4 (Steam update in
        // progress). None is treated like compute_correlation_status: proceed
        // with comparison.
        if let Some(flags) = manifest.state_flags {
            if flags != 4 {
                continue;
            }
        }

        let profile_id = match profile_id_map.get(name) {
            Some(id) => id.as_str(),
            None => {
                tracing::warn!(profile_name = %name, "version scan: profile_id not in metadata, skipping");
                continue;
            }
        };

        let snapshot = metadata_store
            .lookup_latest_version_snapshot(profile_id)
            .unwrap_or_default();

        let trainer_path = profile.trainer.path.trim().to_string();
        let trainer_hash = if trainer_path.is_empty() {
            None
        } else {
            hash_trainer_file(std::path::Path::new(&trainer_path))
        };

        let snapshot_build_id = snapshot.as_ref().and_then(|s| s.steam_build_id.as_deref());
        let snapshot_trainer_hash = snapshot
            .as_ref()
            .and_then(|s| s.trainer_file_hash.as_deref());

        let status = compute_correlation_status(
            &manifest.build_id,
            snapshot_build_id,
            trainer_hash.as_deref(),
            snapshot_trainer_hash,
            manifest.state_flags,
        );

        scanned += 1;

        if matches!(
            status,
            VersionCorrelationStatus::GameUpdated
                | VersionCorrelationStatus::TrainerChanged
                | VersionCorrelationStatus::BothChanged
        ) {
            mismatches += 1;
        }
    }

    tracing::info!(scanned, mismatches, "startup version scan complete");
    VersionScanSummary {
        scanned,
        mismatches,
    }
}
