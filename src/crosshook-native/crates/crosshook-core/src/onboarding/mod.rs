pub mod readiness;

use serde::{Deserialize, Serialize};

use crate::profile::health::HealthIssue;

pub use readiness::check_system_readiness;

/// System readiness check result returned by `check_system_readiness`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessCheckResult {
    pub checks: Vec<HealthIssue>,
    pub all_passed: bool,
    pub critical_failures: usize,
    pub warnings: usize,
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
