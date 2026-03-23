use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use super::index::{self, CommunityProfileIndex, CommunityProfileIndexError};

const DEFAULT_COMMUNITY_TAPS_DIR: &str = "crosshook/community/taps";
const DEFAULT_TAP_BRANCH: &str = "main";

/// Abort HTTP transfers slower than 1 KB/s for 30 seconds.
const GIT_HTTP_LOW_SPEED_LIMIT: &str = "1000";
const GIT_HTTP_LOW_SPEED_TIME: &str = "30";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CommunityTapSubscription {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityTapWorkspace {
    pub subscription: CommunityTapSubscription,
    pub local_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunityTapSyncStatus {
    Cloned,
    Updated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityTapSyncResult {
    pub workspace: CommunityTapWorkspace,
    pub status: CommunityTapSyncStatus,
    pub head_commit: String,
    pub index: CommunityProfileIndex,
}

#[derive(Debug)]
pub enum CommunityTapError {
    EmptyTapUrl,
    InvalidTapUrl(String),
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    Git {
        action: &'static str,
        command: String,
        stderr: String,
    },
    Index(CommunityProfileIndexError),
}

impl fmt::Display for CommunityTapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTapUrl => write!(f, "tap url cannot be empty"),
            Self::InvalidTapUrl(url) => write!(f, "invalid tap url: {url}"),
            Self::Io {
                action,
                path,
                source,
            } => write!(f, "failed to {action} '{}': {source}", path.display()),
            Self::Git {
                action,
                command,
                stderr,
            } => write!(f, "failed to {action} with `{command}`: {stderr}"),
            Self::Index(error) => write!(f, "{error}"),
        }
    }
}

impl Error for CommunityTapError {}

impl From<CommunityProfileIndexError> for CommunityTapError {
    fn from(value: CommunityProfileIndexError) -> Self {
        Self::Index(value)
    }
}

pub struct CommunityTapStore {
    base_path: PathBuf,
}

impl Default for CommunityTapStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CommunityTapStore {
    pub fn new() -> Self {
        let base_path = BaseDirs::new()
            .expect("home directory is required for CrossHook community taps")
            .data_local_dir()
            .join(DEFAULT_COMMUNITY_TAPS_DIR);

        Self { base_path }
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

    pub fn index_workspaces(
        &self,
        workspaces: &[CommunityTapWorkspace],
    ) -> Result<CommunityProfileIndex, CommunityTapError> {
        Ok(index::index_taps(workspaces)?)
    }

    fn sync_workspace(
        &self,
        workspace: &CommunityTapWorkspace,
    ) -> Result<CommunityTapSyncResult, CommunityTapError> {
        fs::create_dir_all(&self.base_path).map_err(|source| CommunityTapError::Io {
            action: "create community taps directory",
            path: self.base_path.clone(),
            source,
        })?;

        let status = if workspace.local_path.exists() {
            self.fetch_and_reset(workspace)?;
            CommunityTapSyncStatus::Updated
        } else {
            self.clone_tap(workspace)?;
            CommunityTapSyncStatus::Cloned
        };

        let head_commit = self.rev_parse_head(&workspace.local_path)?;
        let index = index::index_tap(workspace)?;

        Ok(CommunityTapSyncResult {
            workspace: workspace.clone(),
            status,
            head_commit,
            index,
        })
    }

    fn clone_tap(&self, workspace: &CommunityTapWorkspace) -> Result<(), CommunityTapError> {
        let mut command = git_command();
        command
            .arg("clone")
            .arg("--branch")
            .arg(workspace.branch())
            .arg("--single-branch")
            .arg(&workspace.subscription.url)
            .arg(&workspace.local_path);

        let output = command.output().map_err(|source| CommunityTapError::Io {
            action: "clone community tap",
            path: workspace.local_path.clone(),
            source,
        })?;

        if !output.status.success() {
            return Err(CommunityTapError::Git {
                action: "clone community tap",
                command: format!(
                    "git clone --branch {} --single-branch {} {}",
                    workspace.branch(),
                    workspace.subscription.url,
                    workspace.local_path.display()
                ),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        Ok(())
    }

    fn fetch_and_reset(&self, workspace: &CommunityTapWorkspace) -> Result<(), CommunityTapError> {
        self.run_git(
            workspace,
            "fetch community tap",
            ["fetch", "--prune", "origin", workspace.branch()],
        )?;
        self.run_git(
            workspace,
            "reset community tap",
            ["reset", "--hard", "FETCH_HEAD"],
        )?;
        self.run_git(workspace, "clean community tap", ["clean", "-fdx"])?;
        Ok(())
    }

    fn rev_parse_head(&self, path: &Path) -> Result<String, CommunityTapError> {
        let output = git_command()
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "HEAD"])
            .output()
            .map_err(|source| CommunityTapError::Io {
                action: "resolve tap commit",
                path: path.to_path_buf(),
                source,
            })?;

        if !output.status.success() {
            return Err(CommunityTapError::Git {
                action: "resolve tap commit",
                command: format!("git -C {} rev-parse HEAD", path.display()),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn run_git<const N: usize>(
        &self,
        workspace: &CommunityTapWorkspace,
        action: &'static str,
        args: [&str; N],
    ) -> Result<(), CommunityTapError> {
        let output = git_command()
            .arg("-C")
            .arg(&workspace.local_path)
            .args(args)
            .output()
            .map_err(|source| CommunityTapError::Io {
                action,
                path: workspace.local_path.clone(),
                source,
            })?;

        if !output.status.success() {
            return Err(CommunityTapError::Git {
                action,
                command: format!(
                    "git -C {} {}",
                    workspace.local_path.display(),
                    args.join(" ")
                ),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        Ok(())
    }

    fn workspace_path(&self, subscription: &CommunityTapSubscription) -> PathBuf {
        self.base_path.join(subscription.directory_name())
    }
}

impl CommunityTapWorkspace {
    fn branch(&self) -> &str {
        self.subscription
            .branch
            .as_deref()
            .unwrap_or(DEFAULT_TAP_BRANCH)
    }
}

fn normalize_subscription(
    subscription: &CommunityTapSubscription,
) -> Result<CommunityTapSubscription, CommunityTapError> {
    let url = subscription.url.trim();
    if url.is_empty() {
        return Err(CommunityTapError::EmptyTapUrl);
    }

    if url.chars().any(char::is_whitespace) {
        return Err(CommunityTapError::InvalidTapUrl(subscription.url.clone()));
    }

    Ok(CommunityTapSubscription {
        url: url.to_string(),
        branch: subscription
            .branch
            .as_ref()
            .map(|branch| branch.trim().to_string())
            .filter(|branch| !branch.is_empty()),
    })
}

impl CommunityTapSubscription {
    fn directory_name(&self) -> String {
        let mut slug = slugify(&self.url);
        if let Some(branch) = &self.branch {
            let branch_slug = slugify(branch);
            if !branch_slug.is_empty() {
                slug.push('-');
                slug.push_str(&branch_slug);
            }
        }

        if slug.is_empty() {
            "tap".to_string()
        } else {
            slug
        }
    }
}

fn git_command() -> Command {
    let mut command = Command::new("git");
    command
        .env("GIT_HTTP_LOW_SPEED_LIMIT", GIT_HTTP_LOW_SPEED_LIMIT)
        .env("GIT_HTTP_LOW_SPEED_TIME", GIT_HTTP_LOW_SPEED_TIME);
    command
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::community::{
        CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating,
    };
    use crate::profile::GameProfile;
    use tempfile::tempdir;

    fn init_repo(path: &Path) {
        let output = Command::new("git")
            .args(["init", "-b", "main"])
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["config", "user.email", "crosshook@example.invalid"])
            .output()
            .unwrap();
        assert!(output.status.success());

        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["config", "user.name", "CrossHook"])
            .output()
            .unwrap();
        assert!(output.status.success());
    }

    fn commit_file(path: &Path, relative: &str, content: &str, message: &str) {
        let full_path = path.join(relative);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full_path, content).unwrap();

        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["add", relative])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["commit", "-m", message])
            .env("GIT_AUTHOR_NAME", "CrossHook")
            .env("GIT_AUTHOR_EMAIL", "crosshook@example.invalid")
            .env("GIT_COMMITTER_NAME", "CrossHook")
            .env("GIT_COMMITTER_EMAIL", "crosshook@example.invalid")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn syncs_and_indexes_local_tap_repo() {
        let temp_dir = tempdir().unwrap();
        let source_repo = temp_dir.path().join("source");
        let store_root = temp_dir.path().join("store");
        fs::create_dir_all(&source_repo).unwrap();
        init_repo(&source_repo);

        let manifest = CommunityProfileManifest::new(
            CommunityProfileMetadata {
                game_name: "Elden Ring".to_string(),
                game_version: "1.0".to_string(),
                trainer_name: "Trainer".to_string(),
                trainer_version: "1".to_string(),
                proton_version: "9".to_string(),
                platform_tags: vec!["linux".to_string()],
                compatibility_rating: CompatibilityRating::Working,
                author: "CrossHook".to_string(),
                description: "Test profile".to_string(),
            },
            GameProfile::default(),
        );
        commit_file(
            &source_repo,
            "profiles/elden-ring/community-profile.json",
            &serde_json::to_string_pretty(&manifest).unwrap(),
            "add community profile",
        );

        let store = CommunityTapStore::with_base_path(store_root);
        let subscription = CommunityTapSubscription {
            url: source_repo.display().to_string(),
            branch: Some("main".to_string()),
        };

        let result = store.sync_tap(&subscription).unwrap();
        assert_eq!(result.status, CommunityTapSyncStatus::Cloned);
        assert_eq!(result.index.entries.len(), 1);
        assert_eq!(
            result.index.entries[0].manifest.metadata.game_name,
            "Elden Ring"
        );

        let second = store.sync_tap(&subscription).unwrap();
        assert_eq!(second.status, CommunityTapSyncStatus::Updated);
        assert_eq!(second.index.entries.len(), 1);
    }

    #[test]
    fn rejects_blank_tap_urls() {
        let store = CommunityTapStore::with_base_path(PathBuf::from("/tmp/crosshook-taps-test"));
        let error = store
            .sync_tap(&CommunityTapSubscription {
                url: "   ".to_string(),
                branch: None,
            })
            .unwrap_err();

        assert!(matches!(error, CommunityTapError::EmptyTapUrl));
    }
}
