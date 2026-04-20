use std::fmt;
use std::str::FromStr;

use crate::community::CommunityTapSubscription;
use crate::discovery::models::{
    default_external_trainer_sources, ExternalTrainerSourceSubscription,
};

/// Minimum allowed `recent_files_limit` (inclusive).
pub const RECENT_FILES_LIMIT_MIN: u32 = 1;
/// Maximum allowed `recent_files_limit` (inclusive).
pub const RECENT_FILES_LIMIT_MAX: u32 = 100;

fn default_log_filter() -> String {
    "info".to_string()
}

fn default_protonup_auto_suggest() -> bool {
    true
}

fn default_protonup_default_provider() -> String {
    "ge-proton".to_string()
}

fn default_recent_files_limit() -> u32 {
    10
}

fn default_console_drawer_collapsed() -> bool {
    true
}

/// Clamp recent-files list size to a safe range for persistence and UI.
pub fn clamp_recent_files_limit(n: u32) -> u32 {
    n.clamp(RECENT_FILES_LIMIT_MIN, RECENT_FILES_LIMIT_MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UmuPreference {
    #[default]
    Auto,
    Umu,
    Proton,
}

impl UmuPreference {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Umu => "umu",
            Self::Proton => "proton",
        }
    }
}

impl FromStr for UmuPreference {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "auto" => Ok(Self::Auto),
            "umu" => Ok(Self::Umu),
            "proton" => Ok(Self::Proton),
            other => Err(format!("unsupported umu preference: {other}")),
        }
    }
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,
    pub offline_mode: bool,
    /// High-contrast UI toggle for accessibility. Defaults to false.
    #[serde(default)]
    pub high_contrast: bool,
    pub steamgriddb_api_key: Option<String>,
    /// Default Proton path applied to new profiles when `runtime.proton_path` is empty.
    pub default_proton_path: String,
    /// Default launch method (`proton_run`, `steam_applaunch`, `native`, …) for new profiles.
    pub default_launch_method: String,
    /// Bundled optimization preset id (metadata `preset_id`) for new profiles; empty = none.
    pub default_bundled_optimization_preset_id: String,
    /// `source_directory` or `copy_to_prefix`.
    pub default_trainer_loading_mode: String,
    /// Tracing filter when `RUST_LOG` is unset (e.g. `info`, `debug`, `crosshook_core=debug`).
    #[serde(default = "default_log_filter")]
    pub log_filter: String,
    /// Initial console drawer collapsed state before any log-driven auto-expand.
    #[serde(default = "default_console_drawer_collapsed")]
    pub console_drawer_collapsed_default: bool,
    /// Max recent paths per category; clamped on save.
    #[serde(default = "default_recent_files_limit")]
    pub recent_files_limit: u32,
    /// Override profiles directory; empty = default under config. Restart required to apply.
    pub profiles_directory: String,
    /// Path override for winetricks/protontricks binary; empty = auto-detect from PATH.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub protontricks_binary_path: String,
    /// When true, auto-install missing prefix deps on first launch.
    #[serde(default)]
    pub auto_install_prefix_deps: bool,
    /// User opt-in for trainer discovery feature (external links to third-party sources).
    #[serde(default)]
    pub discovery_enabled: bool,
    /// User-managed list of external trainer discovery source subscriptions.
    /// FLiNG ships as the built-in default via `default_external_trainer_sources()`.
    #[serde(default = "default_external_trainer_sources")]
    pub external_trainer_sources: Vec<ExternalTrainerSourceSubscription>,
    /// When true, show ProtonUp runtime suggestions for community profiles.
    #[serde(default = "default_protonup_auto_suggest")]
    pub protonup_auto_suggest: bool,
    /// Optional path override for ProtonUp binary; empty = auto-detect.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub protonup_binary_path: String,
    /// Which provider to preselect in the Proton manager UI (e.g. `"ge-proton"`).
    #[serde(default = "default_protonup_default_provider")]
    pub protonup_default_provider: String,
    /// Override the auto-picked install root for Proton downloads; empty = auto-resolve.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub protonup_default_install_root: String,
    /// When true, include GitHub prereleases in the Proton catalog.
    #[serde(default)]
    pub protonup_include_prereleases: bool,
    /// User preference for umu-launcher vs direct Proton invocation.
    #[serde(default)]
    pub umu_preference: UmuPreference,
    /// Capability-level host-tool hints the user chose to dismiss permanently.
    #[serde(default)]
    pub host_tool_dashboard_dismissed_hints: Vec<String>,
    /// Optional default category filter for the host-tool dashboard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_tool_dashboard_default_category_filter: Option<String>,
    /// RFC 3339 timestamp of when the user dismissed the umu install nag; `None` = not dismissed.
    ///
    /// TODO(#269): Canonical dismissals live in SQLite `readiness_nag_dismissals`; this field remains
    /// for settings.toml backward compatibility and startup migration into the DB.
    pub install_nag_dismissed_at: Option<String>,
    /// RFC 3339 timestamp of when the user dismissed the Steam Deck gaming-mode caveats;
    /// `None` = not dismissed.
    ///
    /// TODO(#269): Canonical dismissals live in SQLite `readiness_nag_dismissals`; this field remains
    /// for settings.toml backward compatibility and startup migration into the DB.
    pub steam_deck_caveats_dismissed_at: Option<String>,
}

impl Default for AppSettingsData {
    fn default() -> Self {
        Self {
            auto_load_last_profile: false,
            last_used_profile: String::new(),
            community_taps: Vec::new(),
            onboarding_completed: false,
            offline_mode: false,
            high_contrast: false,
            steamgriddb_api_key: None,
            default_proton_path: String::new(),
            default_launch_method: String::new(),
            default_bundled_optimization_preset_id: String::new(),
            default_trainer_loading_mode: "source_directory".to_string(),
            log_filter: default_log_filter(),
            console_drawer_collapsed_default: true,
            recent_files_limit: default_recent_files_limit(),
            profiles_directory: String::new(),
            protontricks_binary_path: String::new(),
            auto_install_prefix_deps: false,
            discovery_enabled: false,
            external_trainer_sources: default_external_trainer_sources(),
            protonup_auto_suggest: default_protonup_auto_suggest(),
            protonup_binary_path: String::new(),
            protonup_default_provider: default_protonup_default_provider(),
            protonup_default_install_root: String::new(),
            protonup_include_prereleases: false,
            umu_preference: UmuPreference::Auto,
            host_tool_dashboard_dismissed_hints: Vec::new(),
            host_tool_dashboard_default_category_filter: None,
            install_nag_dismissed_at: None,
            steam_deck_caveats_dismissed_at: None,
        }
    }
}

impl fmt::Debug for AppSettingsData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppSettingsData")
            .field("auto_load_last_profile", &self.auto_load_last_profile)
            .field("last_used_profile", &self.last_used_profile)
            .field("community_taps", &self.community_taps)
            .field("onboarding_completed", &self.onboarding_completed)
            .field("offline_mode", &self.offline_mode)
            .field("high_contrast", &self.high_contrast)
            .field(
                "steamgriddb_api_key",
                &self
                    .steamgriddb_api_key
                    .as_ref()
                    .map(|_| "<redacted>")
                    .unwrap_or("<none>"),
            )
            .field("default_proton_path", &self.default_proton_path)
            .field("default_launch_method", &self.default_launch_method)
            .field(
                "default_bundled_optimization_preset_id",
                &self.default_bundled_optimization_preset_id,
            )
            .field(
                "default_trainer_loading_mode",
                &self.default_trainer_loading_mode,
            )
            .field("log_filter", &self.log_filter)
            .field(
                "console_drawer_collapsed_default",
                &self.console_drawer_collapsed_default,
            )
            .field("recent_files_limit", &self.recent_files_limit)
            .field("profiles_directory", &self.profiles_directory)
            .field("protontricks_binary_path", &self.protontricks_binary_path)
            .field("auto_install_prefix_deps", &self.auto_install_prefix_deps)
            .field("discovery_enabled", &self.discovery_enabled)
            .field("external_trainer_sources", &self.external_trainer_sources)
            .field("protonup_auto_suggest", &self.protonup_auto_suggest)
            .field("protonup_binary_path", &self.protonup_binary_path)
            .field("protonup_default_provider", &self.protonup_default_provider)
            .field(
                "protonup_default_install_root",
                &self.protonup_default_install_root,
            )
            .field(
                "protonup_include_prereleases",
                &self.protonup_include_prereleases,
            )
            .field("umu_preference", &self.umu_preference)
            .field(
                "host_tool_dashboard_dismissed_hints",
                &self.host_tool_dashboard_dismissed_hints,
            )
            .field(
                "host_tool_dashboard_default_category_filter",
                &self.host_tool_dashboard_default_category_filter,
            )
            .field("install_nag_dismissed_at", &self.install_nag_dismissed_at)
            .field(
                "steam_deck_caveats_dismissed_at",
                &self.steam_deck_caveats_dismissed_at,
            )
            .finish()
    }
}
