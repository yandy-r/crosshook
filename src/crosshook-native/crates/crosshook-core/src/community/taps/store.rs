use std::fs;
use std::path::PathBuf;

use directories::BaseDirs;

use super::git::{
    checkout_pinned_commit, clone_tap, fetch_and_checkout_pinned, fetch_and_reset, rev_parse_head,
};
use super::index;
use super::types::{
    CommunityTapError, CommunityTapSubscription, CommunityTapSyncResult, CommunityTapSyncStatus,
    CommunityTapWorkspace,
};
use super::validation::normalize_subscription;

const DEFAULT_COMMUNITY_TAPS_DIR: &str = "crosshook/community/taps";

pub struct CommunityTapStore {
    base_path: PathBuf,
}

impl Default for CommunityTapStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CommunityTapStore {
    pub fn try_new() -> Result<Self, String> {
        let base_path = BaseDirs::new()
            .ok_or("home directory not found — CrossHook requires a user home directory")?
            .data_local_dir()
            .join(DEFAULT_COMMUNITY_TAPS_DIR);
        Ok(Self { base_path })
    }

    pub fn new() -> Self {
        Self::try_new().expect("home directory is required for CrossHook community taps")
    }

    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    pub fn resolve_workspace(
        &self,
        subscription: &CommunityTapSubscription,
    ) -> Result<CommunityTapWorkspace, CommunityTapError> {
        let subscription = normalize_subscription(subscription)?;
        let local_path = self.workspace_path(&subscription);

        Ok(CommunityTapWorkspace {
            subscription,
            local_path,
        })
    }

    pub fn sync_tap(
        &self,
        subscription: &CommunityTapSubscription,
    ) -> Result<CommunityTapSyncResult, CommunityTapError> {
        let workspace = self.resolve_workspace(subscription)?;
        self.sync_workspace(&workspace)
    }

    pub fn sync_many(
        &self,
        subscriptions: &[CommunityTapSubscription],
    ) -> Result<Vec<CommunityTapSyncResult>, CommunityTapError> {
        let mut results = Vec::with_capacity(subscriptions.len());

        for subscription in subscriptions {
            results.push(self.sync_tap(subscription)?);
        }

        Ok(results)
    }

    /// Returns true when the tap's resolved workspace directory already exists on disk.
    pub fn is_tap_available_offline(&self, subscription: &CommunityTapSubscription) -> bool {
        match self.resolve_workspace(subscription) {
            Ok(workspace) => workspace.local_path.exists(),
            Err(_) => false,
        }
    }

    pub fn index_workspaces(
        &self,
        workspaces: &[CommunityTapWorkspace],
    ) -> Result<super::index::CommunityProfileIndex, CommunityTapError> {
        Ok(index::index_taps(workspaces)?)
    }

    pub(crate) fn sync_workspace(
        &self,
        workspace: &CommunityTapWorkspace,
    ) -> Result<CommunityTapSyncResult, CommunityTapError> {
        fs::create_dir_all(&self.base_path).map_err(|source| CommunityTapError::Io {
            action: "create community taps directory",
            path: self.base_path.clone(),
            source,
        })?;

        let mut from_cache = false;
        let status = if workspace.local_path.exists() {
            let fetch_result = if workspace.subscription.pinned_commit.is_some() {
                fetch_and_checkout_pinned(workspace)
            } else {
                fetch_and_reset(workspace)
            };
            match fetch_result {
                Ok(()) => CommunityTapSyncStatus::Updated,
                Err(err) => {
                    if workspace.local_path.exists() {
                        from_cache = true;
                        CommunityTapSyncStatus::CachedFallback
                    } else {
                        return Err(err);
                    }
                }
            }
        } else {
            clone_tap(workspace)?;
            if workspace.subscription.pinned_commit.is_some() {
                checkout_pinned_commit(workspace)?;
            }
            CommunityTapSyncStatus::Cloned
        };

        let head_commit = rev_parse_head(&workspace.local_path)?;
        let index = index::index_tap(workspace)?;

        Ok(CommunityTapSyncResult {
            workspace: workspace.clone(),
            status,
            head_commit,
            index,
            from_cache,
            last_sync_at: None,
        })
    }

    pub(crate) fn workspace_path(&self, subscription: &CommunityTapSubscription) -> PathBuf {
        self.base_path.join(subscription.directory_name())
    }
}
