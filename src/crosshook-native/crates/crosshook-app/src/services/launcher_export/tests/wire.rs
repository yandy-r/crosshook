//! Frozen IPC wire shapes of the launcher export results and request. These
//! core types cross the Classic boundary unchanged, so their snake_case keys
//! are pinned here next to the service that returns them.

use crosshook_core::export::{
    LauncherDeleteResult, LauncherInfo, LauncherRenameResult, SteamExternalLauncherExportRequest,
    SteamExternalLauncherExportResult,
};

fn sorted_keys<T: serde::Serialize>(value: &T) -> Vec<String> {
    let json = serde_json::to_value(value).unwrap();
    let mut keys: Vec<String> = json.as_object().unwrap().keys().cloned().collect();
    keys.sort_unstable();
    keys
}

#[test]
fn launcher_result_payloads_keep_snake_case_keys() {
    assert_eq!(
        sorted_keys(&LauncherInfo::default()),
        [
            "desktop_entry_exists",
            "desktop_entry_path",
            "display_name",
            "is_stale",
            "launcher_slug",
            "script_exists",
            "script_path",
        ]
    );
    assert_eq!(
        sorted_keys(&LauncherDeleteResult::default()),
        [
            "desktop_entry_deleted",
            "desktop_entry_path",
            "desktop_entry_skipped_reason",
            "script_deleted",
            "script_path",
            "script_skipped_reason",
        ]
    );
    assert_eq!(
        sorted_keys(&LauncherRenameResult::default()),
        [
            "desktop_entry_renamed",
            "new_desktop_entry_path",
            "new_script_path",
            "new_slug",
            "old_desktop_entry_cleanup_warning",
            "old_script_cleanup_warning",
            "old_slug",
            "script_renamed",
        ]
    );
    assert_eq!(
        sorted_keys(&SteamExternalLauncherExportResult::default()),
        [
            "desktop_entry_path",
            "display_name",
            "launcher_slug",
            "script_path",
        ]
    );
}

#[test]
fn export_request_payload_deserializes_from_frontend_shape() {
    let request: SteamExternalLauncherExportRequest = serde_json::from_value(serde_json::json!({
        "method": "proton_run",
        "launcher_name": "Game",
        "trainer_path": "/t.exe",
        "launcher_icon_path": "",
        "prefix_path": "/p",
        "proton_path": "/proton",
        "steam_app_id": "1",
        "steam_client_install_path": "",
        "target_home_path": "/home/u",
    }))
    .unwrap();
    assert_eq!(request.method, "proton_run");
    assert_eq!(request.launcher_name, "Game");
    assert!(request.profile_name.is_none());
    assert!(request.custom_env_vars.is_empty());
}
