use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::platform::{self, host_std_command_with_env};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_commit: Option<String>,
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
    CachedFallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityTapSyncResult {
    pub workspace: CommunityTapWorkspace,
    pub status: CommunityTapSyncStatus,
    pub head_commit: String,
    pub index: CommunityProfileIndex,
    /// True when git fetch failed but an existing local clone was used to build the index.
    #[serde(default)]
    pub from_cache: bool,
    /// Last successful network sync (`community_tap_offline_state.last_sync_at`), if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<String>,
}

#[derive(Debug)]
pub enum CommunityTapError {
    EmptyTapUrl,
    InvalidTapUrl(String),
    InvalidBranch(String),
    InvalidPinnedCommit(String),
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
            Self::InvalidBranch(branch) => write!(
                f,
                "invalid branch name (allowed: a-z A-Z 0-9 / . _ -, max 200 chars, must not start with '-'): {branch}"
            ),
            Self::InvalidPinnedCommit(commit) => write!(
                f,
                "invalid pinned commit (must be 7-64 hex characters): {commit}"
            ),
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

        let mut from_cache = false;
        let status = if workspace.local_path.exists() {
            let fetch_result = if workspace.subscription.pinned_commit.is_some() {
                self.fetch_and_checkout_pinned(workspace)
            } else {
                self.fetch_and_reset(workspace)
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
            self.clone_tap(workspace)?;
            if workspace.subscription.pinned_commit.is_some() {
                self.checkout_pinned_commit(workspace)?;
            }
            CommunityTapSyncStatus::Cloned
        };

        let head_commit = self.rev_parse_head(&workspace.local_path)?;
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
            &["fetch", "--prune", "origin", "--", workspace.branch()],
        )?;
        self.run_git(
            workspace,
            "reset community tap",
            &["reset", "--hard", "FETCH_HEAD"],
        )?;
        self.run_git(workspace, "clean community tap", &["clean", "-fdx"])?;
        Ok(())
    }

    fn fetch_and_checkout_pinned(
        &self,
        workspace: &CommunityTapWorkspace,
    ) -> Result<(), CommunityTapError> {
        self.run_git(
            workspace,
            "fetch community tap",
            &["fetch", "--prune", "origin", "--", workspace.branch()],
        )?;
        self.checkout_pinned_commit(workspace)?;
        self.run_git(workspace, "clean community tap", &["clean", "-fdx"])?;
        Ok(())
    }

    fn checkout_pinned_commit(
        &self,
        workspace: &CommunityTapWorkspace,
    ) -> Result<(), CommunityTapError> {
        let pinned_commit = workspace
            .subscription
            .pinned_commit
            .as_deref()
            .ok_or_else(|| CommunityTapError::Git {
                action: "checkout pinned commit",
                command: "git checkout --detach <commit>".to_string(),
                stderr: "missing pinned commit".to_string(),
            })?;

        if !is_valid_git_sha(pinned_commit) {
            return Err(CommunityTapError::InvalidPinnedCommit(
                pinned_commit.to_string(),
            ));
        }

        self.run_git(
            workspace,
            "checkout pinned commit",
            &["checkout", "--detach", pinned_commit],
        )?;
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

    fn run_git(
        &self,
        workspace: &CommunityTapWorkspace,
        action: &'static str,
        args: &[&str],
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

    validate_tap_url(url)?;

    let branch = subscription
        .branch
        .as_ref()
        .map(|b| b.trim().to_string())
        .filter(|b| !b.is_empty());

    if let Some(ref branch_name) = branch {
        validate_branch_name(branch_name)?;
    }

    Ok(CommunityTapSubscription {
        url: url.to_string(),
        branch,
        pinned_commit: subscription
            .pinned_commit
            .as_ref()
            .map(|commit| commit.trim().to_string())
            .filter(|commit| !commit.is_empty()),
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

/// Validates that a pinned commit string is a safe git SHA (hex-only, 7–64 characters).
///
/// This prevents flag-injection (e.g. `--force`, `-q`) and shell-injection strings
/// from being passed to `git checkout` as a positional argument.
fn is_valid_git_sha(commit: &str) -> bool {
    (7..=64).contains(&commit.len()) && commit.chars().all(|c| c.is_ascii_hexdigit())
}

/// Validates that a branch name is safe to pass as a git positional argument.
///
/// Rejects names starting with `-` (would be interpreted as git flags) and names
/// containing characters outside `[a-zA-Z0-9/._-]` (max 200 chars).
fn validate_branch_name(branch: &str) -> Result<(), CommunityTapError> {
    if branch.starts_with('-') {
        return Err(CommunityTapError::InvalidBranch(branch.to_string()));
    }
    if branch.len() > 200 {
        return Err(CommunityTapError::InvalidBranch(branch.to_string()));
    }
    if !branch
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'))
    {
        return Err(CommunityTapError::InvalidBranch(branch.to_string()));
    }
    Ok(())
}

/// Validates that a community tap URL uses an allowed scheme.
///
/// Accepted forms:
/// - `https://...`
/// - `ssh://git@...`
/// - SCP-style `git@host:path` (the default SSH clone URL on GitHub/GitLab)
/// - Bare absolute paths (`/home/…`) for local development taps
///
/// Rejects `file://`, `git://`, relative paths, and any other scheme not explicitly permitted.
fn validate_tap_url(url: &str) -> Result<(), CommunityTapError> {
    if url.starts_with("https://")
        || url.starts_with("ssh://git@")
        || url.starts_with("git@")
        || url.starts_with('/')
    {
        Ok(())
    } else {
        Err(CommunityTapError::InvalidTapUrl(url.to_string()))
    }
}

fn git_security_env_pairs() -> [(&'static str, &'static str); 5] {
    [
        ("GIT_HTTP_LOW_SPEED_LIMIT", GIT_HTTP_LOW_SPEED_LIMIT),
        ("GIT_HTTP_LOW_SPEED_TIME", GIT_HTTP_LOW_SPEED_TIME),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_TERMINAL_PROMPT", "0"),
    ]
}

fn git_command() -> Command {
    let mut env = BTreeMap::new();
    for (key, value) in git_security_env_pairs() {
        env.insert(key.to_string(), value.to_string());
    }
    if platform::is_flatpak() {
        host_std_command_with_env("git", &env)
    } else {
        let mut command = Command::new("git");
        command.envs(&env);
        command
    }
}

/// Best-effort total size of files under `path` (recursive).
pub fn directory_size_bytes(path: &Path) -> u64 {
    fn walk(dir: &Path) -> std::io::Result<u64> {
        let mut sum = 0u64;
        let Ok(read_dir) = fs::read_dir(dir) else {
            return Ok(0);
        };
        for entry in read_dir.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                sum += walk(&entry.path()).unwrap_or(0);
            } else {
                sum += meta.len();
            }
        }
        Ok(sum)
    }
    walk(path).unwrap_or(0)
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

    fn rev_parse_head(path: &Path) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
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
                trainer_sha256: None,
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
            pinned_commit: None,
        };
        // Use sync_workspace directly to bypass URL scheme validation (test uses local path).
        let workspace = CommunityTapWorkspace {
            local_path: store.workspace_path(&subscription),
            subscription,
        };

        let result = store.sync_workspace(&workspace).unwrap();
        assert_eq!(result.status, CommunityTapSyncStatus::Cloned);
        assert_eq!(result.index.entries.len(), 1);
        assert_eq!(
            result.index.entries[0].manifest.metadata.game_name,
            "Elden Ring"
        );

        let second = store.sync_workspace(&workspace).unwrap();
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
                pinned_commit: None,
            })
            .unwrap_err();

        assert!(matches!(error, CommunityTapError::EmptyTapUrl));
    }

    #[test]
    fn is_valid_git_sha_rejects_injection_attempts() {
        assert!(!is_valid_git_sha("'; rm -rf /"));
        assert!(!is_valid_git_sha("--force"));
        assert!(!is_valid_git_sha("-q"));
        assert!(!is_valid_git_sha("$(reboot)"));
    }

    #[test]
    fn is_valid_git_sha_rejects_invalid_lengths() {
        assert!(!is_valid_git_sha("")); // empty
        assert!(!is_valid_git_sha("abc123")); // 6 chars — one short of minimum
        assert!(!is_valid_git_sha(&"a".repeat(65))); // 65 chars — one over maximum
    }

    #[test]
    fn git_security_env_pairs_include_config_isolation() {
        let keys: Vec<_> = git_security_env_pairs().iter().map(|(k, _)| *k).collect();
        assert!(keys.contains(&"GIT_CONFIG_NOSYSTEM"));
        assert!(keys.contains(&"GIT_CONFIG_GLOBAL"));
        assert!(keys.contains(&"GIT_TERMINAL_PROMPT"));
    }

    #[test]
    fn is_tap_available_offline_false_when_workspace_missing() {
        let temp_dir = tempdir().unwrap();
        let store = CommunityTapStore::with_base_path(temp_dir.path().to_path_buf());
        let subscription = CommunityTapSubscription {
            url: "https://example.invalid/tap.git".to_string(),
            branch: None,
            pinned_commit: None,
        };
        assert!(!store.is_tap_available_offline(&subscription));
    }

    #[test]
    fn is_valid_git_sha_accepts_valid_hashes() {
        assert!(is_valid_git_sha("abc1234")); // 7-char short hash
        assert!(is_valid_git_sha("deadbeef01234567890abcdef0123456789abcde")); // 40-char SHA1
        assert!(is_valid_git_sha(&"a".repeat(64))); // 64-char SHA256
    }

    #[test]
    fn rejects_injection_attempt_as_pinned_commit() {
        let temp_dir = tempdir().unwrap();
        let source_repo = temp_dir.path().join("source");
        let store_root = temp_dir.path().join("store");
        fs::create_dir_all(&source_repo).unwrap();
        init_repo(&source_repo);

        let manifest = CommunityProfileManifest::new(
            CommunityProfileMetadata {
                game_name: "Test".to_string(),
                game_version: "1.0".to_string(),
                trainer_name: "Trainer".to_string(),
                trainer_version: "1".to_string(),
                proton_version: "9".to_string(),
                platform_tags: vec![],
                compatibility_rating: CompatibilityRating::Unknown,
                author: String::new(),
                description: String::new(),
                trainer_sha256: None,
            },
            GameProfile::default(),
        );
        commit_file(
            &source_repo,
            "profiles/test/community-profile.json",
            &serde_json::to_string_pretty(&manifest).unwrap(),
            "add test profile",
        );

        let store = CommunityTapStore::with_base_path(store_root);
        let subscription = CommunityTapSubscription {
            url: source_repo.display().to_string(),
            branch: Some("main".to_string()),
            pinned_commit: Some("'; rm -rf /".to_string()),
        };
        // Use sync_workspace directly to bypass URL scheme validation (test uses local path).
        let workspace = CommunityTapWorkspace {
            local_path: store.workspace_path(&subscription),
            subscription,
        };

        let err = store.sync_workspace(&workspace).unwrap_err();
        assert!(
            matches!(err, CommunityTapError::InvalidPinnedCommit(_)),
            "expected InvalidPinnedCommit, got: {err}"
        );
    }

    #[test]
    fn pinned_tap_stays_on_selected_commit() {
        let temp_dir = tempdir().unwrap();
        let source_repo = temp_dir.path().join("source");
        let store_root = temp_dir.path().join("store");
        fs::create_dir_all(&source_repo).unwrap();
        init_repo(&source_repo);

        let manifest_v1 = CommunityProfileManifest::new(
            CommunityProfileMetadata {
                game_name: "Elden Ring".to_string(),
                game_version: "1.0".to_string(),
                trainer_name: "Trainer".to_string(),
                trainer_version: "1".to_string(),
                proton_version: "9".to_string(),
                platform_tags: vec!["linux".to_string()],
                compatibility_rating: CompatibilityRating::Working,
                author: "CrossHook".to_string(),
                description: "Pinned v1".to_string(),
                trainer_sha256: None,
            },
            GameProfile::default(),
        );
        commit_file(
            &source_repo,
            "profiles/elden-ring/community-profile.json",
            &serde_json::to_string_pretty(&manifest_v1).unwrap(),
            "add v1 profile",
        );
        let pinned_commit = rev_parse_head(&source_repo);

        let store = CommunityTapStore::with_base_path(store_root);
        let subscription = CommunityTapSubscription {
            url: source_repo.display().to_string(),
            branch: Some("main".to_string()),
            pinned_commit: Some(pinned_commit.clone()),
        };
        // Use sync_workspace directly to bypass URL scheme validation (test uses local path).
        let workspace = CommunityTapWorkspace {
            local_path: store.workspace_path(&subscription),
            subscription,
        };

        let first_sync = store.sync_workspace(&workspace).unwrap();
        assert_eq!(first_sync.head_commit, pinned_commit);

        let manifest_v2 = CommunityProfileManifest::new(
            CommunityProfileMetadata {
                game_name: "Elden Ring".to_string(),
                game_version: "1.1".to_string(),
                trainer_name: "Trainer".to_string(),
                trainer_version: "2".to_string(),
                proton_version: "9".to_string(),
                platform_tags: vec!["linux".to_string()],
                compatibility_rating: CompatibilityRating::Working,
                author: "CrossHook".to_string(),
                description: "Pinned v2".to_string(),
                trainer_sha256: None,
            },
            GameProfile::default(),
        );
        commit_file(
            &source_repo,
            "profiles/elden-ring/community-profile.json",
            &serde_json::to_string_pretty(&manifest_v2).unwrap(),
            "update profile",
        );

        let second_sync = store.sync_workspace(&workspace).unwrap();
        assert_eq!(second_sync.status, CommunityTapSyncStatus::Updated);
        assert_eq!(second_sync.head_commit, pinned_commit);
        assert_eq!(second_sync.index.entries.len(), 1);
        assert_eq!(
            second_sync.index.entries[0]
                .manifest
                .metadata
                .trainer_version,
            "1"
        );
    }

    #[test]
    fn validate_branch_name_accepts_valid_names() {
        assert!(validate_branch_name("main").is_ok());
        assert!(validate_branch_name("feature/my-branch").is_ok());
        assert!(validate_branch_name("release_1.0").is_ok());
        assert!(validate_branch_name("v2.3-stable").is_ok());
        assert!(validate_branch_name("a/b/c.d_e-f").is_ok());
    }

    #[test]
    fn validate_branch_name_rejects_leading_dash() {
        assert!(matches!(
            validate_branch_name("--upload-pack=/evil"),
            Err(CommunityTapError::InvalidBranch(_))
        ));
        assert!(matches!(
            validate_branch_name("-q"),
            Err(CommunityTapError::InvalidBranch(_))
        ));
    }

    #[test]
    fn validate_branch_name_rejects_special_chars() {
        assert!(matches!(
            validate_branch_name("branch;rm -rf /"),
            Err(CommunityTapError::InvalidBranch(_))
        ));
        assert!(matches!(
            validate_branch_name("branch$(evil)"),
            Err(CommunityTapError::InvalidBranch(_))
        ));
        assert!(matches!(
            validate_branch_name("branch with spaces"),
            Err(CommunityTapError::InvalidBranch(_))
        ));
    }

    #[test]
    fn validate_tap_url_accepts_https() {
        assert!(validate_tap_url("https://github.com/user/repo").is_ok());
        assert!(validate_tap_url("https://gitlab.com/org/crosshook-taps").is_ok());
    }

    #[test]
    fn validate_tap_url_accepts_ssh_git() {
        assert!(validate_tap_url("ssh://git@github.com/user/repo").is_ok());
        assert!(validate_tap_url("ssh://git@gitlab.com/user/repo").is_ok());
    }

    #[test]
    fn validate_tap_url_rejects_file_scheme() {
        assert!(matches!(
            validate_tap_url("file:///home/user/.ssh/"),
            Err(CommunityTapError::InvalidTapUrl(_))
        ));
        assert!(matches!(
            validate_tap_url("file:///etc/passwd"),
            Err(CommunityTapError::InvalidTapUrl(_))
        ));
    }

    #[test]
    fn validate_tap_url_rejects_git_scheme() {
        assert!(matches!(
            validate_tap_url("git://github.com/user/repo"),
            Err(CommunityTapError::InvalidTapUrl(_))
        ));
    }

    #[test]
    fn validate_tap_url_accepts_absolute_paths() {
        assert!(validate_tap_url("/tmp/local-repo").is_ok());
        assert!(validate_tap_url("/home/user/crosshook-test-tap").is_ok());
    }

    #[test]
    fn validate_tap_url_rejects_relative_paths() {
        assert!(matches!(
            validate_tap_url("../relative-repo"),
            Err(CommunityTapError::InvalidTapUrl(_))
        ));
    }
}
