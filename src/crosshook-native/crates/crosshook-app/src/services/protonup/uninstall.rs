//! Guarded Proton uninstall.
//!
//! Root authority belongs to the backend: the caller-supplied Steam client
//! path is honoured only when it canonically equals a backend-trusted Steam
//! root (see [`ProtonRootSource::steam_roots`]); otherwise it is ignored. The
//! target must not contain `..` components and must be resolved by
//! [`ensure_within_workspace`] inside one of the install roots
//! derived from the trusted Steam root. Core's `plan_uninstall` then stays the
//! containment authority (canonical, direct child of a root, system
//! denylist), and `uninstall` re-plans immediately before deleting, so a
//! client-supplied plan is never trusted. Every response carries a fixed,
//! path-free text; details go to `tracing` only.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crosshook_core::fs_util::{ensure_within_workspace, WorkspacePathError};
use crosshook_core::protonup::uninstall::{
    execute_uninstall_for_path, plan_uninstall as core_plan_uninstall, UninstallError,
};

use super::messages::{uninstall_error_text, UNINSTALL_NOT_FOUND, UNINSTALL_OUTSIDE};
use super::seams::ProtonRootSource;
use super::{ProtonUninstallPlanResponse, ProtonUninstallResponse, ProtonUpService};
use crate::error::AppError;
use crate::services::run_blocking;

/// `InstallRootCandidate::reason` token for a root whose path is unusable.
const INVALID_PATH_REASON: &str = "invalid-path";

/// Which core operation follows a successful containment check.
#[derive(Clone, Copy)]
enum Action {
    /// Plan only; the filesystem is not touched.
    Plan,
    /// Re-plan and delete the tool directory.
    Execute,
}

impl ProtonUpService {
    /// Plans removal of the Proton tool at `tool_path` without touching disk,
    /// so the UI can show `conflicting_app_ids` before confirming.
    ///
    /// Refusals are reported as `success: false` with a fixed, path-free
    /// `error_message`; `Err` is returned only when the blocking work fails
    /// to run.
    pub async fn plan_uninstall(
        &self,
        tool_path: String,
        steam_client_install_path: Option<String>,
    ) -> Result<ProtonUninstallPlanResponse, AppError> {
        let outcome = self
            .run_uninstall(tool_path, steam_client_install_path, Action::Plan)
            .await?;
        Ok(match outcome {
            Ok(conflicting_app_ids) => ProtonUninstallPlanResponse {
                success: true,
                conflicting_app_ids,
                error_message: None,
            },
            Err(text) => ProtonUninstallPlanResponse {
                success: false,
                conflicting_app_ids: Vec::new(),
                error_message: Some(text.to_string()),
            },
        })
    }

    /// Removes the Proton tool at `tool_path`.
    ///
    /// The target is re-validated and re-planned here; nothing from an
    /// earlier [`plan_uninstall`](Self::plan_uninstall) is trusted. Refusals
    /// and failures are reported as `success: false` with a fixed, path-free
    /// `error_message`; `Err` is returned only when the blocking work fails
    /// to run.
    pub async fn uninstall(
        &self,
        tool_path: String,
        steam_client_install_path: Option<String>,
    ) -> Result<ProtonUninstallResponse, AppError> {
        let outcome = self
            .run_uninstall(tool_path, steam_client_install_path, Action::Execute)
            .await?;
        Ok(match outcome {
            Ok(conflicting_app_ids) => ProtonUninstallResponse {
                success: true,
                conflicting_app_ids,
                error_message: None,
            },
            Err(text) => ProtonUninstallResponse {
                success: false,
                conflicting_app_ids: Vec::new(),
                error_message: Some(text.to_string()),
            },
        })
    }

    /// Resolves trusted roots, then runs the blocking containment check and
    /// the core operation. The inner `Err` is a fixed refusal text.
    async fn run_uninstall(
        &self,
        tool_path: String,
        steam_client_install_path: Option<String>,
        action: Action,
    ) -> Result<Result<Vec<String>, &'static str>, AppError> {
        let trusted = self.roots.steam_roots().await;
        let roots = Arc::clone(&self.roots);
        run_blocking(move || {
            Ok(uninstall_blocking(
                roots.as_ref(),
                &trusted,
                &tool_path,
                steam_client_install_path.as_deref(),
                action,
            ))
        })
        .await
    }
}

/// Containment check followed by the core plan or deletion.
///
/// Returns the conflicting Steam app ids (always empty for
/// [`Action::Execute`]) or a fixed refusal text.
fn uninstall_blocking(
    roots: &dyn ProtonRootSource,
    trusted: &[PathBuf],
    tool_path: &str,
    caller_steam_path: Option<&str>,
    action: Action,
) -> Result<Vec<String>, &'static str> {
    let tool = Path::new(tool_path);
    if tool_path.trim().is_empty()
        || !tool.is_absolute()
        || tool
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(UNINSTALL_OUTSIDE);
    }
    let steam = select_steam_root(trusted, caller_steam_path);
    let candidates: Vec<PathBuf> = roots
        .install_roots(steam.as_deref())
        .into_iter()
        .filter(|candidate| candidate.reason.as_deref() != Some(INVALID_PATH_REASON))
        .map(|candidate| candidate.path)
        .collect();
    let tool = canonical_parent_join(tool, &candidates)?;
    let canonical = contain_in_install_root(&candidates, &tool)?;

    match action {
        Action::Plan => core_plan_uninstall(&canonical, steam.as_deref())
            .map(|plan| plan.conflicting_app_ids)
            .map_err(refusal),
        Action::Execute => execute_uninstall_for_path(&canonical, steam.as_deref())
            .map(|()| Vec::new())
            .map_err(refusal),
    }
}

/// `tool` with its parent directory canonicalized and the final component
/// kept as spelled.
///
/// A tool reached through an alias spelling of a Steam root then compares
/// equal to the deduped install root, while a final-component symlink is
/// still walked (and rejected if it escapes) by [`ensure_within_workspace`].
///
/// A missing parent maps to the not-found text only when the spelled `tool`
/// lexically sits under one of the usable install `candidates`; elsewhere it
/// maps to the outside-roots text, exactly like an existing parent would, so
/// the answer is no existence oracle for paths outside every root.
fn canonical_parent_join(tool: &Path, candidates: &[PathBuf]) -> Result<PathBuf, &'static str> {
    let (Some(parent), Some(name)) = (tool.parent(), tool.file_name()) else {
        return Err(UNINSTALL_OUTSIDE);
    };
    match fs::canonicalize(parent) {
        Ok(parent) => Ok(parent.join(name)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if candidates.iter().any(|root| tool.starts_with(root)) {
                Err(UNINSTALL_NOT_FOUND)
            } else {
                Err(UNINSTALL_OUTSIDE)
            }
        }
        Err(error) => {
            tracing::debug!(%error, "uninstall target parent could not be resolved");
            Err(UNINSTALL_OUTSIDE)
        }
    }
}

/// The Steam root to derive install roots from: the caller's path only when
/// it canonically equals a trusted root (the trusted spelling is returned),
/// otherwise the first trusted root.
pub(super) fn select_steam_root(trusted: &[PathBuf], caller: Option<&str>) -> Option<PathBuf> {
    if let Some(raw) = caller.map(str::trim).filter(|raw| !raw.is_empty()) {
        if let Ok(canonical) = fs::canonicalize(raw) {
            let matched = trusted
                .iter()
                .find(|root| fs::canonicalize(root).is_ok_and(|trusted| trusted == canonical));
            if let Some(root) = matched {
                return Some(root.clone());
            }
        }
        tracing::debug!("ignoring caller-supplied steam path that is not a trusted root");
    }
    trusted.first().cloned()
}

/// Canonical form of `tool` when some usable install root (`candidates`)
/// contains it.
///
/// A path inside a root that does not exist maps to the not-found text; any
/// other miss maps to the outside-roots text.
fn contain_in_install_root(candidates: &[PathBuf], tool: &Path) -> Result<PathBuf, &'static str> {
    let mut missing = false;
    for candidate in candidates {
        match ensure_within_workspace(candidate, tool) {
            Ok(canonical) => return Ok(canonical),
            Err(WorkspacePathError::NotFound) => missing = true,
            Err(error) => {
                tracing::debug!(%error, "uninstall target is not inside this install root");
            }
        }
    }
    Err(if missing {
        UNINSTALL_NOT_FOUND
    } else {
        UNINSTALL_OUTSIDE
    })
}

/// Logs the underlying `error` and returns its fixed text.
fn refusal(error: UninstallError) -> &'static str {
    tracing::warn!(%error, "proton uninstall refused or failed");
    uninstall_error_text(&error)
}
