use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

use crate::platform;

use super::super::runtime_helpers::DEFAULT_HOST_PATH;

pub fn is_command_available(binary: &str) -> bool {
    #[cfg(test)]
    {
        let guard = test_command_search_path()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(search_path) = guard.as_ref() {
            return is_executable_file(&search_path.join(binary));
        }
    }

    if platform::is_flatpak() {
        return platform::host_command_exists(binary);
    }

    let path_value = env::var_os("PATH").unwrap_or_else(|| OsString::from(DEFAULT_HOST_PATH));

    env::split_paths(&path_value).any(|directory| is_executable_file(&directory.join(binary)))
}

#[cfg(test)]
fn test_command_search_path() -> &'static Mutex<Option<PathBuf>> {
    static SEARCH_PATH: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    SEARCH_PATH.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
pub(crate) fn swap_test_command_search_path(next: Option<PathBuf>) -> Option<PathBuf> {
    let mut guard = test_command_search_path()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::mem::replace(&mut *guard, next)
}

/// In test mode, check if the test command search path is active and probe for `umu-run`.
///
/// Returns `Some(Some(path))` if found under test path, `Some(None)` if test path active but
/// umu-run absent, and `None` if no test path override is active (caller uses real PATH).
#[cfg(test)]
pub(crate) fn resolve_umu_run_path_for_test() -> Option<Option<String>> {
    let guard = test_command_search_path()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let search_path = guard.as_ref()?;
    let candidate = search_path.join("umu-run");
    Some(if is_executable_file(&candidate) {
        Some(candidate.to_string_lossy().into_owned())
    } else {
        None
    })
}

fn is_executable_file(path: &Path) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}
