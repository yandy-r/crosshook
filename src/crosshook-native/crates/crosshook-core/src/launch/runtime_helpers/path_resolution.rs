use std::env;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

use crate::platform::{
    normalize_flatpak_host_path, normalized_path_is_dir_on_host, normalized_path_is_file_on_host,
};
use crate::runtime_files::open_private_append;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProtonPaths {
    pub wine_prefix_path: PathBuf,
    pub compat_data_path: PathBuf,
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

/// Attaches the log file as the child's stdout and stderr.
///
/// The log must already exist (the app or CLI creates it in the private launch
/// log directory); it is opened without being created or followed through a
/// symlink, and must be a regular file owned by the current user.
pub fn attach_log_stdio(command: &mut Command, log_path: &Path) -> std::io::Result<()> {
    let stdout = open_private_append(log_path).map_err(std::io::Error::other)?;
    let stderr = stdout.try_clone()?;
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

pub(crate) fn resolve_steam_client_install_path_with_home(
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

    if is_steam_client_install_root_str(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn is_steam_client_install_root(path: &Path) -> bool {
    is_steam_client_install_root_str(&path.to_string_lossy())
}

/// Checks if the given path is a valid Steam client install root using host-aware filesystem probes.
/// This works correctly in both native and Flatpak environments.
fn is_steam_client_install_root_str(path: &str) -> bool {
    let steamapps_path = format!("{path}/steamapps");
    let config_path = format!("{path}/config");
    let steam_sh_path = format!("{path}/steam.sh");
    let ubuntu12_32_path = format!("{path}/ubuntu12_32");

    normalized_path_is_dir_on_host(&steamapps_path)
        && (normalized_path_is_dir_on_host(&config_path)
            || normalized_path_is_file_on_host(&steam_sh_path)
            || normalized_path_is_dir_on_host(&ubuntu12_32_path))
}
