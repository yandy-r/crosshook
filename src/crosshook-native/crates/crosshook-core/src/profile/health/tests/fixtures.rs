use std::fs;
use std::path::Path;

use crate::metadata::PrefixDependencyStateRow;
use crate::profile::{
    GameProfile, GameSection, InjectionSection, LaunchSection, LauncherSection, RuntimeSection,
    SteamSection, TrainerSection,
};

/// Create a real executable file at `path`.
pub(super) fn make_executable(path: &Path) {
    fs::write(path, b"#!/bin/sh\n").expect("write executable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod");
    }
}

/// Build a `GameProfile` for `steam_applaunch` where all configured paths exist.
pub(super) fn healthy_steam_profile(tmp: &Path) -> GameProfile {
    let game_exe = tmp.join("game.exe");
    let trainer = tmp.join("trainer.exe");
    let dll = tmp.join("mod.dll");
    let compatdata = tmp.join("compatdata");
    let proton = tmp.join("proton");

    make_executable(&game_exe);
    make_executable(&trainer);
    fs::write(&dll, b"MZ").expect("write dll");
    fs::create_dir_all(&compatdata).expect("mkdir compatdata");
    make_executable(&proton);

    GameProfile {
        game: GameSection {
            name: "Test Game".to_string(),
            executable_path: game_exe.to_string_lossy().to_string(),
            custom_cover_art_path: String::new(),
            custom_portrait_art_path: String::new(),
            custom_background_art_path: String::new(),
        },
        trainer: TrainerSection {
            path: trainer.to_string_lossy().to_string(),
            kind: "fling".to_string(),
            loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
            trainer_type: "unknown".to_string(),
            required_protontricks: Vec::new(),
            community_trainer_sha256: String::new(),
        },
        injection: InjectionSection {
            dll_paths: vec![dll.to_string_lossy().to_string()],
            inject_on_launch: vec![true],
        },
        steam: SteamSection {
            enabled: true,
            app_id: "12345".to_string(),
            compatdata_path: compatdata.to_string_lossy().to_string(),
            proton_path: proton.to_string_lossy().to_string(),
            launcher: LauncherSection {
                icon_path: String::new(),
                display_name: String::new(),
            },
        },
        runtime: RuntimeSection::default(),
        launch: LaunchSection {
            method: "steam_applaunch".to_string(),
            ..Default::default()
        },
        local_override: crate::profile::LocalOverrideSection::default(),
    }
}

pub(super) fn make_dep_row(package: &str, state: &str) -> PrefixDependencyStateRow {
    PrefixDependencyStateRow {
        id: 0,
        profile_id: "test".to_string(),
        package_name: package.to_string(),
        prefix_path: "/tmp/pfx".to_string(),
        state: state.to_string(),
        checked_at: Some("2026-01-01T00:00:00Z".to_string()),
        installed_at: None,
        last_error: None,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    }
}
