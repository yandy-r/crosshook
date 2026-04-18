//! Path derivation helpers for launcher file locations.

use crate::export::launcher::{
    combine_host_unix_path, resolve_display_name, resolve_target_home_path, sanitize_launcher_slug,
};

/// Derives the resolved display name, launcher slug, script path, and desktop entry path
/// from the given inputs. Shared by `check_launcher_exists` and `delete_launcher_files`.
pub(super) fn derive_launcher_paths(
    display_name: &str,
    steam_app_id: &str,
    trainer_path: &str,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> (String, String, String, String) {
    let resolved_name = resolve_display_name(display_name, steam_app_id, trainer_path);
    let slug = sanitize_launcher_slug(&resolved_name);
    let home = resolve_target_home_path(target_home_path, steam_client_install_path);

    let script_path = combine_host_unix_path(
        &home,
        ".local/share/crosshook/launchers",
        &format!("{slug}-trainer.sh"),
    );
    let desktop_entry_path = combine_host_unix_path(
        &home,
        ".local/share/applications",
        &format!("crosshook-{slug}-trainer.desktop"),
    );

    (resolved_name, slug, script_path, desktop_entry_path)
}

pub(super) fn derive_launcher_paths_from_slug(
    launcher_slug: &str,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> (String, String) {
    let home = resolve_target_home_path(target_home_path, steam_client_install_path);
    let normalized_slug = sanitize_launcher_slug(launcher_slug);

    let script_path = combine_host_unix_path(
        &home,
        ".local/share/crosshook/launchers",
        &format!("{normalized_slug}-trainer.sh"),
    );
    let desktop_entry_path = combine_host_unix_path(
        &home,
        ".local/share/applications",
        &format!("crosshook-{normalized_slug}-trainer.desktop"),
    );

    (script_path, desktop_entry_path)
}
