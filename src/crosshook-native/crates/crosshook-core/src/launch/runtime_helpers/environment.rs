use std::collections::BTreeMap;
use std::env;

use crate::platform::{self, normalize_flatpak_host_path};

use super::{
    path_resolution::{resolve_proton_paths, resolve_steam_client_install_path},
    DEFAULT_HOST_PATH, DEFAULT_SHELL,
};

pub(crate) fn env_value(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

pub(crate) fn resolve_flatpak_host_dbus_session_bus_address_with<F>(
    session_bus_address: &str,
    xdg_runtime_dir: &str,
    host_path_exists: F,
) -> String
where
    F: Fn(&str) -> bool,
{
    let trimmed_address = session_bus_address.trim();
    if trimmed_address.is_empty() {
        return String::new();
    }
    if let Some(path) = trimmed_address.strip_prefix("unix:path=") {
        let delimiter_index = path.find([',', ';']);
        let path_component = delimiter_index.map_or(path, |idx| &path[..idx]);
        let suffix = delimiter_index.map_or("", |idx| &path[idx..]);
        if host_path_exists(path_component) {
            return trimmed_address.to_string();
        }
        let trimmed_runtime_dir = xdg_runtime_dir.trim().trim_end_matches('/');
        if trimmed_runtime_dir.is_empty() {
            return String::new();
        }
        let candidate = format!("{trimmed_runtime_dir}/bus");
        if host_path_exists(candidate.as_str()) {
            return format!("unix:path={candidate}{suffix}");
        }
        return String::new();
    }
    trimmed_address.to_string()
}

pub(super) fn resolve_host_dbus_session_bus_address() -> String {
    let session_bus_address = env_value("DBUS_SESSION_BUS_ADDRESS", "");
    if !platform::is_flatpak() {
        return session_bus_address;
    }
    let xdg_runtime_dir = env_value("XDG_RUNTIME_DIR", "");
    resolve_flatpak_host_dbus_session_bus_address_with(
        session_bus_address.as_str(),
        xdg_runtime_dir.as_str(),
        platform::normalized_path_exists_on_host,
    )
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
        resolve_host_dbus_session_bus_address(),
    );
    m
}

pub fn merge_runtime_proton_into_map(
    map: &mut BTreeMap<String, String>,
    prefix_path: &str,
    steam_client_install_path: &str,
) {
    let normalized_prefix_path = normalize_flatpak_host_path(prefix_path);
    let resolved_paths = resolve_proton_paths(std::path::Path::new(normalized_prefix_path.trim()));
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
