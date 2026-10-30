use std::path::Path;

use tokio::process::Command;

use crate::launch::runtime_helpers::proton_command::build_direct_proton_command_lifecycle_with;
use crate::launch::runtime_helpers::{
    host_environment_map, merge_runtime_proton_into_map, resolve_effective_working_directory,
};
use crate::platform::{is_flatpak, normalize_flatpak_host_path};
use crate::run_executable::RunExecutableRequest;

/// Builds the lifecycle-bound `<proton> run <exe>` command (or
/// `<proton> run msiexec /i <msi> /qb` for `.msi` inputs) for `prefix_path`.
///
/// Under Flatpak the host process is bound to the `flatpak-spawn` proxy
/// (`--watch-bus`), so killing the proxy also stops the host process. Stdio,
/// process group and `kill_on_drop` are left to the caller that spawns and
/// supervises the child.
pub fn build_run_executable_command(request: &RunExecutableRequest, prefix_path: &Path) -> Command {
    build_run_executable_command_with(request, prefix_path, is_flatpak())
}

pub(crate) fn build_run_executable_command_with(
    request: &RunExecutableRequest,
    prefix_path: &Path,
    flatpak: bool,
) -> Command {
    let normalized_executable_path = normalize_flatpak_host_path(&request.executable_path);
    let normalized_working_directory = normalize_flatpak_host_path(&request.working_directory);
    let executable_path = normalized_executable_path.trim();
    let mut env = host_environment_map();
    merge_runtime_proton_into_map(
        &mut env,
        prefix_path.to_string_lossy().as_ref(),
        request.steam_client_install_path.trim(),
    );
    let effective_working_directory = resolve_effective_working_directory(
        normalized_working_directory.trim(),
        Path::new(executable_path),
    );
    let mut command = build_direct_proton_command_lifecycle_with(
        request.proton_path.trim(),
        &env,
        effective_working_directory.as_deref(),
        flatpak,
    );

    if is_msi_path(Path::new(executable_path)) {
        // `msiexec` ships with every Proton/Wine prefix; the `/qb` flag asks for
        // basic UI (progress without modal prompts) so the user still sees what
        // is happening but does not have to babysit a fully interactive run.
        command.arg("msiexec");
        command.arg("/i");
        command.arg(executable_path);
        command.arg("/qb");
    } else {
        command.arg(executable_path);
    }

    command
}

fn is_msi_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("msi"))
}
