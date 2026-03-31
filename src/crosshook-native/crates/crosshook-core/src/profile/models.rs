use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    #[serde(default, skip_serializing_if = "LocalOverrideSection::is_empty")]
    pub local_override: LocalOverrideSection,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamescopeFilter {
    Fsr,
    Nis,
    Linear,
    Nearest,
    Pixel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GamescopeConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_rate_limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fsr_sharpness: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upscale_filter: Option<GamescopeFilter>,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub borderless: bool,
    #[serde(default)]
    pub grab_cursor: bool,
    #[serde(default)]
    pub force_grab_cursor: bool,
    #[serde(default)]
    pub hdr_enabled: bool,
    /// When true, launch gamescope even inside an existing gamescope session.
    #[serde(default)]
    pub allow_nested: bool,
    /// Extra CLI arguments passed verbatim to gamescope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_args: Vec<String>,
}

impl GamescopeConfig {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MangoHudPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    TopCenter,
    BottomCenter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MangoHudConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps_limit: Option<u32>,
    #[serde(default)]
    pub gpu_stats: bool,
    #[serde(default)]
    pub cpu_stats: bool,
    #[serde(default)]
    pub ram: bool,
    #[serde(default)]
    pub frametime: bool,
    #[serde(default)]
    pub battery: bool,
    #[serde(default)]
    pub watt: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<MangoHudPosition>,
}

impl MangoHudConfig {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GameSection {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "executable_path", default)]
    pub executable_path: String,
}

fn default_trainer_type() -> String {
    "unknown".to_string()
}

fn is_default_trainer_type(s: &String) -> bool {
    s == "unknown"
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainerSection {
    #[serde(default)]
    pub path: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(rename = "loading_mode", default)]
    pub loading_mode: TrainerLoadingMode,
    #[serde(
        default = "default_trainer_type",
        skip_serializing_if = "is_default_trainer_type"
    )]
    pub trainer_type: String,
}

impl Default for TrainerSection {
    fn default() -> Self {
        Self {
            path: String::new(),
            kind: String::new(),
            loading_mode: TrainerLoadingMode::default(),
            trainer_type: default_trainer_type(),
        }
    }
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
    /// Named optimization bundles (`[launch.presets.<name>]` in TOML).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub presets: BTreeMap<String, LaunchOptimizationsSection>,
    /// When set and present in `presets`, `optimizations` is kept in sync with that entry.
    #[serde(rename = "active_preset", default, skip_serializing_if = "String::is_empty")]
    pub active_preset: String,
    /// User-defined environment variables applied at launch (merged after optimizations).
    #[serde(rename = "custom_env_vars", default, skip_serializing_if = "BTreeMap::is_empty")]
    pub custom_env_vars: BTreeMap<String, String>,
    /// Per-profile gamescope compositor wrapper configuration.
    #[serde(default, skip_serializing_if = "GamescopeConfig::is_default")]
    pub gamescope: GamescopeConfig,
    /// Per-profile MangoHud overlay configuration.
    #[serde(default, skip_serializing_if = "MangoHudConfig::is_default")]
    pub mangohud: MangoHudConfig,
}

impl LaunchSection {
    /// After load: if `active_preset` names a known preset, copy it into `optimizations`;
    /// otherwise clear `active_preset` so the legacy `optimizations` values remain authoritative.
    pub fn normalize_preset_selection(&mut self) {
        let key = self.active_preset.trim();
        if key.is_empty() {
            return;
        }

        if let Some(section) = self.presets.get(key) {
            self.optimizations = section.clone();
        } else {
            self.active_preset.clear();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalOverrideSection {
    #[serde(default)]
    pub game: LocalOverrideGameSection,
    #[serde(default)]
    pub trainer: LocalOverrideTrainerSection,
    #[serde(default)]
    pub steam: LocalOverrideSteamSection,
    #[serde(default)]
    pub runtime: LocalOverrideRuntimeSection,
}

impl LocalOverrideSection {
    pub fn is_empty(&self) -> bool {
        self.game.is_empty()
            && self.trainer.is_empty()
            && self.steam.is_empty()
            && self.runtime.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalOverrideGameSection {
    #[serde(rename = "executable_path", default)]
    pub executable_path: String,
}

impl LocalOverrideGameSection {
    pub fn is_empty(&self) -> bool {
        self.executable_path.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalOverrideTrainerSection {
    #[serde(default)]
    pub path: String,
}

impl LocalOverrideTrainerSection {
    pub fn is_empty(&self) -> bool {
        self.path.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalOverrideSteamSection {
    #[serde(rename = "compatdata_path", default)]
    pub compatdata_path: String,
    #[serde(rename = "proton_path", default)]
    pub proton_path: String,
}

impl LocalOverrideSteamSection {
    pub fn is_empty(&self) -> bool {
        self.compatdata_path.trim().is_empty() && self.proton_path.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalOverrideRuntimeSection {
    #[serde(rename = "prefix_path", default)]
    pub prefix_path: String,
    #[serde(rename = "proton_path", default)]
    pub proton_path: String,
}

impl LocalOverrideRuntimeSection {
    pub fn is_empty(&self) -> bool {
        self.prefix_path.trim().is_empty() && self.proton_path.trim().is_empty()
    }
}

impl GameProfile {
    /// Returns the effective profile used at runtime where local overrides take precedence
    /// over portable base values.
    pub fn effective_profile(&self) -> Self {
        let mut merged = self.clone();

        if !self.local_override.game.executable_path.trim().is_empty() {
            merged.game.executable_path = self.local_override.game.executable_path.clone();
        }
        if !self.local_override.trainer.path.trim().is_empty() {
            merged.trainer.path = self.local_override.trainer.path.clone();
        }
        if !self.local_override.steam.compatdata_path.trim().is_empty() {
            merged.steam.compatdata_path = self.local_override.steam.compatdata_path.clone();
        }
        if !self.local_override.steam.proton_path.trim().is_empty() {
            merged.steam.proton_path = self.local_override.steam.proton_path.clone();
        }
        if !self.local_override.runtime.prefix_path.trim().is_empty() {
            merged.runtime.prefix_path = self.local_override.runtime.prefix_path.clone();
        }
        if !self.local_override.runtime.proton_path.trim().is_empty() {
            merged.runtime.proton_path = self.local_override.runtime.proton_path.clone();
        }

        merged
    }

    /// Returns the storage representation where machine-specific paths are moved into
    /// the local override section and portable base fields are path-free.
    pub fn storage_profile(&self) -> Self {
        let effective = self.effective_profile();
        let mut storage = effective.clone();

        storage.local_override.game.executable_path = effective.game.executable_path.clone();
        storage.local_override.trainer.path = effective.trainer.path.clone();
        storage.local_override.steam.compatdata_path = effective.steam.compatdata_path.clone();
        storage.local_override.steam.proton_path = effective.steam.proton_path.clone();
        storage.local_override.runtime.prefix_path = effective.runtime.prefix_path.clone();
        storage.local_override.runtime.proton_path = effective.runtime.proton_path.clone();

        storage.game.executable_path.clear();
        storage.trainer.path.clear();
        storage.steam.compatdata_path.clear();
        storage.steam.proton_path.clear();
        storage.runtime.prefix_path.clear();
        storage.runtime.proton_path.clear();

        storage
    }

    /// Returns the portable profile representation with all local machine-specific data removed.
    pub fn portable_profile(&self) -> Self {
        let mut portable = self.storage_profile();
        portable.local_override = LocalOverrideSection::default();
        portable
    }
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
                trainer_type: default_trainer_type(),
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

pub fn resolve_launch_method(profile: &GameProfile) -> &str {
    let method = profile.launch.method.trim();

    if matches!(method, "steam_applaunch" | "proton_run" | "native") {
        return method;
    }

    if profile.steam.enabled {
        return "steam_applaunch";
    }

    if looks_like_windows_executable(&profile.game.executable_path) {
        return "proton_run";
    }

    "native"
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile() -> GameProfile {
        GameProfile {
            game: GameSection {
                name: "Test Game".to_string(),
                executable_path: "/games/test.exe".to_string(),
            },
            trainer: TrainerSection::default(),
            injection: InjectionSection::default(),
            steam: SteamSection::default(),
            runtime: RuntimeSection::default(),
            launch: LaunchSection::default(),
            local_override: LocalOverrideSection::default(),
        }
    }

    #[test]
    fn resolve_launch_method_prefers_explicit_method() {
        let mut profile = sample_profile();
        profile.launch.method = "native".to_string();
        profile.steam.enabled = true;

        assert_eq!(resolve_launch_method(&profile), "native");
    }

    #[test]
    fn resolve_launch_method_falls_back_to_steam_enabled() {
        let mut profile = sample_profile();
        profile.launch.method.clear();
        profile.steam.enabled = true;

        assert_eq!(resolve_launch_method(&profile), "steam_applaunch");
    }

    #[test]
    fn resolve_launch_method_falls_back_to_proton_for_windows_games() {
        let mut profile = sample_profile();
        profile.launch.method.clear();
        profile.steam.enabled = false;

        assert_eq!(resolve_launch_method(&profile), "proton_run");
    }

    #[test]
    fn resolve_launch_method_falls_back_to_native_for_non_windows_games() {
        let mut profile = sample_profile();
        profile.launch.method.clear();
        profile.steam.enabled = false;
        profile.game.executable_path = "/games/test.sh".to_string();

        assert_eq!(resolve_launch_method(&profile), "native");
    }

    #[test]
    fn effective_profile_prefers_local_override_paths() {
        let mut profile = sample_profile();
        profile.game.executable_path = "/portable/game.exe".to_string();
        profile.local_override.game.executable_path = "/local/game.exe".to_string();
        profile.runtime.proton_path = "/portable/proton".to_string();
        profile.local_override.runtime.proton_path = "/local/proton".to_string();

        let effective = profile.effective_profile();
        assert_eq!(effective.game.executable_path, "/local/game.exe");
        assert_eq!(effective.runtime.proton_path, "/local/proton");
    }

    #[test]
    fn storage_profile_moves_machine_paths_to_local_override() {
        let mut profile = sample_profile();
        profile.game.executable_path = "/games/test.exe".to_string();
        profile.trainer.path = "/trainers/test.exe".to_string();
        profile.steam.compatdata_path = "/steam/compatdata/123".to_string();
        profile.steam.proton_path = "/steam/proton/proton".to_string();
        profile.runtime.prefix_path = "/prefix/123".to_string();
        profile.runtime.proton_path = "/runtime/proton".to_string();

        let storage = profile.storage_profile();
        assert_eq!(storage.game.executable_path, "");
        assert_eq!(storage.trainer.path, "");
        assert_eq!(storage.steam.compatdata_path, "");
        assert_eq!(storage.steam.proton_path, "");
        assert_eq!(storage.runtime.prefix_path, "");
        assert_eq!(storage.runtime.proton_path, "");
        assert_eq!(
            storage.local_override.game.executable_path,
            "/games/test.exe"
        );
        assert_eq!(storage.local_override.trainer.path, "/trainers/test.exe");
        assert_eq!(
            storage.local_override.steam.compatdata_path,
            "/steam/compatdata/123"
        );
        assert_eq!(
            storage.local_override.steam.proton_path,
            "/steam/proton/proton"
        );
        assert_eq!(storage.local_override.runtime.prefix_path, "/prefix/123");
        assert_eq!(
            storage.local_override.runtime.proton_path,
            "/runtime/proton"
        );
    }

    #[test]
    fn portable_profile_clears_local_override_fields() {
        let mut profile = sample_profile();
        profile.game.executable_path = "/games/test.exe".to_string();
        profile.trainer.path = "/trainers/test.exe".to_string();
        profile.steam.compatdata_path = "/steam/compatdata/123".to_string();
        profile.steam.proton_path = "/steam/proton/proton".to_string();
        profile.runtime.prefix_path = "/prefix/123".to_string();
        profile.runtime.proton_path = "/runtime/proton".to_string();

        let portable = profile.portable_profile();
        assert_eq!(portable.local_override.game.executable_path, "");
        assert_eq!(portable.local_override.trainer.path, "");
        assert_eq!(portable.local_override.steam.compatdata_path, "");
        assert_eq!(portable.local_override.steam.proton_path, "");
        assert_eq!(portable.local_override.runtime.prefix_path, "");
        assert_eq!(portable.local_override.runtime.proton_path, "");
    }

    #[test]
    fn storage_profile_roundtrip_is_idempotent() {
        let mut profile = sample_profile();
        profile.game.executable_path = "/games/test.exe".to_string();
        profile.trainer.path = "/trainers/test.exe".to_string();
        profile.steam.compatdata_path = "/steam/compatdata/123".to_string();
        profile.steam.proton_path = "/steam/proton/proton".to_string();
        profile.runtime.prefix_path = "/prefix/123".to_string();
        profile.runtime.proton_path = "/runtime/proton".to_string();

        let storage_once = profile.storage_profile();
        let storage_twice = storage_once.effective_profile().storage_profile();
        assert_eq!(storage_twice, storage_once);
    }

    #[test]
    fn normalize_preset_selection_clears_unknown_active_preset() {
        let mut launch = LaunchSection::default();
        launch.active_preset = "missing".to_string();
        launch.optimizations.enabled_option_ids = vec!["use_gamemode".to_string()];
        launch.normalize_preset_selection();
        assert!(launch.active_preset.is_empty());
        assert_eq!(
            launch.optimizations.enabled_option_ids,
            vec!["use_gamemode".to_string()]
        );
    }

    #[test]
    fn launch_presets_toml_roundtrip() {
        use std::collections::BTreeMap;

        let mut launch = LaunchSection::default();
        launch.method = "proton_run".to_string();
        launch.optimizations.enabled_option_ids = vec!["use_gamemode".to_string()];
        launch.active_preset = "quality".to_string();
        let mut presets = BTreeMap::new();
        presets.insert(
            "performance".to_string(),
            LaunchOptimizationsSection {
                enabled_option_ids: vec!["disable_steam_input".to_string()],
            },
        );
        presets.insert(
            "quality".to_string(),
            LaunchOptimizationsSection {
                enabled_option_ids: vec!["enable_hdr".to_string()],
            },
        );
        launch.presets = presets;

        let profile = GameProfile {
            launch,
            ..GameProfile::default()
        };
        let serialized = toml::to_string_pretty(&profile).expect("serialize");
        let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(parsed.launch.presets.len(), 2);
        assert_eq!(parsed.launch.active_preset, "quality");
        assert_eq!(
            parsed.launch.optimizations.enabled_option_ids,
            vec!["use_gamemode".to_string()]
        );
    }

    #[test]
    fn custom_env_vars_empty_omitted_from_toml_and_roundtrips() {
        let profile = sample_profile();
        let serialized = toml::to_string_pretty(&profile).expect("serialize");
        assert!(
            !serialized.contains("custom_env_vars"),
            "expected empty map skipped: {serialized}"
        );
        let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
        assert!(parsed.launch.custom_env_vars.is_empty());
    }

    #[test]
    fn custom_env_vars_nonempty_toml_roundtrip() {
        use std::collections::BTreeMap;

        let mut profile = sample_profile();
        profile.launch.custom_env_vars = BTreeMap::from([
            ("DXVK_ASYNC".to_string(), "1".to_string()),
            ("MANGOHUD".to_string(), "1".to_string()),
        ]);
        let serialized = toml::to_string_pretty(&profile).expect("serialize");
        assert!(serialized.contains("custom_env_vars"));
        let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(parsed.launch.custom_env_vars, profile.launch.custom_env_vars);
    }

    #[test]
    fn profile_toml_without_trainer_type_deserializes_unknown() {
        let toml = r#"
[game]
executable_path = "/games/x.exe"

[trainer]
path = "/t/y.exe"
type = "fling"
"#;
        let p: GameProfile = toml::from_str(toml).expect("deserialize");
        assert_eq!(p.trainer.trainer_type, "unknown");
    }

    #[test]
    fn profile_trainer_type_roundtrip_toml() {
        let mut p = sample_profile();
        p.trainer.trainer_type = "aurora".to_string();
        let s = toml::to_string_pretty(&p).expect("serialize");
        let back: GameProfile = toml::from_str(&s).expect("deserialize");
        assert_eq!(back.trainer.trainer_type, "aurora");
    }

    #[test]
    fn mangohud_config_default_omitted_from_profile_toml() {
        let profile = sample_profile();
        let serialized = toml::to_string_pretty(&profile).expect("serialize");
        assert!(
            !serialized.contains("[launch.mangohud]"),
            "default MangoHudConfig should be omitted from TOML output: {serialized}"
        );
    }

    #[test]
    fn mangohud_config_roundtrip() {
        let mut profile = sample_profile();
        profile.launch.mangohud = MangoHudConfig {
            enabled: true,
            fps_limit: Some(144),
            gpu_stats: true,
            cpu_stats: true,
            ram: false,
            frametime: true,
            battery: false,
            watt: false,
            position: Some(MangoHudPosition::TopRight),
        };
        let serialized = toml::to_string_pretty(&profile).expect("serialize");
        let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(parsed.launch.mangohud, profile.launch.mangohud);
    }
}
