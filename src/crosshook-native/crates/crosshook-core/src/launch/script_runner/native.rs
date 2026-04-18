use std::path::Path;

use tokio::process::Command;

use crate::launch::runtime_helpers::{
    apply_working_directory, attach_log_stdio, host_environment_map,
    merge_optimization_and_custom_into_map,
};
use crate::launch::LaunchRequest;
use crate::platform::normalize_flatpak_host_path;

pub fn build_native_game_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<Command> {
    let normalized_game_path = normalize_flatpak_host_path(&request.game_path);
    let normalized_working_directory =
        normalize_flatpak_host_path(&request.runtime.working_directory);
    let mut env = host_environment_map();
    merge_optimization_and_custom_into_map(&mut env, &[], &request.custom_env_vars);
    let mut command = Command::new(normalized_game_path.trim());
    command.envs(&env);
    apply_working_directory(
        &mut command,
        normalized_working_directory.trim(),
        Path::new(normalized_game_path.trim()),
    );
    attach_log_stdio(&mut command, log_path)?;
    Ok(command)
}
