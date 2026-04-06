//! Trainer SHA-256 baseline checks surfaced as non-blocking launch warnings.

use std::path::Path;

use crate::launch::request::LaunchValidationIssue;
use crate::metadata::MetadataStore;
use crate::offline::{
    trainer_hash_launch_check, TrainerHashBaselineResult, TrainerHashLaunchOutcome,
};
use crate::profile::GameProfile;

/// Maps a launch-time hash outcome to UI-facing validation issues (warnings only).
pub fn launch_issues_from_trainer_hash_outcome(outcome: TrainerHashLaunchOutcome) -> Vec<LaunchValidationIssue> {
    let mut issues = Vec::new();
    if let TrainerHashBaselineResult::Mismatch {
        stored_hash,
        current_hash,
    } = outcome.baseline
    {
        issues.push(LaunchValidationIssue::trainer_hash_mismatch(
            &stored_hash,
            &current_hash,
        ));
    }
    if let Some(adv) = outcome.community_advisory {
        issues.push(LaunchValidationIssue::trainer_hash_community_advisory(
            &adv.expected,
            &adv.current,
        ));
    }
    issues
}

/// Collects trainer hash warnings for a saved profile when metadata DB and `profile_id` exist.
pub fn collect_trainer_hash_launch_warnings(
    metadata: &MetadataStore,
    profile_id: &str,
    profile: &GameProfile,
) -> Vec<LaunchValidationIssue> {
    if !metadata.is_available() {
        return Vec::new();
    }
    let effective = profile.effective_profile();
    let trainer_path_str = effective.trainer.path.trim();
    if trainer_path_str.is_empty() {
        return Vec::new();
    }
    let path = Path::new(trainer_path_str);
    let community = effective.trainer.community_trainer_sha256.trim();
    let community_opt = if community.is_empty() {
        None
    } else {
        Some(community)
    };

    let outcome = match metadata.with_sqlite_conn("trainer hash launch check", |conn| {
        trainer_hash_launch_check(conn, profile_id, path, community_opt)
    }) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    launch_issues_from_trainer_hash_outcome(outcome)
}
