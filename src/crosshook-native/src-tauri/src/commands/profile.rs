use crosshook_core::game_images::{
    import_custom_art, import_custom_cover_art, is_in_managed_media_dir, GameImageType,
};
use crosshook_core::metadata::{
    sha256_hex, BundledOptimizationPresetRow, ConfigRevisionSource, MetadataStore,
    MetadataStoreError, ProfileLaunchPresetOrigin, SyncSource, MAX_HISTORY_LIST_LIMIT,
};
use crosshook_core::profile::{
    apply_profile_creation_defaults_from_settings, bundled_optimization_preset_toml_key,
    resolve_art_app_id, validate_steam_app_id, DuplicateProfileResult, GameProfile,
    GamescopeConfig, LaunchOptimizationsSection, MangoHudConfig, ProfileStore, ProfileStoreError,
};
use crosshook_core::settings::SettingsStore;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Emitter, State};

const STEAM_COMPATDATA_MARKER: &str = "/steamapps/compatdata/";
const STEAM_ROOT_SUFFIXES: [&str; 2] = ["/.local/share/Steam", "/.steam/root"];

fn map_error(error: ProfileStoreError) -> String {
    error.to_string()
}

fn derive_steam_client_install_path(profile: &GameProfile) -> String {
    let compatdata_path = profile.steam.compatdata_path.trim().replace('\\', "/");
    compatdata_path
        .split_once(STEAM_COMPATDATA_MARKER)
        .map(|(steam_root, _)| steam_root.to_string())
        .unwrap_or_default()
}

fn derive_target_home_path(steam_client_install_path: &str) -> String {
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

fn cleanup_launchers_for_profile_delete(
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

fn save_launch_optimizations_for_profile(
    name: &str,
    optimizations: &LaunchOptimizationsPayload,
    store: &ProfileStore,
) -> Result<(), String> {
    store
        .save_launch_optimizations(
            name,
            optimizations.enabled_option_ids.clone(),
            optimizations.switch_active_preset.clone(),
        )
        .map_err(map_error)
}

fn emit_profiles_changed(app: &AppHandle, reason: &str) {
    if let Err(error) = app.emit("profiles-changed", reason.to_string()) {
        tracing::warn!(%error, reason, "failed to emit profiles-changed event");
    }
}

fn observe_profile_write_launch_change(
    name: &str,
    store: &ProfileStore,
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
pub(crate) fn capture_config_revision(
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundledOptimizationPresetDto {
    pub preset_id: String,
    pub display_name: String,
    pub vendor: String,
    pub mode: String,
    pub enabled_option_ids: Vec<String>,
    pub catalog_version: i64,
}

fn bundled_row_to_dto(
    row: BundledOptimizationPresetRow,
) -> Result<BundledOptimizationPresetDto, String> {
    let enabled_option_ids: Vec<String> = serde_json::from_str(&row.option_ids_json)
        .map_err(|e| format!("corrupt bundled preset {} option list: {e}", row.preset_id))?;
    Ok(BundledOptimizationPresetDto {
        preset_id: row.preset_id,
        display_name: row.display_name,
        vendor: row.vendor,
        mode: row.mode,
        enabled_option_ids,
        catalog_version: row.catalog_version,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchOptimizationsPayload {
    #[serde(
        rename = "enabled_option_ids",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub enabled_option_ids: Vec<String>,
    /// When set, selects that named preset from `launch.presets` and ignores `enabled_option_ids`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_active_preset: Option<String>,
}

#[tauri::command]
pub fn profile_list(store: State<'_, ProfileStore>) -> Result<Vec<String>, String> {
    store.list().map_err(map_error)
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
fn apply_collection_defaults(
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

#[tauri::command]
pub fn profile_load(
    name: String,
    collection_id: Option<String>,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<GameProfile, String> {
    let profile = store.load(&name).map_err(map_error)?;

    // When a collection context is provided, merge the collection's defaults
    // into the profile via `effective_profile_with`. The returned profile still
    // reflects the machine-specific `local_override` layer that `ProfileStore::load`
    // baked into layer 1, so collection defaults can never clobber portable paths.
    apply_collection_defaults(profile, metadata_store.inner(), collection_id.as_deref())
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub name: String,
    pub game_name: String,
    pub steam_app_id: String,
    pub custom_cover_art_path: Option<String>,
    pub custom_portrait_art_path: Option<String>,
}

#[tauri::command]
pub fn profile_list_summaries(
    store: State<'_, ProfileStore>,
) -> Result<Vec<ProfileSummary>, String> {
    let names = store.list().map_err(map_error)?;
    let mut summaries = Vec::with_capacity(names.len());
    for name in names {
        match store.load(&name) {
            Ok(profile) => {
                let effective = profile.effective_profile();
                let cover_art = effective.game.custom_cover_art_path.trim();
                let portrait_art = effective.game.custom_portrait_art_path.trim();
                summaries.push(ProfileSummary {
                    name,
                    game_name: effective.game.name.clone(),
                    steam_app_id: resolve_art_app_id(&effective).to_string(),
                    custom_cover_art_path: if cover_art.is_empty() {
                        None
                    } else {
                        Some(cover_art.to_string())
                    },
                    custom_portrait_art_path: if portrait_art.is_empty() {
                        None
                    } else {
                        Some(portrait_art.to_string())
                    },
                });
            }
            Err(e) => {
                tracing::warn!(profile_name = %name, %e, "skipping profile in summaries");
            }
        }
    }
    Ok(summaries)
}

#[tauri::command]
pub fn profile_save(
    name: String,
    mut data: GameProfile,
    app: AppHandle,
    store: State<'_, ProfileStore>,
    settings_store: State<'_, SettingsStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    // Validate runtime.steam_app_id before writing to disk (BR-4).
    if let Err(e) = validate_steam_app_id(data.runtime.steam_app_id.trim()) {
        return Err(format!("Invalid Steam App ID in runtime section: {e}"));
    }

    let is_new = !store.profile_exists(&name);
    if is_new {
        let app_settings = settings_store.load().map_err(|e| e.to_string())?;
        apply_profile_creation_defaults_from_settings(&mut data, &app_settings);

        // Only apply the app-settings default bundled preset when the incoming
        // draft has not already selected one — otherwise an explicit wizard
        // selection would be silently clobbered by the user's default.
        let pid = app_settings.default_bundled_optimization_preset_id.trim();
        if !pid.is_empty() && metadata_store.is_available() && data.launch.active_preset.trim().is_empty() {
            match metadata_store.get_bundled_optimization_preset(pid) {
                Ok(Some(row)) => {
                    let enabled_option_ids: Vec<String> =
                        serde_json::from_str(&row.option_ids_json).unwrap_or_default();
                    let toml_key = bundled_optimization_preset_toml_key(pid);
                    data.launch.presets.insert(
                        toml_key.clone(),
                        LaunchOptimizationsSection {
                            enabled_option_ids: enabled_option_ids.clone(),
                        },
                    );
                    data.launch.active_preset = toml_key;
                    data.launch.optimizations = LaunchOptimizationsSection { enabled_option_ids };
                }
                Ok(None) => {
                    tracing::debug!(
                        preset_id = %pid,
                        "default bundled optimization preset not found in metadata; skipping"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        %e,
                        preset_id = %pid,
                        "failed to read default bundled optimization preset from metadata"
                    );
                }
            }
        }
    }

    // Auto-import custom cover art into the managed media directory when the
    // source path points outside it (e.g. a user-typed filesystem path).
    let cover = data.game.custom_cover_art_path.trim().to_string();
    if !cover.is_empty() && !is_in_managed_media_dir(&cover) {
        match import_custom_cover_art(&cover) {
            Ok(imported) => data.game.custom_cover_art_path = imported,
            Err(e) => {
                tracing::warn!(profile_name = %name, %e, "failed to import custom cover art; keeping original path");
            }
        }
    }
    // Portrait auto-import
    let portrait = data.game.custom_portrait_art_path.trim().to_string();
    if !portrait.is_empty() && !is_in_managed_media_dir(&portrait) {
        match import_custom_art(&portrait, GameImageType::Portrait) {
            Ok(imported) => data.game.custom_portrait_art_path = imported,
            Err(e) => {
                tracing::warn!(profile_name = %name, %e, "failed to auto-import portrait art; keeping original path")
            }
        }
    }
    // Background auto-import
    let background = data.game.custom_background_art_path.trim().to_string();
    if !background.is_empty() && !is_in_managed_media_dir(&background) {
        match import_custom_art(&background, GameImageType::Background) {
            Ok(imported) => data.game.custom_background_art_path = imported,
            Err(e) => {
                tracing::warn!(profile_name = %name, %e, "failed to auto-import background art; keeping original path")
            }
        }
    }

    store.save(&name, &data).map_err(map_error)?;

    let profile_path = store.base_path.join(format!("{name}.toml"));
    if let Err(e) = metadata_store.observe_profile_write(
        &name,
        &data,
        &profile_path,
        SyncSource::AppWrite,
        None,
    ) {
        tracing::warn!(%e, profile_name = %name, "metadata sync after profile_save failed");
    }

    capture_config_revision(
        &name,
        &data,
        ConfigRevisionSource::ManualSave,
        None,
        &metadata_store,
    );

    emit_profiles_changed(&app, "saved");
    Ok(())
}

#[tauri::command]
pub fn profile_save_launch_optimizations(
    name: String,
    optimizations: LaunchOptimizationsPayload,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    save_launch_optimizations_for_profile(&name, &optimizations, &store)?;

    if let Ok(updated) = store.load(&name) {
        let profile_path = store.base_path.join(format!("{name}.toml"));
        if let Err(e) = metadata_store.observe_profile_write(
            &name,
            &updated,
            &profile_path,
            SyncSource::AppWrite,
            None,
        ) {
            tracing::warn!(
                %e,
                profile_name = %name,
                "metadata sync after save_launch_optimizations failed"
            );
        }
        capture_config_revision(
            &name,
            &updated,
            ConfigRevisionSource::LaunchOptimizationSave,
            None,
            &metadata_store,
        );
    }

    Ok(())
}

#[tauri::command]
pub fn profile_save_mangohud_config(
    name: String,
    config: MangoHudConfig,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    let mut profile = store.load(&name).map_err(|e| e.to_string())?;
    profile.launch.mangohud = config;
    store.save(&name, &profile).map_err(|e| e.to_string())?;
    let profile_path = store.base_path.join(format!("{name}.toml"));
    if let Err(e) = metadata_store.observe_profile_write(
        &name,
        &profile,
        &profile_path,
        SyncSource::AppWrite,
        None,
    ) {
        tracing::warn!(%e, profile_name = %name, "metadata sync after profile_save_mangohud_config failed");
    }
    capture_config_revision(
        &name,
        &profile,
        ConfigRevisionSource::ManualSave,
        None,
        &metadata_store,
    );
    Ok(())
}

#[tauri::command]
pub fn profile_save_gamescope_config(
    name: String,
    config: GamescopeConfig,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    let mut profile = store.load(&name).map_err(|e| e.to_string())?;
    profile.launch.gamescope = config;
    store.save(&name, &profile).map_err(|e| e.to_string())?;
    let profile_path = store.base_path.join(format!("{name}.toml"));
    if let Err(e) = metadata_store.observe_profile_write(
        &name,
        &profile,
        &profile_path,
        SyncSource::AppWrite,
        None,
    ) {
        tracing::warn!(%e, profile_name = %name, "metadata sync after profile_save_gamescope_config failed");
    }
    capture_config_revision(
        &name,
        &profile,
        ConfigRevisionSource::ManualSave,
        None,
        &metadata_store,
    );
    Ok(())
}

#[tauri::command]
pub fn profile_save_trainer_gamescope_config(
    name: String,
    config: GamescopeConfig,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    let mut profile = store.load(&name).map_err(|e| e.to_string())?;
    profile.launch.trainer_gamescope = config;
    store.save(&name, &profile).map_err(|e| e.to_string())?;
    let profile_path = store.base_path.join(format!("{name}.toml"));
    if let Err(e) = metadata_store.observe_profile_write(
        &name,
        &profile,
        &profile_path,
        SyncSource::AppWrite,
        None,
    ) {
        tracing::warn!(%e, profile_name = %name, "metadata sync after profile_save_trainer_gamescope_config failed");
    }
    capture_config_revision(
        &name,
        &profile,
        ConfigRevisionSource::ManualSave,
        None,
        &metadata_store,
    );
    Ok(())
}

#[tauri::command]
pub fn profile_list_bundled_optimization_presets(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<BundledOptimizationPresetDto>, String> {
    if !metadata_store.is_available() {
        return Ok(Vec::new());
    }
    let rows = metadata_store
        .list_bundled_optimization_presets()
        .map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(bundled_row_to_dto(row)?);
    }
    Ok(out)
}

#[tauri::command]
pub fn profile_apply_bundled_optimization_preset(
    name: String,
    preset_id: String,
    app: AppHandle,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<GameProfile, String> {
    if !metadata_store.is_available() {
        return Err("metadata store is unavailable — cannot apply bundled presets".to_string());
    }

    let profile_name = name.trim();
    if profile_name.is_empty() {
        return Err("profile name is required".to_string());
    }

    let pid = preset_id.trim();
    if pid.is_empty() {
        return Err("preset_id is required".to_string());
    }

    let row = metadata_store
        .get_bundled_optimization_preset(pid)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("unknown bundled preset: {pid}"))?;

    let enabled_option_ids: Vec<String> =
        serde_json::from_str(&row.option_ids_json).map_err(|e| e.to_string())?;

    let toml_key = bundled_optimization_preset_toml_key(&row.preset_id);
    store
        .materialize_launch_optimization_preset(profile_name, &toml_key, enabled_option_ids, true)
        .map_err(map_error)?;

    let updated = store.load(profile_name).map_err(map_error)?;
    observe_profile_write_launch_change(profile_name, &store, &metadata_store, &updated);
    capture_config_revision(
        profile_name,
        &updated,
        ConfigRevisionSource::PresetApply,
        None,
        &metadata_store,
    );

    if let Ok(Some(profile_id)) = metadata_store.lookup_profile_id(profile_name) {
        if let Err(e) = metadata_store.upsert_profile_launch_preset_metadata(
            &profile_id,
            &toml_key,
            ProfileLaunchPresetOrigin::Bundled,
            Some(row.preset_id.as_str()),
        ) {
            tracing::warn!(
                %e,
                profile_name = %profile_name,
                "failed to upsert bundled preset metadata row"
            );
        }
    }

    emit_profiles_changed(&app, "bundled-optimization-preset");
    Ok(updated)
}

#[tauri::command]
pub fn profile_save_manual_optimization_preset(
    name: String,
    preset_name: String,
    enabled_option_ids: Vec<String>,
    app: AppHandle,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<GameProfile, String> {
    let profile_name = name.trim();
    if profile_name.is_empty() {
        return Err("profile name is required".to_string());
    }

    let key = preset_name.trim();
    if key.is_empty() {
        return Err("preset name must not be empty".to_string());
    }

    store
        .save_manual_launch_optimization_preset(profile_name, key, enabled_option_ids)
        .map_err(map_error)?;

    let updated = store.load(profile_name).map_err(map_error)?;
    observe_profile_write_launch_change(profile_name, &store, &metadata_store, &updated);
    capture_config_revision(
        profile_name,
        &updated,
        ConfigRevisionSource::PresetApply,
        None,
        &metadata_store,
    );

    if metadata_store.is_available() {
        if let Ok(Some(profile_id)) = metadata_store.lookup_profile_id(profile_name) {
            if let Err(e) = metadata_store.upsert_profile_launch_preset_metadata(
                &profile_id,
                key,
                ProfileLaunchPresetOrigin::User,
                None,
            ) {
                tracing::warn!(
                    %e,
                    profile_name = %profile_name,
                    "failed to upsert user preset metadata row"
                );
            }
        }
    }

    emit_profiles_changed(&app, "manual-optimization-preset");
    Ok(updated)
}

#[tauri::command]
pub fn profile_delete(
    name: String,
    app: AppHandle,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    // Best-effort launcher cleanup before profile deletion.
    // Profile deletion must succeed even if launcher cleanup fails.
    if let Ok(profile) = store.load(&name) {
        if let Err(error) = cleanup_launchers_for_profile_delete(&name, &profile) {
            tracing::warn!("Launcher cleanup failed for profile {name}: {error}");
        }
    }

    store.delete(&name).map_err(map_error)?;

    if let Err(e) = metadata_store.observe_profile_delete(&name) {
        tracing::warn!(%e, profile_name = %name, "metadata sync after profile_delete failed");
    }

    emit_profiles_changed(&app, "deleted");
    Ok(())
}

/// Duplicates an existing profile under a unique copy name.
///
/// Delegates to [`ProfileStore::duplicate`] which handles name generation, collision
/// avoidance, and persistence. The returned [`DuplicateProfileResult`] is serialized
/// to the frontend where it drives profile list refresh and auto-selection of the copy.
///
/// # Frontend invocation
/// ```ts
/// const result = await invoke<DuplicateProfileResult>('profile_duplicate', { name });
/// ```
///
/// # Errors
/// Returns a stringified error when the source profile does not exist or if the
/// generated copy name cannot pass filesystem validation.
#[tauri::command]
pub fn profile_duplicate(
    name: String,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<DuplicateProfileResult, String> {
    let source_profile_id = metadata_store.lookup_profile_id(&name).ok().flatten();

    let result = store.duplicate(&name).map_err(map_error)?;

    let copy_path = store.base_path.join(format!("{}.toml", result.name));
    if let Err(e) = metadata_store.observe_profile_write(
        &result.name,
        &result.profile,
        &copy_path,
        SyncSource::AppDuplicate,
        source_profile_id.as_deref(),
    ) {
        tracing::warn!(%e, name = %result.name, "metadata sync after profile_duplicate failed");
    }

    Ok(result)
}

#[tauri::command]
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
    settings_store: State<'_, SettingsStore>,
) -> Result<bool, String> {
    // Load profile BEFORE rename for launcher cleanup and display_name update.
    let old_profile = store.load(&old_name).ok();

    store.rename(&old_name, &new_name).map_err(map_error)?;

    let old_path = store.base_path.join(format!("{old_name}.toml"));
    let new_path = store.base_path.join(format!("{new_name}.toml"));
    if let Err(e) =
        metadata_store.observe_profile_rename(&old_name, &new_name, &old_path, &new_path)
    {
        tracing::warn!(%e, %old_name, %new_name, "metadata sync after profile_rename failed");
    }

    // Best-effort: delete old launcher files so the frontend can re-export with correct paths.
    let had_launcher = if let Some(ref profile) = old_profile {
        match cleanup_launchers_for_profile_delete(&old_name, profile) {
            Ok(Some(result)) => result.script_deleted || result.desktop_entry_deleted,
            Ok(None) => false,
            Err(error) => {
                tracing::warn!(%error, %old_name, %new_name, "launcher cleanup during profile rename failed");
                false
            }
        }
    } else {
        false
    };

    // Best-effort: update display_name inside the renamed profile so future exports use the new name.
    if old_profile.is_some() {
        if let Ok(mut profile) = store.load(&new_name) {
            profile.steam.launcher.display_name = new_name.trim().to_string();
            if let Err(err) = store.save(&new_name, &profile) {
                tracing::warn!(%err, %new_name, "display_name update after profile rename failed");
            }
        }
    }

    if let Ok(mut settings) = settings_store.load() {
        if settings.last_used_profile.trim() == old_name.trim() {
            settings.last_used_profile = new_name.trim().to_string();
            if let Err(err) = settings_store.save(&settings) {
                tracing::warn!(%err, %old_name, %new_name, "settings update after profile rename failed");
            }
        }
    }

    Ok(had_launcher)
}

#[tauri::command]
pub fn profile_import_legacy(
    path: String,
    app: AppHandle,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<GameProfile, String> {
    let profile = store.import_legacy(Path::new(&path)).map_err(map_error)?;

    let stem = Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported");
    let import_path = store.base_path.join(format!("{stem}.toml"));
    if let Err(e) =
        metadata_store.observe_profile_write(stem, &profile, &import_path, SyncSource::Import, None)
    {
        tracing::warn!(%e, profile_name = %stem, "metadata sync after import_legacy failed");
    }

    capture_config_revision(
        stem,
        &profile,
        ConfigRevisionSource::Import,
        None,
        &metadata_store,
    );

    emit_profiles_changed(&app, "imported-legacy");
    Ok(profile)
}

/// Serializes the provided in-memory profile to a shareable TOML string
/// with comment headers indicating the save location.
#[tauri::command]
pub fn profile_export_toml(name: String, data: GameProfile) -> Result<String, String> {
    crosshook_core::profile::profile_to_shareable_toml(&name, &data)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn profile_set_favorite(
    name: String,
    favorite: bool,
    app: AppHandle,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    metadata_store
        .set_profile_favorite(&name, favorite)
        .map_err(|e| e.to_string())?;
    emit_profiles_changed(&app, "favorite-updated");
    Ok(())
}

#[tauri::command]
pub fn profile_list_favorites(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<String>, String> {
    metadata_store
        .list_favorite_profiles()
        .map_err(|e| e.to_string())
}

// ── Config history response types ─────────────────────────────────────────────

/// Lightweight summary of a single config revision for list responses.
/// The `snapshot_toml` field is omitted to keep the payload small.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRevisionSummary {
    pub id: i64,
    pub profile_name_at_write: String,
    pub source: String,
    pub content_hash: String,
    pub source_revision_id: Option<i64>,
    pub is_last_known_working: bool,
    pub created_at: String,
}

/// Result from a profile config diff operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDiffResult {
    /// The left-side revision id (the selected revision used as the diff base).
    pub revision_id: i64,
    pub revision_source: String,
    pub revision_created_at: String,
    /// Unified diff text in standard format. Empty when the two sides are identical.
    pub diff_text: String,
    pub added_lines: usize,
    pub removed_lines: usize,
    /// True when either input exceeded the line limit and the diff may be incomplete.
    pub truncated: bool,
}

/// Result from a profile config rollback operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRollbackResult {
    /// The revision id that was restored.
    pub restored_revision_id: i64,
    /// The new revision id appended for the rollback event (None when capture was deduped or failed).
    pub new_revision_id: Option<i64>,
    pub profile: GameProfile,
}

// ── Unified diff helper ───────────────────────────────────────────────────────

const DIFF_CONTEXT_LINES: usize = 3;
/// Maximum lines per side considered for the diff. Profiles are small in practice
/// but this caps the O(m*n) LCS table at a safe memory bound.
const DIFF_MAX_LINES: usize = 2000;
/// Maximum byte length of a computed diff output returned to the frontend.
/// Prevents large IPC payloads when both sides have many long changed lines.
const MAX_DIFF_OUTPUT_BYTES: usize = 512 * 1024;

/// Compute a unified diff between two text strings using an LCS-based algorithm.
/// Returns `(diff_text, added_lines, removed_lines, truncated)`. `diff_text` is
/// empty when the two inputs are identical. `truncated` is true when either input
/// exceeds `DIFF_MAX_LINES` and the diff may be incomplete.
fn compute_unified_diff(
    old_label: &str,
    new_label: &str,
    old_text: &str,
    new_text: &str,
) -> (String, usize, usize, bool) {
    if old_text == new_text {
        return (String::new(), 0, 0, false);
    }

    let old_total = old_text.lines().count();
    let new_total = new_text.lines().count();
    let truncated = old_total > DIFF_MAX_LINES || new_total > DIFF_MAX_LINES;

    let old_lines: Vec<&str> = old_text.lines().take(DIFF_MAX_LINES).collect();
    let new_lines: Vec<&str> = new_text.lines().take(DIFF_MAX_LINES).collect();

    let m = old_lines.len();
    let n = new_lines.len();

    // Build LCS table (O(m*n) time and space, bounded by DIFF_MAX_LINES^2)
    let mut dp = vec![vec![0u32; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if old_lines[i - 1] == new_lines[j - 1] {
                dp[i - 1][j - 1] + 1
            } else {
                dp[i - 1][j].max(dp[i][j - 1])
            };
        }
    }

    // Backtrack to produce ordered edit ops
    #[derive(Clone, Copy)]
    enum Op {
        Equal(usize, usize),
        Delete(usize),
        Insert(usize),
    }

    let mut ops: Vec<Op> = Vec::new();
    let (mut i, mut j) = (m, n);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            ops.push(Op::Equal(i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            ops.push(Op::Insert(j - 1));
            j -= 1;
        } else {
            ops.push(Op::Delete(i - 1));
            i -= 1;
        }
    }
    ops.reverse();

    // Collect indices of changed ops
    let change_positions: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter(|(_, op)| !matches!(op, Op::Equal(_, _)))
        .map(|(idx, _)| idx)
        .collect();

    if change_positions.is_empty() {
        return (String::new(), 0, 0, truncated);
    }

    // Group changes into hunks with context lines
    let mut hunk_ranges: Vec<(usize, usize)> = Vec::new();
    let last_op = ops.len() - 1;
    let mut hstart = change_positions[0].saturating_sub(DIFF_CONTEXT_LINES);
    let mut hend = (change_positions[0] + DIFF_CONTEXT_LINES).min(last_op);

    for &pos in change_positions.iter().skip(1) {
        if pos <= hend + DIFF_CONTEXT_LINES {
            hend = (pos + DIFF_CONTEXT_LINES).min(last_op);
        } else {
            hunk_ranges.push((hstart, hend));
            hstart = pos.saturating_sub(DIFF_CONTEXT_LINES);
            hend = (pos + DIFF_CONTEXT_LINES).min(last_op);
        }
    }
    hunk_ranges.push((hstart, hend));

    let mut output = format!("--- {old_label}\n+++ {new_label}\n");
    let mut added = 0usize;
    let mut removed = 0usize;

    for (hstart, hend) in hunk_ranges {
        let hunk = &ops[hstart..=hend];

        let old_start = hunk
            .iter()
            .find_map(|op| match op {
                Op::Equal(oi, _) | Op::Delete(oi) => Some(oi + 1),
                Op::Insert(_) => None,
            })
            .unwrap_or(0);

        let new_start = hunk
            .iter()
            .find_map(|op| match op {
                Op::Equal(_, ni) | Op::Insert(ni) => Some(ni + 1),
                Op::Delete(_) => None,
            })
            .unwrap_or(0);

        let old_count = hunk
            .iter()
            .filter(|op| !matches!(op, Op::Insert(_)))
            .count();
        let new_count = hunk
            .iter()
            .filter(|op| !matches!(op, Op::Delete(_)))
            .count();

        output.push_str(&format!(
            "@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"
        ));

        for op in hunk {
            match op {
                Op::Equal(oi, _) => {
                    output.push(' ');
                    output.push_str(old_lines[*oi]);
                    output.push('\n');
                }
                Op::Delete(oi) => {
                    output.push('-');
                    output.push_str(old_lines[*oi]);
                    output.push('\n');
                    removed += 1;
                }
                Op::Insert(ni) => {
                    output.push('+');
                    output.push_str(new_lines[*ni]);
                    output.push('\n');
                    added += 1;
                }
            }
        }
    }

    (output, added, removed, truncated)
}

// ── Config history Tauri commands ─────────────────────────────────────────────

/// List config revision history for a profile (newest first).
/// Returns an error when the metadata store is unavailable.
/// Returns an empty list when the profile has no recorded revisions yet.
#[tauri::command]
pub fn profile_config_history(
    name: String,
    limit: Option<usize>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<ConfigRevisionSummary>, String> {
    if !metadata_store.is_available() {
        return Err("config history is unavailable — metadata store is not accessible".to_string());
    }

    let profile_id = match metadata_store
        .lookup_profile_id(&name)
        .map_err(|e| e.to_string())?
    {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };

    let capped_limit = Some(
        limit
            .unwrap_or(MAX_HISTORY_LIST_LIMIT)
            .min(MAX_HISTORY_LIST_LIMIT),
    );
    let rows = metadata_store
        .list_config_revisions(&profile_id, capped_limit)
        .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|row| ConfigRevisionSummary {
            id: row.id,
            profile_name_at_write: row.profile_name_at_write,
            source: row.source,
            content_hash: row.content_hash,
            source_revision_id: row.source_revision_id,
            is_last_known_working: row.is_last_known_working,
            created_at: row.created_at,
        })
        .collect())
}

/// Diff a specific revision against the current live profile (when `right_revision_id` is
/// `None`) or against another revision. The left side is `revision_id`; the right side is
/// `right_revision_id` or the current persisted profile. Returns a unified diff string and
/// line-change counts.
#[tauri::command]
pub fn profile_config_diff(
    name: String,
    revision_id: i64,
    right_revision_id: Option<i64>,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ConfigDiffResult, String> {
    if !metadata_store.is_available() {
        return Err("config diff is unavailable — metadata store is not accessible".to_string());
    }

    if revision_id <= 0 {
        return Err(format!(
            "revision_id must be a positive integer, got {revision_id}"
        ));
    }
    if let Some(right_id) = right_revision_id {
        if right_id <= 0 {
            return Err(format!(
                "right_revision_id must be a positive integer, got {right_id}"
            ));
        }
    }

    let profile_id = metadata_store
        .lookup_profile_id(&name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("profile '{name}' has no revision history"))?;

    let left_row = metadata_store
        .get_config_revision(&profile_id, revision_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!("revision {revision_id} not found or does not belong to profile '{name}'")
        })?;

    let (right_text, right_label) = if let Some(right_id) = right_revision_id {
        let right_row = metadata_store
            .get_config_revision(&profile_id, right_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                format!("revision {right_id} not found or does not belong to profile '{name}'")
            })?;
        (right_row.snapshot_toml, format!("revision/{right_id}"))
    } else {
        let current_profile = store.load(&name).map_err(map_error)?;
        let current_toml = toml::to_string_pretty(&current_profile)
            .map_err(|e| format!("failed to serialize current profile: {e}"))?;
        (current_toml, "current".to_string())
    };

    let left_label = format!("revision/{revision_id}");
    let (diff_text, added_lines, removed_lines, truncated) = compute_unified_diff(
        &left_label,
        &right_label,
        &left_row.snapshot_toml,
        &right_text,
    );

    if diff_text.len() > MAX_DIFF_OUTPUT_BYTES {
        return Err(format!(
            "diff output for revision {revision_id} exceeds the {MAX_DIFF_OUTPUT_BYTES}-byte limit ({} bytes)",
            diff_text.len()
        ));
    }

    Ok(ConfigDiffResult {
        revision_id,
        revision_source: left_row.source,
        revision_created_at: left_row.created_at,
        diff_text,
        added_lines,
        removed_lines,
        truncated,
    })
}

/// Restore a profile from a specific config revision.
/// Verifies that the revision belongs to the named profile, writes the restored TOML
/// via `ProfileStore`, appends a `RollbackApply` revision row with lineage, and emits
/// `profiles-changed`.
#[tauri::command]
pub fn profile_config_rollback(
    name: String,
    revision_id: i64,
    app: AppHandle,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ConfigRollbackResult, String> {
    if !metadata_store.is_available() {
        return Err("rollback is unavailable — metadata store is not accessible".to_string());
    }

    if revision_id <= 0 {
        return Err(format!(
            "revision_id must be a positive integer, got {revision_id}"
        ));
    }

    let profile_id = metadata_store
        .lookup_profile_id(&name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("profile '{name}' has no revision history"))?;

    let revision = metadata_store
        .get_config_revision(&profile_id, revision_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!("revision {revision_id} not found or does not belong to profile '{name}'")
        })?;

    // Integrity check: re-compute the SHA-256 of the stored snapshot and compare
    // against the recorded content_hash to detect DB corruption or tampering.
    {
        let computed = sha256_hex(revision.snapshot_toml.as_bytes());
        if computed != revision.content_hash {
            return Err(format!(
                "integrity check failed for revision {revision_id}: content hash mismatch"
            ));
        }
    }

    let restored_profile: GameProfile = toml::from_str(&revision.snapshot_toml)
        .map_err(|e| format!("failed to parse snapshot for revision {revision_id}: {e}"))?;

    store.save(&name, &restored_profile).map_err(map_error)?;

    let profile_path = store.base_path.join(format!("{name}.toml"));
    if let Err(e) = metadata_store.observe_profile_write(
        &name,
        &restored_profile,
        &profile_path,
        SyncSource::AppWrite,
        None,
    ) {
        tracing::warn!(
            %e,
            profile_name = %name,
            revision_id,
            "metadata sync after config rollback failed"
        );
    }

    let new_revision_id = capture_config_revision(
        &name,
        &restored_profile,
        ConfigRevisionSource::RollbackApply,
        Some(revision_id),
        &metadata_store,
    );

    emit_profiles_changed(&app, "rollback");

    Ok(ConfigRollbackResult {
        restored_revision_id: revision_id,
        new_revision_id,
        profile: restored_profile,
    })
}

/// Manually mark a specific revision as the last known-good baseline for a profile.
/// Clears the known-good marker from all other revisions for that profile.
#[tauri::command]
pub fn profile_mark_known_good(
    name: String,
    revision_id: i64,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    if !metadata_store.is_available() {
        return Err(
            "marking known-good is unavailable — metadata store is not accessible".to_string(),
        );
    }

    if revision_id <= 0 {
        return Err(format!(
            "revision_id must be a positive integer, got {revision_id}"
        ));
    }

    let profile_id = metadata_store
        .lookup_profile_id(&name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("profile '{name}' has no revision history"))?;

    metadata_store
        .set_known_good_revision(&profile_id, revision_id)
        .map_err(|e| match e {
            MetadataStoreError::Corrupt(_) => {
                format!("revision {revision_id} not found or does not belong to profile '{name}'")
            }
            _ => e.to_string(),
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crosshook_core::export::check_launcher_exists;
    use crosshook_core::profile::{
        GameSection, LaunchSection, LauncherSection, SteamSection, TrainerLoadingMode,
        TrainerSection,
    };
    use std::fs;
    use tempfile::tempdir;

    fn steam_profile(home: &str) -> GameProfile {
        GameProfile {
            game: GameSection {
                name: "Test Game".to_string(),
                executable_path: String::new(),
                custom_cover_art_path: String::new(),
                custom_portrait_art_path: String::new(),
                custom_background_art_path: String::new(),
            },
            trainer: TrainerSection {
                path: "/tmp/trainers/test.exe".to_string(),
                kind: String::new(),
                loading_mode: TrainerLoadingMode::SourceDirectory,
                trainer_type: "unknown".to_string(),
                required_protontricks: Vec::new(),
                community_trainer_sha256: String::new(),
            },
            steam: SteamSection {
                app_id: "12345".to_string(),
                compatdata_path: format!("{home}/.local/share/Steam/steamapps/compatdata/12345"),
                launcher: LauncherSection {
                    display_name: "Test Game".to_string(),
                    icon_path: String::new(),
                },
                ..Default::default()
            },
            launch: LaunchSection {
                method: "steam_applaunch".to_string(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn create_watermarked_launcher_files(script_path: &str, desktop_path: &str) {
        fs::create_dir_all(
            std::path::Path::new(script_path)
                .parent()
                .expect("script parent"),
        )
        .expect("script dirs");
        fs::create_dir_all(
            std::path::Path::new(desktop_path)
                .parent()
                .expect("desktop parent"),
        )
        .expect("desktop dirs");
        fs::write(
            script_path,
            "#!/usr/bin/env bash\n# Generated by CrossHook\n",
        )
        .expect("write script");
        fs::write(
            desktop_path,
            "[Desktop Entry]\nName=Test Game - Trainer\nComment=Generated by CrossHook\n",
        )
        .expect("write desktop");
    }

    #[test]
    fn cleanup_launchers_for_profile_delete_uses_derived_steam_paths() {
        let temp = tempdir().expect("temp dir");
        let home = temp.path().to_string_lossy().into_owned();
        let profile = steam_profile(&home);
        let steam_root = format!("{home}/.local/share/Steam");

        let info = check_launcher_exists(
            &profile.steam.launcher.display_name,
            &profile.steam.app_id,
            &profile.trainer.path,
            &home,
            &steam_root,
        )
        .expect("check launcher exists");
        create_watermarked_launcher_files(&info.script_path, &info.desktop_entry_path);

        let result = cleanup_launchers_for_profile_delete("test-profile", &profile)
            .expect("cleanup should succeed");

        assert!(result.is_some());
        assert!(!std::path::Path::new(&info.script_path).exists());
        assert!(!std::path::Path::new(&info.desktop_entry_path).exists());
    }

    #[test]
    fn cleanup_launchers_for_profile_delete_skips_native_profiles() {
        let profile = GameProfile {
            launch: LaunchSection {
                method: "native".to_string(),
                ..Default::default()
            },
            ..GameProfile::default()
        };

        let result = cleanup_launchers_for_profile_delete("native-profile", &profile)
            .expect("native cleanup should not fail");

        assert!(result.is_none());
    }

    #[test]
    fn save_launch_optimizations_for_profile_updates_only_launch_section() {
        let temp = tempdir().expect("temp dir");
        let store = ProfileStore::with_base_path(temp.path().join("profiles"));
        let home = temp.path().to_string_lossy().into_owned();
        let profile = steam_profile(&home);

        store.save("test-profile", &profile).expect("save profile");

        let optimizations = LaunchOptimizationsPayload {
            enabled_option_ids: vec![
                "disable_steam_input".to_string(),
                "use_gamemode".to_string(),
            ],
            switch_active_preset: None,
        };

        save_launch_optimizations_for_profile("test-profile", &optimizations, &store)
            .expect("save launch optimizations");

        let loaded = store.load("test-profile").expect("load profile");
        assert_eq!(loaded.game, profile.game);
        assert_eq!(loaded.trainer, profile.trainer);
        assert_eq!(loaded.injection, profile.injection);
        assert_eq!(loaded.steam, profile.steam);
        assert_eq!(loaded.runtime, profile.runtime);
        assert_eq!(loaded.launch.method, profile.launch.method);
        assert_eq!(
            loaded.launch.optimizations.enabled_option_ids,
            optimizations.enabled_option_ids
        );
    }

    #[test]
    fn save_launch_optimizations_for_profile_rejects_missing_profiles() {
        let temp = tempdir().expect("temp dir");
        let store = ProfileStore::with_base_path(temp.path().join("profiles"));

        let error = save_launch_optimizations_for_profile(
            "missing-profile",
            &LaunchOptimizationsPayload {
                enabled_option_ids: vec!["use_gamemode".to_string()],
                switch_active_preset: None,
            },
            &store,
        )
        .expect_err("missing profile should fail");

        assert!(error.contains("profile file not found"));
    }

    // ── apply_collection_defaults (M3 + M4 regression tests) ─────────────────
    //
    // These unit tests cover the private helper extracted from `profile_load`:
    //   - Fail-open on missing collection_id (M3 cleanup — no layer 3 no-op).
    //   - Fail-open on transient / not-found errors (preserves launch path).
    //   - Bubble `Corrupt` errors so the frontend can surface them via the
    //     existing `useProfile.loadProfile` error channel (M4).

    mod apply_collection_defaults_tests {
        use super::*;
        use crosshook_core::profile::CollectionDefaultsSection;

        fn profile_with_custom_env(name: &str, value: &str) -> GameProfile {
            let mut profile = GameProfile::default();
            profile
                .launch
                .custom_env_vars
                .insert(name.to_string(), value.to_string());
            profile
        }

        #[test]
        fn none_collection_id_returns_profile_unchanged() {
            let store = MetadataStore::open_in_memory().expect("open in-memory metadata store");
            let profile = profile_with_custom_env("PROFILE_ONLY", "keep-me");

            let result = apply_collection_defaults(profile.clone(), &store, None)
                .expect("None collection id must succeed");

            assert_eq!(result, profile);
        }

        #[test]
        fn empty_collection_id_returns_profile_unchanged() {
            let store = MetadataStore::open_in_memory().expect("open in-memory metadata store");
            let profile = profile_with_custom_env("PROFILE_ONLY", "keep-me");

            // Whitespace-only ids are treated as "no collection context" — this
            // mirrors the normalization in `useProfile.loadProfile` which drops
            // empty trimmed ids before calling the command.
            let result = apply_collection_defaults(profile.clone(), &store, Some("   "))
                .expect("empty collection id must succeed");

            assert_eq!(result, profile);
        }

        #[test]
        fn valid_defaults_merge_into_profile() {
            let store = MetadataStore::open_in_memory().expect("open in-memory metadata store");
            let collection_id = store
                .create_collection("Speedrun Tools")
                .expect("create collection");

            let mut defaults = CollectionDefaultsSection::default();
            defaults.method = Some("proton_run".to_string());
            defaults
                .custom_env_vars
                .insert("CROSSHOOK_PROBE".to_string(), "1".to_string());
            store
                .set_collection_defaults(&collection_id, Some(&defaults))
                .expect("seed collection defaults");

            let profile = profile_with_custom_env("PROFILE_ONLY", "keep-me");
            let result =
                apply_collection_defaults(profile, &store, Some(collection_id.as_str()))
                    .expect("valid defaults must merge");

            assert_eq!(result.launch.method, "proton_run");
            assert_eq!(
                result.launch.custom_env_vars.get("CROSSHOOK_PROBE").cloned(),
                Some("1".to_string()),
                "collection env vars must merge on top of profile env vars"
            );
            assert_eq!(
                result.launch.custom_env_vars.get("PROFILE_ONLY").cloned(),
                Some("keep-me".to_string()),
                "profile env vars without a collision must be preserved"
            );
        }

        #[test]
        fn unknown_collection_id_fails_open_with_unmodified_profile() {
            let store = MetadataStore::open_in_memory().expect("open in-memory metadata store");
            let profile = profile_with_custom_env("PROFILE_ONLY", "keep-me");

            // With the M1 fix, `get_collection_defaults` on a nonexistent
            // collection returns `Validation(...)`. The helper must treat that
            // as fail-open and return the raw profile rather than hard-block
            // the launch.
            let result = apply_collection_defaults(profile.clone(), &store, Some("no-such-id"))
                .expect("unknown collection id must fail open, not propagate Validation");

            assert_eq!(result, profile);
        }

        #[test]
        fn corrupt_defaults_bubble_error_to_caller() {
            let store = MetadataStore::open_in_memory().expect("open in-memory metadata store");
            let collection_id = store.create_collection("Broken").expect("create collection");

            // Force a corrupt JSON payload via raw SQL. `set_collection_defaults`
            // would refuse to write invalid JSON, so we go under it via
            // `with_sqlite_conn`. We avoid the `rusqlite::params!` macro so
            // src-tauri doesn't need a direct rusqlite dev-dep: tuple params
            // implement `rusqlite::Params` directly.
            store
                .with_sqlite_conn("seed corrupt defaults", |conn| {
                    conn.execute(
                        "UPDATE collections SET defaults_json = ?1 WHERE collection_id = ?2",
                        ("{not-valid-json", collection_id.as_str()),
                    )
                    .expect("raw update");
                    Ok(())
                })
                .expect("with_sqlite_conn");

            let profile = profile_with_custom_env("PROFILE_ONLY", "keep-me");
            let err =
                apply_collection_defaults(profile, &store, Some(collection_id.as_str()))
                    .expect_err("corrupt defaults must bubble up");

            assert!(
                matches!(err, MetadataStoreError::Corrupt(_)),
                "corrupt JSON must surface as Corrupt so the launch entrypoint can \
                 show the error, got {err:?}"
            );
        }
    }
}
