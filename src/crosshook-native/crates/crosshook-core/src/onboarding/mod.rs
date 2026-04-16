pub mod readiness;

use serde::{Deserialize, Serialize};

use crate::profile::health::HealthIssue;

pub use readiness::{
    apply_install_nag_dismissal, apply_steam_deck_caveats_dismissal, check_system_readiness,
};

/// Actionable installation guidance for umu-launcher on the host, emitted when
/// running inside a Flatpak sandbox and `umu-run` cannot be resolved from the
/// host environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub struct SteamDeckCaveats {
    /// Human-readable summary of the Steam Deck caveat context.
    pub description: String,
    /// Individual caveat items the user should be aware of.
    pub items: Vec<String>,
    /// URL pointing to relevant documentation for Steam Deck usage.
    pub docs_url: String,
}

/// System readiness check result returned by `check_system_readiness`.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

/// A single trainer source or loading mode entry in onboarding guidance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerGuidanceEntry {
    pub id: String,
    pub title: String,
    pub description: String,
    pub when_to_use: String,
    pub examples: Vec<String>,
}

/// Static compiled guidance content returned by `get_trainer_guidance`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerGuidanceContent {
    pub loading_modes: Vec<TrainerGuidanceEntry>,
    pub trainer_sources: Vec<TrainerGuidanceEntry>,
    pub verification_steps: Vec<String>,
}
