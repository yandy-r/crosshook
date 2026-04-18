use std::path::Path;

use crate::platform::{self, normalize_flatpak_host_path};
use crate::steam::{discover_steam_root_candidates, proton::prefer_user_local_compat_tool_path};

pub(crate) fn resolve_launch_proton_path(
    proton_path: &str,
    steam_client_install_path: &str,
) -> String {
    resolve_launch_proton_path_with_mode(
        proton_path,
        steam_client_install_path,
        platform::is_flatpak(),
    )
}

fn resolve_launch_proton_path_with_mode(
    proton_path: &str,
    steam_client_install_path: &str,
    flatpak: bool,
) -> String {
    let normalized_proton_path = normalize_flatpak_host_path(proton_path);
    let trimmed_proton_path = normalized_proton_path.trim();
    if trimmed_proton_path.is_empty() || !flatpak {
        return normalized_proton_path;
    }

    let configured_steam_client_install_path =
        normalize_flatpak_host_path(steam_client_install_path);
    let mut diagnostics = Vec::new();
    let steam_root_candidates = discover_steam_root_candidates(
        configured_steam_client_install_path.as_str(),
        &mut diagnostics,
    );
    let resolved_path = prefer_user_local_compat_tool_path(
        Path::new(trimmed_proton_path),
        &steam_root_candidates,
        &mut diagnostics,
    );

    for entry in diagnostics {
        tracing::debug!(entry, "proton launch resolution diagnostic");
    }

    resolved_path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::resolve_launch_proton_path_with_mode;

    fn write_steam_client_root(path: &std::path::Path) {
        fs::create_dir_all(path.join("steamapps")).expect("steamapps dir");
        fs::create_dir_all(path.join("config")).expect("config dir");
    }

    #[test]
    fn resolve_launch_proton_path_with_mode_keeps_system_tool_without_local_override() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let steam_root = temp_dir.path().join("Steam");
        write_steam_client_root(&steam_root);

        let proton_path =
            "/usr/share/steam/compatibilitytools.d/crosshook-missing-system-tool/proton"
                .to_string();

        let resolved = resolve_launch_proton_path_with_mode(
            &proton_path,
            steam_root.to_string_lossy().as_ref(),
            true,
        );

        assert_eq!(resolved, proton_path);
    }

    #[test]
    fn resolve_launch_proton_path_with_mode_prefers_matching_local_tool_in_flatpak() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let steam_root = temp_dir.path().join("Steam");
        let local_tool = steam_root.join("compatibilitytools.d/Proton-CachyOS-SLR");
        write_steam_client_root(&steam_root);
        fs::create_dir_all(&local_tool).expect("local tool dir");
        fs::write(local_tool.join("proton"), b"proton").expect("local proton");

        let resolved = resolve_launch_proton_path_with_mode(
            "/usr/share/steam/compatibilitytools.d/Proton-CachyOS-SLR/proton",
            steam_root.to_string_lossy().as_ref(),
            true,
        );

        assert_eq!(resolved, local_tool.join("proton").to_string_lossy());
    }
}
