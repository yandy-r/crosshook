use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use super::models::PrefixDepsError;
use super::validation::validate_protontricks_verbs;
use super::PrefixDepsTool;
use crate::launch::runtime_helpers::{host_environment_map, resolve_wine_prefix_path};
use crate::platform::{
    host_command_with_env_and_directory_inner, is_flatpak, normalize_flatpak_host_path,
};

fn build_prefix_dep_command(binary_path: &str, prefix_path: &str) -> (Command, std::path::PathBuf) {
    build_prefix_dep_command_with_platform(binary_path, prefix_path, is_flatpak())
}

fn build_prefix_dep_command_with_platform(
    binary_path: &str,
    prefix_path: &str,
    flatpak: bool,
) -> (Command, std::path::PathBuf) {
    let normalized_prefix = normalize_flatpak_host_path(prefix_path);
    let resolved_prefix = resolve_wine_prefix_path(Path::new(&normalized_prefix));
    let normalized_binary = normalize_flatpak_host_path(binary_path);
    let mut environment = host_environment_map();
    environment.insert(
        "WINEPREFIX".to_string(),
        resolved_prefix.to_string_lossy().into_owned(),
    );
    (
        host_command_with_env_and_directory_inner(
            &normalized_binary,
            &environment,
            None,
            flatpak,
            &BTreeMap::new(),
        ),
        resolved_prefix,
    )
}

/// Default timeout for check operations (seconds).
const CHECK_TIMEOUT_SECS: u64 = 30;

/// Strip absolute filesystem paths from stderr before user display.
///
/// Replaces path-looking tokens (starting with `/home/`, `/tmp/`, `/var/`) with `<path>`.
/// This is a simple character-scan approach -- no regex dependency required.
fn strip_ansi_codes(raw: &str) -> String {
    let mut result = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn sanitize_stderr(raw: &str) -> String {
    let raw = strip_ansi_codes(raw);
    let prefixes: &[&str] = &["/home/", "/tmp/", "/var/"];
    let mut result = String::with_capacity(raw.len());
    let mut i = 0;
    let bytes = raw.as_bytes();
    while i < bytes.len() {
        let remaining = &raw[i..];
        let mut matched = false;
        for &prefix in prefixes {
            if remaining.starts_with(prefix) {
                // Consume the path token up to the next whitespace or colon.
                let end = remaining
                    .find(|c: char| c.is_whitespace() || c == ':')
                    .unwrap_or(remaining.len());
                result.push_str("<path>");
                i += end;
                matched = true;
                break;
            }
        }
        if !matched {
            // Advance one character safely.
            let ch_len = raw[i..].chars().next().map(char::len_utf8).unwrap_or(1);
            result.push_str(&raw[i..i + ch_len]);
            i += ch_len;
        }
    }
    // Truncate to prevent huge error messages (character-safe).
    if result.chars().count() > 500 {
        let truncated: String = result.chars().take(500).collect();
        format!("{truncated}...(truncated)")
    } else {
        result
    }
}

/// Sanitize a single runner output line for UI display.
pub fn sanitize_output_for_ui(raw: &str) -> String {
    sanitize_stderr(raw)
}

/// Check which packages are already installed in the given prefix.
///
/// Runs `binary_path list-installed` with WINEPREFIX set.
pub async fn check_installed(
    binary_path: &str,
    prefix_path: &str,
    tool_type: PrefixDepsTool,
    steam_app_id: Option<&str>,
) -> Result<Vec<String>, PrefixDepsError> {
    let (mut cmd, _resolved_prefix) = build_prefix_dep_command(binary_path, prefix_path);
    if matches!(tool_type, PrefixDepsTool::Protontricks) {
        let app_id = steam_app_id.ok_or_else(|| {
            PrefixDepsError::ValidationError(
                "steam app id is required when using protontricks".to_string(),
            )
        })?;
        cmd.arg(app_id);
    }
    cmd.arg("list-installed");
    cmd.kill_on_drop(true);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let child = cmd.spawn().map_err(|e| PrefixDepsError::ProcessFailed {
        exit_code: None,
        stderr: format!("failed to spawn: {e}"),
    })?;

    let output = tokio::time::timeout(
        Duration::from_secs(CHECK_TIMEOUT_SECS),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| PrefixDepsError::Timeout {
        seconds: CHECK_TIMEOUT_SECS,
    })?
    .map_err(|e| PrefixDepsError::ProcessFailed {
        exit_code: None,
        stderr: format!("failed to wait for process: {e}"),
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PrefixDepsError::ProcessFailed {
            exit_code: output.status.code(),
            stderr: sanitize_stderr(&stderr),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let packages: Vec<String> = stdout
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(std::string::ToString::to_string)
        .collect();

    Ok(packages)
}

/// Spawn the install process for the given packages.
///
/// Returns the `Child` process -- the caller is responsible for reading
/// stdout/stderr and streaming to the UI.
///
/// Security checklist:
/// - validate_protontricks_verbs() called first
/// - Per-verb .arg() calls (never joined)
/// - Flag-like and malformed verbs rejected before argv construction
/// - apply_host_environment() used (NOT env_clear())
/// - .kill_on_drop(true)
/// - Prefix path normalized via resolve_wine_prefix_path()
/// - pfx/ existence check
pub async fn install_packages(
    binary_path: &str,
    prefix_path: &str,
    verbs: &[String],
    tool_type: PrefixDepsTool,
    steam_app_id: Option<&str>,
) -> Result<tokio::process::Child, PrefixDepsError> {
    // Validate verbs first (security gate).
    validate_protontricks_verbs(verbs)?;

    let (mut cmd, resolved_prefix) = build_prefix_dep_command(binary_path, prefix_path);

    // Check prefix is initialized: the resolved path (pfx/ or prefix itself) must exist as a dir.
    if !resolved_prefix.exists() || !resolved_prefix.is_dir() {
        return Err(PrefixDepsError::PrefixNotInitialized {
            path: resolved_prefix.to_string_lossy().into_owned(),
        });
    }

    // Protontricks takes app_id first, then -q.
    if matches!(tool_type, PrefixDepsTool::Protontricks) {
        let app_id = steam_app_id.ok_or_else(|| {
            PrefixDepsError::ValidationError(
                "steam app id is required when using protontricks".to_string(),
            )
        })?;
        cmd.arg(app_id);
    }

    // Quiet mode.
    cmd.arg("-q");

    // Each validated verb is passed as an individual argument. Winetricks does
    // not support a `--` option separator, so structural validation above is
    // the flag-injection boundary.
    for verb in verbs {
        cmd.arg(verb);
    }

    cmd.kill_on_drop(true);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let child = cmd.spawn().map_err(|e| PrefixDepsError::ProcessFailed {
        exit_code: None,
        stderr: format!("failed to spawn install process: {e}"),
    })?;

    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Create a fake shell script that acts as winetricks/protontricks.
    fn make_fake_binary(dir: &std::path::Path, name: &str, script: &str) -> String {
        let path = dir.join(name);
        fs::write(&path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).unwrap();
        }
        path.to_string_lossy().into_owned()
    }

    #[tokio::test]
    async fn check_installed_parses_whitespace_output() {
        let tmp = tempdir().unwrap();
        let binary = make_fake_binary(
            tmp.path(),
            "winetricks",
            "#!/bin/sh\necho 'vcrun2019 dotnet48'\n",
        );
        // Create a fake prefix directory (resolve_wine_prefix_path will look for pfx/).
        let pfx = tmp.path().join("pfx");
        fs::create_dir_all(&pfx).unwrap();

        let result = check_installed(
            &binary,
            tmp.path().to_str().unwrap(),
            PrefixDepsTool::Winetricks,
            None,
        )
        .await;
        assert!(result.is_ok(), "error: {:?}", result.err());
        let packages = result.unwrap();
        assert_eq!(packages, vec!["vcrun2019", "dotnet48"]);
    }

    #[tokio::test]
    async fn install_packages_rejects_invalid_verbs() {
        let tmp = tempdir().unwrap();
        let binary = make_fake_binary(tmp.path(), "winetricks", "#!/bin/sh\n");
        let pfx = tmp.path().join("pfx");
        fs::create_dir_all(&pfx).unwrap();

        let result = install_packages(
            &binary,
            tmp.path().to_str().unwrap(),
            &["-q".to_string()],
            PrefixDepsTool::Winetricks,
            None,
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, PrefixDepsError::ValidationError(_)),
            "expected ValidationError, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn install_packages_uses_winetricks_compatible_arguments() {
        let tmp = tempdir().unwrap();
        let captured_args = tmp.path().join("args.txt");
        let binary = make_fake_binary(
            tmp.path(),
            "winetricks",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit 0\n",
                captured_args.display()
            ),
        );
        let pfx = tmp.path().join("pfx");
        fs::create_dir_all(&pfx).unwrap();

        let mut child = install_packages(
            &binary,
            tmp.path().to_str().unwrap(),
            &["dotnet48".to_string()],
            PrefixDepsTool::Winetricks,
            None,
        )
        .await
        .unwrap();
        assert!(child.wait().await.unwrap().success());

        let args = fs::read_to_string(captured_args).unwrap();
        assert_eq!(args, "-q\ndotnet48\n");
    }

    #[tokio::test]
    async fn install_rejects_uninitialized_prefix() {
        let tmp = tempdir().unwrap();
        let binary = make_fake_binary(tmp.path(), "winetricks", "#!/bin/sh\n");
        // Do NOT create pfx/ directory -- use a nonexistent path entirely.

        let result = install_packages(
            &binary,
            "/nonexistent/path/that/does/not/exist",
            &["vcrun2019".to_string()],
            PrefixDepsTool::Winetricks,
            None,
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, PrefixDepsError::PrefixNotInitialized { .. }),
            "expected PrefixNotInitialized, got: {err:?}"
        );
    }

    #[test]
    fn sanitize_stderr_replaces_home_paths() {
        let raw = "error: failed to open /home/user/.wine/drive_c/file.dll: no such file";
        let sanitized = sanitize_stderr(raw);
        assert!(
            !sanitized.contains("/home/"),
            "still contains path: {sanitized}"
        );
        assert!(sanitized.contains("<path>"), "no replacement: {sanitized}");
    }

    #[test]
    fn sanitize_stderr_truncates_long_output() {
        let long = "error: ".to_string() + &"x".repeat(600);
        let sanitized = sanitize_stderr(&long);
        assert!(sanitized.ends_with("...(truncated)"));
        assert!(sanitized.len() <= 515); // 500 chars + "...(truncated)"
    }

    #[test]
    fn flatpak_install_delegates_to_host_with_wineprefix() {
        let tmp = tempdir().unwrap();
        let prefix = tmp.path().join("prefix");
        fs::create_dir_all(prefix.join("pfx")).unwrap();
        let (command, _resolved_prefix) =
            build_prefix_dep_command_with_platform("winetricks", prefix.to_str().unwrap(), true);

        let program = command.as_std().get_program().to_string_lossy();
        let args = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(program, "flatpak-spawn");
        assert_eq!(args[0], "--host");
        assert_eq!(args[1], "--clear-env");
        assert!(args.contains(&format!(
            "--env=WINEPREFIX={}",
            prefix.join("pfx").display()
        )));
        assert_eq!(args.last().map(String::as_str), Some("winetricks"));
    }
}
