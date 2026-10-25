use std::path::PathBuf;

/// Returns the first readable umu-database CSV path, in precedence order:
/// 1. CrossHook HTTP cache (data_local_dir()/crosshook/umu-database.csv)
/// 2. Packaged umu-protonfixes (Arch multilib, Fedora: /usr/share/umu-protonfixes/)
/// 3. Alternate packaged path (/usr/share/umu/)
/// 4. Manual installs (/opt/umu-launcher/umu-protonfixes/)
/// 5. $XDG_DATA_DIRS/umu-protonfixes/
pub fn resolve_umu_database_path() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(base) = crate::platform::base_dirs() {
        candidates.push(
            base.data_local_dir()
                .join(super::CROSSHOOK_UMU_DATABASE_CSV_SUBPATH),
        );
    }
    candidates.push(PathBuf::from("/usr/share/umu-protonfixes/umu-database.csv"));
    candidates.push(PathBuf::from("/usr/share/umu/umu-database.csv"));
    candidates.push(PathBuf::from(
        "/opt/umu-launcher/umu-protonfixes/umu-database.csv",
    ));
    for data_dir in std::env::var("XDG_DATA_DIRS")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
    {
        let base = PathBuf::from(data_dir);
        if !base.is_absolute() {
            tracing::warn!(
                entry = data_dir,
                "XDG_DATA_DIRS entry is not absolute; skipping"
            );
            continue;
        }
        if base
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            tracing::warn!(
                entry = data_dir,
                "XDG_DATA_DIRS entry contains '..'; skipping"
            );
            continue;
        }
        candidates.push(base.join("umu-protonfixes/umu-database.csv"));
    }
    for cand in candidates {
        if std::fs::metadata(&cand)
            .map(|m| m.is_file())
            .unwrap_or(false)
        {
            tracing::debug!(path = %cand.display(), "resolved umu-database CSV");
            return Some(cand);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::{Mutex, MutexGuard};

    // Process-global env mutation — serialize tests that touch XDG_DATA_DIRS / HOME.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Saves the listed env vars and restores them on drop, so this test
    /// never leaks its tempdir `HOME` / `XDG_*` values into other tests in
    /// the same libtest process.
    struct EnvGuard {
        saved: Vec<(&'static str, Option<OsString>)>,
        _guard: MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn set(vars: &[(&'static str, &std::ffi::OsStr)]) -> Self {
            let guard = ENV_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let saved = vars
                .iter()
                .map(|(k, _)| (*k, std::env::var_os(k)))
                .collect();
            for (k, v) in vars {
                // SAFETY: serialized through ENV_LOCK for the guard's lifetime.
                unsafe { std::env::set_var(k, v) };
            }
            Self {
                saved,
                _guard: guard,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, v) in self.saved.drain(..) {
                match v {
                    // SAFETY: ENV_LOCK is still held by the guard.
                    Some(val) => unsafe { std::env::set_var(k, val) },
                    None => unsafe { std::env::remove_var(k) },
                }
            }
        }
    }

    #[test]
    fn resolve_returns_none_when_no_candidate_exists() {
        let tmp = tempfile::tempdir().unwrap();
        // Point HOME + XDG_DATA_DIRS at an empty tempdir so no candidate resolves.
        // The guard restores the original values when the test ends.
        let _env = EnvGuard::set(&[
            ("HOME", tmp.path().as_os_str()),
            ("XDG_DATA_HOME", tmp.path().join("local/share").as_os_str()),
            (
                "XDG_DATA_DIRS",
                tmp.path().join("xdg-data-dirs").as_os_str(),
            ),
        ]);
        // The /usr/share/... candidates may exist on the test host; we cannot unset those.
        // On a build host without umu-launcher packaged there, this returns None.
        // If the host has one of /usr/share/umu-protonfixes/, /usr/share/umu/, or
        // /opt/umu-launcher/umu-protonfixes/ populated, this test is skipped by design.
        if [
            "/usr/share/umu-protonfixes/umu-database.csv",
            "/usr/share/umu/umu-database.csv",
            "/opt/umu-launcher/umu-protonfixes/umu-database.csv",
        ]
        .iter()
        .any(|p| std::fs::metadata(p).map(|m| m.is_file()).unwrap_or(false))
        {
            return;
        }
        assert!(resolve_umu_database_path().is_none());
    }
}
