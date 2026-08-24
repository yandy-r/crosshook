use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::process::{Child, ChildStderr, ChildStdout};

use super::models::{
    PrefixDependencyInstallOutcome, PrefixDepsError, PrefixVersionRestoreOutcome,
    PrefixVersionRestorePlan, PrefixVersionRestoreState,
};
use super::validation::validate_protontricks_verbs;
use super::PrefixDepsTool;
use crate::launch::runtime_helpers::{host_environment_map, resolve_wine_prefix_path};
use crate::platform::{
    host_command_with_env_and_directory_inner, host_command_with_env_and_directory_lifecycle_inner,
    is_flatpak, normalize_flatpak_host_path,
};

fn build_prefix_dep_command(
    binary_path: &str,
    prefix_path: &str,
) -> Result<(Command, PathBuf), PrefixDepsError> {
    build_prefix_dep_command_with_platform(binary_path, prefix_path, is_flatpak())
}

fn build_prefix_dep_command_with_platform(
    binary_path: &str,
    prefix_path: &str,
    flatpak: bool,
) -> Result<(Command, PathBuf), PrefixDepsError> {
    build_prefix_dep_command_with_mode(binary_path, prefix_path, flatpak, false)
}

fn build_prefix_dep_mutation_command(
    binary_path: &str,
    prefix_path: &str,
) -> Result<(Command, PathBuf), PrefixDepsError> {
    build_prefix_dep_mutation_command_with_platform(binary_path, prefix_path, is_flatpak())
}

fn build_prefix_dep_mutation_command_with_platform(
    binary_path: &str,
    prefix_path: &str,
    flatpak: bool,
) -> Result<(Command, PathBuf), PrefixDepsError> {
    build_prefix_dep_command_with_mode(binary_path, prefix_path, flatpak, true)
}

fn build_prefix_dep_command_with_mode(
    binary_path: &str,
    prefix_path: &str,
    flatpak: bool,
    lifecycle_bound: bool,
) -> Result<(Command, PathBuf), PrefixDepsError> {
    let resolved_prefix = resolved_prefix_path(prefix_path)?;
    let normalized_binary = normalize_flatpak_host_path(binary_path);
    let mut environment = host_environment_map();
    environment.insert(
        "WINEPREFIX".to_string(),
        resolved_prefix.to_string_lossy().into_owned(),
    );
    let command = if lifecycle_bound {
        host_command_with_env_and_directory_lifecycle_inner(
            &normalized_binary,
            &environment,
            None,
            flatpak,
            &BTreeMap::new(),
        )
    } else {
        host_command_with_env_and_directory_inner(
            &normalized_binary,
            &environment,
            None,
            flatpak,
            &BTreeMap::new(),
        )
    };
    Ok((command, resolved_prefix))
}

fn resolved_prefix_path(prefix_path: &str) -> Result<PathBuf, PrefixDepsError> {
    let normalized_prefix = normalize_flatpak_host_path(prefix_path);
    let resolved_prefix = resolve_wine_prefix_path(Path::new(&normalized_prefix));
    if !resolved_prefix.is_dir() {
        return Err(PrefixDepsError::PrefixNotInitialized {
            path: resolved_prefix.to_string_lossy().into_owned(),
        });
    }
    fs::canonicalize(&resolved_prefix).map_err(|error| {
        PrefixDepsError::ValidationError(format!(
            "could not canonicalize prefix path {}: {error}",
            resolved_prefix.display()
        ))
    })
}

/// Normalize a host-visible prefix path and resolve its effective `pfx` directory.
pub fn normalize_prefix_path(prefix_path: &str) -> Result<String, PrefixDepsError> {
    Ok(resolved_prefix_path(prefix_path)?
        .to_string_lossy()
        .into_owned())
}

/// Default timeout for check operations (seconds).
const CHECK_TIMEOUT_SECS: u64 = 30;
/// Maximum time allowed for compatibility restoration in production.
const RESTORE_TIMEOUT_SECS: u64 = 90;

#[derive(Debug, Default, PartialEq, Eq)]
struct WindowsVersionRegistryFields {
    current_build: Option<String>,
    current_version: Option<String>,
    product_name: Option<String>,
}

fn snapshot_prefix_windows_version(prefix_path: &Path) -> Result<String, PrefixDepsError> {
    let registry_path = prefix_path.join("system.reg");
    let registry = fs::read_to_string(&registry_path).map_err(|error| {
        PrefixDepsError::ValidationError(format!(
            "could not read prefix Windows version registry: {error}"
        ))
    })?;
    let mut fields = WindowsVersionRegistryFields::default();
    let mut in_current_version = false;

    for line in registry.lines() {
        if line.starts_with('[') {
            in_current_version =
                line.starts_with(r#"[Software\\Microsoft\\Windows NT\\CurrentVersion]"#);
            continue;
        }
        if !in_current_version {
            continue;
        }

        if let Some(value) = parse_registry_string(line, "CurrentBuild") {
            fields.current_build = Some(value.to_string());
        } else if let Some(value) = parse_registry_string(line, "CurrentVersion") {
            fields.current_version = Some(value.to_string());
        } else if let Some(value) = parse_registry_string(line, "ProductName") {
            fields.product_name = Some(value.to_string());
        }
    }

    windows_version_verb(&fields)
        .map(str::to_string)
        .ok_or_else(|| {
            PrefixDepsError::ValidationError(
            "could not derive a supported Windows compatibility version from the prefix registry"
                .to_string(),
        )
        })
}

fn parse_registry_string<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let value = line.strip_prefix(&format!(r#""{key}"="#))?;
    value.strip_prefix('"')?.strip_suffix('"')
}

fn windows_version_verb(fields: &WindowsVersionRegistryFields) -> Option<&'static str> {
    let product_name = fields.product_name.as_deref().map(str::to_ascii_lowercase);
    if let Some(product_name) = product_name.as_deref() {
        if product_name.contains("server 2008 r2") {
            return Some("win2k8r2");
        }
        if product_name.contains("server 2008") {
            return Some("win2k8");
        }
        if product_name.contains("server 2003") {
            return Some("win2k3");
        }
    }

    if let Some(build) = fields
        .current_build
        .as_deref()
        .and_then(|build| build.parse::<u32>().ok())
    {
        match build {
            22_000.. => return Some("win11"),
            10_240.. => return Some("win10"),
            9_600 => return Some("win81"),
            9_200 => return Some("win8"),
            7_600 | 7_601 => return Some("win7"),
            _ => {}
        }
    }

    if let Some(product_name) = product_name.as_deref() {
        let verb = if product_name.contains("windows 8.1") {
            Some("win81")
        } else if product_name.contains("windows 8") {
            Some("win8")
        } else if product_name.contains("windows 7") {
            Some("win7")
        } else if product_name.contains("windows vista") {
            Some("vista")
        } else if product_name.contains("windows xp") {
            Some("winxp")
        } else if product_name.contains("windows 2000") {
            Some("win2k")
        } else {
            None
        };
        if verb.is_some() {
            return verb;
        }
    }

    match fields.current_version.as_deref() {
        Some("6.3") => Some("win81"),
        Some("6.2") => Some("win8"),
        Some("6.1") => Some("win7"),
        Some("6.0") => Some("vista"),
        Some("5.2") => Some("win2k3"),
        Some("5.1") => Some("winxp"),
        Some("5.0") => Some("win2k"),
        _ => None,
    }
}

fn is_windows_version_verb(verb: &str) -> bool {
    matches!(
        verb,
        "nt351"
            | "nt40"
            | "vista"
            | "win10"
            | "win11"
            | "win20"
            | "win2k"
            | "win2k3"
            | "win2k8"
            | "win2k8r2"
            | "win30"
            | "win31"
            | "win7"
            | "win8"
            | "win81"
            | "win95"
            | "win98"
            | "winme"
            | "winxp"
    )
}

/// Validated dependency installation that has not spawned a process yet.
///
/// Callers can persist `restore_plan()` before consuming this value with
/// `spawn()`, ensuring crash recovery is durable before prefix mutation begins.
#[derive(Debug)]
pub struct PreparedPrefixDependencyInstall {
    command: Command,
    restore_plan: Option<PrefixVersionRestorePlan>,
    restore_timeout: Duration,
}

impl PreparedPrefixDependencyInstall {
    pub fn restore_plan(&self) -> Option<&PrefixVersionRestorePlan> {
        self.restore_plan.as_ref()
    }

    pub fn spawn(mut self) -> Result<PrefixDependencyInstall, PrefixDepsError> {
        let child = self
            .command
            .spawn()
            .map_err(|error| PrefixDepsError::ProcessFailed {
                exit_code: None,
                stderr: format!("failed to spawn install process: {error}"),
            })?;

        Ok(PrefixDependencyInstall {
            child,
            restore_plan: self.restore_plan,
            restore_timeout: self.restore_timeout,
        })
    }

    #[cfg(test)]
    fn spawn_with_restore_timeout(
        mut self,
        restore_timeout: Duration,
    ) -> Result<PrefixDependencyInstall, PrefixDepsError> {
        self.restore_timeout = restore_timeout;
        self.spawn()
    }
}

/// Running prefix-dependency installation with an optional Windows-version restore.
#[derive(Debug)]
pub struct PrefixDependencyInstall {
    child: Child,
    restore_plan: Option<PrefixVersionRestorePlan>,
    restore_timeout: Duration,
}

impl PrefixDependencyInstall {
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    /// Wait for installation and restore the prefix's original Windows version.
    ///
    /// Restoration runs even when dependency installation exits unsuccessfully,
    /// and the two results remain separate for persistence and UI reporting.
    pub async fn wait_and_restore(mut self) -> PrefixDependencyInstallOutcome {
        let install_result = self.child.wait().await;
        let (install_succeeded, install_exit_code, install_error) = match install_result {
            Ok(status) => (status.success(), status.code(), None),
            Err(error) => (
                false,
                None,
                Some(sanitize_stderr(&format!(
                    "failed to wait for dependency install: {error}"
                ))),
            ),
        };

        let restore = match self.restore_plan.as_ref() {
            Some(plan) => {
                restore_prefix_windows_version_with_timeout(plan, self.restore_timeout).await
            }
            None => PrefixVersionRestoreOutcome {
                state: PrefixVersionRestoreState::NotRequired,
                error: None,
            },
        };

        PrefixDependencyInstallOutcome {
            install_succeeded,
            install_exit_code,
            install_error,
            restore_state: restore.state,
            restore_error: restore.error,
        }
    }
}

/// Restore and verify a prefix's captured Windows compatibility tier.
pub async fn restore_prefix_windows_version(
    plan: &PrefixVersionRestorePlan,
) -> PrefixVersionRestoreOutcome {
    restore_prefix_windows_version_with_timeout(plan, Duration::from_secs(RESTORE_TIMEOUT_SECS))
        .await
}

async fn restore_prefix_windows_version_with_timeout(
    plan: &PrefixVersionRestorePlan,
    timeout: Duration,
) -> PrefixVersionRestoreOutcome {
    if !is_windows_version_verb(&plan.restore_verb) {
        return failed_restore("invalid Windows version restore verb".to_string());
    }

    let (mut cmd, resolved_prefix) =
        match build_prefix_dep_mutation_command(&plan.binary_path, &plan.prefix_path) {
            Ok(command) => command,
            Err(error) => return failed_restore(error.to_string()),
        };
    if matches!(plan.tool_type, PrefixDepsTool::Protontricks) {
        let Some(app_id) = plan.steam_app_id.as_deref() else {
            return failed_restore(
                "steam app id is required to restore the prefix Windows version".to_string(),
            );
        };
        cmd.arg(app_id);
    }
    cmd.arg("-q").arg(&plan.restore_verb);
    cmd.kill_on_drop(true);
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) => {
            return failed_restore(sanitize_stderr(&format!(
                "failed to spawn Windows version restore: {error}"
            )));
        }
    };
    let stderr = child.stderr.take();
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        if let Some(mut stderr) = stderr {
            let _ = stderr.read_to_end(&mut bytes).await;
        }
        bytes
    });

    let status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => {
            return failed_restore(sanitize_stderr(&format!(
                "failed to wait for Windows version restore: {error}"
            )));
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return failed_restore(format!(
                "restoring Windows version {} timed out after {} seconds",
                plan.restore_verb,
                timeout.as_secs_f64()
            ));
        }
    };
    let stderr = stderr_task.await.unwrap_or_default();

    if !status.success() {
        let stderr = sanitize_stderr(&String::from_utf8_lossy(&stderr));
        return failed_restore(format!(
            "restoring Windows version {} failed (exit code {:?}): {}",
            plan.restore_verb,
            status.code(),
            stderr
        ));
    }

    match snapshot_prefix_windows_version(&resolved_prefix) {
        Ok(actual) if actual == plan.restore_verb => PrefixVersionRestoreOutcome {
            state: PrefixVersionRestoreState::Succeeded,
            error: None,
        },
        Ok(actual) => failed_restore(format!(
            "Windows version restore verification failed: expected {}, found {actual}",
            plan.restore_verb
        )),
        Err(error) => failed_restore(format!(
            "Windows version restore verification failed: {error}"
        )),
    }
}

fn failed_restore(error: String) -> PrefixVersionRestoreOutcome {
    PrefixVersionRestoreOutcome {
        state: PrefixVersionRestoreState::Failed,
        error: Some(sanitize_stderr(&error)),
    }
}

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
    let (mut cmd, _resolved_prefix) = build_prefix_dep_command(binary_path, prefix_path)?;
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

/// Prepare an install process without spawning it.
///
/// The caller must persist `restore_plan()` before calling `spawn()` so an
/// interrupted process remains recoverable after restart.
///
/// Security checklist:
/// - validate_protontricks_verbs() called first
/// - Per-verb .arg() calls (never joined)
/// - Flag-like and malformed verbs rejected before argv construction
/// - apply_host_environment() used (NOT env_clear())
/// - .kill_on_drop(true)
/// - Prefix path normalized via resolve_wine_prefix_path()
/// - pfx/ existence check
pub fn prepare_install_packages(
    binary_path: &str,
    prefix_path: &str,
    verbs: &[String],
    tool_type: PrefixDepsTool,
    steam_app_id: Option<&str>,
) -> Result<PreparedPrefixDependencyInstall, PrefixDepsError> {
    // Validate verbs first (security gate).
    validate_protontricks_verbs(verbs)?;

    let (mut cmd, resolved_prefix) = build_prefix_dep_mutation_command(binary_path, prefix_path)?;

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

    let restore_plan = if verbs.iter().any(|verb| is_windows_version_verb(verb)) {
        None
    } else {
        Some(PrefixVersionRestorePlan {
            binary_path: normalize_flatpak_host_path(binary_path),
            prefix_path: resolved_prefix.to_string_lossy().into_owned(),
            tool_type,
            steam_app_id: steam_app_id.map(str::to_string),
            restore_verb: snapshot_prefix_windows_version(&resolved_prefix)?,
        })
    };

    cmd.kill_on_drop(true);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    Ok(PreparedPrefixDependencyInstall {
        command: cmd,
        restore_plan,
        restore_timeout: Duration::from_secs(RESTORE_TIMEOUT_SECS),
    })
}

/// Compatibility wrapper for callers that do not need to persist a repair plan.
///
/// New application integrations must use `prepare_install_packages`, persist
/// the exposed restore plan, and only then call `spawn`.
pub async fn install_packages(
    binary_path: &str,
    prefix_path: &str,
    verbs: &[String],
    tool_type: PrefixDepsTool,
    steam_app_id: Option<&str>,
) -> Result<PrefixDependencyInstall, PrefixDepsError> {
    prepare_install_packages(binary_path, prefix_path, verbs, tool_type, steam_app_id)?.spawn()
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

    #[test]
    fn install_packages_uses_winetricks_compatible_arguments() {
        let tmp = tempdir().unwrap();
        let binary = make_fake_binary(tmp.path(), "winetricks", "#!/bin/sh\nexit 0\n");
        let pfx = tmp.path().join("pfx");
        write_registry_fixture(
            &pfx,
            "\"CurrentBuild\"=\"19045\"\n\"ProductName\"=\"Microsoft Windows 10\"",
        );

        let prepared = prepare_install_packages(
            &binary,
            tmp.path().to_str().unwrap(),
            &["dotnet48".to_string()],
            PrefixDepsTool::Winetricks,
            None,
        )
        .unwrap();

        let args = prepared
            .command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(args, vec!["-q", "dotnet48"]);
    }

    fn write_registry_fixture(prefix: &Path, body: &str) {
        fs::create_dir_all(prefix).unwrap();
        fs::write(
            prefix.join("system.reg"),
            format!(
                "WINE REGISTRY Version 2\n\n[Software\\\\Microsoft\\\\Windows NT\\\\CurrentVersion] 123\n{body}\n\n[System\\\\CurrentControlSet] 124\n"
            ),
        )
        .unwrap();
    }

    fn make_install_restore_binary(
        dir: &Path,
        captured_calls: &Path,
        install_exit: i32,
        restore_exit: i32,
        restore_delay_secs: u64,
        update_registry_on_restore: bool,
    ) -> String {
        let restore_registry = if update_registry_on_restore {
            r#"printf '%s\n' 'WINE REGISTRY Version 2' '' '[Software\\Microsoft\\Windows NT\\CurrentVersion] 123' '"CurrentBuild"="19045"' '"CurrentVersion"="6.3"' '"ProductName"="Microsoft Windows 10"' > "$WINEPREFIX/system.reg""#
        } else {
            ":"
        };
        make_fake_binary(
            dir,
            "winetricks",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nif [ \"$2\" = \"dotnet48\" ]; then exit {install_exit}; fi\nif [ \"$2\" = \"win10\" ]; then sleep {restore_delay_secs}; {restore_registry}; exit {restore_exit}; fi\nexit 0\n",
                captured_calls.display()
            ),
        )
    }

    #[test]
    fn snapshots_windows_7_from_prefix_registry() {
        let tmp = tempdir().unwrap();
        let pfx = tmp.path().join("pfx");
        write_registry_fixture(
            &pfx,
            "\"CurrentBuild\"=\"7601\"\n\"CurrentVersion\"=\"6.1\"\n\"ProductName\"=\"Microsoft Windows 7\"",
        );

        assert_eq!(snapshot_prefix_windows_version(&pfx).unwrap(), "win7");
    }

    #[test]
    fn snapshots_windows_10_from_prefix_registry() {
        let tmp = tempdir().unwrap();
        let pfx = tmp.path().join("pfx");
        write_registry_fixture(
            &pfx,
            "\"CurrentBuild\"=\"19045\"\n\"CurrentVersion\"=\"6.3\"\n\"ProductName\"=\"Microsoft Windows 10\"",
        );

        assert_eq!(snapshot_prefix_windows_version(&pfx).unwrap(), "win10");
    }

    #[test]
    fn snapshots_windows_11_from_build_when_product_name_says_windows_10() {
        let tmp = tempdir().unwrap();
        let pfx = tmp.path().join("pfx");
        write_registry_fixture(
            &pfx,
            "\"CurrentBuild\"=\"22000\"\n\"CurrentVersion\"=\"6.3\"\n\"ProductName\"=\"Microsoft Windows 10\"",
        );

        assert_eq!(snapshot_prefix_windows_version(&pfx).unwrap(), "win11");
    }

    #[test]
    fn snapshots_server_2008_r2_before_ambiguous_windows_7_build() {
        let tmp = tempdir().unwrap();
        let pfx = tmp.path().join("pfx");
        write_registry_fixture(
            &pfx,
            "\"CurrentBuild\"=\"7601\"\n\"CurrentVersion\"=\"6.1\"\n\"ProductName\"=\"Microsoft Windows Server 2008 R2\"",
        );

        assert_eq!(snapshot_prefix_windows_version(&pfx).unwrap(), "win2k8r2");
    }

    #[test]
    fn rejects_unknown_prefix_windows_version() {
        let tmp = tempdir().unwrap();
        let pfx = tmp.path().join("pfx");
        write_registry_fixture(
            &pfx,
            "\"CurrentBuild\"=\"1234\"\n\"CurrentVersion\"=\"1.2\"\n\"ProductName\"=\"Unknown Windows\"",
        );

        assert!(matches!(
            snapshot_prefix_windows_version(&pfx),
            Err(PrefixDepsError::ValidationError(_))
        ));
    }

    #[tokio::test]
    async fn prepared_install_exposes_restore_plan_before_spawn() {
        let tmp = tempdir().unwrap();
        let captured_calls = tmp.path().join("calls.txt");
        let binary = make_install_restore_binary(tmp.path(), &captured_calls, 0, 0, 0, true);
        let pfx = tmp.path().join("pfx");
        write_registry_fixture(
            &pfx,
            "\"CurrentBuild\"=\"19045\"\n\"ProductName\"=\"Microsoft Windows 10\"",
        );

        let prepared = prepare_install_packages(
            &binary,
            tmp.path().to_str().unwrap(),
            &["dotnet48".to_string()],
            PrefixDepsTool::Winetricks,
            None,
        )
        .unwrap();
        let plan = prepared.restore_plan().expect("restore plan");
        assert_eq!(plan.restore_verb, "win10");
        assert_eq!(plan.prefix_path, pfx.to_string_lossy());
        assert!(!captured_calls.exists(), "preparation must not spawn");

        let child = prepared.spawn().unwrap();
        let outcome = child.wait_and_restore().await;
        assert!(outcome.install_succeeded);
        assert_eq!(outcome.install_exit_code, Some(0));
        assert_eq!(outcome.restore_state, PrefixVersionRestoreState::Succeeded);
        assert_eq!(outcome.restore_error, None);

        assert_eq!(
            fs::read_to_string(captured_calls).unwrap(),
            "-q dotnet48\n-q win10\n"
        );
    }

    #[tokio::test]
    async fn failed_dependency_install_still_restores_prefix_windows_version() {
        let tmp = tempdir().unwrap();
        let captured_calls = tmp.path().join("calls.txt");
        let binary = make_install_restore_binary(tmp.path(), &captured_calls, 42, 0, 0, true);
        let pfx = tmp.path().join("pfx");
        write_registry_fixture(
            &pfx,
            "\"CurrentBuild\"=\"19045\"\n\"ProductName\"=\"Microsoft Windows 10\"",
        );

        let prepared = prepare_install_packages(
            &binary,
            tmp.path().to_str().unwrap(),
            &["dotnet48".to_string()],
            PrefixDepsTool::Winetricks,
            None,
        )
        .unwrap();
        let outcome = prepared.spawn().unwrap().wait_and_restore().await;
        assert!(!outcome.install_succeeded);
        assert_eq!(outcome.install_exit_code, Some(42));
        assert_eq!(outcome.restore_state, PrefixVersionRestoreState::Succeeded);

        assert_eq!(
            fs::read_to_string(captured_calls).unwrap(),
            "-q dotnet48\n-q win10\n"
        );
    }

    #[tokio::test]
    async fn restoration_timeout_is_reported_separately() {
        let tmp = tempdir().unwrap();
        let captured_calls = tmp.path().join("calls.txt");
        let binary = make_install_restore_binary(tmp.path(), &captured_calls, 0, 0, 2, true);
        let pfx = tmp.path().join("pfx");
        write_registry_fixture(
            &pfx,
            "\"CurrentBuild\"=\"19045\"\n\"ProductName\"=\"Microsoft Windows 10\"",
        );

        let prepared = prepare_install_packages(
            &binary,
            tmp.path().to_str().unwrap(),
            &["dotnet48".to_string()],
            PrefixDepsTool::Winetricks,
            None,
        )
        .unwrap();
        let child = prepared
            .spawn_with_restore_timeout(Duration::from_millis(50))
            .unwrap();
        let outcome = child.wait_and_restore().await;
        assert!(outcome.install_succeeded);
        assert_eq!(outcome.restore_state, PrefixVersionRestoreState::Failed);
        assert!(outcome
            .restore_error
            .as_deref()
            .is_some_and(|error| error.contains("timed out")));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn dropping_install_handle_kills_lifecycle_bound_process() {
        let tmp = tempdir().unwrap();
        let pid_file = tmp.path().join("pid.txt");
        let binary = make_fake_binary(
            tmp.path(),
            "winetricks",
            &format!(
                "#!/bin/sh\nprintf '%s' \"$$\" > '{}'\nexec sleep 60\n",
                pid_file.display()
            ),
        );
        let pfx = tmp.path().join("pfx");
        fs::create_dir_all(&pfx).unwrap();

        let install = prepare_install_packages(
            &binary,
            tmp.path().to_str().unwrap(),
            &["win10".to_string()],
            PrefixDepsTool::Winetricks,
            None,
        )
        .unwrap()
        .spawn()
        .unwrap();

        let pid = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Ok(raw) = fs::read_to_string(&pid_file) {
                    break raw.parse::<u32>().unwrap();
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("fake dependency process did not write its pid");

        drop(install);

        tokio::time::timeout(Duration::from_secs(1), async {
            while Path::new(&format!("/proc/{pid}")).exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("dependency process survived handle drop");
    }

    #[tokio::test]
    async fn restoration_verification_mismatch_is_reported_as_failure() {
        let tmp = tempdir().unwrap();
        let captured_calls = tmp.path().join("calls.txt");
        let binary = make_install_restore_binary(tmp.path(), &captured_calls, 0, 0, 0, false);
        let pfx = tmp.path().join("pfx");
        write_registry_fixture(
            &pfx,
            "\"CurrentBuild\"=\"19045\"\n\"ProductName\"=\"Microsoft Windows 10\"",
        );

        let prepared = prepare_install_packages(
            &binary,
            tmp.path().to_str().unwrap(),
            &["dotnet48".to_string()],
            PrefixDepsTool::Winetricks,
            None,
        )
        .unwrap();
        write_registry_fixture(
            &pfx,
            "\"CurrentBuild\"=\"7601\"\n\"ProductName\"=\"Microsoft Windows 7\"",
        );

        let outcome = prepared.spawn().unwrap().wait_and_restore().await;
        assert_eq!(outcome.restore_state, PrefixVersionRestoreState::Failed);
        assert!(outcome
            .restore_error
            .as_deref()
            .is_some_and(|error| error.contains("verification")));
    }

    #[tokio::test]
    async fn explicit_windows_version_verb_has_no_restore_plan() {
        let tmp = tempdir().unwrap();
        let captured_calls = tmp.path().join("calls.txt");
        let binary = make_fake_binary(
            tmp.path(),
            "winetricks",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 0\n",
                captured_calls.display()
            ),
        );
        let pfx = tmp.path().join("pfx");
        fs::create_dir_all(&pfx).unwrap();

        let prepared = prepare_install_packages(
            &binary,
            tmp.path().to_str().unwrap(),
            &["win7".to_string()],
            PrefixDepsTool::Winetricks,
            None,
        )
        .unwrap();
        assert!(prepared.restore_plan().is_none());
        let outcome = prepared.spawn().unwrap().wait_and_restore().await;
        assert!(outcome.install_succeeded);
        assert_eq!(
            outcome.restore_state,
            PrefixVersionRestoreState::NotRequired
        );

        assert_eq!(fs::read_to_string(captured_calls).unwrap(), "-q win7\n");
    }

    #[tokio::test]
    async fn restart_restore_rejects_invalid_restore_verb_before_spawn() {
        let tmp = tempdir().unwrap();
        let captured_calls = tmp.path().join("calls.txt");
        let binary = make_fake_binary(
            tmp.path(),
            "winetricks",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 0\n",
                captured_calls.display()
            ),
        );
        let pfx = tmp.path().join("pfx");
        fs::create_dir_all(&pfx).unwrap();
        let plan = PrefixVersionRestorePlan {
            binary_path: binary,
            prefix_path: pfx.to_string_lossy().into_owned(),
            tool_type: PrefixDepsTool::Winetricks,
            steam_app_id: None,
            restore_verb: "-q".to_string(),
        };

        let outcome = restore_prefix_windows_version(&plan).await;

        assert_eq!(outcome.state, PrefixVersionRestoreState::Failed);
        assert!(!captured_calls.exists(), "invalid restore must not spawn");
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
    fn flatpak_install_delegates_to_lifecycle_bound_host_with_wineprefix() {
        let tmp = tempdir().unwrap();
        let prefix = tmp.path().join("prefix");
        fs::create_dir_all(prefix.join("pfx")).unwrap();
        let (command, _resolved_prefix) = build_prefix_dep_mutation_command_with_platform(
            "winetricks",
            prefix.to_str().unwrap(),
            true,
        )
        .unwrap();

        let program = command.as_std().get_program().to_string_lossy();
        let args = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(program, "flatpak-spawn");
        assert_eq!(args[0], "--host");
        assert_eq!(args[1], "--watch-bus");
        assert_eq!(args[2], "--clear-env");
        assert!(args.contains(&format!(
            "--env=WINEPREFIX={}",
            prefix.join("pfx").display()
        )));
        assert_eq!(args.last().map(String::as_str), Some("winetricks"));
    }

    #[test]
    fn normalize_prefix_path_returns_resolved_pfx_directory() {
        let tmp = tempdir().unwrap();
        let pfx = tmp.path().join("pfx");
        fs::create_dir_all(&pfx).unwrap();

        assert_eq!(
            normalize_prefix_path(tmp.path().to_str().unwrap()).unwrap(),
            pfx.to_string_lossy()
        );
        assert_eq!(
            normalize_prefix_path(pfx.to_str().unwrap()).unwrap(),
            pfx.to_string_lossy()
        );
    }

    #[cfg(unix)]
    #[test]
    fn normalize_prefix_path_canonicalizes_symlink_aliases() {
        use std::os::unix::fs::symlink;

        let tmp = tempdir().unwrap();
        let prefix = tmp.path().join("prefix");
        let pfx = prefix.join("pfx");
        fs::create_dir_all(&pfx).unwrap();
        let alias = tmp.path().join("alias");
        symlink(&prefix, &alias).unwrap();

        assert_eq!(
            normalize_prefix_path(alias.to_str().unwrap()).unwrap(),
            fs::canonicalize(&pfx).unwrap().to_string_lossy()
        );
    }

    #[test]
    fn normalize_prefix_path_canonicalizes_parent_segments() {
        let tmp = tempdir().unwrap();
        let base = tmp.path().join("base");
        let prefix = base.join("prefix");
        let pfx = prefix.join("pfx");
        fs::create_dir_all(&pfx).unwrap();
        fs::create_dir_all(base.join("nested")).unwrap();
        let alias = base.join("nested").join("..").join("prefix");

        assert_eq!(
            normalize_prefix_path(alias.to_str().unwrap()).unwrap(),
            fs::canonicalize(&pfx).unwrap().to_string_lossy()
        );
    }
}
