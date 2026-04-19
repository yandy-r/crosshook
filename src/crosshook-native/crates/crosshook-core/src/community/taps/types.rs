use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::index::CommunityProfileIndexError;

const DEFAULT_TAP_BRANCH: &str = "main";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CommunityTapSubscription {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_commit: Option<String>,
}

impl CommunityTapSubscription {
    pub(crate) fn directory_name(&self) -> String {
        use super::utils::slugify;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityTapWorkspace {
    pub subscription: CommunityTapSubscription,
    pub local_path: PathBuf,
}

impl CommunityTapWorkspace {
    pub(crate) fn branch(&self) -> &str {
        self.subscription
            .branch
            .as_deref()
            .unwrap_or(DEFAULT_TAP_BRANCH)
    }
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
    pub index: super::index::CommunityProfileIndex,
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
