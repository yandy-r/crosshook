//! Profile Tauri commands: thin async delegates to
//! `crosshook_app::services::profiles::ProfileWriter` (CRUD and import/export,
//! launch optimization, section and preset saves, favorites, command arguments
//! and config revision history).

use std::path::PathBuf;

use crosshook_app::ipc::unavailable_or;
use crosshook_app::services::profiles::{
    BundledOptimizationPresetDto, CommandArgumentsPayload, ConfigDiffResult, ConfigRevisionSummary,
    ConfigRollbackResult, LaunchOptimizationsPayload, ProfileSummary,
};
use crosshook_core::profile::{
    DuplicateProfileResult, GameProfile, GamescopeConfig, MangoHudConfig,
};

use super::{AppState, IpcResult};
use crate::error_map::{commit, ipc};

#[tauri::command]
pub async fn profile_list(app: AppState<'_>) -> IpcResult<Vec<String>> {
    ipc(app.services().profiles.list().await)
}

#[tauri::command]
pub async fn profile_load(
    name: String,
    collection_id: Option<String>,
    app: AppState<'_>,
) -> IpcResult<GameProfile> {
    let services = app.services();
    let profile = ipc(services.profiles.load(name).await)?;
    let mut profile = ipc(services
        .collections
        .apply_defaults(profile, collection_id.as_deref())
        .await)?;
    profile.clear_extra();
    Ok(profile)
}

#[tauri::command]
pub async fn profile_list_summaries(
    collection_id: Option<String>,
    app: AppState<'_>,
) -> IpcResult<Vec<ProfileSummary>> {
    let services = app.services();
    let collections = services.collections.clone();
    ipc(services
        .profiles
        .list_summaries(move || collections.resolve_defaults_sync(collection_id.as_deref()))
        .await)
}

#[tauri::command]
pub async fn profile_save(name: String, data: GameProfile, app: AppState<'_>) -> IpcResult<()> {
    commit(app.services().profiles.save(name, data).await)
}

#[tauri::command]
pub async fn profile_delete(name: String, app: AppState<'_>) -> IpcResult<()> {
    commit(app.services().profiles.delete(name).await)
}

/// Duplicates an existing profile under a unique copy name.
///
/// The returned [`DuplicateProfileResult`] is serialized to the frontend where it
/// drives profile list refresh and auto-selection of the copy.
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
pub async fn profile_duplicate(
    name: String,
    app: AppState<'_>,
) -> IpcResult<DuplicateProfileResult> {
    commit(app.services().profiles.duplicate(name).await)
}

#[tauri::command]
pub async fn profile_rename(
    old_name: String,
    new_name: String,
    app: AppState<'_>,
) -> IpcResult<bool> {
    commit(app.services().profiles.rename(old_name, new_name).await)
}

#[tauri::command]
pub async fn profile_import_legacy(path: String, app: AppState<'_>) -> IpcResult<GameProfile> {
    commit(
        app.services()
            .profiles
            .import_legacy(PathBuf::from(path))
            .await,
    )
}

#[tauri::command]
pub async fn profile_export_toml(
    name: String,
    data: GameProfile,
    include_hooks: Option<bool>,
    app: AppState<'_>,
) -> IpcResult<String> {
    ipc(app
        .services()
        .profiles
        .export_toml(name, data, include_hooks.unwrap_or(false))
        .await)
}

#[tauri::command]
pub async fn profile_set_favorite(
    name: String,
    favorite: bool,
    app: AppState<'_>,
) -> IpcResult<()> {
    commit(app.services().profiles.set_favorite(name, favorite).await)
}

#[tauri::command]
pub async fn profile_list_favorites(app: AppState<'_>) -> IpcResult<Vec<String>> {
    ipc(app.services().profiles.list_favorites().await)
}

#[tauri::command]
pub async fn profile_save_launch_optimizations(
    name: String,
    optimizations: LaunchOptimizationsPayload,
    app: AppState<'_>,
) -> IpcResult<()> {
    let services = app.services();
    commit(
        services
            .profiles
            .save_launch_optimizations(name, optimizations)
            .await,
    )
}

#[tauri::command]
pub async fn profile_save_mangohud_config(
    name: String,
    config: MangoHudConfig,
    app: AppState<'_>,
) -> IpcResult<()> {
    commit(
        app.services()
            .profiles
            .save_mangohud_config(name, config)
            .await,
    )
}

#[tauri::command]
pub async fn profile_save_gamescope_config(
    name: String,
    config: GamescopeConfig,
    app: AppState<'_>,
) -> IpcResult<()> {
    commit(
        app.services()
            .profiles
            .save_gamescope_config(name, config)
            .await,
    )
}

#[tauri::command]
pub async fn profile_save_trainer_gamescope_config(
    name: String,
    config: GamescopeConfig,
    app: AppState<'_>,
) -> IpcResult<()> {
    let services = app.services();
    commit(
        services
            .profiles
            .save_trainer_gamescope_config(name, config)
            .await,
    )
}

/// Lists bundled presets; an unavailable metadata store yields an empty list,
/// as the editor treats "no presets" and "no metadata" alike.
#[tauri::command]
pub async fn profile_list_bundled_optimization_presets(
    app: AppState<'_>,
) -> IpcResult<Vec<BundledOptimizationPresetDto>> {
    ipc(unavailable_or(
        app.services()
            .profiles
            .list_bundled_optimization_presets()
            .await,
        Vec::new(),
    ))
}

#[tauri::command]
pub async fn profile_apply_bundled_optimization_preset(
    name: String,
    preset_id: String,
    app: AppState<'_>,
) -> IpcResult<GameProfile> {
    let services = app.services();
    commit(
        services
            .profiles
            .apply_bundled_optimization_preset(name, preset_id)
            .await,
    )
}

#[tauri::command]
pub async fn profile_save_manual_optimization_preset(
    name: String,
    preset_name: String,
    enabled_option_ids: Vec<String>,
    app: AppState<'_>,
) -> IpcResult<GameProfile> {
    let services = app.services();
    commit(
        services
            .profiles
            .save_manual_optimization_preset(name, preset_name, enabled_option_ids)
            .await,
    )
}

#[tauri::command]
pub async fn profile_save_command_arguments(
    name: String,
    command_arguments: CommandArgumentsPayload,
    resolved_launch_method: Option<String>,
    app: AppState<'_>,
) -> IpcResult<()> {
    let services = app.services();
    commit(
        services
            .profiles
            .save_command_arguments(name, command_arguments, resolved_launch_method)
            .await,
    )
}

/// List config revision history for a profile (newest first).
/// Returns an error when the metadata store is unavailable.
/// Returns an empty list when the profile has no recorded revisions yet.
#[tauri::command]
pub async fn profile_config_history(
    name: String,
    limit: Option<usize>,
    app: AppState<'_>,
) -> IpcResult<Vec<ConfigRevisionSummary>> {
    ipc(app.services().profiles.config_history(name, limit).await)
}

/// Diff a specific revision against the current live profile (when `right_revision_id` is
/// `None`) or against another revision. The left side is `revision_id`; the right side is
/// `right_revision_id` or the current persisted profile. Returns a unified diff string and
/// line-change counts.
#[tauri::command]
pub async fn profile_config_diff(
    name: String,
    revision_id: i64,
    right_revision_id: Option<i64>,
    mode: Option<String>,
    app: AppState<'_>,
) -> IpcResult<ConfigDiffResult> {
    ipc(app
        .services()
        .profiles
        .config_diff(name, revision_id, right_revision_id, mode)
        .await)
}

/// Restore a profile from a specific config revision.
/// Verifies ownership and integrity before writing the restored TOML, appends a
/// `RollbackApply` revision row with lineage, and emits `profiles-changed`.
#[tauri::command]
pub async fn profile_config_rollback(
    name: String,
    revision_id: i64,
    app: AppState<'_>,
) -> IpcResult<ConfigRollbackResult> {
    commit(
        app.services()
            .profiles
            .config_rollback(name, revision_id)
            .await,
    )
}

#[tauri::command]
pub async fn profile_mark_known_good(
    name: String,
    revision_id: i64,
    app: AppState<'_>,
) -> IpcResult<()> {
    ipc(app
        .services()
        .profiles
        .mark_known_good(name, revision_id)
        .await)
}
