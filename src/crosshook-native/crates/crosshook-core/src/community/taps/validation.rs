use super::types::{CommunityTapError, CommunityTapSubscription};

/// Validates that a pinned commit string is a safe git SHA (hex-only, 7–64 characters).
///
/// This prevents flag-injection (e.g. `--force`, `-q`) and shell-injection strings
/// from being passed to `git checkout` as a positional argument.
pub(crate) fn is_valid_git_sha(commit: &str) -> bool {
    (7..=64).contains(&commit.len()) && commit.chars().all(|c| c.is_ascii_hexdigit())
}

/// Validates that a branch name is safe to pass as a git positional argument.
///
/// Rejects names starting with `-` (would be interpreted as git flags) and names
/// containing characters outside `[a-zA-Z0-9/._-]` (max 200 chars).
pub(crate) fn validate_branch_name(branch: &str) -> Result<(), CommunityTapError> {
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
pub(crate) fn validate_tap_url(url: &str) -> Result<(), CommunityTapError> {
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

pub(crate) fn normalize_subscription(
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
