use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

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

fn set_env(command: &mut Command, key: &str, value: impl AsRef<str>) {
    command.env(key, value.as_ref());
}
