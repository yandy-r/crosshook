mod checks;
mod dependency;
mod path_probe;
mod profile;
mod types;

pub use dependency::build_dependency_health_issues;
pub use profile::{batch_check_health, batch_check_health_with_enrich, check_profile_health};
pub use types::{
    HealthCheckSummary, HealthIssue, HealthIssueSeverity, HealthStatus, ProfileHealthReport,
};

#[cfg(test)]
mod tests;
