pub mod readiness;

use serde::{Deserialize, Serialize};

use crate::profile::health::HealthIssue;

pub use readiness::{apply_install_nag_dismissal, check_system_readiness};

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
