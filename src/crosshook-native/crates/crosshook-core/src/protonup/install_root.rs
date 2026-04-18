//! Environment-aware resolver for Proton install root candidates.
//!
//! Enumerates and ranks candidate `compatibilitytools.d` directories for the
//! current environment (native vs Flatpak Steam), probes each for writability,
//! and surfaces a preference-ordered list for the install orchestrator and UI.
//!
//! This module is deliberately free of HTTP, SQLite, and Tauri imports. It is
//! pure filesystem + environment detection and is recomputed per call.

use std::path::{Component, Path, PathBuf};

use crate::platform;

// ── public types ──────────────────────────────────────────────────────────────

/// Discriminates between the Steam install flavour that owns the directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallRootKind {
    NativeSteam,
    FlatpakSteam,
}

/// One candidate install root with writability status and an optional
/// human-readable reason string explaining why the path is not writable.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstallRootCandidate {
    pub kind: InstallRootKind,
    pub path: PathBuf,
    pub writable: bool,
    /// Human-readable token explaining a writability failure. `None` when the
    /// path is writable or when the directory can be created on demand.
    pub reason: Option<String>,
}

// ── public API ────────────────────────────────────────────────────────────────

/// Returns every plausible install root for the current environment,
/// preference-ordered (NativeSteam variants before FlatpakSteam).
///
/// `configured_steam_client_path` is the resolved Steam client install path
/// from settings (used to derive `<path>/compatibilitytools.d`). Pass `None`
/// to fall back to default home-relative paths only.
pub fn resolve_install_root_candidates(
    configured_steam_client_path: Option<&Path>,
) -> Vec<InstallRootCandidate> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    resolve_candidates_with(
        configured_steam_client_path,
        home.as_deref(),
        platform::is_flatpak(),
    )
}

/// Picks the default candidate the UI should pre-select.
///
/// Under Flatpak, a writable `NativeSteam` candidate is preferred over any
/// `FlatpakSteam` candidate (mirrors `prefer_user_local_compat_tool_path`
/// semantics). Outside Flatpak, the first writable candidate in enumeration
/// order is returned. Returns `None` if no writable candidate exists.
pub fn pick_default_install_root(
    candidates: &[InstallRootCandidate],
) -> Option<&InstallRootCandidate> {
    pick_default_with(candidates, platform::is_flatpak())
}

// ── internal implementations (pub(crate) for tests) ──────────────────────────

/// Testable core of [`resolve_install_root_candidates`].
///
/// `home_override` replaces the `HOME` env lookup. `is_flatpak` is threaded
/// as a boolean so tests can exercise Flatpak logic without mutating
/// `FLATPAK_ID`.
pub(crate) fn resolve_candidates_with(
    configured_steam_client_path: Option<&Path>,
    home_override: Option<&Path>,
    is_flatpak: bool,
) -> Vec<InstallRootCandidate> {
    let _ = is_flatpak; // Flatpak context does not change the enumeration list,
                        // only `pick_default_with` uses it for ranking.

    let Some(home) = home_override else {
        tracing::warn!("install root resolver: HOME is unset, returning empty candidate list");
        return Vec::new();
    };

    let mut raw: Vec<(InstallRootKind, PathBuf)> = Vec::new();
    let mut seen_canonical: Vec<PathBuf> = Vec::new();

    // ── 1. Native Steam paths ─────────────────────────────────────────────────

    let native1 = home.join(".local/share/Steam/compatibilitytools.d");
    let native2 = home.join(".steam/root/compatibilitytools.d");

    push_deduped(
        &native1,
        InstallRootKind::NativeSteam,
        &mut raw,
        &mut seen_canonical,
    );
    push_deduped(
        &native2,
        InstallRootKind::NativeSteam,
        &mut raw,
        &mut seen_canonical,
    );

    // Optional configured Steam client path.
    if let Some(client_path) = configured_steam_client_path {
        let s = client_path.to_string_lossy();
        if !s.trim().is_empty() {
            let configured_compat = client_path.join("compatibilitytools.d");
            let configured_kind = classify_steam_root_kind(client_path);
            push_deduped(
                &configured_compat,
                configured_kind,
                &mut raw,
                &mut seen_canonical,
            );
        }
    }

    // ── 2. Flatpak Steam path ─────────────────────────────────────────────────

    let flatpak_steam =
        home.join(".var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d");
    push_deduped(
        &flatpak_steam,
        InstallRootKind::FlatpakSteam,
        &mut raw,
        &mut seen_canonical,
    );

    // ── 3. Probe each candidate ───────────────────────────────────────────────

    raw.into_iter()
        .map(|(kind, path)| probe_candidate(kind, path))
        .collect()
}

/// Testable core of [`pick_default_install_root`].
pub(crate) fn pick_default_with(
    candidates: &[InstallRootCandidate],
    is_flatpak: bool,
) -> Option<&InstallRootCandidate> {
    if is_flatpak {
        // Under Flatpak prefer a writable NativeSteam candidate first.
        if let Some(c) = candidates
            .iter()
            .find(|c| c.kind == InstallRootKind::NativeSteam && c.writable)
        {
            return Some(c);
        }
    }
    // Fall back to first writable candidate in enumeration order.
    candidates.iter().find(|c| c.writable)
}

// ── private helpers ───────────────────────────────────────────────────────────

/// Appends `(kind, path)` to `raw` unless `path` (or its canonical form)
/// is already present in `seen_canonical`.
fn push_deduped(
    path: &Path,
    kind: InstallRootKind,
    raw: &mut Vec<(InstallRootKind, PathBuf)>,
    seen_canonical: &mut Vec<PathBuf>,
) {
    // Canonicalize when the path exists; fall back to the raw path otherwise.
    let key = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if seen_canonical.contains(&key) {
        tracing::debug!(
            path = %path.display(),
            "install root resolver: skipping duplicate candidate (same canonical path)"
        );
        return;
    }
    seen_canonical.push(key);
    raw.push((kind, path.to_path_buf()));
}

/// Validates `path` and probes writability, returning a fully populated
/// [`InstallRootCandidate`].
fn probe_candidate(kind: InstallRootKind, path: PathBuf) -> InstallRootCandidate {
    // ── 1. Path validation (mirrors validate_install_destination rules) ───────

    if !path.is_absolute() {
        tracing::debug!(path = %path.display(), "install root: rejected (not absolute)");
        return InstallRootCandidate {
            kind,
            path,
            writable: false,
            reason: Some("invalid-path".to_string()),
        };
    }

    for component in path.components() {
        if component == Component::ParentDir {
            tracing::debug!(path = %path.display(), "install root: rejected ('..'' component)");
            return InstallRootCandidate {
                kind,
                path,
                writable: false,
                reason: Some("invalid-path".to_string()),
            };
        }
    }

    // ── 2. Writability probe ──────────────────────────────────────────────────

    if path.exists() && !path.is_dir() {
        tracing::debug!(
            path = %path.display(),
            "install root: rejected (path exists but is not a directory)"
        );
        return InstallRootCandidate {
            kind,
            path,
            writable: false,
            reason: Some("invalid-path".to_string()),
        };
    }

    if path.is_dir() {
        // Directory exists — try creating a tempfile inside it.
        match tempfile::NamedTempFile::new_in(&path) {
            Ok(_tmp) => {
                // `_tmp` is dropped here, deleting the probe file.
                tracing::debug!(path = %path.display(), "install root: writable (dir exists)");
                InstallRootCandidate {
                    kind,
                    path,
                    writable: true,
                    reason: None,
                }
            }
            Err(err) => {
                tracing::debug!(
                    path = %path.display(),
                    error = %err,
                    "install root: not writable (dir exists but tempfile creation failed)"
                );
                let reason = flatpak_or_generic_reason(kind, "parent-path-read-only");
                InstallRootCandidate {
                    kind,
                    path,
                    writable: false,
                    reason: Some(reason),
                }
            }
        }
    } else {
        // Directory does not exist — probe the parent.
        let parent = match path.parent() {
            Some(p) => p,
            None => {
                return InstallRootCandidate {
                    kind,
                    path,
                    writable: false,
                    reason: Some("invalid-path".to_string()),
                };
            }
        };

        if !parent.exists() {
            tracing::debug!(
                path = %path.display(),
                parent = %parent.display(),
                "install root: not writable (parent path missing)"
            );
            return InstallRootCandidate {
                kind,
                path,
                writable: false,
                reason: Some("parent-path-missing".to_string()),
            };
        }

        // Parent exists — probe writability there.
        match tempfile::NamedTempFile::new_in(parent) {
            Ok(_tmp) => {
                tracing::debug!(
                    path = %path.display(),
                    "install root: writable (dir will be created on install)"
                );
                InstallRootCandidate {
                    kind,
                    path,
                    writable: true,
                    reason: None,
                }
            }
            Err(err) => {
                tracing::debug!(
                    path = %path.display(),
                    error = %err,
                    "install root: not writable (parent exists but is read-only)"
                );
                let reason = flatpak_or_generic_reason(kind, "parent-path-read-only");
                InstallRootCandidate {
                    kind,
                    path,
                    writable: false,
                    reason: Some(reason),
                }
            }
        }
    }
}

fn classify_steam_root_kind(client_path: &Path) -> InstallRootKind {
    let marker = Path::new(".var/app/com.valvesoftware.Steam/data/Steam");
    if client_path.ends_with(marker) {
        InstallRootKind::FlatpakSteam
    } else {
        InstallRootKind::NativeSteam
    }
}

/// Returns a Flatpak-specific reason token for `FlatpakSteam` kind, otherwise
/// returns `generic`.
fn flatpak_or_generic_reason(kind: InstallRootKind, generic: &str) -> String {
    if kind == InstallRootKind::FlatpakSteam {
        "flatpak-steam-path-read-only".to_string()
    } else {
        generic.to_string()
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    // Mutex that serialises env-var mutations across all tests in this module.
    static FLATPAK_ID_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Scoped guard that sets an env var and restores it on drop.
    struct ScopedEnv {
        key: &'static str,
        original: Option<std::ffi::OsString>,
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl ScopedEnv {
        fn unset(key: &'static str) -> Self {
            let guard = FLATPAK_ID_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let original = std::env::var_os(key);
            // SAFETY: serialised by the mutex.
            unsafe { std::env::remove_var(key) };
            Self {
                key,
                original,
                _guard: guard,
            }
        }
    }

    impl Drop for ScopedEnv {
        fn drop(&mut self) {
            match &self.original {
                // SAFETY: mutex is still held.
                Some(val) => unsafe { std::env::set_var(self.key, val) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    // ── test 1 ────────────────────────────────────────────────────────────────

    /// In a non-Flatpak environment the resolver must enumerate NativeSteam
    /// candidates before the FlatpakSteam candidate.
    #[test]
    fn resolver_returns_native_then_flatpak_in_nonflatpak_env() {
        let home = tempdir().unwrap();
        // Do not set FLATPAK_ID — simulate non-Flatpak.
        let _guard = ScopedEnv::unset("FLATPAK_ID");

        let candidates = resolve_candidates_with(None, Some(home.path()), false);

        // There must be at least 2 candidates: some NativeSteam and one FlatpakSteam.
        assert!(
            candidates.len() >= 2,
            "expected at least 2 candidates, got {}",
            candidates.len()
        );

        // All NativeSteam entries must come before any FlatpakSteam entry.
        let mut seen_flatpak = false;
        for c in &candidates {
            if c.kind == InstallRootKind::FlatpakSteam {
                seen_flatpak = true;
            }
            if seen_flatpak {
                assert_ne!(
                    c.kind,
                    InstallRootKind::NativeSteam,
                    "NativeSteam candidate appeared after FlatpakSteam: {}",
                    c.path.display()
                );
            }
        }

        // The last candidate must be FlatpakSteam.
        assert_eq!(
            candidates.last().unwrap().kind,
            InstallRootKind::FlatpakSteam,
            "last candidate should be FlatpakSteam"
        );
    }

    // ── test 2 ────────────────────────────────────────────────────────────────

    /// Under Flatpak, `pick_default_install_root` must prefer a writable
    /// NativeSteam candidate over any FlatpakSteam candidate.
    #[test]
    fn resolver_prefers_native_over_flatpak_under_flatpak() {
        let home = tempdir().unwrap();
        // Create the NativeSteam directory so it passes the writable probe.
        let native_path = home.path().join(".local/share/Steam/compatibilitytools.d");
        fs::create_dir_all(&native_path).unwrap();

        // Simulate Flatpak environment (is_flatpak = true).
        let candidates = resolve_candidates_with(None, Some(home.path()), true);

        let default = pick_default_with(&candidates, true);
        assert!(default.is_some(), "expected a writable default candidate");
        let default = default.unwrap();
        assert_eq!(
            default.kind,
            InstallRootKind::NativeSteam,
            "under Flatpak the default must be NativeSteam, got path: {}",
            default.path.display()
        );
        assert!(default.writable, "default candidate must be writable");
    }

    // ── test 3 ────────────────────────────────────────────────────────────────

    /// A read-only Flatpak Steam directory must produce `writable = false`
    /// and `reason = Some("flatpak-steam-path-read-only")`.
    #[test]
    fn resolver_marks_unwritable_flatpak_path_with_reason() {
        let home = tempdir().unwrap();

        // Create the Flatpak Steam path and make it read-only.
        let flatpak_compat = home
            .path()
            .join(".var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d");
        fs::create_dir_all(&flatpak_compat).unwrap();
        let mut perms = fs::metadata(&flatpak_compat).unwrap().permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&flatpak_compat, perms).unwrap();

        let candidates = resolve_candidates_with(None, Some(home.path()), false);

        let flatpak_candidate = candidates
            .iter()
            .find(|c| c.kind == InstallRootKind::FlatpakSteam)
            .expect("expected a FlatpakSteam candidate");

        // Restore permissions so the tempdir cleanup works.
        let mut perms = fs::metadata(&flatpak_compat).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&flatpak_compat, perms).unwrap();

        assert!(
            !flatpak_candidate.writable,
            "read-only Flatpak Steam path should not be writable"
        );
        assert_eq!(
            flatpak_candidate.reason.as_deref(),
            Some("flatpak-steam-path-read-only"),
            "expected flatpak-specific reason, got: {:?}",
            flatpak_candidate.reason
        );
    }

    // ── test 4 ────────────────────────────────────────────────────────────────

    /// A configured Steam client path containing `..` must produce a candidate
    /// with `writable = false` and `reason = Some("invalid-path")`.
    #[test]
    fn resolver_rejects_parent_traversal_paths() {
        let home = tempdir().unwrap();

        // Pass a configured_steam_client_path that contains "..".
        let traversal_path = Path::new("/home/user/../../../etc");
        let candidates = resolve_candidates_with(Some(traversal_path), Some(home.path()), false);

        // The derived path would be `/home/user/../../../etc/compatibilitytools.d`.
        // Find a candidate with `..` in its path.
        let bad_candidate = candidates
            .iter()
            .find(|c| c.path.components().any(|comp| comp == Component::ParentDir));

        // If the path was included as a candidate it must be marked invalid.
        if let Some(c) = bad_candidate {
            assert!(
                !c.writable,
                "path with '..' should not be writable: {}",
                c.path.display()
            );
            assert_eq!(
                c.reason.as_deref(),
                Some("invalid-path"),
                "expected invalid-path reason, got: {:?}",
                c.reason
            );
        }
        // If it was excluded entirely that is also acceptable — the test
        // verifies no traversal path sneaks through as writable.
        for c in &candidates {
            if c.path.components().any(|comp| comp == Component::ParentDir) {
                assert!(!c.writable, "traversal path must not be writable");
            }
        }
    }

    // ── test 5 ────────────────────────────────────────────────────────────────

    /// When no candidate is writable, `pick_default_install_root` must return
    /// `None`.
    #[test]
    fn pick_default_install_root_returns_none_when_nothing_writable() {
        let candidates = vec![
            InstallRootCandidate {
                kind: InstallRootKind::NativeSteam,
                path: PathBuf::from("/nonexistent/path/one/compatibilitytools.d"),
                writable: false,
                reason: Some("parent-path-missing".to_string()),
            },
            InstallRootCandidate {
                kind: InstallRootKind::FlatpakSteam,
                path: PathBuf::from("/nonexistent/path/two/compatibilitytools.d"),
                writable: false,
                reason: Some("flatpak-steam-path-read-only".to_string()),
            },
        ];

        assert!(
            pick_default_with(&candidates, false).is_none(),
            "expected None when no candidate is writable"
        );
        assert!(
            pick_default_with(&candidates, true).is_none(),
            "expected None under Flatpak when no candidate is writable"
        );
    }

    #[test]
    fn configured_flatpak_steam_path_is_classified_as_flatpak() {
        let home = tempdir().unwrap();
        let configured = home
            .path()
            .join(".var/app/com.valvesoftware.Steam/data/Steam");
        let candidates = resolve_candidates_with(Some(&configured), Some(home.path()), false);

        let configured_candidate = candidates
            .iter()
            .find(|c| c.path == configured.join("compatibilitytools.d"))
            .expect("configured candidate must be present");
        assert_eq!(configured_candidate.kind, InstallRootKind::FlatpakSteam);
    }

    #[test]
    fn probe_marks_existing_non_directory_path_as_invalid() {
        let dir = tempdir().unwrap();
        let steam_root = dir.path().join(".local/share/Steam");
        fs::create_dir_all(&steam_root).unwrap();
        let fake_compat_file = steam_root.join("compatibilitytools.d");
        fs::write(&fake_compat_file, "not a directory").unwrap();

        let candidates = resolve_candidates_with(Some(&steam_root), Some(dir.path()), false);
        let configured_candidate = candidates
            .iter()
            .find(|c| c.path == fake_compat_file)
            .expect("configured candidate should be discovered");
        assert!(!configured_candidate.writable);
        assert_eq!(configured_candidate.reason.as_deref(), Some("invalid-path"));
    }
}
