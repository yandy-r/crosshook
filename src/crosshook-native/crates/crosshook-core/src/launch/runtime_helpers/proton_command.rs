use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use tokio::process::Command;

use crate::launch::request::LaunchRequest;
use crate::platform::{
    self, host_command_with_env_and_directory, host_command_with_env_and_directory_inner,
    normalize_flatpak_host_path,
};
use crate::profile::{GamescopeConfig, GamescopeFilter, TrainerLoadingMode};

use super::FLATPAK_GAMESCOPE_PID_CAPTURE_SCRIPT;

fn normalize_host_command_entry(raw_entry: &str) -> String {
    normalize_flatpak_host_path(raw_entry).trim().to_string()
}

fn push_unique_pressure_vessel_path(
    collected: &mut Vec<String>,
    seen: &mut HashSet<String>,
    candidate: String,
) {
    if !candidate.is_empty() && seen.insert(candidate.clone()) {
        collected.push(candidate);
    }
}

fn collect_pressure_vessel_parent(
    collected: &mut Vec<String>,
    seen: &mut HashSet<String>,
    raw_path: &str,
) {
    let normalized = normalize_flatpak_host_path(raw_path);
    let trimmed = normalized.trim();
    let Some(parent) = Path::new(trimmed).parent() else {
        return;
    };
    if parent.as_os_str().is_empty() {
        return;
    }

    push_unique_pressure_vessel_path(collected, seen, parent.to_string_lossy().into_owned());
}

pub fn collect_pressure_vessel_paths(request: &LaunchRequest) -> Vec<String> {
    let mut collected = Vec::new();
    let mut seen = HashSet::new();

    collect_pressure_vessel_parent(&mut collected, &mut seen, &request.game_path);

    if request.trainer_loading_mode == TrainerLoadingMode::SourceDirectory
        && !request.trainer_host_path.trim().is_empty()
    {
        collect_pressure_vessel_parent(&mut collected, &mut seen, &request.trainer_host_path);
    }

    if !request.runtime.working_directory.trim().is_empty() {
        let working_directory = normalize_flatpak_host_path(&request.runtime.working_directory)
            .trim()
            .to_string();
        push_unique_pressure_vessel_path(&mut collected, &mut seen, working_directory);
    }

    collected
}

/// Builds a direct Proton `run` command with wrappers, threading `env` through
/// [`host_command_with_env`] so Flatpak preserves `WINEPREFIX` / `STEAM_COMPAT_*`.
pub fn build_direct_proton_command_with_wrappers(
    proton_path: &str,
    wrappers: &[String],
    env: &BTreeMap<String, String>,
) -> Command {
    build_direct_proton_command_with_wrappers_in_directory(
        proton_path,
        wrappers,
        env,
        None,
        &BTreeMap::new(),
        false,
    )
}

pub fn build_direct_proton_command_with_wrappers_in_directory(
    proton_path: &str,
    wrappers: &[String],
    env: &BTreeMap<String, String>,
    working_directory: Option<&str>,
    custom_env_vars: &BTreeMap<String, String>,
    use_umu: bool,
) -> Command {
    let trimmed_proton = normalize_host_command_entry(proton_path);
    let normalized_wrappers: Vec<_> = wrappers
        .iter()
        .map(|wrapper| normalize_host_command_entry(wrapper))
        .filter(|entry| !entry.is_empty())
        .collect();
    if normalized_wrappers.is_empty() {
        let mut command = host_command_with_env_and_directory(
            trimmed_proton.as_str(),
            env,
            working_directory,
            custom_env_vars,
        );
        if !use_umu {
            command.arg("run");
        }
        return command;
    }

    let mut command = host_command_with_env_and_directory(
        normalized_wrappers[0].as_str(),
        env,
        working_directory,
        custom_env_vars,
    );
    for wrapper in normalized_wrappers.iter().skip(1) {
        command.arg(wrapper);
    }
    command.arg(trimmed_proton);
    if !use_umu {
        command.arg("run");
    }
    command
}

pub fn new_direct_proton_command(proton_path: &str) -> Command {
    build_direct_proton_command_with_wrappers(proton_path, &[], &BTreeMap::new())
}

pub fn new_direct_proton_command_with_wrappers(proton_path: &str, wrappers: &[String]) -> Command {
    build_direct_proton_command_with_wrappers(proton_path, wrappers, &BTreeMap::new())
}

pub fn build_gamescope_args(config: &GamescopeConfig) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();

    if let Some(w) = config.internal_width {
        args.push("-w".to_string());
        args.push(w.to_string());
    }
    if let Some(h) = config.internal_height {
        args.push("-h".to_string());
        args.push(h.to_string());
    }
    if let Some(w) = config.output_width {
        args.push("-W".to_string());
        args.push(w.to_string());
    }
    if let Some(h) = config.output_height {
        args.push("-H".to_string());
        args.push(h.to_string());
    }
    if let Some(r) = config.frame_rate_limit {
        args.push("-r".to_string());
        args.push(r.to_string());
    }
    if let Some(sharpness) = config.fsr_sharpness {
        args.push("--fsr-sharpness".to_string());
        args.push(sharpness.to_string());
    }
    if let Some(filter) = &config.upscale_filter {
        let filter_str = match filter {
            GamescopeFilter::Fsr => "fsr",
            GamescopeFilter::Nis => "nis",
            GamescopeFilter::Linear => "linear",
            GamescopeFilter::Nearest => "nearest",
            GamescopeFilter::Pixel => "pixel",
        };
        args.push("--filter".to_string());
        args.push(filter_str.to_string());
    }
    if config.fullscreen {
        args.push("-f".to_string());
    }
    if config.borderless {
        args.push("-b".to_string());
    }
    if config.grab_cursor {
        args.push("--grab".to_string());
    }
    if config.force_grab_cursor {
        args.push("--force-grab-cursor".to_string());
    }
    if config.hdr_enabled {
        args.push("--hdr-enabled".to_string());
    }
    for extra in &config.extra_args {
        args.push(extra.clone());
    }

    args
}

pub fn build_proton_command_with_gamescope(
    proton_path: &str,
    wrappers: &[String],
    gamescope_args: &[String],
    env: &BTreeMap<String, String>,
) -> Command {
    build_proton_command_with_gamescope_in_directory(
        proton_path,
        wrappers,
        gamescope_args,
        env,
        None,
        &BTreeMap::new(),
        false,
    )
}

pub fn build_proton_command_with_gamescope_in_directory(
    proton_path: &str,
    wrappers: &[String],
    gamescope_args: &[String],
    env: &BTreeMap<String, String>,
    working_directory: Option<&str>,
    custom_env_vars: &BTreeMap<String, String>,
    use_umu: bool,
) -> Command {
    build_proton_command_with_gamescope_pid_capture_in_directory(
        proton_path,
        wrappers,
        gamescope_args,
        env,
        working_directory,
        custom_env_vars,
        None,
        use_umu,
    )
}

pub fn build_proton_command_with_gamescope_pid_capture_in_directory(
    proton_path: &str,
    wrappers: &[String],
    gamescope_args: &[String],
    env: &BTreeMap<String, String>,
    working_directory: Option<&str>,
    custom_env_vars: &BTreeMap<String, String>,
    pid_capture_path: Option<&Path>,
    use_umu: bool,
) -> Command {
    build_proton_command_with_gamescope_pid_capture_in_directory_inner(
        proton_path,
        wrappers,
        gamescope_args,
        env,
        working_directory,
        custom_env_vars,
        pid_capture_path,
        platform::is_flatpak(),
        use_umu,
    )
}

pub(crate) fn build_proton_command_with_gamescope_pid_capture_in_directory_inner(
    proton_path: &str,
    wrappers: &[String],
    gamescope_args: &[String],
    env: &BTreeMap<String, String>,
    working_directory: Option<&str>,
    custom_env_vars: &BTreeMap<String, String>,
    pid_capture_path: Option<&Path>,
    flatpak: bool,
    use_umu: bool,
) -> Command {
    let normalized_proton = normalize_host_command_entry(proton_path);
    let normalized_wrappers: Vec<_> = wrappers
        .iter()
        .map(|wrapper| normalize_host_command_entry(wrapper))
        .filter(|entry| !entry.is_empty())
        .collect();

    if flatpak {
        if let Some(pid_capture_path) = pid_capture_path {
            let mut command = host_command_with_env_and_directory_inner(
                "bash",
                env,
                working_directory,
                flatpak,
                custom_env_vars,
            );
            command.arg("-c");
            // The shell writes its PID, then `exec`s gamescope so the recorded host PID
            // becomes the real compositor PID that the watchdog must later signal.
            command.arg(FLATPAK_GAMESCOPE_PID_CAPTURE_SCRIPT);
            command.arg("bash");
            command.arg(pid_capture_path);
            command.arg("gamescope");
            for arg in gamescope_args {
                command.arg(arg.trim());
            }
            command.arg("--");
            for wrapper in &normalized_wrappers {
                command.arg(wrapper);
            }
            command.arg(normalized_proton);
            if !use_umu {
                command.arg("run");
            }
            return command;
        }
    }

    let mut command = host_command_with_env_and_directory_inner(
        "gamescope",
        env,
        working_directory,
        flatpak,
        custom_env_vars,
    );
    for arg in gamescope_args {
        command.arg(arg.trim());
    }
    command.arg("--");
    for wrapper in normalized_wrappers {
        command.arg(wrapper);
    }
    command.arg(normalized_proton);
    if !use_umu {
        command.arg("run");
    }
    command
}

pub fn new_proton_command_with_gamescope(
    proton_path: &str,
    wrappers: &[String],
    gamescope_args: &[String],
) -> Command {
    build_proton_command_with_gamescope(proton_path, wrappers, gamescope_args, &BTreeMap::new())
}
