use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GameSection {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "executable_path", default)]
    pub executable_path: String,
    #[serde(
        rename = "custom_cover_art_path",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub custom_cover_art_path: String,
    #[serde(
        rename = "custom_portrait_art_path",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub custom_portrait_art_path: String,
    #[serde(
        rename = "custom_background_art_path",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub custom_background_art_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LoadedDllHook {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub path: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InjectionMethod {
    #[default]
    Disabled,
    LoadLibrary,
    ManualMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InjectionStage {
    #[default]
    TrainerLaunch,
    GameProcessReady,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InjectionFallback {
    #[default]
    WarnAndContinue,
    DisableHook,
    AbortLaunch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct InjectionSection {
    #[serde(
        rename = "loaded_hooks",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub loaded_hooks: Vec<LoadedDllHook>,
    pub method: InjectionMethod,
    pub stage: InjectionStage,
    #[serde(rename = "timeout_ms", default)]
    pub timeout_ms: u64,
    pub fallback: InjectionFallback,
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
