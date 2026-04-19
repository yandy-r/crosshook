pub mod capability;
pub mod capability_loader;
pub mod catalog;
pub mod details;
pub mod distro;
mod install_advice;
pub mod readiness;

use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::profile::health::HealthIssue;

pub use capability::{derive_capabilities, Capability, CapabilityMap, CapabilityState};
pub use capability_loader::{
    global_capability_map, initialize_capability_map, load_capability_map,
};
pub use catalog::{
    global_readiness_catalog, initialize_readiness_catalog, load_readiness_catalog,
    ReadinessCatalog,
};
pub use details::{probe_host_tool_details, HostToolDetails};
pub use distro::detect_host_distro_family_from_os_release;
pub use readiness::{
    apply_install_nag_dismissal, apply_readiness_nag_dismissals,
    apply_steam_deck_caveats_dismissal, check_generalized_readiness, check_system_readiness,
};

/// Host distribution family for install guidance (from `/etc/os-release` on the host).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/onboarding.ts"))]
#[serde(rename_all = "PascalCase")]
pub enum HostDistroFamily {
    Arch,
    Nobara,
    Fedora,
    Debian,
    Nix,
    Unknown,
    /// SteamOS / Steam Deck image.
    SteamOS,
    /// Gaming-first immutables (e.g. Bazzite, ChimeraOS).
    GamingImmutable,
    /// Bare immutables without a full gaming stack pre-installed (e.g. Fedora Atomic variants).
    BareImmutable,
}

impl HostDistroFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            HostDistroFamily::Arch => "Arch",
            HostDistroFamily::Nobara => "Nobara",
            HostDistroFamily::Fedora => "Fedora",
            HostDistroFamily::Debian => "Debian",
            HostDistroFamily::Nix => "Nix",
            HostDistroFamily::Unknown => "Unknown",
            HostDistroFamily::SteamOS => "SteamOS",
            HostDistroFamily::GamingImmutable => "GamingImmutable",
            HostDistroFamily::BareImmutable => "BareImmutable",
        }
    }

    /// Parse from catalog `distro_family` string (PascalCase, matching [`Self::as_str`]).
    pub fn from_catalog_key(s: &str) -> Option<Self> {
        match s {
            "Arch" => Some(Self::Arch),
            "Nobara" => Some(Self::Nobara),
            "Fedora" => Some(Self::Fedora),
            "Debian" => Some(Self::Debian),
            "Nix" => Some(Self::Nix),
            "Unknown" => Some(Self::Unknown),
            "SteamOS" => Some(Self::SteamOS),
            "GamingImmutable" => Some(Self::GamingImmutable),
            "BareImmutable" => Some(Self::BareImmutable),
            _ => None,
        }
    }
}

/// One install-hint row for a host tool and distro family (from TOML / DB).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/onboarding.ts"))]
pub struct HostToolInstallCommand {
    pub distro_family: String,
    pub command: String,
    pub alternatives: String,
}

/// A host tool definition from the readiness catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub struct HostToolEntry {
    pub tool_id: String,
    pub binary_name: String,
    pub display_name: String,
    pub description: String,
    pub docs_url: String,
    pub required: bool,
    pub category: String,
    #[serde(default)]
    pub install_commands: Vec<HostToolInstallCommand>,
}

/// Result row for one host tool probe (onboarding / generalized readiness).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/onboarding.ts"))]
pub struct HostToolCheckResult {
    pub tool_id: String,
    pub display_name: String,
    pub is_available: bool,
    pub is_required: bool,
    pub category: String,
    /// Project docs / upstream URL for this tool (from catalog).
    #[serde(default)]
    pub docs_url: String,
    /// Reported tool version when the probe can resolve it.
    #[serde(default)]
    pub tool_version: Option<String>,
    /// Resolved runtime path for the detected tool binary.
    #[serde(default)]
    pub resolved_path: Option<String>,
    /// Populated when the tool is missing and guidance applies (e.g. Flatpak).
    pub install_guidance: Option<HostToolInstallCommand>,
}

/// Actionable installation guidance for umu-launcher on the host, emitted when
/// running inside a Flatpak sandbox and `umu-run` cannot be resolved from the
/// host environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/onboarding.ts"))]
pub struct UmuInstallGuidance {
    /// Host shell command the user can run to install umu-launcher.
    pub install_command: String,
    /// URL pointing to official umu-launcher install documentation.
    pub docs_url: String,
    /// Human-readable description for the guidance row.
    pub description: String,
}

/// Caveats and known limitations for Steam Deck users, surfaced during onboarding
/// when the system is identified as a Steam Deck.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/onboarding.ts"))]
pub struct SteamDeckCaveats {
    /// Human-readable summary of the Steam Deck caveat context.
    pub description: String,
    /// Individual caveat items the user should be aware of.
    pub items: Vec<String>,
    /// URL pointing to relevant documentation for Steam Deck usage.
    pub docs_url: String,
}

/// System readiness check result returned by `check_system_readiness` / `check_generalized_readiness`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/onboarding.ts"))]
pub struct ReadinessCheckResult {
    pub checks: Vec<HealthIssue>,
    pub all_passed: bool,
    pub critical_failures: usize,
    pub warnings: usize,
    /// Actionable umu install guidance; present only when running inside a
    /// Flatpak sandbox and `umu-run` cannot be resolved on the host.
    /// `None` for native installs and when umu-run is already available.
    pub umu_install_guidance: Option<UmuInstallGuidance>,
    /// Known Steam Deck caveats surfaced during onboarding when the system is
    /// identified as a Steam Deck. `None` on non-Steam-Deck systems.
    pub steam_deck_caveats: Option<SteamDeckCaveats>,
    /// Host tool rows from the readiness catalog (empty unless `check_generalized_readiness` ran).
    #[serde(default)]
    pub tool_checks: Vec<HostToolCheckResult>,
    /// Detected host distro family key (e.g. `Arch`, `SteamOS`).
    #[serde(default)]
    pub detected_distro_family: String,
}

/// A single trainer source or loading mode entry in onboarding guidance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/onboarding.ts"))]
pub struct TrainerGuidanceEntry {
    pub id: String,
    pub title: String,
    pub description: String,
    pub when_to_use: String,
    pub examples: Vec<String>,
}

/// Static compiled guidance content returned by `get_trainer_guidance`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/onboarding.ts"))]
pub struct TrainerGuidanceContent {
    pub loading_modes: Vec<TrainerGuidanceEntry>,
    pub trainer_sources: Vec<TrainerGuidanceEntry>,
    pub verification_steps: Vec<String>,
}
