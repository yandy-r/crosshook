use crosshook_app::services::profiles::{CommandArgumentsPayload, ProfileCommit};
use crosshook_app::ProfileWriter;
use tauri::State;

#[tauri::command]
pub async fn profile_save_command_arguments(
    name: String,
    command_arguments: CommandArgumentsPayload,
    resolved_launch_method: Option<String>,
    writer: State<'_, ProfileWriter>,
) -> Result<(), String> {
    writer
        .save_command_arguments(name, command_arguments, resolved_launch_method)
        .await
        .map(ProfileCommit::into_output)
        .map_err(|error| error.to_string())
}
