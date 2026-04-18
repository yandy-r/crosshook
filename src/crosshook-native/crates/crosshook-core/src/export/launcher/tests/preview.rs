use super::super::{
    preview_desktop_entry_content, preview_trainer_script_content,
    SteamExternalLauncherExportRequest,
};

#[test]
fn preview_script_contains_placement_header_before_shebang() {
    let request = SteamExternalLauncherExportRequest {
        method: "proton_run".to_string(),
        launcher_name: "Witcher 3".to_string(),
        trainer_path: "/opt/trainers/Aurora.exe".to_string(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        launcher_icon_path: String::new(),
        prefix_path: "/games/prefixes/the-witcher-3".to_string(),
        proton_path: "/opt/proton/proton".to_string(),
        steam_app_id: String::new(),
        steam_client_install_path: String::new(),
        target_home_path: "/home/user".to_string(),
        profile_name: None,
        ..Default::default()
    };

    let content = preview_trainer_script_content(&request).expect("preview script");
    assert!(content.starts_with(
        "# Save this file to: /home/user/.local/share/crosshook/launchers/witcher-3-trainer.sh\n"
    ));
    assert!(content.contains("# Make executable: chmod +x"));
    assert!(content.contains("#!/usr/bin/env bash"));
}

#[test]
fn preview_desktop_contains_placement_header_before_entry() {
    let request = SteamExternalLauncherExportRequest {
        method: "proton_run".to_string(),
        launcher_name: "Witcher 3".to_string(),
        trainer_path: "/opt/trainers/Aurora.exe".to_string(),
        trainer_loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
        launcher_icon_path: String::new(),
        prefix_path: "/games/prefixes/the-witcher-3".to_string(),
        proton_path: "/opt/proton/proton".to_string(),
        steam_app_id: String::new(),
        steam_client_install_path: String::new(),
        target_home_path: "/home/user".to_string(),
        profile_name: None,
        ..Default::default()
    };

    let content = preview_desktop_entry_content(&request).expect("preview desktop");
    assert!(content.starts_with("# Save this file to: /home/user/.local/share/applications/crosshook-witcher-3-trainer.desktop\n"));
    assert!(content.contains("# Permissions: 644"));
    assert!(content.contains("[Desktop Entry]"));
}

#[test]
fn preview_script_fails_when_trainer_path_empty() {
    let request = SteamExternalLauncherExportRequest {
        method: "proton_run".to_string(),
        trainer_path: String::new(),
        prefix_path: "/tmp/prefix".to_string(),
        proton_path: "/tmp/proton".to_string(),
        target_home_path: "/home/user".to_string(),
        ..Default::default()
    };

    let result = preview_trainer_script_content(&request);
    assert!(result.is_err());
}

#[test]
fn preview_desktop_fails_when_trainer_path_empty() {
    let request = SteamExternalLauncherExportRequest {
        method: "proton_run".to_string(),
        trainer_path: String::new(),
        prefix_path: "/tmp/prefix".to_string(),
        proton_path: "/tmp/proton".to_string(),
        target_home_path: "/home/user".to_string(),
        ..Default::default()
    };

    let result = preview_desktop_entry_content(&request);
    assert!(result.is_err());
}
