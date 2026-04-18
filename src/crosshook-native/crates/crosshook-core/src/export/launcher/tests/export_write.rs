use std::fs;

use tempfile::tempdir;

use super::super::{export_launchers, SteamExternalLauncherExportRequest};
use crate::profile::TrainerLoadingMode;
use crate::settings::UmuPreference;

#[test]
fn export_writes_expected_paths_and_content() {
    let temp_home = tempdir().expect("temp home");
    let icon_path = temp_home.path().join("launcher icon.png");
    fs::write(&icon_path, b"icon").expect("icon");

    let request = SteamExternalLauncherExportRequest {
        method: "steam_applaunch".to_string(),
        launcher_name: "Elden Ring Deluxe".to_string(),
        trainer_path: "/opt/Trainers/Trainer's Edition.exe".to_string(),
        trainer_loading_mode: TrainerLoadingMode::CopyToPrefix,
        launcher_icon_path: icon_path.to_string_lossy().into_owned(),
        prefix_path: "/tmp/compatdata/1245620".to_string(),
        proton_path: "/opt/Proton/proton".to_string(),
        steam_app_id: "1245620".to_string(),
        steam_client_install_path: temp_home
            .path()
            .join(".local/share/Steam")
            .to_string_lossy()
            .into_owned(),
        target_home_path: "/tmp/not-a-home/compatdata/steam".to_string(),
        profile_name: None,
        ..Default::default()
    };

    let result = export_launchers(&request).expect("export");

    assert_eq!(result.display_name, "Elden Ring Deluxe");
    assert_eq!(result.launcher_slug, "elden-ring-deluxe");
    assert_eq!(
        result.script_path,
        temp_home
            .path()
            .join(".local/share/crosshook/launchers/elden-ring-deluxe-trainer.sh")
            .to_string_lossy()
            .into_owned()
    );
    assert_eq!(
        result.desktop_entry_path,
        temp_home
            .path()
            .join(".local/share/applications/crosshook-elden-ring-deluxe-trainer.desktop")
            .to_string_lossy()
            .into_owned()
    );

    let script_content = fs::read_to_string(&result.script_path).expect("script");
    assert!(script_content.contains("PREFIX_ROOT='/tmp/compatdata/1245620'"));
    assert!(script_content.contains("elif [[ -d \"$PREFIX_ROOT/pfx\" ]]; then"));
    assert!(script_content.contains("export WINEPREFIX=\"$PREFIX_ROOT/pfx\""));
    assert!(script_content.contains("export STEAM_COMPAT_DATA_PATH=\"$PREFIX_ROOT\""));
    assert!(script_content.contains("export STEAM_COMPAT_CLIENT_INSTALL_PATH='"));
    assert!(script_content.contains("PROTON='/opt/Proton/proton'"));
    assert!(
        script_content.contains("TRAINER_HOST_PATH='/opt/Trainers/Trainer'\"'\"'s Edition.exe'")
    );
    assert!(script_content
        .contains("staged_trainer_root=\"$WINEPREFIX/drive_c/CrossHook/StagedTrainers\""));
    assert!(script_content.contains("staged_trainer_windows_path=\"C:\\\\CrossHook\\\\StagedTrainers\\\\$trainer_base_name\\\\$trainer_file_name\""));
    assert!(script_content.contains(r#"exec "$PROTON" run "$staged_trainer_windows_path""#));
    assert!(script_content.contains(r#"exec umu-run "$staged_trainer_windows_path""#));

    let desktop_content = fs::read_to_string(&result.desktop_entry_path).expect("desktop");
    assert!(desktop_content.contains("Name=Elden Ring Deluxe - Trainer"));
    assert!(desktop_content.contains("Exec=/bin/bash "));
    assert!(desktop_content.contains("Icon="));
    assert!(desktop_content.contains(&icon_path.to_string_lossy().replace('\\', "\\\\")));
    assert!(desktop_content.contains("X-CrossHook-Profile=Elden Ring Deluxe\n"));
    assert!(desktop_content.contains("X-CrossHook-Slug=elden-ring-deluxe\n"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let script_mode = fs::metadata(&result.script_path)
            .expect("script metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(script_mode, 0o755, "scripts should be executable");

        let desktop_mode = fs::metadata(&result.desktop_entry_path)
            .expect("desktop metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            desktop_mode, 0o644,
            ".desktop files should not be executable"
        );
    }
}

#[test]
fn export_does_not_duplicate_trainer_suffix_when_name_already_has_it() {
    let temp_home = tempdir().expect("temp home");
    let request = SteamExternalLauncherExportRequest {
        method: "steam_applaunch".to_string(),
        launcher_name: "Elden Ring - Trainer".to_string(),
        trainer_path: "/opt/trainers/Elden Ring.exe".to_string(),
        trainer_loading_mode: TrainerLoadingMode::CopyToPrefix,
        launcher_icon_path: String::new(),
        prefix_path: "/tmp/compatdata/1245620".to_string(),
        proton_path: "/opt/Proton/proton".to_string(),
        steam_app_id: "1245620".to_string(),
        steam_client_install_path: temp_home
            .path()
            .join(".local/share/Steam")
            .to_string_lossy()
            .into_owned(),
        target_home_path: temp_home.path().to_string_lossy().into_owned(),
        profile_name: None,
        ..Default::default()
    };

    let result = export_launchers(&request).expect("export");
    assert_eq!(result.display_name, "Elden Ring");
    assert_eq!(result.launcher_slug, "elden-ring");

    let desktop_content = fs::read_to_string(&result.desktop_entry_path).expect("desktop");
    assert!(desktop_content.contains("Name=Elden Ring - Trainer\n"));
    assert!(!desktop_content.contains("Name=Elden Ring - Trainer - Trainer\n"));
}

#[test]
fn proton_run_export_writes_generic_prefix_bootstrap() {
    let request = SteamExternalLauncherExportRequest {
        method: "proton_run".to_string(),
        launcher_name: "Witcher 3".to_string(),
        trainer_path: "/opt/trainers/Aurora.exe".to_string(),
        trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
        launcher_icon_path: String::new(),
        prefix_path: "/games/prefixes/the-witcher-3".to_string(),
        proton_path: "/opt/proton/proton".to_string(),
        steam_app_id: String::new(),
        steam_client_install_path: String::new(),
        target_home_path: "/home/user".to_string(),
        profile_name: None,
        ..Default::default()
    };

    let script_content = super::super::content::build_trainer_script_content(&request, "Witcher 3");
    assert!(script_content.contains("PREFIX_ROOT='/games/prefixes/the-witcher-3'"));
    assert!(script_content.contains("elif [[ -d \"$PREFIX_ROOT/pfx\" ]]; then"));
    assert!(script_content.contains("trainer_host_path=\"$(realpath \"$TRAINER_HOST_PATH\")\""));
    assert!(script_content.contains(r#"exec "$PROTON" run "$trainer_host_path""#));
    assert!(script_content.contains(r#"exec umu-run "$trainer_host_path""#));
    assert!(!script_content
        .contains("staged_trainer_root=\"$WINEPREFIX/drive_c/CrossHook/StagedTrainers\""));
    assert!(!script_content.contains("export STEAM_COMPAT_CLIENT_INSTALL_PATH="));
}

#[test]
fn steam_export_trainer_script_includes_gamescope_when_request_carries_effective_game_config() {
    let request = SteamExternalLauncherExportRequest {
        method: "steam_applaunch".to_string(),
        launcher_name: "Hitman".to_string(),
        trainer_path: "/games/trainers/t.exe".to_string(),
        trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
        launcher_icon_path: String::new(),
        prefix_path: "/steam/compatdata/123".to_string(),
        proton_path: "/opt/proton".to_string(),
        steam_app_id: "863550".to_string(),
        steam_client_install_path: "/home/u/.local/share/Steam".to_string(),
        target_home_path: "/home/user".to_string(),
        profile_name: None,
        runtime_steam_app_id: String::new(),
        umu_game_id: String::new(),
        umu_preference: UmuPreference::Auto,
        network_isolation: false,
        gamescope: crate::profile::GamescopeConfig {
            enabled: true,
            fullscreen: true,
            ..Default::default()
        },
    };

    let script_content = super::super::content::build_trainer_script_content(&request, "Hitman");
    assert!(script_content.contains("Gamescope wrapper"));
    assert!(script_content.contains("_GAMESCOPE_ARGS"));
}
