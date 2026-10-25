//! Config revision capture and retention (moved from the Tauri shell).
//!
//! The snapshot serializes the supplied `GameProfile` with
//! `toml::to_string_pretty` (not its storage form) so existing revision hashes
//! stay byte-compatible.

use crosshook_core::metadata::{
    sha256_hex, ConfigRevisionSource, MetadataStore, MAX_CONFIG_REVISIONS_PER_PROFILE,
};
use crosshook_core::profile::GameProfile;
use crosshook_core::settings::{config_history_max_revisions_from_settings, SettingsStore};

/// Outcome of one revision capture attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RevisionOutcome {
    /// A new revision row was inserted.
    Inserted(i64),
    /// The snapshot hash matched the latest revision; nothing inserted.
    Deduped,
    /// Capture was skipped or failed (already logged).
    Skipped,
}

/// Resolves the configured per-profile revision retention cap, falling back
/// to [`MAX_CONFIG_REVISIONS_PER_PROFILE`] when settings are unreadable.
pub(super) fn resolve_config_history_max_revisions(settings: &SettingsStore) -> usize {
    settings
        .load()
        .map(|s| config_history_max_revisions_from_settings(&s))
        .unwrap_or(MAX_CONFIG_REVISIONS_PER_PROFILE)
}

/// Captures a deduped config revision snapshot after a successful profile write.
///
/// Serializes the profile to canonical TOML, computes a SHA-256 content hash,
/// looks up the stable `profile_id`, then inserts. Never fails the caller.
pub(super) fn capture_config_revision(
    profile_name: &str,
    profile: &GameProfile,
    source: ConfigRevisionSource,
    source_revision_id: Option<i64>,
    metadata: &MetadataStore,
    max_revisions: usize,
) -> RevisionOutcome {
    if !metadata.is_available() {
        tracing::warn!(
            profile_name,
            "metadata store unavailable; config revision not recorded"
        );
        return RevisionOutcome::Skipped;
    }

    let snapshot_toml = match toml::to_string_pretty(profile) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                %e,
                profile_name,
                "failed to serialize profile for config revision capture"
            );
            return RevisionOutcome::Skipped;
        }
    };

    let content_hash = sha256_hex(snapshot_toml.as_bytes());

    let profile_id = match metadata.lookup_profile_id(profile_name) {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::warn!(
                profile_name,
                "profile_id not found in metadata — skipping config revision capture"
            );
            return RevisionOutcome::Skipped;
        }
        Err(e) => {
            tracing::warn!(
                %e,
                profile_name,
                "failed to look up profile_id for config revision capture"
            );
            return RevisionOutcome::Skipped;
        }
    };

    match metadata.insert_config_revision(
        &profile_id,
        profile_name,
        source,
        &content_hash,
        &snapshot_toml,
        source_revision_id,
        max_revisions,
    ) {
        Ok(Some(id)) => RevisionOutcome::Inserted(id),
        Ok(None) => RevisionOutcome::Deduped,
        Err(e) => {
            tracing::warn!(%e, profile_name, "failed to capture config revision");
            RevisionOutcome::Skipped
        }
    }
}
