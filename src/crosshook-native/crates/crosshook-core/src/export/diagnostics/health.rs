use crate::profile::health::{batch_check_health, HealthStatus, ProfileHealthReport};
use crate::profile::ProfileStore;

use super::profiles::CollectedProfiles;
use super::secrets::SecretScrubber;

/// Prefix of the health issue reported for a profile that fails to load.
const LOAD_FAILURE_PREFIX: &str = "Profile could not be loaded: ";

/// First line of a TOML parse error, which states only the error position.
const TOML_ERROR_POSITION_PREFIX: &str = "TOML parse error at line ";

/// Runs batch health check across all profiles and serializes the result.
///
/// Reports for files skipped as profiles (settings, recent files) are dropped
/// so the counts match the exported profiles. A load-failure message keeps
/// only the position of the TOML error, because the rest of the error quotes
/// the offending source line, which can hold a credential of any length. Every
/// issue string is then scrubbed with `scrubber` before serialization, because
/// JSON escaping (`"` → `\"`) would hide a secret from the byte-level bundle
/// scrubber.
pub(super) fn collect_health_summary(
    store: &ProfileStore,
    profiles: &CollectedProfiles,
    scrubber: &SecretScrubber,
) -> String {
    let mut summary = batch_check_health(store);
    summary
        .profiles
        .retain(|report| !profiles.excluded_names.contains(&report.name));
    for report in &mut summary.profiles {
        scrub_report(report, scrubber);
    }
    let count = |status: fn(&HealthStatus) -> bool| {
        summary
            .profiles
            .iter()
            .filter(|report| status(&report.status))
            .count()
    };
    summary.healthy_count = count(|status| matches!(status, HealthStatus::Healthy));
    summary.stale_count = count(|status| matches!(status, HealthStatus::Stale));
    summary.broken_count = count(|status| matches!(status, HealthStatus::Broken));
    summary.total_count = summary.profiles.len();
    serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "{}".to_string())
}

fn scrub_report(report: &mut ProfileHealthReport, scrubber: &SecretScrubber) {
    for issue in &mut report.issues {
        strip_load_error_source(&mut issue.message);
        for text in [
            &mut issue.field,
            &mut issue.path,
            &mut issue.message,
            &mut issue.remediation,
        ] {
            scrubber.scrub_string(text);
        }
    }
}

/// Reduces a load-failure `message` to its TOML error position line, dropping
/// the quoted source snippet and the error detail that can repeat a value.
/// Other messages are left unchanged.
fn strip_load_error_source(message: &mut String) {
    let Some(error) = message.strip_prefix(LOAD_FAILURE_PREFIX) else {
        return;
    };
    let position = error.lines().next().unwrap_or("");
    let kept = if position.starts_with(TOML_ERROR_POSITION_PREFIX) {
        position
    } else {
        "error details omitted"
    };
    *message = format!("{LOAD_FAILURE_PREFIX}{kept}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stripped(message: &str) -> String {
        let mut message = message.to_string();
        strip_load_error_source(&mut message);
        message
    }

    #[test]
    fn load_failure_keeps_only_the_toml_error_position() {
        assert_eq!(
            stripped(
                "Profile could not be loaded: TOML parse error at line 2, column 15\n  |\n\
                 2 | DB_PASSWORD = 7654321\n  |               ^^^^^^^\n\
                 invalid type: integer `7654321`, expected a string\n"
            ),
            "Profile could not be loaded: TOML parse error at line 2, column 15"
        );
        assert_eq!(
            stripped("Profile could not be loaded: invalid type: integer `7654321`"),
            "Profile could not be loaded: error details omitted"
        );
        assert_eq!(
            stripped("Path does not exist: /games/1.exe"),
            "Path does not exist: /games/1.exe"
        );
    }
}
