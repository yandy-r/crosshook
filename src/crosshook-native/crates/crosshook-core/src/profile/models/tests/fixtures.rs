#![cfg(test)]

use super::super::*; // GameProfile and all section types re-exported by profile/models/mod.rs

pub(super) fn sample_profile() -> GameProfile {
    GameProfile {
        game: GameSection {
            name: "Test Game".to_string(),
            executable_path: "/games/test.exe".to_string(),
            custom_cover_art_path: String::new(),
            custom_portrait_art_path: String::new(),
            custom_background_art_path: String::new(),
        },
        trainer: TrainerSection::default(),
        injection: InjectionSection::default(),
        steam: SteamSection::default(),
        runtime: RuntimeSection::default(),
        launch: LaunchSection::default(),
        local_override: LocalOverrideSection::default(),
    }
}
