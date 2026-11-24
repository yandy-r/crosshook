//! Conversions from `crosshook-app` / `crosshook-core` types (`app` feature).
//!
//! [`ProfileChangeReason`] is matched exhaustively so a new backend variant fails
//! to compile here instead of silently mapping to something else.
//! `#[non_exhaustive]` [`AppEvent`] and `&str` launch methods use explicit
//! fallbacks (`Vec::new()` and [`LaunchMethod::Native`]).

use crosshook_app::{AppEvent, ProfileChangeReason};
use crosshook_core::launch::request::{METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};
use crosshook_core::profile::{resolve_launch_method, GameProfile};

use crate::launch::LaunchMethod;

use super::events::{ExternalChange, ProfileChangeKind};
use super::state::{AutosaveScope, RevisionHash};

impl From<&ProfileChangeReason> for ProfileChangeKind {
    fn from(reason: &ProfileChangeReason) -> Self {
        match reason {
            ProfileChangeReason::Saved => Self::Saved,
            ProfileChangeReason::Deleted => Self::Deleted,
            ProfileChangeReason::Renamed { from } => Self::Renamed { from: from.clone() },
            ProfileChangeReason::Duplicated => Self::Duplicated,
            ProfileChangeReason::FavoriteToggled => Self::FavoriteToggled,
            ProfileChangeReason::RolledBack => Self::RolledBack,
            ProfileChangeReason::PresetApplied => Self::PresetApplied,
            ProfileChangeReason::Imported => Self::Imported,
            ProfileChangeReason::Migrated => Self::Migrated,
        }
    }
}

/// Converts a `crosshook-app` bus event into [`ExternalChange`] inputs for
/// [`super::ProfileAutosave`].
///
/// `AppEvent::Resync` maps to one broadcast resync (`profile_name: None`);
/// `AppEvent::ProfilesChanged { names, .. }` emits a broadcast change when
/// `names` is empty (conservative resync) or one per affected profile name.
/// Other `AppEvent` variants return an empty `Vec`.
#[must_use]
pub fn external_changes_from_app_event(
    event: &AppEvent,
    revision: Option<RevisionHash>,
) -> Vec<ExternalChange> {
    match event {
        AppEvent::Resync { .. } => vec![ExternalChange {
            profile_name: None,
            reason: ProfileChangeKind::Resync,
            revision: None,
        }],
        AppEvent::ProfilesChanged { reason, names } => {
            let reason = ProfileChangeKind::from(reason);
            if names.is_empty() {
                vec![ExternalChange {
                    profile_name: None,
                    reason,
                    revision,
                }]
            } else {
                names
                    .iter()
                    .map(|name| ExternalChange {
                        profile_name: Some(name.clone()),
                        reason: reason.clone(),
                        revision: revision.clone(),
                    })
                    .collect()
            }
        }
        _ => Vec::new(),
    }
}

impl AutosaveScope {
    /// Builds an [`AutosaveScope`] from a `crosshook-core` [`GameProfile`]
    /// using [`crosshook_core::profile::resolve_launch_method`].
    #[must_use]
    pub fn from_profile(
        selected_profile: &str,
        draft_name: &str,
        profile_exists: bool,
        profile: &GameProfile,
    ) -> Self {
        let launch_method = match resolve_launch_method(profile) {
            METHOD_STEAM_APPLAUNCH => LaunchMethod::SteamApplaunch,
            METHOD_PROTON_RUN => LaunchMethod::ProtonRun,
            METHOD_NATIVE => LaunchMethod::Native,
            _ => LaunchMethod::Native,
        };
        Self {
            selected_profile: selected_profile.to_owned(),
            draft_name: draft_name.to_owned(),
            profile_exists,
            launch_method,
        }
    }
}

#[cfg(test)]
mod tests;
