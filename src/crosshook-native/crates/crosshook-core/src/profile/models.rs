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

fn default_network_isolation() -> bool {
    true
}

fn is_default_network_isolation(v: &bool) -> bool {
    *v
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_protontricks: Vec<String>,
    /// Optional SHA-256 from a community profile manifest (advisory comparison at launch).
    #[serde(
        rename = "community_trainer_sha256",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub community_trainer_sha256: String,
}

impl Default for TrainerSection {
    fn default() -> Self {
        Self {
            path: String::new(),
            kind: String::new(),
            loading_mode: TrainerLoadingMode::default(),
            trainer_type: default_trainer_type(),
            required_protontricks: Vec::new(),
            community_trainer_sha256: String::new(),
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
    /// Optional Steam App ID for media/metadata lookup only.
    /// Does NOT affect launch behavior.
    #[serde(
        rename = "steam_app_id",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub steam_app_id: String,
}

impl RuntimeSection {
    pub fn is_empty(&self) -> bool {
        self.prefix_path.trim().is_empty()
            && self.proton_path.trim().is_empty()
            && self.working_directory.trim().is_empty()
            && self.steam_app_id.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchSection {
    #[serde(default)]
    pub method: String,
    #[serde(default, skip_serializing_if = "LaunchOptimizationsSection::is_empty")]
    pub optimizations: LaunchOptimizationsSection,
    /// Named optimization bundles (`[launch.presets.<name>]` in TOML).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub presets: BTreeMap<String, LaunchOptimizationsSection>,
    /// When set and present in `presets`, `optimizations` is kept in sync with that entry.
    #[serde(
        rename = "active_preset",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub active_preset: String,
    /// User-defined environment variables applied at launch (merged after optimizations).
    #[serde(
        rename = "custom_env_vars",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub custom_env_vars: BTreeMap<String, String>,
    /// When true, trainer processes are launched in an isolated network namespace
    /// via `unshare --net`, preventing outbound connections.
    #[serde(
        default = "default_network_isolation",
        skip_serializing_if = "is_default_network_isolation"
    )]
    pub network_isolation: bool,
    /// Per-profile gamescope compositor wrapper configuration.
    #[serde(default, skip_serializing_if = "GamescopeConfig::is_default")]
    pub gamescope: GamescopeConfig,
    /// Gamescope configuration for trainer launcher exports.
    /// Separate from the game config so the trainer can use a smaller
    /// compositor window (e.g. 800x400) while the game uses full resolution.
    #[serde(default, skip_serializing_if = "GamescopeConfig::is_default")]
    pub trainer_gamescope: GamescopeConfig,
    /// Per-profile MangoHud overlay configuration.
    #[serde(default, skip_serializing_if = "MangoHudConfig::is_default")]
    pub mangohud: MangoHudConfig,
}

impl Default for LaunchSection {
    fn default() -> Self {
        Self {
            method: String::new(),
            optimizations: LaunchOptimizationsSection::default(),
            presets: BTreeMap::new(),
            active_preset: String::new(),
            custom_env_vars: BTreeMap::new(),
            network_isolation: true,
            gamescope: GamescopeConfig::default(),
            trainer_gamescope: GamescopeConfig::default(),
            mangohud: MangoHudConfig::default(),
        }
    }
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

/// Collection-scoped overrides for the `LaunchSection` overrideable subset.
///
/// Each `Option<T>` field means "inherit from profile when None, replace when Some".
/// `custom_env_vars` is an **additive merge**: collection entries are union'd with the
/// profile's `launch.custom_env_vars` and collection keys win on collision.
///
/// Excluded fields (per PRD): `presets`, `active_preset` — preset coupling is too complex
/// to override at the collection level. Users wanting per-collection preset overrides go
/// through the profile-level editor via the modal's "Open in Profiles page →" link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CollectionDefaultsSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optimizations: Option<LaunchOptimizationsSection>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub custom_env_vars: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_isolation: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gamescope: Option<GamescopeConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_gamescope: Option<GamescopeConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mangohud: Option<MangoHudConfig>,
}

impl CollectionDefaultsSection {
    /// Returns true when no field would influence a profile merge.
    pub fn is_empty(&self) -> bool {
        self.method.is_none()
            && self.optimizations.is_none()
            && self.custom_env_vars.is_empty()
            && self.network_isolation.is_none()
            && self.gamescope.is_none()
            && self.trainer_gamescope.is_none()
            && self.mangohud.is_none()
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
    #[serde(rename = "custom_cover_art_path", default)]
    pub custom_cover_art_path: String,
    #[serde(rename = "custom_portrait_art_path", default)]
    pub custom_portrait_art_path: String,
    #[serde(rename = "custom_background_art_path", default)]
    pub custom_background_art_path: String,
}

impl LocalOverrideGameSection {
    pub fn is_empty(&self) -> bool {
        self.executable_path.trim().is_empty()
            && self.custom_cover_art_path.trim().is_empty()
            && self.custom_portrait_art_path.trim().is_empty()
            && self.custom_background_art_path.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalOverrideTrainerSection {
    #[serde(default)]
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_protontricks: Vec<String>,
}

impl LocalOverrideTrainerSection {
    pub fn is_empty(&self) -> bool {
        self.path.trim().is_empty() && self.extra_protontricks.is_empty()
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
    ///
    /// Backward-compat shim: forwards to [`Self::effective_profile_with`] with no
    /// collection-defaults layer. Existing call sites that don't know about per-collection
    /// launch defaults continue to get base + `local_override` only.
    pub fn effective_profile(&self) -> Self {
        self.effective_profile_with(None)
    }

    /// Returns the effective profile, optionally merging a collection-defaults layer
    /// between the base profile and the local override layer.
    ///
    /// Precedence (lowest → highest):
    ///   1. Base profile (`self`)
    ///   2. Collection defaults (if `Some`) — per-collection overrides from
    ///      [`CollectionDefaultsSection`]. `custom_env_vars` is an additive merge
    ///      where collection keys win on collision; all other fields are replacement.
    ///      Whitespace-only `method` is ignored so it cannot accidentally clobber
    ///      a profile's launch method.
    ///   3. `local_override.*` — machine-specific paths always win last so a power
    ///      user's portable executable_path is never trampled by collection defaults.
    ///
    /// # Runtime precedence after [`crate::profile::ProfileStore::load`]
    ///
    /// The 3-layer precedence above applies when this method is called on a
    /// profile that still carries a populated `local_override` section (e.g.,
    /// a freshly-constructed fixture in tests or a raw-storage profile read from
    /// TOML without going through the store loader).
    ///
    /// In production, the only caller that threads collection defaults is
    /// `profile_load`, which first calls `ProfileStore::load`. That loader
    /// already collapses `local_override` into layer 1 (baking the overrides
    /// into the base profile fields) and clears `self.local_override` to
    /// `LocalOverrideSection::default()`. By the time this method runs on a
    /// post-load profile, layer 3 is a no-op — the `local_override`-guarded
    /// branches below all see empty strings. The effective precedence at that
    /// call site is therefore:
    ///
    /// ```text
    /// (base ⊕ local_override, baked into layer 1)  →  collection defaults  →  ∅
    /// ```
    ///
    /// Today this has no user-visible effect because `CollectionDefaultsSection`
    /// and `LocalOverrideSection` have zero field overlap (collection = launch
    /// subset; local_override = machine-specific paths). If a future contributor
    /// adds overlapping fields, the "local_override always wins" guarantee still
    /// holds at the call site because layer 1 already contains the override,
    /// but any new fields must be audited here to preserve that invariant.
    pub fn effective_profile_with(&self, defaults: Option<&CollectionDefaultsSection>) -> Self {
        let mut merged = self.clone();

        // ── Layer 2: collection defaults ────────────────────────────────────
        if let Some(d) = defaults {
            if let Some(ref method) = d.method {
                if !method.trim().is_empty() {
                    merged.launch.method = method.clone();
                }
            }
            if let Some(ref opts) = d.optimizations {
                merged.launch.optimizations = opts.clone();
            }
            if !d.custom_env_vars.is_empty() {
                // Additive merge — collection keys win on collision, profile keys
                // without a collision are preserved.
                for (k, v) in &d.custom_env_vars {
                    merged.launch.custom_env_vars.insert(k.clone(), v.clone());
                }
            }
            if let Some(ni) = d.network_isolation {
                merged.launch.network_isolation = ni;
            }
            if let Some(ref gs) = d.gamescope {
                merged.launch.gamescope = gs.clone();
            }
            if let Some(ref tgs) = d.trainer_gamescope {
                merged.launch.trainer_gamescope = tgs.clone();
            }
            if let Some(ref mh) = d.mangohud {
                merged.launch.mangohud = mh.clone();
            }
        }

        // ── Layer 3: local_override (unchanged) ─────────────────────────────
        if !self.local_override.game.executable_path.trim().is_empty() {
            merged.game.executable_path = self.local_override.game.executable_path.clone();
        }
        if !self
            .local_override
            .game
            .custom_cover_art_path
            .trim()
            .is_empty()
        {
            merged.game.custom_cover_art_path =
                self.local_override.game.custom_cover_art_path.clone();
        }
        if !self
            .local_override
            .game
            .custom_portrait_art_path
            .trim()
            .is_empty()
        {
            merged.game.custom_portrait_art_path =
                self.local_override.game.custom_portrait_art_path.clone();
        }
        if !self
            .local_override
            .game
            .custom_background_art_path
            .trim()
            .is_empty()
        {
            merged.game.custom_background_art_path =
                self.local_override.game.custom_background_art_path.clone();
        }
        if !self.local_override.trainer.path.trim().is_empty() {
            merged.trainer.path = self.local_override.trainer.path.clone();
        }
        if !self.local_override.trainer.extra_protontricks.is_empty() {
            merged
                .trainer
                .required_protontricks
                .extend(self.local_override.trainer.extra_protontricks.clone());
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
        storage.local_override.game.custom_cover_art_path =
            effective.game.custom_cover_art_path.clone();
        storage.local_override.game.custom_portrait_art_path =
            effective.game.custom_portrait_art_path.clone();
        storage.local_override.game.custom_background_art_path =
            effective.game.custom_background_art_path.clone();
        storage.local_override.trainer.path = effective.trainer.path.clone();
        storage.local_override.steam.compatdata_path = effective.steam.compatdata_path.clone();
        storage.local_override.steam.proton_path = effective.steam.proton_path.clone();
        storage.local_override.runtime.prefix_path = effective.runtime.prefix_path.clone();
        storage.local_override.runtime.proton_path = effective.runtime.proton_path.clone();

        storage.game.executable_path.clear();
        storage.game.custom_cover_art_path.clear();
        storage.game.custom_portrait_art_path.clear();
        storage.game.custom_background_art_path.clear();
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

/// Returns the effective Steam App ID to use for art/metadata resolution.
///
/// Priority: `steam.app_id` (non-empty) → `runtime.steam_app_id`.
/// This field is media-only and does NOT affect how games launch (BR-9).
pub fn resolve_art_app_id(profile: &GameProfile) -> &str {
    let steam = profile.steam.app_id.trim();
    if !steam.is_empty() {
        return steam;
    }
    profile.runtime.steam_app_id.trim()
}

/// Validates a Steam App ID string.
///
/// Accepts: pure ASCII decimal digits, 1–12 characters.
/// Accepts: empty string (means "not set").
/// Rejects: non-digit characters, strings longer than 12 digits.
pub fn validate_steam_app_id(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Ok(());
    }
    if value.len() > 12 {
        return Err(format!(
            "Steam App ID must be at most 12 digits, got {}",
            value.len()
        ));
    }
    if !value.chars().all(|c| c.is_ascii_digit()) {
        return Err("Steam App ID must contain only numeric digits (0-9)".to_string());
    }
    Ok(())
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
        assert_eq!(
            parsed.launch.custom_env_vars,
            profile.launch.custom_env_vars
        );
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
    fn trainer_gamescope_default_omitted_from_profile_toml() {
        let profile = sample_profile();
        let serialized = toml::to_string_pretty(&profile).expect("serialize");
        assert!(
            !serialized.contains("[launch.trainer_gamescope]"),
            "default GamescopeConfig should be omitted from TOML output: {serialized}"
        );
    }

    #[test]
    fn trainer_gamescope_roundtrip() {
        let mut profile = sample_profile();
        profile.launch.trainer_gamescope = GamescopeConfig {
            enabled: true,
            internal_width: Some(800),
            internal_height: Some(400),
            fullscreen: true,
            grab_cursor: true,
            ..GamescopeConfig::default()
        };
        let serialized = toml::to_string_pretty(&profile).expect("serialize");
        assert!(serialized.contains("[launch.trainer_gamescope]"));
        let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(
            parsed.launch.trainer_gamescope,
            profile.launch.trainer_gamescope
        );
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

    // --- RuntimeSection::steam_app_id ---

    #[test]
    fn runtime_section_is_empty_returns_false_when_only_steam_app_id_set() {
        let section = RuntimeSection {
            steam_app_id: "1245620".to_string(),
            ..RuntimeSection::default()
        };
        assert!(
            !section.is_empty(),
            "is_empty() must return false when steam_app_id is set"
        );
    }

    #[test]
    fn runtime_section_is_empty_returns_true_when_all_fields_empty() {
        assert!(RuntimeSection::default().is_empty());
    }

    #[test]
    fn runtime_steam_app_id_roundtrips_through_toml() {
        let mut profile = sample_profile();
        profile.runtime.steam_app_id = "1245620".to_string();
        let serialized = toml::to_string_pretty(&profile).expect("serialize");
        assert!(
            serialized.contains("steam_app_id"),
            "serialized TOML must contain steam_app_id: {serialized}"
        );
        let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(parsed.runtime.steam_app_id, "1245620");
    }

    #[test]
    fn runtime_steam_app_id_empty_omitted_from_toml() {
        let profile = sample_profile();
        let serialized = toml::to_string_pretty(&profile).expect("serialize");
        assert!(
            !serialized.contains("steam_app_id"),
            "empty steam_app_id must be omitted from TOML: {serialized}"
        );
    }

    // --- resolve_art_app_id ---

    #[test]
    fn resolve_art_app_id_prefers_steam_app_id_when_both_set() {
        let mut profile = sample_profile();
        profile.steam.app_id = "111111".to_string();
        profile.runtime.steam_app_id = "222222".to_string();
        assert_eq!(resolve_art_app_id(&profile), "111111");
    }

    #[test]
    fn resolve_art_app_id_falls_back_to_runtime_when_steam_empty() {
        let mut profile = sample_profile();
        profile.steam.app_id = String::new();
        profile.runtime.steam_app_id = "1245620".to_string();
        assert_eq!(resolve_art_app_id(&profile), "1245620");
    }

    #[test]
    fn resolve_art_app_id_returns_empty_when_neither_set() {
        let profile = sample_profile();
        assert_eq!(resolve_art_app_id(&profile), "");
    }

    #[test]
    fn resolve_art_app_id_trims_whitespace() {
        let mut profile = sample_profile();
        profile.steam.app_id = "  ".to_string();
        profile.runtime.steam_app_id = " 1245620 ".to_string();
        assert_eq!(resolve_art_app_id(&profile), "1245620");
    }

    // --- validate_steam_app_id ---

    #[test]
    fn validate_steam_app_id_accepts_empty_string() {
        assert!(validate_steam_app_id("").is_ok());
    }

    #[test]
    fn validate_steam_app_id_accepts_valid_ids() {
        assert!(validate_steam_app_id("1245620").is_ok());
        assert!(validate_steam_app_id("570").is_ok());
        assert!(validate_steam_app_id("730").is_ok());
        assert!(validate_steam_app_id("123456789012").is_ok()); // 12 digits — max
    }

    #[test]
    fn validate_steam_app_id_rejects_non_numeric() {
        assert!(validate_steam_app_id("abc").is_err());
        assert!(validate_steam_app_id("123abc").is_err());
        assert!(validate_steam_app_id("12.3").is_err());
        assert!(validate_steam_app_id("12 3").is_err());
    }

    #[test]
    fn validate_steam_app_id_rejects_more_than_12_digits() {
        assert!(validate_steam_app_id("1234567890123").is_err()); // 13 digits
    }

    #[test]
    fn validate_steam_app_id_accepts_exactly_12_digits() {
        assert!(validate_steam_app_id("123456789012").is_ok());
    }

    // --- Tri-art fields (Task 2.1) ---

    #[test]
    fn local_override_game_section_not_empty_when_portrait_art_set() {
        let section = LocalOverrideGameSection {
            custom_portrait_art_path: "/art/portrait.png".to_string(),
            ..LocalOverrideGameSection::default()
        };
        assert!(!section.is_empty());
    }

    #[test]
    fn local_override_game_section_not_empty_when_background_art_set() {
        let section = LocalOverrideGameSection {
            custom_background_art_path: "/art/bg.png".to_string(),
            ..LocalOverrideGameSection::default()
        };
        assert!(!section.is_empty());
    }

    #[test]
    fn storage_profile_moves_portrait_and_background_to_local_override() {
        let mut profile = sample_profile();
        profile.game.custom_portrait_art_path = "/art/portrait.png".to_string();
        profile.game.custom_background_art_path = "/art/bg.png".to_string();

        let storage = profile.storage_profile();
        assert!(storage.game.custom_portrait_art_path.is_empty());
        assert!(storage.game.custom_background_art_path.is_empty());
        assert_eq!(
            storage.local_override.game.custom_portrait_art_path,
            "/art/portrait.png"
        );
        assert_eq!(
            storage.local_override.game.custom_background_art_path,
            "/art/bg.png"
        );
    }

    #[test]
    fn effective_profile_merges_portrait_and_background_from_local_override() {
        let mut profile = sample_profile();
        profile.local_override.game.custom_portrait_art_path = "/override/portrait.png".to_string();
        profile.local_override.game.custom_background_art_path = "/override/bg.png".to_string();

        let effective = profile.effective_profile();
        assert_eq!(
            effective.game.custom_portrait_art_path,
            "/override/portrait.png"
        );
        assert_eq!(
            effective.game.custom_background_art_path,
            "/override/bg.png"
        );
    }

    // --- Phase 3: per-collection launch defaults merge layer ---

    #[test]
    fn effective_profile_with_none_equals_shim() {
        let mut profile = sample_profile();
        profile
            .launch
            .custom_env_vars
            .insert("DXVK_HUD".to_string(), "1".to_string());
        profile.local_override.game.executable_path = "/local/game.exe".to_string();

        let via_shim = profile.effective_profile();
        let via_with_none = profile.effective_profile_with(None);
        assert_eq!(via_shim, via_with_none, "shim must equal explicit None");
    }

    #[test]
    fn effective_profile_with_merges_collection_defaults_between_base_and_local_override() {
        let mut profile = sample_profile();
        profile.launch.method = "native".to_string();
        profile
            .launch
            .custom_env_vars
            .insert("PROFILE_ONLY".to_string(), "A".to_string());
        profile.game.executable_path = "/portable/game.exe".to_string();
        profile.local_override.game.executable_path = "/local/game.exe".to_string();

        let mut defaults = CollectionDefaultsSection::default();
        defaults
            .custom_env_vars
            .insert("COLLECTION_ONLY".to_string(), "B".to_string());
        defaults
            .custom_env_vars
            .insert("PROFILE_ONLY".to_string(), "OVERRIDDEN".to_string());
        defaults.network_isolation = Some(false);
        defaults.method = Some("proton_run".to_string());

        let merged = profile.effective_profile_with(Some(&defaults));

        // ── Layer 3 (local_override) still wins last ──
        assert_eq!(merged.game.executable_path, "/local/game.exe");

        // ── Layer 2 (collection defaults) applies ──
        assert_eq!(merged.launch.method, "proton_run");
        assert!(!merged.launch.network_isolation);
        assert_eq!(
            merged.launch.custom_env_vars.get("COLLECTION_ONLY").cloned(),
            Some("B".to_string())
        );
        // ── Collection key wins on collision ──
        assert_eq!(
            merged.launch.custom_env_vars.get("PROFILE_ONLY").cloned(),
            Some("OVERRIDDEN".to_string())
        );
    }

    #[test]
    fn effective_profile_with_none_fields_do_not_overwrite_profile() {
        let mut profile = sample_profile();
        profile.launch.method = "native".to_string();
        profile.launch.network_isolation = true;
        profile.launch.gamescope = GamescopeConfig::default();
        profile
            .launch
            .custom_env_vars
            .insert("PROFILE_KEY".to_string(), "retained".to_string());

        // Empty defaults: every Option is None, BTreeMap is empty → no-op merge.
        let defaults = CollectionDefaultsSection::default();
        assert!(defaults.is_empty());
        let merged = profile.effective_profile_with(Some(&defaults));

        assert_eq!(merged.launch.method, "native");
        assert!(merged.launch.network_isolation);
        assert_eq!(
            merged.launch.custom_env_vars.get("PROFILE_KEY").cloned(),
            Some("retained".to_string())
        );
        // ── Profile env vars never dropped ──
        assert_eq!(merged.launch.custom_env_vars.len(), 1);
    }

    #[test]
    fn effective_profile_with_ignores_whitespace_only_method() {
        let mut profile = sample_profile();
        profile.launch.method = "native".to_string();

        let mut defaults = CollectionDefaultsSection::default();
        defaults.method = Some("   ".to_string()); // whitespace-only must NOT clobber profile

        let merged = profile.effective_profile_with(Some(&defaults));
        assert_eq!(
            merged.launch.method, "native",
            "whitespace method must not clobber profile"
        );
    }

    #[test]
    fn portrait_and_background_art_paths_roundtrip_through_toml() {
        let mut profile = sample_profile();
        profile.game.custom_portrait_art_path = "/art/portrait.png".to_string();
        profile.game.custom_background_art_path = "/art/bg.png".to_string();

        let serialized = toml::to_string_pretty(&profile).expect("serialize");
        assert!(serialized.contains("custom_portrait_art_path"));
        assert!(serialized.contains("custom_background_art_path"));

        let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(parsed.game.custom_portrait_art_path, "/art/portrait.png");
        assert_eq!(parsed.game.custom_background_art_path, "/art/bg.png");
    }

    #[test]
    fn empty_portrait_and_background_art_paths_omitted_from_toml() {
        let profile = sample_profile();
        let serialized = toml::to_string_pretty(&profile).expect("serialize");
        assert!(!serialized.contains("custom_portrait_art_path"));
        assert!(!serialized.contains("custom_background_art_path"));
    }

    // --- required_protontricks / extra_protontricks (Task 1.2) ---

    #[test]
    fn trainer_section_roundtrip_with_required_protontricks() {
        let section = TrainerSection {
            required_protontricks: vec!["vcrun2019".to_string(), "dotnet48".to_string()],
            ..Default::default()
        };
        let toml_str = toml::to_string_pretty(&section).unwrap();
        let deserialized: TrainerSection = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            deserialized.required_protontricks,
            section.required_protontricks
        );
    }

    #[test]
    fn trainer_section_roundtrip_without_required_protontricks() {
        let section = TrainerSection::default();
        let toml_str = toml::to_string_pretty(&section).unwrap();
        assert!(
            !toml_str.contains("required_protontricks"),
            "empty vec should be skipped in serialization"
        );
        let deserialized: TrainerSection = toml::from_str(&toml_str).unwrap();
        assert!(deserialized.required_protontricks.is_empty());
    }

    #[test]
    fn trainer_section_deserialize_without_field() {
        // Simulate existing TOML that doesn't have the field (backward compatibility)
        let toml_str = r#"
path = "/some/path"
type = "fling"
loading_mode = "source_directory"
"#;
        let section: TrainerSection = toml::from_str(toml_str).unwrap();
        assert!(section.required_protontricks.is_empty());
    }

    // --- network_isolation ---

    #[test]
    fn network_isolation_defaults_true_when_absent_from_toml() {
        let toml = r#"
[game]
executable_path = "/games/x.exe"
[trainer]
path = "/t/y.exe"
type = "fling"
[launch]
method = "proton_run"
"#;
        let p: GameProfile = toml::from_str(toml).expect("deserialize");
        assert!(p.launch.network_isolation);
    }

    #[test]
    fn network_isolation_false_roundtrips_through_toml() {
        let mut p = sample_profile();
        p.launch.network_isolation = false;
        let s = toml::to_string_pretty(&p).expect("serialize");
        assert!(s.contains("network_isolation = false"));
        let back: GameProfile = toml::from_str(&s).expect("deserialize");
        assert!(!back.launch.network_isolation);
    }

    #[test]
    fn network_isolation_true_omitted_from_toml() {
        let mut p = sample_profile();
        p.launch.network_isolation = true;
        let s = toml::to_string_pretty(&p).expect("serialize");
        assert!(
            !s.contains("network_isolation"),
            "true (default) should be omitted: {s}"
        );
    }

    #[test]
    fn launch_section_default_has_network_isolation_true() {
        let launch = LaunchSection::default();
        assert!(launch.network_isolation);
    }

    #[test]
    fn local_override_trainer_section_with_extra_protontricks() {
        let section = LocalOverrideTrainerSection {
            path: String::new(),
            extra_protontricks: vec!["xact".to_string()],
        };
        assert!(!section.is_empty());
        let toml_str = toml::to_string_pretty(&section).unwrap();
        assert!(toml_str.contains("extra_protontricks"));
    }

    #[test]
    fn local_override_trainer_section_empty_when_both_empty() {
        let section = LocalOverrideTrainerSection::default();
        assert!(section.is_empty());
    }
}
