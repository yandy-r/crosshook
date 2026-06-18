use crosshook_core::metadata::{ConfigRevisionSource, MetadataStore};
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::SettingsStore;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::shared::{
    capture_config_revision, map_error, observe_profile_write_launch_change,
    resolve_config_history_max_revisions,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CommandArgumentsPayload {
    #[serde(
        rename = "enabled_argument_ids",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub enabled_argument_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_args: Vec<String>,
}

#[tauri::command]
pub fn profile_save_command_arguments(
    name: String,
    command_arguments: CommandArgumentsPayload,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
    settings_store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let profile_name = name.trim();
    if profile_name.is_empty() {
        return Err("profile name is required".to_string());
    }

    store
        .save_command_arguments(
            profile_name,
            command_arguments.enabled_argument_ids,
            command_arguments.custom_args,
        )
        .map_err(map_error)?;

    if let Ok(updated) = store.load(profile_name) {
        observe_profile_write_launch_change(profile_name, &store, &metadata_store, &updated);
        let max_revisions = resolve_config_history_max_revisions(&settings_store);
        capture_config_revision(
            profile_name,
            &updated,
            ConfigRevisionSource::LaunchCommandArgumentsSave,
            None,
            &metadata_store,
            max_revisions,
        );
    }

    Ok(())
}
