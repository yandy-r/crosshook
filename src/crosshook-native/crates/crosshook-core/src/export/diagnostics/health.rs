use crate::profile::health::{batch_check_health, HealthStatus, ProfileHealthReport};
use crate::profile::ProfileStore;

use super::profiles::CollectedProfiles;
use super::secrets::SecretScrubber;

/// Runs batch health check across all profiles and serializes the result.
///
/// Reports for files skipped as profiles (settings, recent files) are dropped
/// so the counts match the exported profiles. Every issue string is scrubbed
/// before serialization, because JSON escaping (`"` → `\"`) would hide a
/// secret from the byte-level bundle scrubber: first with `scrubber`, then
/// with the profile's own removed values at any length (see
/// [`SecretScrubber::with_exact`]), since a load error quotes those values.
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
        let exact = SecretScrubber::with_exact(
            std::iter::empty(),
            profiles.secrets_of(&report.name).iter().cloned(),
        );
        scrub_report(report, scrubber, &exact);
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

fn scrub_report(
    report: &mut ProfileHealthReport,
    scrubber: &SecretScrubber,
    exact: &SecretScrubber,
) {
    for issue in &mut report.issues {
        for text in [
            &mut issue.field,
            &mut issue.path,
            &mut issue.message,
            &mut issue.remediation,
        ] {
            scrubber.scrub_string(text);
            exact.scrub_string(text);
        }
    }
}
