use crosshook_core::community::{
    is_valid_git_sha, CommunityProfileIndex, CommunityTapError, CommunityTapStore,
    CommunityTapSubscription, CommunityTapWorkspace,
};
use crosshook_core::metadata::CommunityProfileRow;

use super::CommunityService;
use crate::services::run_blocking;
use crate::AppError;

impl CommunityService {
    /// Normalizes and validates a subscription before atomically adding its identity.
    pub async fn add_tap(
        &self,
        tap: CommunityTapSubscription,
    ) -> Result<Vec<CommunityTapSubscription>, AppError> {
        let settings = self.settings.clone();
        run_blocking(move || {
            let tap = validate_incoming(&tap)?;
            settings.update(move |current| {
                let mut taps = normalize_taps(&current.community_taps);
                if !taps.iter().any(|existing| same_identity(existing, &tap)) {
                    taps.push(tap);
                }
                current.community_taps = taps;
                Ok::<_, AppError>(cleared_taps(&current.community_taps))
            })?
        })
        .await
    }

    /// Removes a normalized identity without deleting its clone or indexed metadata.
    pub async fn remove_tap(
        &self,
        tap: CommunityTapSubscription,
    ) -> Result<Vec<CommunityTapSubscription>, AppError> {
        let settings = self.settings.clone();
        run_blocking(move || {
            // An invalid identity can still name an invalid stored entry, which
            // must stay removable; it is matched by its raw triple.
            let validated = validate_incoming(&tap);
            settings.update(move |current| {
                let mut taps = normalize_taps(&current.community_taps);
                let target = match validated {
                    Ok(valid) => valid,
                    Err(_) if taps.iter().any(|existing| same_identity(existing, &tap)) => tap,
                    Err(error) => return Err(error),
                };
                taps.retain(|existing| !same_identity(existing, &target));
                current.community_taps = taps;
                Ok::<_, AppError>(cleared_taps(&current.community_taps))
            })?
        })
        .await
    }

    /// Reads profiles from currently subscribed local workspaces, without a network gate.
    pub async fn list_profiles(&self) -> Result<CommunityProfileIndex, AppError> {
        let settings = self.settings.clone();
        let taps = self.taps.clone();
        run_blocking(move || {
            let workspaces = current_workspaces(&taps, &settings.load()?.community_taps)?;
            Ok(taps.index_workspaces(&workspaces)?)
        })
        .await
    }

    /// Reads the persisted community profile index even when offline.
    pub async fn list_indexed_profiles(&self) -> Result<Vec<CommunityProfileRow>, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || Ok(metadata.list_community_tap_profiles(None)?)).await
    }
}

pub(super) fn current_workspaces(
    taps: &CommunityTapStore,
    subscriptions: &[CommunityTapSubscription],
) -> Result<Vec<CommunityTapWorkspace>, AppError> {
    subscriptions
        .iter()
        .map(|tap| taps.resolve_workspace(tap).map_err(AppError::from))
        .collect()
}

/// Strictly validates the incoming tap, including its pin, which the store's
/// normalization leaves to checkout time.
fn validate_incoming(tap: &CommunityTapSubscription) -> Result<CommunityTapSubscription, AppError> {
    let tap = CommunityTapStore::normalize_and_validate(tap)?;
    if let Some(commit) = &tap.pinned_commit {
        if !is_valid_git_sha(commit) {
            return Err(CommunityTapError::InvalidPinnedCommit(commit.clone()).into());
        }
    }
    Ok(tap)
}

/// Normalizes and dedupes stored taps. A stored tap that no longer validates is
/// kept unchanged (compared by its raw triple) so it can never block add or
/// remove of other taps.
fn normalize_taps(taps: &[CommunityTapSubscription]) -> Vec<CommunityTapSubscription> {
    let mut unique: Vec<CommunityTapSubscription> = Vec::with_capacity(taps.len());
    for tap in taps {
        let normalized = match CommunityTapStore::normalize_and_validate(tap) {
            Ok(mut normalized) => {
                normalized.extra.clone_from(&tap.extra);
                normalized
            }
            Err(_) => tap.clone(),
        };
        if !unique
            .iter()
            .any(|existing| same_identity(existing, &normalized))
        {
            unique.push(normalized);
        }
    }
    unique
}

fn same_identity(left: &CommunityTapSubscription, right: &CommunityTapSubscription) -> bool {
    left.url == right.url
        && left.branch == right.branch
        && left.pinned_commit == right.pinned_commit
}

fn cleared_taps(taps: &[CommunityTapSubscription]) -> Vec<CommunityTapSubscription> {
    let mut result = taps.to_vec();
    for tap in &mut result {
        tap.extra.clear();
    }
    result
}
