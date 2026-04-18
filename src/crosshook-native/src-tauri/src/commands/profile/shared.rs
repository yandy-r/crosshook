use crosshook_core::metadata::{
    sha256_hex, ConfigRevisionSource, MetadataStore, MetadataStoreError, SyncSource,
};
use crosshook_core::profile::{GameProfile, ProfileStoreError};
use tauri::{AppHandle, Emitter};

pub(super) const STEAM_COMPATDATA_MARKER: &str = "/steamapps/compatdata/";
pub(super) const STEAM_ROOT_SUFFIXES: [&str; 2] = ["/.local/share/Steam", "/.steam/root"];

pub(super) fn map_error(error: ProfileStoreError) -> String {
    error.to_string()
}

pub(super) fn derive_steam_client_install_path(profile: &GameProfile) -> String {
    let compatdata_path = profile.steam.compatdata_path.trim().replace('\\', "/");
    compatdata_path
        .split_once(STEAM_COMPATDATA_MARKER)
        .map(|(steam_root, _)| steam_root.to_string())
        .unwrap_or_default()
}

pub(super) fn derive_target_home_path(steam_client_install_path: &str) -> String {
    let normalized = steam_client_install_path.trim().replace('\\', "/");

    for suffix in STEAM_ROOT_SUFFIXES {
        if let Some(home_path) = normalized.strip_suffix(suffix) {
            return home_path.to_string();
        }
    }

    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => home,
        _ => {
            tracing::warn!(
                "HOME is unset or empty and Steam client path did not match known patterns; derived home for launcher cleanup will be empty"
            );
            String::new()
        }
    }
}

pub(super) fn cleanup_launchers_for_profile_delete(
    profile_name: &str,
    profile: &GameProfile,
) -> Result<Option<crosshook_core::export::LauncherDeleteResult>, String> {
    if profile.launch.method == "native" {
        tracing::debug!(
            profile_name,
            "skipping launcher cleanup for native profile delete"
        );
        return Ok(None);
    }

    let steam_client_install_path = derive_steam_client_install_path(profile);
    let target_home_path = derive_target_home_path(&steam_client_install_path);

    crosshook_core::export::delete_launcher_for_profile(
        profile,
        &target_home_path,
        &steam_client_install_path,
    )
    .map(Some)
    .map_err(|error| error.to_string())
}

pub(super) fn emit_profiles_changed(app: &AppHandle, reason: &str) {
    if let Err(error) = app.emit("profiles-changed", reason.to_string()) {
        tracing::warn!(%error, reason, "failed to emit profiles-changed event");
    }
}

pub(super) fn observe_profile_write_launch_change(
    name: &str,
    store: &crosshook_core::profile::ProfileStore,
    metadata_store: &MetadataStore,
    updated: &GameProfile,
) {
    let profile_path = store.base_path.join(format!("{name}.toml"));
    if let Err(e) = metadata_store.observe_profile_write(
        name,
        updated,
        &profile_path,
        SyncSource::AppWrite,
        None,
    ) {
        tracing::warn!(
            %e,
            profile_name = %name,
            "metadata sync after launch optimization / preset change failed"
        );
    }
}

/// Captures a deduped config revision snapshot after a successful profile write.
///
/// Serializes the profile to canonical TOML, computes a SHA-256 content hash, looks
/// up the stable `profile_id` from the metadata store, then calls
/// [`MetadataStore::insert_config_revision`]. Silently skips on any error so that
/// snapshot capture never fails a user-facing save operation.
///
/// Returns the new revision id when a row is inserted, `None` on dedup, skip, or error.
pub fn capture_config_revision(
    profile_name: &str,
    profile: &GameProfile,
    source: ConfigRevisionSource,
    source_revision_id: Option<i64>,
    metadata_store: &MetadataStore,
) -> Option<i64> {
    if !metadata_store.is_available() {
        return None;
    }

    let snapshot_toml = match toml::to_string_pretty(profile) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                %e,
                profile_name,
                "failed to serialize profile for config revision capture"
            );
            return None;
        }
    };

    let content_hash = sha256_hex(snapshot_toml.as_bytes());

    let profile_id = match metadata_store.lookup_profile_id(profile_name) {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::warn!(
                profile_name,
                "profile_id not found in metadata — skipping config revision capture"
            );
            return None;
        }
        Err(e) => {
            tracing::warn!(
                %e,
                profile_name,
                "failed to look up profile_id for config revision capture"
            );
            return None;
        }
    };

    match metadata_store.insert_config_revision(
        &profile_id,
        profile_name,
        source,
        &content_hash,
        &snapshot_toml,
        source_revision_id,
    ) {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!(%e, profile_name, "failed to capture config revision");
            None
        }
    }
}

/// Merges the per-collection launch defaults layer into a profile that was just
/// returned from [`ProfileStore::load`].
///
/// Behaviour:
/// - `collection_id` is `None` or trims to empty → returns `profile` unchanged.
/// - `get_collection_defaults` returns `Ok(None)` (no defaults set on the
///   collection) → merges an empty layer, i.e. returns the profile unchanged.
/// - `get_collection_defaults` returns `Ok(Some(defaults))` → returns the
///   merged profile via [`GameProfile::effective_profile_with`].
/// - `get_collection_defaults` returns `Err(MetadataStoreError::Corrupt(_))`
///   → bubbles the error up so the frontend surfaces a loud failure. Corrupt
///   defaults are a data-integrity issue the user needs to fix, not a transient
///   glitch to silently swallow.
/// - `get_collection_defaults` returns any other `Err` (missing row via
///   `Validation`, `Database`, `Io`, …) → logs a `tracing::warn!` and falls
///   back to `Ok(profile)`. This preserves fail-open launch semantics so a
///   stale collection id or a transient SQLite glitch never hard-blocks a
///   game launch.
///
/// Note on layer precedence: `profile` here has already been flattened by
/// `ProfileStore::load`, which bakes `local_override` into layer 1 and clears
/// the section. That's why we can call `effective_profile_with` on it directly
/// — layer 3 is a no-op at this call site but layer 1 still carries every
/// machine-specific override, so the "local_override always wins" guarantee
/// holds at runtime. See the doc comment on `effective_profile_with` for the
/// full explanation.
pub(super) fn apply_collection_defaults(
    profile: GameProfile,
    metadata_store: &MetadataStore,
    collection_id: Option<&str>,
) -> Result<GameProfile, MetadataStoreError> {
    let Some(cid) = collection_id.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(profile);
    };

    match metadata_store.get_collection_defaults(cid) {
        Ok(defaults) => Ok(profile.effective_profile_with(defaults.as_ref())),
        Err(MetadataStoreError::Corrupt(msg)) => {
            tracing::warn!(
                collection_id = %cid,
                error = %msg,
                "collection defaults JSON is corrupt; surfacing error to launch entrypoint"
            );
            Err(MetadataStoreError::Corrupt(msg))
        }
        Err(other) => {
            tracing::warn!(
                collection_id = %cid,
                error = %other,
                "failed to load collection defaults; launching with raw profile"
            );
            Ok(profile)
        }
    }
}
