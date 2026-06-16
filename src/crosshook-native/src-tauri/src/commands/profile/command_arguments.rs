use crosshook_core::metadata::{ConfigRevisionSource, MetadataStore};
use crosshook_core::profile::ProfileStore;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::shared::{capture_config_revision, map_error, observe_profile_write_launch_change};

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
        capture_config_revision(
            profile_name,
            &updated,
            ConfigRevisionSource::LaunchOptimizationSave,
            None,
            &metadata_store,
        );
    }

    Ok(())
}
