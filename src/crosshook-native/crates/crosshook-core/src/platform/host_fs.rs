use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::Stdio;

use super::detect::{is_flatpak, normalize_flatpak_host_path};
use super::gateway::host_std_command;

pub(crate) fn is_executable_file_sync(path: &Path) -> bool {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
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

/// Returns true if `path` may be probed on the host for system Steam compat-tool directories.
/// Only absolute paths under `/usr` or `/usr/local` (no `..`) are allowed.
pub fn is_allowed_host_system_compat_listing_path(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return false;
    }
    let root = Path::new("/usr");
    let local = Path::new("/usr/local");
    path.starts_with(root) || path.starts_with(local)
}

/// Returns whether `path` exists as a directory on the host when in Flatpak.
pub fn host_path_is_dir(path: &Path) -> bool {
    if !is_allowed_host_system_compat_listing_path(path) {
        return false;
    }
    if !is_flatpak() {
        return path.is_dir();
    }
    let mut cmd = host_std_command("test");
    cmd.arg("-d").arg(path);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

/// Reads directory entry names from a fixed system location on the host (Flatpak) or locally.
pub fn host_read_dir_names(path: &Path) -> io::Result<Vec<OsString>> {
    if !is_allowed_host_system_compat_listing_path(path) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path is not an allowed host system compat listing root",
        ));
    }
    if !is_flatpak() {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            out.push(entry.file_name());
        }
        out.sort();
        return Ok(out);
    }
    let mut cmd = host_std_command("ls");
    cmd.arg("-1").arg("--").arg(path);
    cmd.stdin(Stdio::null());
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "host ls failed with status {}",
            output.status.code().unwrap_or(-1)
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut names: Vec<OsString> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(OsString::from)
        .collect();
    names.sort();
    Ok(names)
}

/// Reads a file from the host filesystem when in Flatpak (via `cat`); `path` must pass
/// [`is_allowed_host_system_compat_listing_path`] and include a final component (tool directory).
pub fn host_read_file_bytes_if_system_path(path: &Path) -> io::Result<Vec<u8>> {
    if !is_allowed_host_system_compat_listing_path(path) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path is not under an allowed host system prefix",
        ));
    }
    if !is_flatpak() {
        return std::fs::read(path);
    }
    let mut cmd = host_std_command("cat");
    cmd.arg(path);
    cmd.stdin(Stdio::null());
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "host cat failed: {}",
            output.status
        )));
    }
    Ok(output.stdout)
}

/// True if `path` points to a regular file on the host (Flatpak) or locally.
pub fn host_path_is_file(path: &Path) -> bool {
    if !is_allowed_host_system_compat_listing_path(path) {
        return false;
    }
    if !is_flatpak() {
        return path.is_file();
    }
    let mut cmd = host_std_command("test");
    cmd.arg("-f").arg(path);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

/// True if `path` points to an executable file on the host (Flatpak) or locally.
pub fn host_path_is_executable_file(path: &Path) -> bool {
    if !is_allowed_host_system_compat_listing_path(path) {
        return false;
    }
    if !is_flatpak() {
        return is_executable_file_sync(path);
    }
    let mut cmd = host_std_command("test");
    cmd.arg("-f").arg(path).arg("-a").arg("-x").arg(path);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

pub fn normalized_path_is_file(path: &str) -> bool {
    let normalized = normalize_flatpak_host_path(path);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return false;
    }
    let path = Path::new(trimmed);
    if is_allowed_host_system_compat_listing_path(path) {
        host_path_is_file(path)
    } else {
        path.is_file()
    }
}

pub fn normalized_path_is_dir(path: &str) -> bool {
    let normalized = normalize_flatpak_host_path(path);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return false;
    }
    let path = Path::new(trimmed);
    if is_allowed_host_system_compat_listing_path(path) {
        host_path_is_dir(path)
    } else {
        path.is_dir()
    }
}

pub fn normalized_path_is_executable_file(path: &str) -> bool {
    let normalized = normalize_flatpak_host_path(path);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return false;
    }
    let path = Path::new(trimmed);
    if is_allowed_host_system_compat_listing_path(path) {
        host_path_is_executable_file(path)
    } else {
        is_executable_file_sync(path)
    }
}

fn normalized_path_host_test(path: &str, flag: &str) -> bool {
    let normalized = normalize_flatpak_host_path(path);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return false;
    }

    let path = Path::new(trimmed);
    if !is_flatpak() {
        return match flag {
            "-e" => path.exists(),
            "-f" => path.is_file(),
            "-d" => path.is_dir(),
            "-x" => is_executable_file_sync(path),
            _ => false,
        };
    }

    if !path.is_absolute() {
        return false;
    }

    let mut cmd = host_std_command("test");
    cmd.arg(flag).arg(path);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.status().map(|status| status.success()).unwrap_or(false)
}

/// Returns whether `path` exists on the host-visible filesystem after Flatpak normalization.
pub fn normalized_path_exists_on_host(path: &str) -> bool {
    normalized_path_host_test(path, "-e")
}

/// Returns whether `path` is a regular file on the host-visible filesystem after Flatpak normalization.
pub fn normalized_path_is_file_on_host(path: &str) -> bool {
    normalized_path_host_test(path, "-f")
}

/// Returns whether `path` is a directory on the host-visible filesystem after Flatpak normalization.
pub fn normalized_path_is_dir_on_host(path: &str) -> bool {
    normalized_path_host_test(path, "-d")
}

/// Returns whether `path` is an executable regular file on the host-visible filesystem after Flatpak normalization.
pub fn normalized_path_is_executable_file_on_host(path: &str) -> bool {
    normalized_path_host_test(path, "-f") && normalized_path_host_test(path, "-x")
}
