use crate::profile::GameProfile;

pub fn sample_profile() -> GameProfile {
    GameProfile {
        game: crate::profile::GameSection {
            name: "Elden Ring".to_string(),
            executable_path: "/games/elden-ring/eldenring.exe".to_string(),
            custom_cover_art_path: String::new(),
            custom_portrait_art_path: String::new(),
            custom_background_art_path: String::new(),
        },
        trainer: crate::profile::TrainerSection {
            path: "/trainers/elden-ring.exe".to_string(),
            kind: "fling".to_string(),
            loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
            trainer_type: "unknown".to_string(),
            required_protontricks: Vec::new(),
            community_trainer_sha256: String::new(),
        },
        injection: crate::profile::InjectionSection {
            dll_paths: vec!["/dlls/a.dll".to_string(), "/dlls/b.dll".to_string()],
            inject_on_launch: vec![true, false],
        },
        steam: crate::profile::SteamSection {
            enabled: true,
            app_id: "1245620".to_string(),
            compatdata_path: "/steam/compatdata/1245620".to_string(),
            proton_path: "/steam/proton/proton".to_string(),
            launcher: crate::profile::LauncherSection {
                icon_path: "/icons/elden-ring.png".to_string(),
                display_name: "Elden Ring".to_string(),
            },
        },
        runtime: crate::profile::RuntimeSection {
            prefix_path: String::new(),
            proton_path: String::new(),
            working_directory: String::new(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
            umu_preference: None,
        },
        launch: crate::profile::LaunchSection {
            method: "steam_applaunch".to_string(),
            ..Default::default()
        },
        local_override: crate::profile::LocalOverrideSection::default(),
    }
}
