//! Metadata reconciliation, auto-load resolution, onboarding derivation and
//! legacy readiness-nag sync.
//!
//! Moved from `src-tauri/src/startup.rs` (deleted by the shell slice). Every
//! helper is best-effort at its call site per the Phase-B contract: fatal
//! store failures are typed [`BootstrapError`]s from `stores.rs`, while
//! reconciliation / nag-sync problems are logged and startup continues.
//!
//! [`resolve_auto_load_profile`] and [`derive_onboarding`] take an
//! already-loaded [`AppSettingsData`] so the startup snapshot can recompute
//! them live against the settings snapshot it already loaded.

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::AppSettingsData;

use super::report::OnboardingStartup;
use super::BootstrapError;

/// Synchronizes the metadata profile table from the profile store and sweeps
/// abandoned launch operations. Returns a [`BootstrapError`] only when the
/// profile sync itself fails; the caller decides whether to continue degraded
/// (the bootstrap does: warn and continue).
pub(crate) fn run_metadata_reconciliation(
    metadata_store: &MetadataStore,
    profile_store: &ProfileStore,
) -> Result<(), BootstrapError> {
    let report = metadata_store
        .sync_profiles_from_store(profile_store)
        .map_err(|error| BootstrapError::StoreInit {
            store: super::StoreKind::Profiles,
            detail: format!("metadata reconciliation: {error}"),
        })?;
    if report.created > 0 || report.updated > 0 {
        tracing::info!(
            created = report.created,
            updated = report.updated,
            "startup metadata reconciliation complete"
        );
    }
    match metadata_store.sweep_abandoned_operations() {
        Ok(count) if count > 0 => {
            tracing::info!(swept = count, "startup abandoned operation sweep complete");
        }
        Err(error) => {
            tracing::warn!(%error, "startup abandoned operation sweep failed");
        }
        _ => {}
    }
    Ok(())
}

/// Resolves the profile to auto-load from already-loaded settings and names.
pub(crate) fn resolve_auto_load_profile(
    settings: &AppSettingsData,
    available_profiles: &[String],
) -> Option<String> {
    if !settings.auto_load_last_profile {
        return None;
    }

    let last_used_profile = settings.last_used_profile.trim();
    if last_used_profile.is_empty() {
        return None;
    }

    if available_profiles
        .iter()
        .any(|profile_name| profile_name == last_used_profile)
    {
        return Some(last_used_profile.to_string());
    }

    None
}

/// Derives the onboarding prompt state from already-loaded settings and names.
pub(crate) fn derive_onboarding(
    settings: &AppSettingsData,
    profiles: &[String],
) -> OnboardingStartup {
    let show = !settings.onboarding_completed;
    let has_profiles = !profiles.is_empty();
    OnboardingStartup { show, has_profiles }
}

/// Persists legacy readiness-nag dismissals carried in settings into the
/// metadata dismissal table. Best-effort: failures are logged, never fatal.
pub(crate) fn sync_nags(settings: &AppSettingsData, metadata_store: &MetadataStore) {
    if !metadata_store.is_available() {
        return;
    }
    if settings.install_nag_dismissed_at.is_some() {
        if let Err(error) = metadata_store.dismiss_readiness_nag("umu_run", 36500) {
            tracing::warn!(%error, "failed to persist legacy umu nag dismissal");
        }
    }
    if settings.steam_deck_caveats_dismissed_at.is_some() {
        if let Err(error) = metadata_store.dismiss_readiness_nag("steam_deck_caveats", 36500) {
            tracing::warn!(%error, "failed to persist legacy Steam Deck nag dismissal");
        }
    }
}

#[cfg(test)]
mod tests;
