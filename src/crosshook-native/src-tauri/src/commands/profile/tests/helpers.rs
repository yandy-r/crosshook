use crosshook_core::profile::{
    GameProfile, GameSection, LaunchSection, LauncherSection, SteamSection, TrainerLoadingMode,
    TrainerSection,
};

pub(super) fn steam_profile(home: &str) -> GameProfile {
    GameProfile {
        game: GameSection {
            name: "Test Game".to_string(),
            executable_path: String::new(),
            custom_cover_art_path: String::new(),
            custom_portrait_art_path: String::new(),
            custom_background_art_path: String::new(),
        },
        trainer: TrainerSection {
            path: "/tmp/trainers/test.exe".to_string(),
            kind: String::new(),
            loading_mode: TrainerLoadingMode::SourceDirectory,
            trainer_type: "unknown".to_string(),
            required_protontricks: Vec::new(),
            community_trainer_sha256: String::new(),
        },
        steam: SteamSection {
            app_id: "12345".to_string(),
            compatdata_path: format!("{home}/.local/share/Steam/steamapps/compatdata/12345"),
            launcher: LauncherSection {
                display_name: "Test Game".to_string(),
                icon_path: String::new(),
            },
            ..Default::default()
        },
        launch: LaunchSection {
            method: "steam_applaunch".to_string(),
            ..Default::default()
        },
        ..Default::default()
    }
}
