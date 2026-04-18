use serde::{Deserialize, Serialize};

use super::trainer::default_trainer_type;
use super::{
    GameProfile, GameSection, InjectionSection, LaunchSection, LauncherSection,
    LocalOverrideSection, RuntimeSection, SteamSection, TrainerLoadingMode, TrainerSection,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LegacyProfileData {
    #[serde(rename = "GamePath")]
    pub game_path: String,
    #[serde(rename = "TrainerPath")]
    pub trainer_path: String,
    #[serde(rename = "Dll1Path")]
    pub dll1_path: String,
    #[serde(rename = "Dll2Path")]
    pub dll2_path: String,
    #[serde(rename = "LaunchInject1")]
    pub launch_inject1: bool,
    #[serde(rename = "LaunchInject2")]
    pub launch_inject2: bool,
    #[serde(rename = "LaunchMethod")]
    pub launch_method: String,
    #[serde(rename = "UseSteamMode")]
    pub use_steam_mode: bool,
    #[serde(rename = "SteamAppId")]
    pub steam_app_id: String,
    #[serde(rename = "SteamCompatDataPath")]
    pub steam_compat_data_path: String,
    #[serde(rename = "SteamProtonPath")]
    pub steam_proton_path: String,
    #[serde(rename = "SteamLauncherIconPath")]
    pub steam_launcher_icon_path: String,
}

impl From<LegacyProfileData> for GameProfile {
    fn from(value: LegacyProfileData) -> Self {
        let method = derive_launch_method_from_legacy(&value);

        Self {
            game: GameSection {
                name: String::default(),
                executable_path: value.game_path,
                custom_cover_art_path: String::new(),
                custom_portrait_art_path: String::new(),
                custom_background_art_path: String::new(),
            },
            trainer: TrainerSection {
                path: value.trainer_path,
                kind: String::default(),
                loading_mode: TrainerLoadingMode::default(),
                trainer_type: default_trainer_type(),
                required_protontricks: Vec::new(),
                community_trainer_sha256: String::new(),
            },
            injection: InjectionSection {
                dll_paths: vec![value.dll1_path, value.dll2_path],
                inject_on_launch: vec![value.launch_inject1, value.launch_inject2],
            },
            steam: SteamSection {
                enabled: value.use_steam_mode,
                app_id: value.steam_app_id,
                compatdata_path: value.steam_compat_data_path,
                proton_path: value.steam_proton_path,
                launcher: LauncherSection {
                    icon_path: value.steam_launcher_icon_path,
                    display_name: String::default(),
                },
            },
            runtime: RuntimeSection::default(),
            launch: LaunchSection {
                method,
                ..Default::default()
            },
            local_override: LocalOverrideSection::default(),
        }
    }
}

fn derive_launch_method_from_legacy(value: &LegacyProfileData) -> String {
    if value.use_steam_mode {
        return "steam_applaunch".to_string();
    }

    if looks_like_windows_executable(&value.game_path) {
        return "proton_run".to_string();
    }

    "native".to_string()
}

pub(super) fn looks_like_windows_executable(path: &str) -> bool {
    path.trim().to_ascii_lowercase().ends_with(".exe")
}
