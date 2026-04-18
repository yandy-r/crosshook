use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::gamescope::GamescopeConfig;
use super::mangohud::MangoHudConfig;

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

fn default_network_isolation() -> bool {
    true
}

fn is_default_network_isolation(v: &bool) -> bool {
    *v
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

    /// Returns the trainer gamescope config to use at launch/export time.
    ///
    /// Priority:
    /// 1. Explicit trainer gamescope override when enabled
    /// 2. Auto-generated windowed config derived from the game gamescope config
    /// 3. Default disabled config when the game gamescope config is disabled
    pub fn resolved_trainer_gamescope(&self) -> GamescopeConfig {
        crate::launch::resolve_trainer_gamescope(&self.gamescope, Some(&self.trainer_gamescope))
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
