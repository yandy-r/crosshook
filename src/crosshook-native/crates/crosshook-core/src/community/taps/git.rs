use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use crate::platform::{self, host_std_command_with_env};

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

pub(crate) fn git_command() -> Command {
    let mut env = BTreeMap::new();
    for (key, value) in git_security_env_pairs() {
        env.insert(key.to_string(), value.to_string());
    }
    if platform::is_flatpak() {
        host_std_command_with_env("git", &env, &BTreeMap::new())
    } else {
        let mut command = Command::new("git");
        command.envs(&env);
        command
    }
}

pub(crate) fn clone_tap(workspace: &CommunityTapWorkspace) -> Result<(), CommunityTapError> {
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

pub(crate) fn fetch_and_reset(workspace: &CommunityTapWorkspace) -> Result<(), CommunityTapError> {
    run_git(
        workspace,
        "fetch community tap",
        &["fetch", "--prune", "origin", "--", workspace.branch()],
    )?;
    run_git(
        workspace,
        "reset community tap",
        &["reset", "--hard", "FETCH_HEAD"],
    )?;
    run_git(workspace, "clean community tap", &["clean", "-fdx"])?;
    Ok(())
}

pub(crate) fn fetch_and_checkout_pinned(
    workspace: &CommunityTapWorkspace,
) -> Result<(), CommunityTapError> {
    run_git(
        workspace,
        "fetch community tap",
        &["fetch", "--prune", "origin", "--", workspace.branch()],
    )?;
    checkout_pinned_commit(workspace)?;
    run_git(workspace, "clean community tap", &["clean", "-fdx"])?;
    Ok(())
}

pub(crate) fn checkout_pinned_commit(
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

    run_git(
        workspace,
        "checkout pinned commit",
        &["checkout", "--detach", pinned_commit],
    )?;
    Ok(())
}

pub(crate) fn rev_parse_head(path: &Path) -> Result<String, CommunityTapError> {
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

pub(crate) fn run_git(
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
