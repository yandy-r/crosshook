use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

use crate::profile::{GamescopeConfig, GamescopeFilter};

/// Default `PATH` used when the host environment does not set `PATH` (matches `apply_host_environment`).
pub const DEFAULT_HOST_PATH: &str = "/usr/bin:/bin";
const DEFAULT_SHELL: &str = "/bin/bash";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProtonPaths {
    pub wine_prefix_path: PathBuf,
    pub compat_data_path: PathBuf,
}

pub fn new_direct_proton_command(proton_path: &str) -> Command {
    new_direct_proton_command_with_wrappers(proton_path, &[])
}

pub fn new_direct_proton_command_with_wrappers(proton_path: &str, wrappers: &[String]) -> Command {
    if wrappers.is_empty() {
        let mut command = Command::new(proton_path.trim());
        command.arg("run");
        command.env_clear();
        return command;
    }

    let mut command = Command::new(wrappers[0].trim());
    for wrapper in wrappers.iter().skip(1) {
        command.arg(wrapper.trim());
    }
    command.arg(proton_path.trim());
    command.arg("run");
    command.env_clear();
    command
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

/// Builds a `Command` that invokes `umu-run` instead of the Proton binary directly.
///
/// Sets `GAMEID` and `PROTONPATH` as command-level environment variables.
/// `PROTONPATH` is the parent directory of `proton_path` (umu-run convention).
/// `GAMEID` uses `steam_app_id` when provided, otherwise `"0"`.
/// `env_clear()` is called, so the caller must apply host/optimization env afterward.
pub fn new_umu_run_command(
    umu_run_path: &str,
    proton_path: &str,
    steam_app_id: &str,
    wrappers: &[String],
) -> Command {
    let proton_dir = resolve_proton_dir(proton_path);
    let game_id = resolve_game_id(steam_app_id);

    let mut command = if wrappers.is_empty() {
        Command::new(umu_run_path.trim())
    } else {
        let mut cmd = Command::new(wrappers[0].trim());
        for wrapper in wrappers.iter().skip(1) {
            cmd.arg(wrapper.trim());
        }
        cmd.arg(umu_run_path.trim());
        cmd
    };
    command.env_clear();
    command.env("GAMEID", &game_id);
    command.env("PROTONPATH", &proton_dir);
    command
}

pub fn new_umu_run_command_with_gamescope(
    umu_run_path: &str,
    proton_path: &str,
    steam_app_id: &str,
    wrappers: &[String],
    gamescope_args: &[String],
) -> Command {
    let proton_dir = resolve_proton_dir(proton_path);
    let game_id = resolve_game_id(steam_app_id);

    let mut command = Command::new("gamescope");
    for arg in gamescope_args {
        command.arg(arg.trim());
    }
    command.arg("--");
    for wrapper in wrappers {
        command.arg(wrapper.trim());
    }
    command.arg(umu_run_path.trim());
    command.env_clear();
    command.env("GAMEID", &game_id);
    command.env("PROTONPATH", &proton_dir);
    command
}

fn resolve_proton_dir(proton_path: &str) -> String {
    Path::new(proton_path.trim())
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn resolve_game_id(steam_app_id: &str) -> String {
    let trimmed = steam_app_id.trim();
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn new_proton_command_with_gamescope(
    proton_path: &str,
    wrappers: &[String],
    gamescope_args: &[String],
) -> Command {
    let mut command = Command::new("gamescope");
    for arg in gamescope_args {
        command.arg(arg.trim());
    }
    command.arg("--");
    for wrapper in wrappers {
        command.arg(wrapper.trim());
    }
    command.arg(proton_path.trim());
    command.arg("run");
    command.env_clear();
    command
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

pub fn apply_host_environment(command: &mut Command) {
    set_env(command, "HOME", env_value("HOME", ""));
    set_env(command, "USER", env_value("USER", ""));
    set_env(command, "LOGNAME", env_value("LOGNAME", ""));
    set_env(command, "SHELL", env_value("SHELL", DEFAULT_SHELL));
    set_env(command, "PATH", env_value("PATH", DEFAULT_HOST_PATH));
    set_env(command, "DISPLAY", env_value("DISPLAY", ""));
    set_env(command, "WAYLAND_DISPLAY", env_value("WAYLAND_DISPLAY", ""));
    set_env(command, "XDG_RUNTIME_DIR", env_value("XDG_RUNTIME_DIR", ""));
    set_env(
        command,
        "DBUS_SESSION_BUS_ADDRESS",
        env_value("DBUS_SESSION_BUS_ADDRESS", ""),
    );
}

pub fn apply_runtime_proton_environment(
    command: &mut Command,
    prefix_path: &str,
    steam_client_install_path: &str,
) {
    let resolved_paths = resolve_proton_paths(Path::new(prefix_path.trim()));
    set_env(
        command,
        "WINEPREFIX",
        resolved_paths.wine_prefix_path.to_string_lossy().as_ref(),
    );

    set_env(
        command,
        "STEAM_COMPAT_DATA_PATH",
        resolved_paths.compat_data_path.to_string_lossy().as_ref(),
    );

    if let Some(steam_client_install_path) =
        resolve_steam_client_install_path(steam_client_install_path)
    {
        set_env(
            command,
            "STEAM_COMPAT_CLIENT_INSTALL_PATH",
            steam_client_install_path.as_str(),
        );
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
    if !configured_directory.is_empty() {
        command.current_dir(configured_directory);
        return;
    }

    if let Some(parent) = primary_path.parent() {
        if !parent.as_os_str().is_empty() {
            command.current_dir(parent);
        }
    }
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
    let trimmed_configured_path = configured_path.trim();
    if !trimmed_configured_path.is_empty() {
        return Some(trimmed_configured_path.to_string());
    }

    if let Ok(steam_client_install_path) = env::var("STEAM_COMPAT_CLIENT_INSTALL_PATH") {
        let trimmed = steam_client_install_path.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let home_path = env::var_os("HOME").map(PathBuf::from)?;
    for candidate in [
        home_path.join(".local/share/Steam"),
        home_path.join(".steam/root"),
        home_path.join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ] {
        if candidate.join("steamapps").is_dir() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }

    None
}

pub(crate) fn env_value(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Returns the absolute path to `umu-run` if found on `PATH`, otherwise `None`.
///
/// In test builds, returns `None` when `disable_test_umu_run` is active to
/// avoid coupling test assertions to the host system's `umu-run` installation.
pub fn resolve_umu_run_path() -> Option<String> {
    #[cfg(test)]
    {
        if test_umu_run_disable_count().load(std::sync::atomic::Ordering::SeqCst) > 0 {
            return None;
        }
    }

    let path_value =
        env::var_os("PATH").unwrap_or_else(|| std::ffi::OsString::from(DEFAULT_HOST_PATH));
    for directory in env::split_paths(&path_value) {
        let candidate = directory.join("umu-run");
        if is_executable_file(&candidate) {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

#[cfg(test)]
fn test_umu_run_disable_count() -> &'static std::sync::atomic::AtomicUsize {
    use std::sync::OnceLock;
    static COUNT: OnceLock<std::sync::atomic::AtomicUsize> = OnceLock::new();
    COUNT.get_or_init(|| std::sync::atomic::AtomicUsize::new(0))
}

/// Increment the disable counter (thread-safe). While > 0, `resolve_umu_run_path` returns `None`.
#[cfg(test)]
pub(crate) fn disable_test_umu_run() {
    test_umu_run_disable_count().fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// Decrement the disable counter.
#[cfg(test)]
pub(crate) fn enable_test_umu_run() {
    test_umu_run_disable_count().fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
}

fn is_executable_file(path: &Path) -> bool {
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

fn set_env(command: &mut Command, key: &str, value: impl AsRef<str>) {
    command.env(key, value.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::GamescopeFilter;

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
}
