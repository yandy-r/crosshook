use std::env;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

use crate::platform::normalize_flatpak_host_path;

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
