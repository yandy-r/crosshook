//! Install root authority for Proton installs.
//!
//! The install destination is bound to backend-resolved roots: the trusted
//! candidates are the install roots derived from every backend-discovered
//! Steam root (see [`ProtonRootSource::steam_roots`]); core includes the
//! home-relative defaults in each derivation. The install root listing
//! returns exactly this set. A caller-supplied `target_root` is accepted only
//! when its canonical form equals a trusted candidate's, and the installer
//! then receives the candidate's own spelling. An empty `target_root` selects
//! the default among the trusted candidates. Refusals carry fixed, path-free
//! texts; details go to `tracing` only.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crosshook_core::protonup::install_root::{pick_default_install_root, InstallRootCandidate};

use super::messages::{MSG_INSTALL_ROOT_UNTRUSTED, MSG_NO_WRITABLE_INSTALL_ROOT};
use super::seams::ProtonRootSource;
use super::ProtonUpService;
use crate::error::AppError;
use crate::services::run_blocking;

/// `InstallRootCandidate::reason` token for a root whose path is unusable.
const INVALID_PATH_REASON: &str = "invalid-path";

/// Install root candidates for `steam_root`, without `invalid-path` roots.
/// Blocking.
pub(super) fn usable_install_roots(
    roots: &dyn ProtonRootSource,
    steam_root: Option<&Path>,
) -> impl Iterator<Item = InstallRootCandidate> {
    roots
        .install_roots(steam_root)
        .into_iter()
        .filter(|candidate| candidate.reason.as_deref() != Some(INVALID_PATH_REASON))
}

impl ProtonUpService {
    /// The trusted install root for a caller-supplied `target_root`, as the
    /// string the installer must receive. The inner `Err` is a fixed refusal
    /// text.
    ///
    /// # Errors
    /// Task failures only.
    pub(super) async fn authorize_install_root(
        &self,
        target_root: String,
    ) -> Result<Result<String, &'static str>, AppError> {
        let steam = self.roots.steam_roots().await;
        let roots = Arc::clone(&self.roots);
        run_blocking(move || Ok(resolve_install_root(roots.as_ref(), &steam, &target_root))).await
    }

    /// The trusted install roots in enumeration order: exactly the set
    /// [`authorize_install_root`](Self::authorize_install_root) accepts.
    ///
    /// # Errors
    /// Task failures only.
    pub(super) async fn trusted_install_roots(
        &self,
    ) -> Result<Vec<InstallRootCandidate>, AppError> {
        let steam = self.roots.steam_roots().await;
        let roots = Arc::clone(&self.roots);
        run_blocking(move || {
            Ok(trusted_candidates(roots.as_ref(), &steam)
                .into_iter()
                .map(|entry| entry.candidate)
                .collect())
        })
        .await
    }
}

/// A usable install root candidate and its normalized form.
struct Trusted {
    candidate: InstallRootCandidate,
    normalized: PathBuf,
}

/// Resolves `target_root` against the trusted candidates. Blocking.
fn resolve_install_root(
    roots: &dyn ProtonRootSource,
    steam: &[PathBuf],
    target_root: &str,
) -> Result<String, &'static str> {
    let trusted = trusted_candidates(roots, steam);
    let raw = target_root.trim();
    let candidate = if raw.is_empty() {
        let candidates: Vec<InstallRootCandidate> =
            trusted.into_iter().map(|entry| entry.candidate).collect();
        match pick_default_install_root(&candidates) {
            Some(candidate) if candidate.writable => candidate.clone(),
            _ => return Err(MSG_NO_WRITABLE_INSTALL_ROOT),
        }
    } else {
        let target = expand_home(raw)
            .as_deref()
            .and_then(normalize)
            .ok_or(MSG_INSTALL_ROOT_UNTRUSTED)?;
        match trusted.into_iter().find(|entry| entry.normalized == target) {
            Some(entry) => entry.candidate,
            None => {
                tracing::debug!("install target is not a trusted install root");
                return Err(MSG_INSTALL_ROOT_UNTRUSTED);
            }
        }
    };
    candidate.path.to_str().map(str::to_string).ok_or_else(|| {
        tracing::debug!("trusted install root is not valid UTF-8");
        MSG_INSTALL_ROOT_UNTRUSTED
    })
}

/// Usable candidates derived from each trusted Steam root (or, with no Steam
/// root, the home-relative defaults alone), in enumeration order and deduped
/// by normalized form.
fn trusted_candidates(roots: &dyn ProtonRootSource, steam: &[PathBuf]) -> Vec<Trusted> {
    let sources: Vec<Option<&Path>> = if steam.is_empty() {
        vec![None]
    } else {
        steam.iter().map(|root| Some(root.as_path())).collect()
    };
    let mut trusted: Vec<Trusted> = Vec::new();
    for candidate in sources
        .into_iter()
        .flat_map(|root| usable_install_roots(roots, root))
    {
        let Some(normalized) = normalize(&candidate.path) else {
            continue;
        };
        if trusted.iter().all(|entry| entry.normalized != normalized) {
            trusted.push(Trusted {
                candidate,
                normalized,
            });
        }
    }
    trusted
}

/// `raw` with a leading `~` or `~/` expanded to the current user's home.
/// Other `~` forms are not supported.
fn expand_home(raw: &str) -> Option<PathBuf> {
    let Some(rest) = raw.strip_prefix('~') else {
        return Some(PathBuf::from(raw));
    };
    let home = crosshook_core::platform::base_dirs()?
        .home_dir()
        .to_path_buf();
    if rest.is_empty() {
        Some(home)
    } else {
        rest.strip_prefix('/').map(|rest| home.join(rest))
    }
}

/// Canonical form of an absolute, `..`-free `path`. A path that does not
/// exist yet resolves through its deepest existing ancestor, with the
/// missing components appended as spelled.
fn normalize(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return None;
    }
    let mut missing = Vec::new();
    let mut current = path;
    loop {
        match fs::canonicalize(current) {
            Ok(canonical) => {
                return Some(
                    missing
                        .iter()
                        .rev()
                        .fold(canonical, |acc, name| acc.join(name)),
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(current.file_name()?);
                current = current.parent()?;
            }
            Err(error) => {
                tracing::debug!(%error, "install root could not be resolved");
                return None;
            }
        }
    }
}
