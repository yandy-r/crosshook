use std::env;
use std::path::Path;

use crate::launch::runtime_helpers::is_executable_file;

use super::models::BinaryDetectionResult;

const DEFAULT_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// Walk PATH for the given binary name, returning its absolute path if found.
fn resolve_binary_on_path(name: &str) -> Option<String> {
    let path_value =
        env::var_os("PATH").unwrap_or_else(|| std::ffi::OsString::from(DEFAULT_PATH));
    for directory in env::split_paths(&path_value) {
        let candidate = directory.join(name);
        if is_executable_file(&candidate) {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

/// Detect the best available winetricks/protontricks binary.
///
/// Priority: (1) settings override path if non-empty and executable,
/// (2) `winetricks` on PATH, (3) `protontricks` on PATH.
pub fn detect_binary(settings_path: &str) -> BinaryDetectionResult {
    // Priority 1: Settings override
    if !settings_path.is_empty() {
        let p = Path::new(settings_path);
        if is_executable_file(p) {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "winetricks".to_string());
            return BinaryDetectionResult {
                found: true,
                binary_path: Some(settings_path.to_string()),
                binary_name: name,
                source: "settings".to_string(),
            };
        }
    }

    // Priority 2: winetricks on PATH
    if let Some(path) = resolve_binary_on_path("winetricks") {
        return BinaryDetectionResult {
            found: true,
            binary_path: Some(path),
            binary_name: "winetricks".to_string(),
            source: "path".to_string(),
        };
    }

    // Priority 3: protontricks on PATH
    if let Some(path) = resolve_binary_on_path("protontricks") {
        return BinaryDetectionResult {
            found: true,
            binary_path: Some(path),
            binary_name: "protontricks".to_string(),
            source: "path".to_string(),
        };
    }

    BinaryDetectionResult {
        found: false,
        binary_path: None,
        binary_name: String::new(),
        source: "not_found".to_string(),
    }
}

/// Resolve winetricks binary on PATH.
pub fn resolve_winetricks_path() -> Option<String> {
    resolve_binary_on_path("winetricks")
}

/// Resolve protontricks binary on PATH.
pub fn resolve_protontricks_path() -> Option<String> {
    resolve_binary_on_path("protontricks")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Create a fake executable in a temp dir.
    fn make_fake_executable(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).unwrap();
        }
        path
    }

    #[test]
    fn detect_binary_returns_not_found_when_no_tool_on_path() {
        // Use empty PATH
        let _guard = ScopedPath::new("");
        let result = detect_binary("");
        assert!(!result.found);
        assert!(result.binary_path.is_none());
        assert_eq!(result.source, "not_found");
    }

    #[test]
    fn detect_binary_prefers_settings_override() {
        let tmp = tempdir().unwrap();
        let exe = make_fake_executable(tmp.path(), "my-winetricks");
        let result = detect_binary(exe.to_str().unwrap());
        assert!(result.found);
        assert_eq!(result.source, "settings");
        assert_eq!(result.binary_name, "my-winetricks");
    }

    #[test]
    fn detect_binary_finds_winetricks_on_path() {
        let tmp = tempdir().unwrap();
        make_fake_executable(tmp.path(), "winetricks");
        let _guard = ScopedPath::new(tmp.path().to_str().unwrap());
        let result = detect_binary("");
        assert!(result.found);
        assert_eq!(result.binary_name, "winetricks");
        assert_eq!(result.source, "path");
    }

    #[test]
    fn detect_binary_falls_back_to_protontricks() {
        let tmp = tempdir().unwrap();
        make_fake_executable(tmp.path(), "protontricks");
        let _guard = ScopedPath::new(tmp.path().to_str().unwrap());
        let result = detect_binary("");
        assert!(result.found);
        assert_eq!(result.binary_name, "protontricks");
        assert_eq!(result.source, "path");
    }

    /// Mutex that serialises all tests that mutate `PATH`.
    static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Scoped PATH override for testing.
    ///
    /// Acquires `PATH_LOCK` for its lifetime so that concurrent tests do not
    /// clobber each other's environment.  Drop order in Rust is deterministic
    /// (LIFO), so the lock is released only after `PATH` has been restored.
    struct ScopedPath {
        original: Option<std::ffi::OsString>,
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl ScopedPath {
        fn new(path: &str) -> Self {
            let guard = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let original = env::var_os("PATH");
            // SAFETY: single-threaded access is guaranteed by the mutex.
            unsafe { env::set_var("PATH", path) };
            Self { original, _guard: guard }
        }
    }

    impl Drop for ScopedPath {
        fn drop(&mut self) {
            match &self.original {
                // SAFETY: mutex is still held; no other thread touches PATH.
                Some(val) => unsafe { env::set_var("PATH", val) },
                None => unsafe { env::remove_var("PATH") },
            }
        }
    }
}
