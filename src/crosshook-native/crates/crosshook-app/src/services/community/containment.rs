use std::path::{Component, Path, PathBuf};

use crosshook_core::community::CommunityTapWorkspace;
use crosshook_core::fs_util::{ensure_within_workspace, WorkspacePathError};

use crate::{AppError, AppErrorKind};

pub(super) fn contain_in_taps(
    workspaces: &[CommunityTapWorkspace],
    path: &Path,
) -> Result<PathBuf, AppError> {
    if path.components().any(|part| part == Component::ParentDir) {
        return Err(outside_workspace());
    }
    let mut missing = false;
    let mut resolution_failed = false;
    for workspace in workspaces {
        match ensure_within_workspace(&workspace.local_path, path) {
            Ok(resolved) => return Ok(resolved),
            Err(WorkspacePathError::NotFound) => missing = true,
            Err(WorkspacePathError::Io { .. }) => resolution_failed = true,
            Err(
                WorkspacePathError::InvalidRoot
                | WorkspacePathError::Traversal
                | WorkspacePathError::OutsideWorkspace,
            ) => {}
        }
    }
    if missing {
        Err(AppError::new(
            AppErrorKind::NotFound,
            "community profile path was not found",
        ))
    } else if resolution_failed {
        Err(AppError::new(
            AppErrorKind::Io,
            "failed to resolve community profile path",
        ))
    } else {
        Err(outside_workspace())
    }
}

fn outside_workspace() -> AppError {
    AppError::validation(
        "path",
        "community profile path is not inside a known tap workspace",
    )
}

pub(super) fn require_regular_json(path: &Path) -> Result<(), AppError> {
    if !path.is_file() {
        return Err(AppError::validation(
            "path",
            "community profile path is not a regular file",
        ));
    }
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("json"))
    {
        return Err(AppError::validation(
            "path",
            "community profile path must point to a .json file",
        ));
    }
    Ok(())
}
