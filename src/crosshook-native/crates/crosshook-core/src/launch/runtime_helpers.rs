use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;

use tokio::process::Command;

use crate::launch::request::LaunchRequest;
use crate::platform::{
    self, host_command_with_env_and_directory, host_command_with_env_and_directory_inner,
    normalize_flatpak_host_path,
};
use crate::profile::{GamescopeConfig, GamescopeFilter, TrainerLoadingMode};

/// Default `PATH` used when the host environment does not set `PATH` (matches `apply_host_environment`).
pub const DEFAULT_HOST_PATH: &str = "/usr/bin:/bin";
const DEFAULT_SHELL: &str = "/bin/bash";
const FLATPAK_GAMESCOPE_PID_CAPTURE_SCRIPT: &str =
    "capture_dir=$(dirname -- \"$1\"); mkdir -p -- \"$capture_dir\"; printf '%s' \"$$\" > \"$1\"; shift; exec \"$@\"";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProtonPaths {
    pub wine_prefix_path: PathBuf,
    pub compat_data_path: PathBuf,
}

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

fn build_proton_command_with_gamescope_pid_capture_in_directory_inner(
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

pub fn apply_launch_optimization_environment(
    command: &mut Command,
    env_pairs: &[(String, String)],
) {
    apply_env_pairs(command, env_pairs);
}

/// Applies `KEY=value` pairs to the command environment (last write wins per key).
pub fn apply_env_pairs(command: &mut Command, env_pairs: &[(String, String)]) {
    for (key, value) in env_pairs {
        set_env(command, key, value);
    }
}

/// Applies profile `custom_env_vars` after optimizations so custom values win on duplicate keys.
pub fn apply_custom_env_vars(command: &mut Command, custom: &BTreeMap<String, String>) {
    for (key, value) in custom {
        set_env(command, key, value);
    }
}

/// Applies launch optimization env, then custom env (custom overrides on key conflict).
pub fn apply_optimization_and_custom_environment(
    command: &mut Command,
    optimization_env: &[(String, String)],
    custom_env_vars: &BTreeMap<String, String>,
) {
    apply_launch_optimization_environment(command, optimization_env);
    apply_custom_env_vars(command, custom_env_vars);
}

/// Host-style environment keys shared by Proton helpers and trainer/game launches.
pub fn host_environment_map() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("HOME".to_string(), env_value("HOME", ""));
    m.insert("USER".to_string(), env_value("USER", ""));
    m.insert("LOGNAME".to_string(), env_value("LOGNAME", ""));
    m.insert("SHELL".to_string(), env_value("SHELL", DEFAULT_SHELL));
    m.insert("PATH".to_string(), env_value("PATH", DEFAULT_HOST_PATH));
    m.insert("DISPLAY".to_string(), env_value("DISPLAY", ""));
    m.insert(
        "WAYLAND_DISPLAY".to_string(),
        env_value("WAYLAND_DISPLAY", ""),
    );
    m.insert(
        "XDG_RUNTIME_DIR".to_string(),
        env_value("XDG_RUNTIME_DIR", ""),
    );
    m.insert(
        "DBUS_SESSION_BUS_ADDRESS".to_string(),
        env_value("DBUS_SESSION_BUS_ADDRESS", ""),
    );
    m
}

pub fn merge_runtime_proton_into_map(
    map: &mut BTreeMap<String, String>,
    prefix_path: &str,
    steam_client_install_path: &str,
) {
    let normalized_prefix_path = normalize_flatpak_host_path(prefix_path);
    let resolved_paths = resolve_proton_paths(Path::new(normalized_prefix_path.trim()));
    map.insert(
        "WINEPREFIX".to_string(),
        resolved_paths
            .wine_prefix_path
            .to_string_lossy()
            .into_owned(),
    );
    map.insert(
        "STEAM_COMPAT_DATA_PATH".to_string(),
        resolved_paths
            .compat_data_path
            .to_string_lossy()
            .into_owned(),
    );
    if let Some(steam_client_install_path) =
        resolve_steam_client_install_path(steam_client_install_path)
    {
        map.insert(
            "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
            steam_client_install_path,
        );
    }
}

pub fn merge_optimization_and_custom_into_map(
    map: &mut BTreeMap<String, String>,
    optimization_env: &[(String, String)],
    custom_env_vars: &BTreeMap<String, String>,
) {
    for (key, value) in optimization_env {
        map.insert(key.clone(), value.clone());
    }
    for (key, value) in custom_env_vars {
        map.insert(key.clone(), value.clone());
    }
}

pub fn apply_host_environment(command: &mut Command) {
    for (key, value) in host_environment_map() {
        set_env(command, &key, &value);
    }
}

pub fn apply_runtime_proton_environment(
    command: &mut Command,
    prefix_path: &str,
    steam_client_install_path: &str,
) {
    let mut m = BTreeMap::new();
    merge_runtime_proton_into_map(&mut m, prefix_path, steam_client_install_path);
    for (k, v) in m {
        set_env(command, &k, &v);
    }
}

pub fn resolve_wine_prefix_path(prefix_path: &Path) -> PathBuf {
    if prefix_path.file_name().and_then(|value| value.to_str()) == Some("pfx") {
        return prefix_path.to_path_buf();
    }

    let pfx_path = prefix_path.join("pfx");
    if pfx_path.is_dir() {
        pfx_path
    } else {
        prefix_path.to_path_buf()
    }
}

pub fn resolve_proton_paths(prefix_path: &Path) -> ResolvedProtonPaths {
    let wine_prefix_path = resolve_wine_prefix_path(prefix_path);
    let compat_data_path = resolve_compat_data_path(prefix_path, &wine_prefix_path);
    ResolvedProtonPaths {
        wine_prefix_path,
        compat_data_path,
    }
}

fn resolve_compat_data_path(configured_prefix_path: &Path, wine_prefix_path: &Path) -> PathBuf {
    if wine_prefix_path
        .file_name()
        .and_then(|value| value.to_str())
        == Some("pfx")
    {
        wine_prefix_path
            .parent()
            .unwrap_or(configured_prefix_path)
            .to_path_buf()
    } else {
        configured_prefix_path.to_path_buf()
    }
}

pub fn apply_working_directory(
    command: &mut Command,
    configured_directory: &str,
    primary_path: &Path,
) {
    if let Some(directory) = resolve_effective_working_directory(configured_directory, primary_path)
    {
        command.current_dir(directory);
    }
}

pub fn resolve_effective_working_directory(
    configured_directory: &str,
    primary_path: &Path,
) -> Option<String> {
    let trimmed = configured_directory.trim();
    if !trimmed.is_empty() {
        return Some(trimmed.to_string());
    }

    if let Some(parent) = primary_path.parent() {
        if !parent.as_os_str().is_empty() {
            return Some(parent.to_string_lossy().into_owned());
        }
    }

    None
}

pub fn attach_log_stdio(command: &mut Command, log_path: &Path) -> std::io::Result<()> {
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let stderr = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    command.stdout(Stdio::from(stdout));
    command.stderr(Stdio::from(stderr));
    Ok(())
}

pub fn resolve_steam_client_install_path(configured_path: &str) -> Option<String> {
    let steam_client_install_path = env::var("STEAM_COMPAT_CLIENT_INSTALL_PATH").ok();
    resolve_steam_client_install_path_with_home(
        configured_path,
        steam_client_install_path.as_deref(),
        env::var_os("HOME").map(PathBuf::from),
    )
}

fn resolve_steam_client_install_path_with_home(
    configured_path: &str,
    env_steam_client_install_path: Option<&str>,
    home_path: Option<PathBuf>,
) -> Option<String> {
    if let Some(path) = validated_steam_client_install_path(configured_path) {
        return Some(path);
    }

    if let Some(path) = env_steam_client_install_path.and_then(validated_steam_client_install_path)
    {
        return Some(path);
    }

    let home_path = home_path?;
    for candidate in [
        home_path.join(".local/share/Steam"),
        home_path.join(".steam/root"),
        home_path.join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ] {
        if is_steam_client_install_root(&candidate) {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }

    None
}

fn validated_steam_client_install_path(raw_path: &str) -> Option<String> {
    let normalized = normalize_flatpak_host_path(raw_path);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }

    let candidate = Path::new(trimmed);
    if is_steam_client_install_root(candidate) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn is_steam_client_install_root(path: &Path) -> bool {
    path.join("steamapps").is_dir()
        && (path.join("config").is_dir()
            || path.join("steam.sh").is_file()
            || path.join("ubuntu12_32").is_dir())
}

pub(crate) fn env_value(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Returns the absolute path to `umu-run` if found on `PATH`, otherwise `None`.
pub fn resolve_umu_run_path() -> Option<String> {
    #[cfg(test)]
    if let Some(path) = crate::launch::optimizations::resolve_umu_run_path_for_test() {
        return path;
    }

    fn first_executable_umu_on_path(path_value: &std::ffi::OsStr) -> Option<String> {
        for directory in env::split_paths(path_value) {
            let candidate = directory.join("umu-run");
            if is_executable_file(&candidate) {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
        None
    }

    // Under Flatpak, `PATH` reflects the sandbox — do not probe it. Prefer the
    // host environment file Flatpak exposes, then a static host default; never
    // fall back to the sandbox process `PATH`.
    if env::var_os("FLATPAK_ID").is_some() {
        const HOST_ENV_PATH: &str = "/run/host/env/PATH";
        if let Ok(bytes) = fs::read(HOST_ENV_PATH) {
            let host_path = String::from_utf8_lossy(&bytes);
            let trimmed = host_path.trim();
            if !trimmed.is_empty() {
                if let Some(path) = first_executable_umu_on_path(std::ffi::OsStr::new(trimmed)) {
                    return Some(path);
                }
            }
        }
        if let Some(path) = first_executable_umu_on_path(std::ffi::OsStr::new(DEFAULT_HOST_PATH)) {
            return Some(path);
        }
        return None;
    }

    let path_value =
        env::var_os("PATH").unwrap_or_else(|| std::ffi::OsString::from(DEFAULT_HOST_PATH));
    first_executable_umu_on_path(path_value.as_os_str())
}

pub(crate) fn is_executable_file(path: &Path) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

/// Returns `true` if `unshare --net` is available for the current user.
///
/// Probes by running `unshare --net true` (which immediately exits).
/// Returns `false` if the binary is missing or the kernel blocks the operation.
///
/// The result is cached for the lifetime of the process via `OnceLock` since
/// kernel policy does not change within a single application session.
/// Runtime facts for Flatpak UI badges and launch validation (not persisted).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPlatformCapabilities {
    pub is_flatpak: bool,
    pub unshare_net_available: bool,
}

/// Returns whether CrossHook is sandboxed as Flatpak and whether host `unshare --net` works.
pub fn launch_platform_capabilities() -> LaunchPlatformCapabilities {
    LaunchPlatformCapabilities {
        is_flatpak: platform::is_flatpak(),
        unshare_net_available: is_unshare_net_available(),
    }
}

pub fn is_unshare_net_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let mut cmd = if platform::is_flatpak() {
            platform::host_std_command("unshare")
        } else {
            std::process::Command::new("unshare")
        };
        cmd.args(["--net", "true"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        cmd.status().map(|s| s.success()).unwrap_or(false)
    })
}

fn set_env(command: &mut Command, key: &str, value: impl AsRef<str>) {
    command.env(key, value.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::request::RuntimeLaunchConfig;
    use crate::profile::GamescopeFilter;
    use std::fs;

    fn write_steam_client_root(path: &Path) {
        fs::create_dir_all(path.join("steamapps")).expect("steamapps");
        fs::create_dir_all(path.join("config")).expect("config");
    }

    #[test]
    fn build_gamescope_args_default_returns_empty() {
        let config = GamescopeConfig::default();
        let args = build_gamescope_args(&config);
        assert!(args.is_empty());
    }

    #[test]
    fn build_gamescope_args_resolution_and_fps() {
        let config = GamescopeConfig {
            internal_width: Some(1280),
            internal_height: Some(800),
            output_width: Some(1920),
            output_height: Some(1080),
            frame_rate_limit: Some(60),
            ..Default::default()
        };
        let args = build_gamescope_args(&config);
        assert_eq!(
            args,
            vec!["-w", "1280", "-h", "800", "-W", "1920", "-H", "1080", "-r", "60"]
        );
    }

    #[test]
    fn build_gamescope_args_all_flags() {
        let config = GamescopeConfig {
            fullscreen: true,
            borderless: true,
            grab_cursor: true,
            force_grab_cursor: true,
            hdr_enabled: true,
            fsr_sharpness: Some(5),
            upscale_filter: Some(GamescopeFilter::Fsr),
            ..Default::default()
        };
        let args = build_gamescope_args(&config);
        assert!(args.contains(&"--fsr-sharpness".to_string()));
        assert!(args.contains(&"5".to_string()));
        assert!(args.contains(&"--filter".to_string()));
        assert!(args.contains(&"fsr".to_string()));
        assert!(args.contains(&"-f".to_string()));
        assert!(args.contains(&"-b".to_string()));
        assert!(args.contains(&"--grab".to_string()));
        assert!(args.contains(&"--force-grab-cursor".to_string()));
        assert!(args.contains(&"--hdr-enabled".to_string()));
    }

    #[test]
    fn build_gamescope_args_extra_args_passthrough() {
        let config = GamescopeConfig {
            extra_args: vec!["--expose-wayland".to_string(), "--rt".to_string()],
            ..Default::default()
        };
        let args = build_gamescope_args(&config);
        assert_eq!(args, vec!["--expose-wayland", "--rt"]);
    }

    #[test]
    fn collect_pressure_vessel_paths_empty_request_returns_empty() {
        assert!(collect_pressure_vessel_paths(&LaunchRequest::default()).is_empty());
    }

    #[test]
    fn collect_pressure_vessel_paths_game_trainer_working_dir_deduped() {
        let request = LaunchRequest {
            game_path: "/opt/games/TheGame/game.exe".to_string(),
            trainer_host_path: "/opt/trainers/trainer.exe".to_string(),
            trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
            runtime: RuntimeLaunchConfig {
                working_directory: "/opt/games/TheGame".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            collect_pressure_vessel_paths(&request),
            vec![
                "/opt/games/TheGame".to_string(),
                "/opt/trainers".to_string(),
            ]
        );
    }

    #[test]
    fn collect_pressure_vessel_paths_game_equals_working_dir_collapses() {
        let request = LaunchRequest {
            game_path: "/opt/games/TheGame/game.exe".to_string(),
            runtime: RuntimeLaunchConfig {
                working_directory: "/opt/games/TheGame".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            collect_pressure_vessel_paths(&request),
            vec!["/opt/games/TheGame".to_string()]
        );
    }

    #[test]
    fn collect_pressure_vessel_paths_copy_to_prefix_omits_trainer_dir() {
        let request = LaunchRequest {
            game_path: "/opt/games/TheGame/game.exe".to_string(),
            trainer_host_path: "/opt/trainers/trainer.exe".to_string(),
            trainer_loading_mode: TrainerLoadingMode::CopyToPrefix,
            runtime: RuntimeLaunchConfig {
                working_directory: "/opt/games/TheGame".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            collect_pressure_vessel_paths(&request),
            vec!["/opt/games/TheGame".to_string()]
        );
    }

    #[test]
    fn collect_pressure_vessel_paths_empty_trainer_host_path_source_directory_omits_entry() {
        let request = LaunchRequest {
            game_path: "/opt/games/TheGame/game.exe".to_string(),
            trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
            runtime: RuntimeLaunchConfig {
                working_directory: "/opt/working".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            collect_pressure_vessel_paths(&request),
            vec!["/opt/games/TheGame".to_string(), "/opt/working".to_string()]
        );
    }

    #[test]
    fn collect_pressure_vessel_paths_flatpak_host_prefix_normalized() {
        let request = LaunchRequest {
            game_path: "/run/host/opt/games/TheGame/game.exe".to_string(),
            trainer_host_path: "/run/host/opt/trainers/trainer.exe".to_string(),
            trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
            runtime: RuntimeLaunchConfig {
                working_directory: " /run/host/opt/games/TheGame ".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            collect_pressure_vessel_paths(&request),
            vec![
                "/opt/games/TheGame".to_string(),
                "/opt/trainers".to_string(),
            ]
        );
    }

    #[test]
    fn collect_pressure_vessel_paths_root_directory_preserved() {
        let request = LaunchRequest {
            game_path: "/game.exe".to_string(),
            ..Default::default()
        };

        assert_eq!(
            collect_pressure_vessel_paths(&request),
            vec!["/".to_string()]
        );
    }

    #[test]
    fn resolve_steam_client_install_path_accepts_valid_configured_root() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let configured_root = temp_home.path().join("steam-root");
        write_steam_client_root(&configured_root);

        let resolved = resolve_steam_client_install_path_with_home(
            configured_root.to_string_lossy().as_ref(),
            None,
            Some(temp_home.path().to_path_buf()),
        );

        assert_eq!(
            resolved,
            Some(configured_root.to_string_lossy().into_owned())
        );
    }

    #[test]
    fn resolve_steam_client_install_path_rejects_library_root_and_falls_back_to_default() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let library_root = temp_home.path().join("SteamLibrary");
        let default_root = temp_home.path().join(".local/share/Steam");
        fs::create_dir_all(library_root.join("steamapps")).expect("library steamapps");
        write_steam_client_root(&default_root);

        let resolved = resolve_steam_client_install_path_with_home(
            library_root.to_string_lossy().as_ref(),
            None,
            Some(temp_home.path().to_path_buf()),
        );

        assert_eq!(resolved, Some(default_root.to_string_lossy().into_owned()));
    }

    #[test]
    fn direct_proton_command_skips_empty_wrappers() {
        let command = build_direct_proton_command_with_wrappers(
            "/run/host/usr/share/steam/compatibilitytools.d/proton/proton",
            &["   ".to_string(), " \t ".to_string()],
            &BTreeMap::new(),
        );

        assert_eq!(
            command.as_std().get_program(),
            "/usr/share/steam/compatibilitytools.d/proton/proton"
        );
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(args, vec!["run".to_string()]);
    }

    #[test]
    fn direct_proton_command_normalizes_wrappers_and_proton_path() {
        let command = build_direct_proton_command_with_wrappers(
            "/run/host/usr/share/steam/compatibilitytools.d/proton/proton",
            &[
                " /run/host/usr/bin/env ".to_string(),
                " /run/host/usr/bin/mangohud ".to_string(),
            ],
            &BTreeMap::new(),
        );

        assert_eq!(command.as_std().get_program(), "/usr/bin/env");
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            vec![
                "/usr/bin/mangohud".to_string(),
                "/usr/share/steam/compatibilitytools.d/proton/proton".to_string(),
                "run".to_string(),
            ]
        );
    }

    #[test]
    fn gamescope_proton_command_normalizes_wrappers_and_proton_path() {
        let command = build_proton_command_with_gamescope(
            "/run/host/usr/share/steam/compatibilitytools.d/proton/proton",
            &[" /run/host/usr/bin/mangohud ".to_string()],
            &["-f".to_string()],
            &BTreeMap::new(),
        );

        assert_eq!(command.as_std().get_program(), "gamescope");
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            vec![
                "-f".to_string(),
                "--".to_string(),
                "/usr/bin/mangohud".to_string(),
                "/usr/share/steam/compatibilitytools.d/proton/proton".to_string(),
                "run".to_string(),
            ]
        );
    }

    #[test]
    fn flatpak_gamescope_pid_capture_command_creates_parent_directory_on_host() {
        let command = build_proton_command_with_gamescope_pid_capture_in_directory_inner(
            "/run/host/usr/share/steam/compatibilitytools.d/proton/proton",
            &[" /run/host/usr/bin/mangohud ".to_string()],
            &["-f".to_string()],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
            Some(Path::new("/tmp/crosshook-logs/game.gamescope.pid")),
            true,
            false,
        );

        assert_eq!(command.as_std().get_program(), "flatpak-spawn");
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            vec![
                "--host".to_string(),
                "--clear-env".to_string(),
                "bash".to_string(),
                "-c".to_string(),
                FLATPAK_GAMESCOPE_PID_CAPTURE_SCRIPT.to_string(),
                "bash".to_string(),
                "/tmp/crosshook-logs/game.gamescope.pid".to_string(),
                "gamescope".to_string(),
                "-f".to_string(),
                "--".to_string(),
                "/usr/bin/mangohud".to_string(),
                "/usr/share/steam/compatibilitytools.d/proton/proton".to_string(),
                "run".to_string(),
            ]
        );
    }

    #[test]
    fn resolve_umu_run_path_returns_none_when_no_umu_run_present() {
        let dir = tempfile::tempdir().unwrap();
        // empty directory — no umu-run binary
        let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());
        assert!(resolve_umu_run_path().is_none());
    }

    #[test]
    fn resolve_umu_run_path_returns_path_when_executable_present() {
        let dir = tempfile::tempdir().unwrap();
        let umu_stub = dir.path().join("umu-run");
        std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();

        let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());
        let resolved = resolve_umu_run_path();
        assert!(resolved.is_some(), "expected Some(path), got None");
        assert!(resolved.unwrap().ends_with("/umu-run"));
    }

    #[test]
    fn resolve_umu_run_path_returns_none_when_file_not_executable() {
        let dir = tempfile::tempdir().unwrap();
        let umu_stub = dir.path().join("umu-run");
        std::fs::write(&umu_stub, "not a real executable\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o644)).unwrap();

        let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());
        assert!(resolve_umu_run_path().is_none());
    }
}
