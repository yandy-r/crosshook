use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::git::{
    clone_invocations, git_failure, rev_parse_invocation, run_failure, update_invocations,
};
use super::index;
use super::runner::{GitInvocation, GitOutput, TapGitRunner};
use super::types::{
    CommunityTapError, CommunityTapSubscription, CommunityTapSyncResult, CommunityTapSyncStatus,
    CommunityTapWorkspace,
};
use super::validation::normalize_subscription;

const DEFAULT_COMMUNITY_TAPS_DIR: &str = "crosshook/community/taps";

#[derive(Debug, Clone)]
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
        let base_path = crate::platform::base_dirs()
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

    /// `normalize_subscription`; extras are cleared. The pinned commit is **not**
    /// validated here: a stored tap with a bad pin must not break listing, import
    /// or the catalog. New taps are pin-validated by the caller, and checkout
    /// re-validates in `git.rs`.
    pub fn normalize_and_validate(
        subscription: &CommunityTapSubscription,
    ) -> Result<CommunityTapSubscription, CommunityTapError> {
        normalize_subscription(subscription)
    }

    /// Syncs taps sequentially through `runner`; the first hard error aborts.
    pub async fn sync_many_with_runner(
        &self,
        subscriptions: &[CommunityTapSubscription],
        runner: Arc<dyn TapGitRunner>,
    ) -> Result<Vec<CommunityTapSyncResult>, CommunityTapError> {
        let mut results = Vec::with_capacity(subscriptions.len());

        for subscription in subscriptions {
            results.push(
                self.sync_tap_with_runner(subscription, Arc::clone(&runner))
                    .await?,
            );
        }

        Ok(results)
    }

    pub async fn sync_tap_with_runner(
        &self,
        subscription: &CommunityTapSubscription,
        runner: Arc<dyn TapGitRunner>,
    ) -> Result<CommunityTapSyncResult, CommunityTapError> {
        let workspace = self.resolve_workspace(subscription)?;
        self.sync_workspace_with_runner(&workspace, runner).await
    }

    pub(crate) async fn sync_workspace_with_runner(
        &self,
        workspace: &CommunityTapWorkspace,
        runner: Arc<dyn TapGitRunner>,
    ) -> Result<CommunityTapSyncResult, CommunityTapError> {
        ensure_not_cancelled(runner.as_ref())?;
        tokio::fs::create_dir_all(&self.base_path)
            .await
            .map_err(|source| CommunityTapError::Io {
                action: "create community taps directory",
                path: self.base_path.clone(),
                source,
            })?;

        let local_path = &workspace.local_path;
        let exists = path_exists(local_path).await;
        let mut from_cache = false;
        let status = if exists {
            let steps = update_invocations(workspace)?;
            match run_steps(runner.as_ref(), &steps, local_path).await {
                Ok(()) => CommunityTapSyncStatus::Updated,
                Err(CommunityTapError::Cancelled) => return Err(CommunityTapError::Cancelled),
                Err(err) => {
                    if path_exists(local_path).await {
                        from_cache = true;
                        CommunityTapSyncStatus::CachedFallback
                    } else {
                        return Err(err);
                    }
                }
            }
        } else {
            let steps = clone_invocations(workspace)?;
            match run_steps(runner.as_ref(), &steps, local_path).await {
                Ok(()) => CommunityTapSyncStatus::Cloned,
                Err(CommunityTapError::Cancelled) => {
                    // A SIGKILLed partial clone must not wedge later syncs: the
                    // workspace did not exist before this clone, so remove it
                    // (best effort) instead of leaving a broken cache behind.
                    if let Err(source) = tokio::fs::remove_dir_all(local_path).await {
                        if source.kind() != std::io::ErrorKind::NotFound {
                            tracing::warn!(
                                path = %local_path.display(),
                                error = %source,
                                "failed to remove partial community tap clone after cancellation"
                            );
                        }
                    }
                    return Err(CommunityTapError::Cancelled);
                }
                Err(err) => return Err(err),
            }
        };

        let rev_parse = rev_parse_invocation(local_path);
        let output = run_step(runner.as_ref(), &rev_parse, local_path).await?;
        let head_commit = output.stdout.trim().to_string();

        ensure_not_cancelled(runner.as_ref())?;
        let index_workspace = workspace.clone();
        let index_runner = Arc::clone(&runner);
        let index = tokio::task::spawn_blocking(move || {
            ensure_not_cancelled(index_runner.as_ref())?;
            index::index_tap(&index_workspace).map_err(CommunityTapError::from)
        })
        .await
        .map_err(|_| CommunityTapError::Io {
            action: "index community tap",
            path: local_path.clone(),
            source: std::io::Error::other("index task failed"),
        })??;

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

fn ensure_not_cancelled(runner: &dyn TapGitRunner) -> Result<(), CommunityTapError> {
    if runner.is_cancelled() {
        Err(CommunityTapError::Cancelled)
    } else {
        Ok(())
    }
}

async fn path_exists(path: &Path) -> bool {
    tokio::fs::try_exists(path).await.unwrap_or(false)
}

/// Runs one step; non-success output becomes `CommunityTapError::Git`.
async fn run_step(
    runner: &dyn TapGitRunner,
    invocation: &GitInvocation,
    fallback_path: &Path,
) -> Result<GitOutput, CommunityTapError> {
    ensure_not_cancelled(runner)?;
    let output = runner
        .run(invocation.clone())
        .await
        .map_err(|error| run_failure(invocation, error, fallback_path))?;
    if output.success {
        return Ok(output);
    }
    // A killed child reports failure; cancellation must never look like a git error.
    ensure_not_cancelled(runner)?;
    Err(git_failure(invocation, &output))
}

async fn run_steps(
    runner: &dyn TapGitRunner,
    steps: &[GitInvocation],
    fallback_path: &Path,
) -> Result<(), CommunityTapError> {
    for step in steps {
        run_step(runner, step, fallback_path).await?;
    }
    Ok(())
}
