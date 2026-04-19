use directories::BaseDirs;
use std::path::{Path, PathBuf};

use super::AppSettingsData;

/// Resolve the profiles directory: empty config uses `crosshook_config_dir/profiles`.
pub fn resolve_profiles_directory_from_config(
    settings: &AppSettingsData,
    crosshook_config_dir: &Path,
) -> Result<PathBuf, String> {
    let raw = settings.profiles_directory.trim();
    if raw.is_empty() {
        return Ok(crosshook_config_dir.join("profiles"));
    }
    expand_path_with_tilde(raw)
}

fn current_user_home() -> Result<PathBuf, String> {
    BaseDirs::new()
        .ok_or_else(|| {
            "home directory not found — CrossHook requires a user home directory".to_string()
        })
        .map(|dirs| dirs.home_dir().to_path_buf())
}

/// Resolve `~username` to that user's home directory.
#[cfg(unix)]
fn resolve_user_home(username: &str) -> Result<PathBuf, String> {
    use std::process::Command;

    let output = if crate::platform::is_flatpak() {
        crate::platform::host_std_command("getent")
    } else {
        Command::new("getent")
    }
    .args(["passwd", username])
    .output()
    .map_err(|e| format!("failed to look up user '{username}': {e}"))?;

    if !output.status.success() {
        return Err(format!("user '{username}' not found"));
    }

    let line = String::from_utf8_lossy(&output.stdout);
    let home_field = line
        .split(':')
        .nth(5)
        .ok_or_else(|| format!("could not determine home directory for user '{username}'"))?;

    let home = PathBuf::from(home_field.trim());
    if home.as_os_str().is_empty() {
        return Err(format!("home directory for user '{username}' is empty"));
    }
    Ok(home)
}

#[cfg(not(unix))]
fn resolve_user_home(username: &str) -> Result<PathBuf, String> {
    Err(format!(
        "~{username} expansion is not supported on this platform"
    ))
}

pub(crate) fn expand_path_with_tilde(raw: &str) -> Result<PathBuf, String> {
    let t = raw.trim();

    // ~/path — current user's home
    if let Some(rest) = t.strip_prefix("~/") {
        return Ok(current_user_home()?.join(rest));
    }

    // bare ~ — current user's home (canonicalized)
    if t == "~" {
        return current_user_home()?
            .canonicalize()
            .map_err(|e| e.to_string());
    }

    // ~username or ~username/path — named user's home
    if let Some(after_tilde) = t.strip_prefix('~') {
        let (username, subpath) = match after_tilde.find('/') {
            Some(i) => (&after_tilde[..i], Some(&after_tilde[i + 1..])),
            None => (after_tilde, None),
        };
        let home = resolve_user_home(username)?;
        return Ok(match subpath {
            Some(p) => home.join(p),
            None => home,
        });
    }

    Ok(PathBuf::from(t))
}
