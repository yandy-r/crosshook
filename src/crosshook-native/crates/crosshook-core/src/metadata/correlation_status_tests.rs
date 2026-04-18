#![cfg(test)]

use super::{compute_correlation_status, VersionCorrelationStatus};

#[test]
fn test_compute_correlation_status_update_in_progress() {
    // state_flags Some(non-4) → UpdateInProgress regardless of other inputs.
    assert!(matches!(
        compute_correlation_status("build1", Some("build1"), None, None, Some(0)),
        VersionCorrelationStatus::UpdateInProgress
    ));
    assert!(matches!(
        compute_correlation_status("build1", Some("build1"), None, None, Some(6)),
        VersionCorrelationStatus::UpdateInProgress
    ));
    // state_flags None (manifest not found) → falls through to comparison, not UpdateInProgress.
    assert!(matches!(
        compute_correlation_status("build1", Some("build1"), None, None, None),
        VersionCorrelationStatus::Matched
    ));
}

#[test]
fn test_compute_correlation_status_untracked() {
    // No snapshot → Untracked (when state_flags is stable).
    assert!(matches!(
        compute_correlation_status("build1", None, None, None, Some(4)),
        VersionCorrelationStatus::Untracked
    ));
}

#[test]
fn test_compute_correlation_status_matched() {
    assert!(matches!(
        compute_correlation_status(
            "build1",
            Some("build1"),
            Some("hash-a"),
            Some("hash-a"),
            Some(4)
        ),
        VersionCorrelationStatus::Matched
    ));
    // Both trainer hashes None → also matched.
    assert!(matches!(
        compute_correlation_status("build1", Some("build1"), None, None, Some(4)),
        VersionCorrelationStatus::Matched
    ));
}

#[test]
fn test_compute_correlation_status_game_updated() {
    assert!(matches!(
        compute_correlation_status(
            "build-new",
            Some("build-old"),
            Some("hash-a"),
            Some("hash-a"),
            Some(4)
        ),
        VersionCorrelationStatus::GameUpdated
    ));
}

#[test]
fn test_compute_correlation_status_trainer_changed() {
    assert!(matches!(
        compute_correlation_status(
            "build1",
            Some("build1"),
            Some("hash-new"),
            Some("hash-old"),
            Some(4)
        ),
        VersionCorrelationStatus::TrainerChanged
    ));
}

#[test]
fn test_compute_correlation_status_both_changed() {
    assert!(matches!(
        compute_correlation_status(
            "build-new",
            Some("build-old"),
            Some("hash-new"),
            Some("hash-old"),
            Some(4)
        ),
        VersionCorrelationStatus::BothChanged
    ));
}
