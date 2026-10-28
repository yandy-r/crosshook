use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;

use super::runner::{GitInvocation, GitOutput, GitRunError};
use super::types::{CommunityTapError, CommunityTapWorkspace};
use super::validation::is_valid_git_sha;

/// Abort HTTP transfers slower than 1 KB/s for 30 seconds.
const GIT_HTTP_LOW_SPEED_LIMIT: &str = "1000";
const GIT_HTTP_LOW_SPEED_TIME: &str = "30";

pub(super) fn git_security_env_pairs() -> [(&'static str, &'static str); 5] {
    [
        ("GIT_HTTP_LOW_SPEED_LIMIT", GIT_HTTP_LOW_SPEED_LIMIT),
        ("GIT_HTTP_LOW_SPEED_TIME", GIT_HTTP_LOW_SPEED_TIME),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_TERMINAL_PROMPT", "0"),
    ]
}

fn invocation(action: &'static str, args: &[&str], directory: Option<&Path>) -> GitInvocation {
    let env: BTreeMap<String, String> = git_security_env_pairs()
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    GitInvocation {
        action,
        args: args.iter().map(OsString::from).collect(),
        env,
        directory: directory.map(Path::to_path_buf),
    }
}

/// `git clone --branch B --single-branch -- URL PATH`
pub(super) fn clone_invocation(ws: &CommunityTapWorkspace) -> GitInvocation {
    let mut inv = invocation(
        "clone community tap",
        &["clone", "--branch", ws.branch(), "--single-branch", "--"],
        None,
    );
    inv.args.push(OsString::from(&ws.subscription.url));
    inv.args.push(ws.local_path.as_os_str().to_owned());
    inv
}

/// `git fetch --prune origin -- B` in the workspace.
pub(super) fn fetch_invocation(ws: &CommunityTapWorkspace) -> GitInvocation {
    invocation(
        "fetch community tap",
        &["fetch", "--prune", "origin", "--", ws.branch()],
        Some(&ws.local_path),
    )
}

/// `git reset --hard FETCH_HEAD` in the workspace.
pub(super) fn reset_invocation(ws: &CommunityTapWorkspace) -> GitInvocation {
    invocation(
        "reset community tap",
        &["reset", "--hard", "FETCH_HEAD"],
        Some(&ws.local_path),
    )
}

/// `git clean -fdx` in the workspace.
pub(super) fn clean_invocation(ws: &CommunityTapWorkspace) -> GitInvocation {
    invocation(
        "clean community tap",
        &["clean", "-fdx"],
        Some(&ws.local_path),
    )
}

/// `git checkout --detach <pinned>` in the workspace; the pin is validated first.
pub(super) fn checkout_invocation(
    ws: &CommunityTapWorkspace,
) -> Result<GitInvocation, CommunityTapError> {
    let pinned_commit =
        ws.subscription
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

    Ok(invocation(
        "checkout pinned commit",
        &["checkout", "--detach", pinned_commit],
        Some(&ws.local_path),
    ))
}

/// `git rev-parse HEAD` in `path`.
pub(super) fn rev_parse_invocation(path: &Path) -> GitInvocation {
    invocation("resolve tap commit", &["rev-parse", "HEAD"], Some(path))
}

/// Steps that refresh an existing clone: fetch, then reset or pinned checkout, then clean.
pub(super) fn update_invocations(
    ws: &CommunityTapWorkspace,
) -> Result<Vec<GitInvocation>, CommunityTapError> {
    let middle = if ws.subscription.pinned_commit.is_some() {
        checkout_invocation(ws)?
    } else {
        reset_invocation(ws)
    };
    Ok(vec![fetch_invocation(ws), middle, clean_invocation(ws)])
}

/// Steps that populate a fresh workspace: clone, then pinned checkout when pinned.
pub(super) fn clone_invocations(
    ws: &CommunityTapWorkspace,
) -> Result<Vec<GitInvocation>, CommunityTapError> {
    let mut steps = vec![clone_invocation(ws)];
    if ws.subscription.pinned_commit.is_some() {
        steps.push(checkout_invocation(ws)?);
    }
    Ok(steps)
}

/// Builds the `CommunityTapError::Git` for a finished-but-unsuccessful process.
pub(super) fn git_failure(invocation: &GitInvocation, output: &GitOutput) -> CommunityTapError {
    let mut command = String::from("git");
    if let Some(directory) = &invocation.directory {
        command.push_str(" -C ");
        command.push_str(&directory.display().to_string());
    }
    for arg in &invocation.args {
        command.push(' ');
        command.push_str(&arg.to_string_lossy());
    }
    CommunityTapError::Git {
        action: invocation.action,
        command,
        stderr: output.stderr.trim().to_string(),
    }
}

/// Maps a runner failure to the matching tap error; `path` is the step's workspace path.
pub(super) fn run_failure(
    invocation: &GitInvocation,
    error: GitRunError,
    fallback_path: &Path,
) -> CommunityTapError {
    match error {
        GitRunError::Cancelled => CommunityTapError::Cancelled,
        GitRunError::Failed(source) => CommunityTapError::Io {
            action: invocation.action,
            path: invocation
                .directory
                .clone()
                .unwrap_or_else(|| fallback_path.to_path_buf()),
            source,
        },
    }
}
