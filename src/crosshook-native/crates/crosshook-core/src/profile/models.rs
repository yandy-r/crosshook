use serde::{Deserialize, Serialize};
use std::str::FromStr;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GameProfile {
    #[serde(default)]
    pub game: GameSection,
    #[serde(default)]
    pub trainer: TrainerSection,
    #[serde(default)]
    pub injection: InjectionSection,
    #[serde(default)]
    pub steam: SteamSection,
    #[serde(default, skip_serializing_if = "RuntimeSection::is_empty")]
    pub runtime: RuntimeSection,
    #[serde(default)]
    pub launch: LaunchSection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrainerLoadingMode {
    #[default]
    SourceDirectory,
    CopyToPrefix,
}

impl TrainerLoadingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SourceDirectory => "source_directory",
            Self::CopyToPrefix => "copy_to_prefix",
        }
    }
}

impl FromStr for TrainerLoadingMode {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "source_directory" => Ok(Self::SourceDirectory),
            "copy_to_prefix" => Ok(Self::CopyToPrefix),
            _ => Err("unsupported trainer loading mode"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchOptimizationsSection {
    #[serde(
        rename = "enabled_option_ids",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub enabled_option_ids: Vec<String>,
}

impl LaunchOptimizationsSection {
    pub fn is_empty(&self) -> bool {
        self.enabled_option_ids.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GameSection {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "executable_path", default)]
    pub executable_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TrainerSection {
    #[serde(default)]
    pub path: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(rename = "loading_mode", default)]
    pub loading_mode: TrainerLoadingMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InjectionSection {
    #[serde(rename = "dll_paths", default)]
    pub dll_paths: Vec<String>,
    #[serde(rename = "inject_on_launch", default)]
    pub inject_on_launch: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SteamSection {
    #[serde(default)]
    pub enabled: bool,
    #[serde(rename = "app_id", default)]
    pub app_id: String,
    #[serde(rename = "compatdata_path", default)]
    pub compatdata_path: String,
    #[serde(rename = "proton_path", default)]
    pub proton_path: String,
    #[serde(default)]
    pub launcher: LauncherSection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LauncherSection {
    #[serde(rename = "icon_path", default)]
    pub icon_path: String,
    #[serde(rename = "display_name", default)]
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuntimeSection {
    #[serde(rename = "prefix_path", default)]
    pub prefix_path: String,
    #[serde(rename = "proton_path", default)]
    pub proton_path: String,
    #[serde(rename = "working_directory", default)]
    pub working_directory: String,
}

impl RuntimeSection {
    pub fn is_empty(&self) -> bool {
        self.prefix_path.trim().is_empty()
            && self.proton_path.trim().is_empty()
            && self.working_directory.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchSection {
    #[serde(default)]
    pub method: String,
    #[serde(default, skip_serializing_if = "LaunchOptimizationsSection::is_empty")]
    pub optimizations: LaunchOptimizationsSection,
}

impl From<LegacyProfileData> for GameProfile {
    fn from(value: LegacyProfileData) -> Self {
        let method = derive_launch_method_from_legacy(&value);

        Self {
            game: GameSection {
                name: String::default(),
                executable_path: value.game_path,
            },
            trainer: TrainerSection {
                path: value.trainer_path,
                kind: String::default(),
                loading_mode: TrainerLoadingMode::default(),
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

fn looks_like_windows_executable(path: &str) -> bool {
    path.trim().to_ascii_lowercase().ends_with(".exe")
}
