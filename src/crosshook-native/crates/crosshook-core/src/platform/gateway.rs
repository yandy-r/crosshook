use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command as StdCommand, Stdio};

use tokio::process::Command;
use uuid::Uuid;

use super::detect::{is_flatpak, normalize_flatpak_host_path};
use super::host_fs::is_executable_file_sync;

/// Creates a [`tokio::process::Command`] that executes on the host when
/// running inside a Flatpak sandbox, and as a normal child process otherwise.
///
/// Inside Flatpak the returned command is equivalent to
/// `flatpak-spawn --host <program>`; outside Flatpak it is `Command::new(program)`.
///
/// # Warning — env vars are silently dropped inside Flatpak
///
/// **Do NOT call `.env()` or `.envs()` on the `Command` returned by this
/// function when running inside Flatpak.** `flatpak-spawn --host` does not
/// forward env vars set on the `Command` object; they are silently ignored.
/// Any env vars set this way will be absent from the spawned process, causing
/// subtly wrong behaviour for Proton/Wine callers that depend on
/// `STEAM_COMPAT_*`, `WINEPREFIX`, `MANGOHUD_CONFIG`, etc.
///
/// **Use [`host_command_with_env`] instead**, which threads env vars through
/// `--env=KEY=VALUE` arguments automatically when inside Flatpak and falls
/// back to `.envs()` outside Flatpak.
pub fn host_command(program: &str) -> Command {
    host_command_with(program, is_flatpak())
}

/// Creates a [`tokio::process::Command`] that executes on the host when
/// running inside a Flatpak sandbox, and as a normal child process otherwise,
/// forwarding the given environment variables correctly in both cases.
///
/// Inside Flatpak, `flatpak-spawn --host` does not propagate env vars set via
/// `.env()` / `.envs()` on the `Command`. This helper threads every entry from
/// `envs` through explicit `--env=KEY=VALUE` arguments before the program name.
/// It also uses `--clear-env` so the host child does not inherit sandbox-only
/// variables that can poison Proton/Wine launches.
/// Outside Flatpak the vars are forwarded normally via `.envs()`.
///
/// Phase 3 Proton/Wine callers should use this helper unconditionally so the
/// code behaves correctly in both AppImage and Flatpak deployments.
/// `custom_env_vars` are user-controlled launch overrides. Under Flatpak they
/// are applied via a short-lived `0600` env file and `bash` so values are not
/// exposed on the `flatpak-spawn --env=` argv.
pub fn host_command_with_env(
    program: &str,
    envs: &BTreeMap<String, String>,
    custom_env_vars: &BTreeMap<String, String>,
) -> Command {
    host_command_with_env_inner(program, envs, custom_env_vars, is_flatpak())
}

/// Like [`host_command_with_env`], but also sets the host working directory
/// explicitly via Flatpak's documented `--directory=DIR` option.
pub fn host_command_with_env_and_directory(
    program: &str,
    envs: &BTreeMap<String, String>,
    directory: Option<&str>,
    custom_env_vars: &BTreeMap<String, String>,
) -> Command {
    host_command_with_env_and_directory_inner(
        program,
        envs,
        directory,
        is_flatpak(),
        custom_env_vars,
    )
}

pub(crate) fn host_command_with(program: &str, flatpak: bool) -> Command {
    if flatpak {
        tracing::debug!(program, "wrapping command with flatpak-spawn --host");
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg(program);
        cmd
    } else {
        Command::new(program)
    }
}

pub(crate) fn host_command_with_env_inner(
    program: &str,
    envs: &BTreeMap<String, String>,
    custom_env_vars: &BTreeMap<String, String>,
    flatpak: bool,
) -> Command {
    host_command_with_env_and_directory_inner(program, envs, None, flatpak, custom_env_vars)
}

pub(crate) fn host_command_with_env_and_directory_inner(
    program: &str,
    envs: &BTreeMap<String, String>,
    directory: Option<&str>,
    flatpak: bool,
    custom_env_vars: &BTreeMap<String, String>,
) -> Command {
    let normalized_directory = normalize_host_working_directory(directory);
    if flatpak {
        tracing::debug!(
            program,
            "wrapping command with flatpak-spawn --host (with env)"
        );
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg("--clear-env");
        if let Some(directory) = normalized_directory.as_deref() {
            cmd.arg(format!("--directory={directory}"));
        }
        for (key, value) in envs {
            cmd.arg(format!("--env={key}={value}"));
        }
        if custom_env_vars.is_empty() {
            cmd.arg(program);
        } else {
            let env_path = match write_flatpak_custom_env_file(custom_env_vars) {
                Ok(path) => path,
                Err(error) => {
                    tracing::error!(
                        ?error,
                        "failed to write custom env file for flatpak host spawn"
                    );
                    cmd.arg(program);
                    return cmd;
                }
            };
            cmd.arg("bash");
            cmd.arg("-c");
            cmd.arg("set -a; source \"$1\"; rm -f \"$1\"; set +a; shift; exec \"$@\"");
            cmd.arg("bash");
            cmd.arg(env_path);
            cmd.arg(program);
        }
        cmd
    } else {
        let mut combined = envs.clone();
        for (key, value) in custom_env_vars {
            combined.insert(key.clone(), value.clone());
        }
        let mut cmd = Command::new(program);
        cmd.envs(&combined);
        if let Some(directory) = normalized_directory {
            cmd.current_dir(directory);
        }
        cmd
    }
}

fn is_valid_shell_env_key(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn shell_single_quote_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn flatpak_custom_env_directory_with(
    xdg_cache_home: Option<PathBuf>,
    home_path: Option<PathBuf>,
) -> PathBuf {
    if let Some(path) = xdg_cache_home {
        return path.join("crosshook");
    }
    if let Some(path) = home_path {
        return path.join(".cache").join("crosshook");
    }
    std::env::temp_dir().join("crosshook")
}

fn write_flatpak_custom_env_file(
    custom_env_vars: &BTreeMap<String, String>,
) -> io::Result<PathBuf> {
    let target_directory = flatpak_custom_env_directory_with(
        std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    );
    std::fs::create_dir_all(&target_directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&target_directory)?.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&target_directory, permissions)?;
    }
    let path = target_directory.join(format!("crosshook-host-env-{}.env", Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .truncate(true)
        .open(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(0o600);
        file.set_permissions(perms)?;
    }
    for (key, value) in custom_env_vars {
        if !is_valid_shell_env_key(key) {
            tracing::warn!(
                key = %key,
                "skipping invalid custom env key for flatpak host env file handoff"
            );
            continue;
        }
        writeln!(file, "{}={}", key, shell_single_quote_escape(value))?;
    }
    file.sync_all()?;
    Ok(path)
}

fn normalize_host_working_directory(directory: Option<&str>) -> Option<String> {
    let directory = directory?;
    let normalized = normalize_flatpak_host_path(directory);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Sync [`std::process::Command`] that runs on the host under Flatpak (see [`host_command`]).
///
/// Do not call `.env()` / `.envs()` after construction in Flatpak; use
/// [`host_std_command_with_env`] instead.
pub fn host_std_command(program: &str) -> StdCommand {
    host_std_command_with(program, is_flatpak())
}

pub(crate) fn host_std_command_with(program: &str, flatpak: bool) -> StdCommand {
    if flatpak {
        tracing::debug!(program, "wrapping std command with flatpak-spawn --host");
        let mut cmd = StdCommand::new("flatpak-spawn");
        cmd.arg("--host").arg(program);
        cmd
    } else {
        StdCommand::new(program)
    }
}

/// Like [`host_command_with_env`], but for synchronous [`std::process::Command`].
pub fn host_std_command_with_env(
    program: &str,
    envs: &BTreeMap<String, String>,
    custom_env_vars: &BTreeMap<String, String>,
) -> StdCommand {
    host_std_command_with_env_inner(program, envs, custom_env_vars, is_flatpak())
}

pub(crate) fn host_std_command_with_env_inner(
    program: &str,
    envs: &BTreeMap<String, String>,
    custom_env_vars: &BTreeMap<String, String>,
    flatpak: bool,
) -> StdCommand {
    if flatpak {
        tracing::debug!(
            program,
            "wrapping std command with flatpak-spawn --host (with env)"
        );
        let mut cmd = StdCommand::new("flatpak-spawn");
        cmd.arg("--host").arg("--clear-env");
        for (key, value) in envs {
            cmd.arg(format!("--env={key}={value}"));
        }
        if custom_env_vars.is_empty() {
            cmd.arg(program);
        } else {
            let env_path = match write_flatpak_custom_env_file(custom_env_vars) {
                Ok(path) => path,
                Err(error) => {
                    tracing::error!(
                        ?error,
                        "failed to write custom env file for flatpak host spawn"
                    );
                    cmd.arg(program);
                    return cmd;
                }
            };
            cmd.arg("bash");
            cmd.arg("-c");
            cmd.arg("set -a; source \"$1\"; rm -f \"$1\"; set +a; shift; exec \"$@\"");
            cmd.arg("bash");
            cmd.arg(env_path);
            cmd.arg(program);
        }
        cmd
    } else {
        let mut combined = envs.clone();
        for (key, value) in custom_env_vars {
            combined.insert(key.clone(), value.clone());
        }
        let mut cmd = StdCommand::new(program);
        cmd.envs(&combined);
        cmd
    }
}

/// Returns true when `name` is a single PATH-style binary name (no `/`, no shell metacharacters).
pub(crate) fn is_safe_host_path_lookup_name(name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() || name.contains('/') || name.contains("..") {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
}

/// Returns whether `binary` exists on the **host** when running in Flatpak (via `which` on the host),
/// otherwise checks the current process `PATH` like a normal native binary probe.
pub fn host_command_exists(binary: &str) -> bool {
    if !is_safe_host_path_lookup_name(binary) {
        return false;
    }
    if is_flatpak() {
        let mut cmd = host_std_command("which");
        cmd.arg(binary);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        return cmd.status().map(|s| s.success()).unwrap_or(false);
    }
    let path_value = std::env::var_os("PATH").unwrap_or_else(|| OsString::from("/usr/bin:/bin"));
    for directory in std::env::split_paths(&path_value) {
        let candidate = directory.join(binary);
        if is_executable_file_sync(&candidate) {
            return true;
        }
    }
    false
}
