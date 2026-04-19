use serde::{Deserialize, Serialize};

/// Profile-level health roll-up.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Stale,
    Broken,
}

/// Per-issue severity — distinct from `ValidationSeverity` which always returns Fatal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthIssueSeverity {
    Error,
    Warning,
    Info,
}

/// A single path-field issue found during health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIssue {
    pub field: String,
    pub path: String,
    pub message: String,
    pub remediation: String,
    pub severity: HealthIssueSeverity,
}

/// Per-profile health check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileHealthReport {
    pub name: String,
    pub status: HealthStatus,
    pub launch_method: String,
    pub issues: Vec<HealthIssue>,
    pub checked_at: String,
}

/// Batch health check result across all profiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckSummary {
    pub profiles: Vec<ProfileHealthReport>,
    pub healthy_count: usize,
    pub stale_count: usize,
    pub broken_count: usize,
    pub total_count: usize,
    pub validated_at: String,
}
