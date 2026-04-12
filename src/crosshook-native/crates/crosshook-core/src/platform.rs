//! Runtime platform detection for Flatpak sandboxing.
//!
//! CrossHook runs both as a native Linux binary (AppImage, dev build) and
//! inside a Flatpak sandbox. Several subsystems need to know which of the two
//! environments they are running in so they can adjust process spawning and
//! resource path resolution. This module is the single source of truth for
//! that decision.

use std::collections::BTreeMap;
use std::ffi::{CStr, CString, OsString};
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};

use tokio::process::Command;
use uuid::Uuid;

const FLATPAK_ID_ENV: &str = "FLATPAK_ID";
const FLATPAK_INFO_PATH: &str = "/.flatpak-info";
const FLATPAK_HOST_ROOT_PREFIX: &str = "/run/host";
const FLATPAK_DOCUMENT_PORTAL_PREFIX: &str = "/run/user/";
const FLATPAK_DOCUMENT_PORTAL_SEGMENT: &str = "/doc/";
const DOCUMENT_PORTAL_HOST_PATH_XATTR: &[u8] = b"user.document-portal.host-path\0";

/// Returns `true` when running inside a Flatpak sandbox.
///
/// Detection uses the two signals documented by the Flatpak runtime:
/// the `FLATPAK_ID` environment variable (set automatically by `flatpak run`)
/// and the presence of `/.flatpak-info` (always mounted inside the sandbox).
pub fn is_flatpak() -> bool {
    is_flatpak_with(FLATPAK_ID_ENV, Path::new(FLATPAK_INFO_PATH))
}

/// Normalizes a Flatpak host-mount path like `/run/host/usr/bin/foo` back to
/// the corresponding host path (`/usr/bin/foo`).
///
/// This repair is applied unconditionally so paths persisted by the Flatpak
/// build continue to work when reused later by the native/AppImage build.
/// Non-Unix paths (for example `C:\Games\foo.exe`) and relative paths are
/// returned unchanged aside from trimming outer whitespace.
pub fn normalize_flatpak_host_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if trimmed == FLATPAK_HOST_ROOT_PREFIX {
        return "/".to_string();
    }

    if let Some(stripped) = trimmed.strip_prefix(&format!("{FLATPAK_HOST_ROOT_PREFIX}/")) {
        return format!("/{}", stripped.trim_start_matches('/'));
    }

    if let Some(host_path) = read_document_portal_host_path(trimmed) {
        return host_path;
    }

    path.to_string()
}

fn looks_like_document_portal_path(path: &str) -> bool {
    path.starts_with(FLATPAK_DOCUMENT_PORTAL_PREFIX)
        && path.contains(FLATPAK_DOCUMENT_PORTAL_SEGMENT)
}

fn read_document_portal_host_path(path: &str) -> Option<String> {
    if !looks_like_document_portal_path(path) {
        return None;
    }

    read_document_portal_host_path_xattr(path)
}

fn read_document_portal_host_path_xattr(path: &str) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let c_path = CString::new(path.as_bytes()).ok()?;
        let attr_name = CStr::from_bytes_with_nul(DOCUMENT_PORTAL_HOST_PATH_XATTR).ok()?;

        // SAFETY: `c_path` and `attr_name` are NUL-terminated and live across
        // both libc calls. We first probe for the required buffer size, then
        // allocate exactly that many bytes before reading the xattr value.
        unsafe {
            let size =
                nix::libc::getxattr(c_path.as_ptr(), attr_name.as_ptr(), std::ptr::null_mut(), 0);
            if size <= 0 {
                return None;
            }

            let mut buffer = vec![0u8; size as usize];
            let written = nix::libc::getxattr(
                c_path.as_ptr(),
                attr_name.as_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
            );
            if written <= 0 {
                return None;
            }

            buffer.truncate(written as usize);
            Some(String::from_utf8_lossy(&buffer).trim().to_string())
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        None
    }
}

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
/// `custom_env_vars` are user-controlled launch overrides. Under Flatpak they are applied via a
/// short-lived `0600` env file and `bash` so values are not exposed on the `flatpak-spawn --env=`
/// argv.
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

/// Redirects `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, and `XDG_CACHE_HOME` to the
/// host's real XDG locations when running inside a Flatpak sandbox, so the
/// Flatpak build and the AppImage share the same data on disk.
///
/// Flatpak normally remaps these three variables to per-app directories under
/// `~/.var/app/<app-id>/`, which means `directories::BaseDirs::new()` — and
/// therefore every CrossHook store that derives its base path from it —
/// resolves to an empty sandbox location instead of the user's existing
/// `~/.config/crosshook/`, `~/.local/share/crosshook/`, and
/// `~/.cache/crosshook/`. The data is visible to the sandbox via
/// `--filesystem=home`; only the env var remap is hiding it.
///
/// The override honours Flatpak's `HOST_XDG_CONFIG_HOME`, `HOST_XDG_DATA_HOME`,
/// `HOST_XDG_CACHE_HOME`, and `HOST_XDG_STATE_HOME` env vars (set by the
/// Flatpak runtime to carry the host's real XDG values) when they are present.
/// This ensures users with a customised XDG layout (e.g. `XDG_CONFIG_HOME=/data/configs`)
/// get the correct paths rather than the `$HOME`-derived defaults.
///
/// For Phase 1 this function restores the default XDG paths so the Flatpak
/// build and the AppImage share the same data on disk. Called from the very
/// top of `crosshook_native::run()` before any store initializes.
///
/// Phase 4 (Flathub submission) will replace this with a proper per-app
/// isolation model and a first-run migration — see the tracking issue linked
/// from `docs/prps/prds/flatpak-distribution.prd.md` §10.2.
///
/// # Safety
///
/// Must only be called during single-threaded process startup, before any
/// threads are spawned and before any code reads XDG env vars. This function
/// mutates process environment variables through `SystemEnv::set`; see that
/// method's SAFETY note for the concrete preconditions.
pub unsafe fn override_xdg_for_flatpak_host_access() {
    if !is_flatpak() {
        return;
    }
    let mut sink = SystemEnv;
    apply_xdg_host_override(std::env::var_os("HOME").map(PathBuf::from), &mut sink);
}

fn is_flatpak_with(env_key: &str, info_path: &Path) -> bool {
    std::env::var_os(env_key).is_some() || info_path.exists()
}

fn host_command_with(program: &str, flatpak: bool) -> Command {
    if flatpak {
        tracing::debug!(program, "wrapping command with flatpak-spawn --host");
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg(program);
        cmd
    } else {
        Command::new(program)
    }
}

fn host_command_with_env_inner(
    program: &str,
    envs: &BTreeMap<String, String>,
    custom_env_vars: &BTreeMap<String, String>,
    flatpak: bool,
) -> Command {
    host_command_with_env_and_directory_inner(program, envs, None, flatpak, custom_env_vars)
}

fn host_command_with_env_and_directory_inner(
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

fn write_flatpak_custom_env_file(
    custom_env_vars: &BTreeMap<String, String>,
) -> io::Result<PathBuf> {
    let path = std::env::temp_dir().join(format!("crosshook-host-env-{}.env", Uuid::new_v4()));
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

fn host_std_command_with(program: &str, flatpak: bool) -> StdCommand {
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

fn host_std_command_with_env_inner(
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

fn is_executable_file_sync(path: &Path) -> bool {
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
        .any(|c| matches!(c, std::path::Component::ParentDir))
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
    cmd.arg("-x").arg(path);
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

/// Indirection for env-var access so unit tests can observe writes and
/// inject reads without mutating the real process environment.
trait EnvSink {
    /// Write an environment variable.
    fn set(&mut self, key: &str, value: &OsString);
    /// Read an environment variable. Returns `None` when the variable is unset.
    fn get(&self, key: &str) -> Option<OsString>;
}

struct SystemEnv;

impl EnvSink for SystemEnv {
    fn set(&mut self, key: &str, value: &OsString) {
        // SAFETY: called once from `run()` before any threads spawn; the
        // Tauri Builder is not yet constructed, so there are no concurrent
        // readers of the environment. Unit tests exercise this through a
        // mock `EnvSink` and never touch the real env via this code path.
        unsafe { std::env::set_var(key, value) };
    }

    fn get(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }
}

/// Resolve one XDG path: prefer the Flatpak `HOST_XDG_*_HOME` var when the
/// runtime has set it (carries the host's real XDG value), otherwise fall
/// back to `<home>/<default_rel…>`.
fn host_xdg_or_default(
    host_var: &str,
    home: &Path,
    default_rel: &[&str],
    env: &dyn EnvSink,
) -> OsString {
    if let Some(v) = env.get(host_var) {
        return v;
    }
    let mut p = home.to_path_buf();
    for s in default_rel {
        p.push(s);
    }
    p.into_os_string()
}

/// Applies XDG path overrides so the Flatpak sandbox sees the host's real XDG
/// directories rather than the per-app sandbox locations.
///
/// `HOST_XDG_CONFIG_HOME`, `HOST_XDG_DATA_HOME`, and `HOST_XDG_CACHE_HOME`
/// are preferred when set (Flatpak exposes them for exactly this purpose);
/// the `$HOME`-derived defaults are used as fallbacks.
fn apply_xdg_host_override(home: Option<PathBuf>, sink: &mut dyn EnvSink) -> bool {
    let Some(home) = home else {
        tracing::warn!("xdg host override skipped: HOME is unset");
        return false;
    };

    let config = host_xdg_or_default("HOST_XDG_CONFIG_HOME", &home, &[".config"], sink);
    let data = host_xdg_or_default("HOST_XDG_DATA_HOME", &home, &[".local", "share"], sink);
    let cache = host_xdg_or_default("HOST_XDG_CACHE_HOME", &home, &[".cache"], sink);

    sink.set("XDG_CONFIG_HOME", &config);
    sink.set("XDG_DATA_HOME", &data);
    sink.set("XDG_CACHE_HOME", &cache);
    tracing::info!(
        home = %home.display(),
        "xdg host override applied (flatpak → host XDG paths)"
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::ffi::OsString;
    use std::fs;

    use tempfile::tempdir;

    /// Test-only env key so we never mutate the real `FLATPAK_ID` variable.
    const TEST_ENV_KEY: &str = "CROSSHOOK_TEST_FLATPAK_ID";

    /// Mutex that serialises all tests mutating `CROSSHOOK_TEST_FLATPAK_ID`.
    static FLATPAK_ID_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Scoped env var override for testing.
    ///
    /// Acquires `FLATPAK_ID_LOCK` for its lifetime so concurrent tests do not
    /// race on the environment. Drop order is LIFO, so the lock is released
    /// only after the original value has been restored.
    struct ScopedEnv {
        key: &'static str,
        original: Option<OsString>,
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl ScopedEnv {
        fn set(key: &'static str, value: &str) -> Self {
            let guard = FLATPAK_ID_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let original = env::var_os(key);
            // SAFETY: single-threaded access guaranteed by the mutex.
            unsafe { env::set_var(key, value) };
            Self {
                key,
                original,
                _guard: guard,
            }
        }

        fn unset(key: &'static str) -> Self {
            let guard = FLATPAK_ID_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let original = env::var_os(key);
            // SAFETY: single-threaded access guaranteed by the mutex.
            unsafe { env::remove_var(key) };
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
                // SAFETY: mutex is still held; no other thread touches the key.
                Some(val) => unsafe { env::set_var(self.key, val) },
                None => unsafe { env::remove_var(self.key) },
            }
        }
    }

    #[test]
    fn returns_true_when_env_var_set_and_file_absent() {
        let _guard = ScopedEnv::set(TEST_ENV_KEY, "dev.crosshook.CrossHook");
        let tmp = tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        assert!(is_flatpak_with(TEST_ENV_KEY, &missing));
    }

    #[test]
    fn returns_true_when_file_present_and_env_var_unset() {
        let _guard = ScopedEnv::unset(TEST_ENV_KEY);
        let tmp = tempdir().unwrap();
        let present = tmp.path().join(".flatpak-info");
        fs::write(&present, b"[Application]\nname=test\n").unwrap();
        assert!(is_flatpak_with(TEST_ENV_KEY, &present));
    }

    #[test]
    fn returns_true_when_both_present() {
        let _guard = ScopedEnv::set(TEST_ENV_KEY, "dev.crosshook.CrossHook");
        let tmp = tempdir().unwrap();
        let present = tmp.path().join(".flatpak-info");
        fs::write(&present, b"[Application]\nname=test\n").unwrap();
        assert!(is_flatpak_with(TEST_ENV_KEY, &present));
    }

    #[test]
    fn returns_false_when_neither_present() {
        let _guard = ScopedEnv::unset(TEST_ENV_KEY);
        let tmp = tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        assert!(!is_flatpak_with(TEST_ENV_KEY, &missing));
    }

    #[test]
    fn normalize_flatpak_host_path_strips_host_mount_prefix() {
        assert_eq!(
            normalize_flatpak_host_path(
                "/run/host/usr/share/steam/compatibilitytools.d/proton/proton"
            ),
            "/usr/share/steam/compatibilitytools.d/proton/proton"
        );
        assert_eq!(
            normalize_flatpak_host_path("/run/host/home/alice/Games/test.exe"),
            "/home/alice/Games/test.exe"
        );
    }

    #[test]
    fn normalize_flatpak_host_path_leaves_non_host_paths_unchanged() {
        assert_eq!(
            normalize_flatpak_host_path(r"C:\Games\Test Game\game.exe"),
            r"C:\Games\Test Game\game.exe"
        );
        assert_eq!(
            normalize_flatpak_host_path("relative/path/to/file"),
            "relative/path/to/file"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn normalize_flatpak_host_path_resolves_document_portal_host_path_xattr() {
        let temp_dir = tempdir().unwrap();
        let portal_file = temp_dir.path().join("proton");
        std::fs::write(&portal_file, b"test").unwrap();

        let target_host_path = "/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/proton";
        let c_path = CString::new(portal_file.to_string_lossy().as_bytes()).unwrap();
        let attr_name = CStr::from_bytes_with_nul(DOCUMENT_PORTAL_HOST_PATH_XATTR).unwrap();
        let attr_value = CString::new(target_host_path).unwrap();

        // SAFETY: all pointers are valid NUL-terminated strings for the
        // duration of the call; the path names a temp file owned by the test.
        let rc = unsafe {
            nix::libc::setxattr(
                c_path.as_ptr(),
                attr_name.as_ptr(),
                attr_value.as_ptr().cast(),
                target_host_path.len(),
                0,
            )
        };
        assert_eq!(rc, 0, "setxattr should succeed for test portal path");

        assert_eq!(
            read_document_portal_host_path_xattr(&portal_file.to_string_lossy()),
            Some(target_host_path.to_string())
        );
    }

    #[test]
    fn host_command_wraps_program_when_flatpak() {
        let cmd = host_command_with("ls", true);
        let std_cmd = cmd.as_std();
        assert_eq!(std_cmd.get_program(), "flatpak-spawn");
        let args: Vec<&std::ffi::OsStr> = std_cmd.get_args().collect();
        assert_eq!(
            args,
            vec![std::ffi::OsStr::new("--host"), std::ffi::OsStr::new("ls")]
        );
    }

    #[test]
    fn host_command_passes_through_when_not_flatpak() {
        let cmd = host_command_with("ls", false);
        let std_cmd = cmd.as_std();
        assert_eq!(std_cmd.get_program(), "ls");
        assert_eq!(std_cmd.get_args().count(), 0);
    }

    #[test]
    fn host_command_with_env_threads_envs_as_env_args_in_flatpak() {
        // In Flatpak, each env entry must become a --env=KEY=VALUE arg placed
        // before the program name.  .env()/.envs() on the Command object are
        // silently dropped by flatpak-spawn --host.
        let envs = BTreeMap::from([
            ("WINEPREFIX".to_string(), "/home/alice/.wine".to_string()),
            ("DXVK_ASYNC".to_string(), "1".to_string()),
        ]);
        let cmd = host_command_with_env_inner("wine", &envs, &BTreeMap::new(), true);
        let std_cmd = cmd.as_std();
        assert_eq!(std_cmd.get_program(), "flatpak-spawn");
        let args: Vec<&std::ffi::OsStr> = std_cmd.get_args().collect();
        // First arg must be --host; the two --env args come next (BTreeMap is
        // sorted so DXVK_ASYNC < WINEPREFIX); last arg is the program.
        assert_eq!(args[0], std::ffi::OsStr::new("--host"));
        assert!(
            args.iter()
                .any(|a| *a == std::ffi::OsStr::new("--env=DXVK_ASYNC=1")),
            "expected --env=DXVK_ASYNC=1 in args, got: {args:?}"
        );
        assert!(
            args.iter()
                .any(|a| *a == std::ffi::OsStr::new("--env=WINEPREFIX=/home/alice/.wine")),
            "expected --env=WINEPREFIX=/home/alice/.wine in args, got: {args:?}"
        );
        assert_eq!(*args.last().unwrap(), std::ffi::OsStr::new("wine"));
    }

    #[test]
    fn host_command_with_env_and_directory_threads_directory_in_flatpak() {
        let envs = BTreeMap::from([("DXVK_ASYNC".to_string(), "1".to_string())]);
        let cmd = host_command_with_env_and_directory_inner(
            "wine",
            &envs,
            Some("/run/host/mnt/games/The Witcher 3"),
            true,
            &BTreeMap::new(),
        );
        let std_cmd = cmd.as_std();
        assert_eq!(std_cmd.get_program(), "flatpak-spawn");
        let args: Vec<&std::ffi::OsStr> = std_cmd.get_args().collect();
        assert_eq!(args[0], std::ffi::OsStr::new("--host"));
        assert!(
            args.iter()
                .any(|arg| *arg == std::ffi::OsStr::new("--directory=/mnt/games/The Witcher 3")),
            "expected normalized --directory arg, got: {args:?}"
        );
        assert!(
            args.iter()
                .any(|arg| *arg == std::ffi::OsStr::new("--env=DXVK_ASYNC=1")),
            "expected env passthrough, got: {args:?}"
        );
        assert_eq!(*args.last().unwrap(), std::ffi::OsStr::new("wine"));
    }

    #[test]
    fn host_command_with_env_and_directory_sets_current_dir_when_not_flatpak() {
        let envs = BTreeMap::from([("DXVK_ASYNC".to_string(), "1".to_string())]);
        let cmd = host_command_with_env_and_directory_inner(
            "wine",
            &envs,
            Some("/tmp/workdir"),
            false,
            &BTreeMap::new(),
        );
        let std_cmd = cmd.as_std();
        assert_eq!(std_cmd.get_program(), "wine");
        assert_eq!(
            std_cmd
                .get_current_dir()
                .map(|path| path.to_string_lossy().into_owned()),
            Some("/tmp/workdir".to_string())
        );
    }

    #[test]
    fn host_command_with_env_uses_envs_method_when_not_flatpak() {
        // Outside Flatpak, env vars should be forwarded via .envs(), not as
        // --env=K=V arguments (there is no flatpak-spawn wrapper).
        let envs = BTreeMap::from([("DXVK_ASYNC".to_string(), "1".to_string())]);
        let cmd = host_command_with_env_inner("wine", &envs, &BTreeMap::new(), false);
        let std_cmd = cmd.as_std();
        assert_eq!(std_cmd.get_program(), "wine");
        // No --env= style args; the env var is set on the Command directly.
        let args: Vec<&std::ffi::OsStr> = std_cmd.get_args().collect();
        assert!(
            args.is_empty(),
            "expected no extra args for non-flatpak, got: {args:?}"
        );
        let envs_on_cmd: Vec<(&std::ffi::OsStr, Option<&std::ffi::OsStr>)> =
            std_cmd.get_envs().collect();
        assert!(
            envs_on_cmd
                .iter()
                .any(|(k, v)| *k == std::ffi::OsStr::new("DXVK_ASYNC")
                    && *v == Some(std::ffi::OsStr::new("1"))),
            "expected DXVK_ASYNC=1 in command envs, got: {envs_on_cmd:?}"
        );
    }

    #[test]
    fn host_std_command_wraps_program_when_flatpak() {
        let cmd = host_std_command_with("ls", true);
        assert_eq!(cmd.get_program(), "flatpak-spawn");
        let args: Vec<&std::ffi::OsStr> = cmd.get_args().collect();
        assert_eq!(
            args,
            vec![std::ffi::OsStr::new("--host"), std::ffi::OsStr::new("ls")]
        );
    }

    #[test]
    fn host_std_command_passes_through_when_not_flatpak() {
        let cmd = host_std_command_with("ls", false);
        assert_eq!(cmd.get_program(), "ls");
        assert_eq!(cmd.get_args().count(), 0);
    }

    #[test]
    fn host_std_command_with_env_threads_envs_as_env_args_in_flatpak() {
        let envs = BTreeMap::from([
            ("WINEPREFIX".to_string(), "/home/alice/.wine".to_string()),
            ("DXVK_ASYNC".to_string(), "1".to_string()),
        ]);
        let cmd = host_std_command_with_env_inner("wine", &envs, &BTreeMap::new(), true);
        assert_eq!(cmd.get_program(), "flatpak-spawn");
        let args: Vec<&std::ffi::OsStr> = cmd.get_args().collect();
        assert_eq!(args[0], std::ffi::OsStr::new("--host"));
        assert!(
            args.iter()
                .any(|a| *a == std::ffi::OsStr::new("--env=DXVK_ASYNC=1")),
            "expected --env=DXVK_ASYNC=1 in args, got: {args:?}"
        );
        assert!(
            args.iter()
                .any(|a| *a == std::ffi::OsStr::new("--env=WINEPREFIX=/home/alice/.wine")),
            "expected --env=WINEPREFIX=/home/alice/.wine in args, got: {args:?}"
        );
        assert_eq!(*args.last().unwrap(), std::ffi::OsStr::new("wine"));
    }

    #[test]
    fn host_std_command_with_env_uses_envs_method_when_not_flatpak() {
        let envs = BTreeMap::from([("DXVK_ASYNC".to_string(), "1".to_string())]);
        let cmd = host_std_command_with_env_inner("wine", &envs, &BTreeMap::new(), false);
        assert_eq!(cmd.get_program(), "wine");
        let args: Vec<&std::ffi::OsStr> = cmd.get_args().collect();
        assert!(
            args.is_empty(),
            "expected no extra args for non-flatpak, got: {args:?}"
        );
        let envs_on_cmd: Vec<(&std::ffi::OsStr, Option<&std::ffi::OsStr>)> =
            cmd.get_envs().collect();
        assert!(
            envs_on_cmd
                .iter()
                .any(|(k, v)| *k == std::ffi::OsStr::new("DXVK_ASYNC")
                    && *v == Some(std::ffi::OsStr::new("1"))),
            "expected DXVK_ASYNC=1 in command envs, got: {envs_on_cmd:?}"
        );
    }

    /// In-memory `EnvSink` that records writes instead of mutating the real
    /// process environment. Lets xdg-override tests run concurrently without
    /// racing on `XDG_*_HOME`.
    ///
    /// Pre-populate `reads` to inject env-var values that `get()` will return,
    /// simulating `HOST_XDG_*_HOME` vars set by the Flatpak runtime.
    #[derive(Default)]
    struct FakeEnv {
        writes: Vec<(String, OsString)>,
        reads: std::collections::HashMap<String, OsString>,
    }

    impl EnvSink for FakeEnv {
        fn set(&mut self, key: &str, value: &OsString) {
            self.writes.push((key.to_string(), value.clone()));
        }

        fn get(&self, key: &str) -> Option<OsString> {
            self.reads.get(key).cloned()
        }
    }

    #[test]
    fn xdg_override_sets_all_three_paths_from_home() {
        let mut env = FakeEnv::default();
        let applied = apply_xdg_host_override(Some(PathBuf::from("/home/alice")), &mut env);
        assert!(applied);
        assert_eq!(
            env.writes,
            vec![
                (
                    "XDG_CONFIG_HOME".to_string(),
                    OsString::from("/home/alice/.config")
                ),
                (
                    "XDG_DATA_HOME".to_string(),
                    OsString::from("/home/alice/.local/share")
                ),
                (
                    "XDG_CACHE_HOME".to_string(),
                    OsString::from("/home/alice/.cache")
                ),
            ]
        );
    }

    #[test]
    fn xdg_override_noop_when_home_unset() {
        let mut env = FakeEnv::default();
        let applied = apply_xdg_host_override(None, &mut env);
        assert!(!applied);
        assert!(env.writes.is_empty());
    }

    #[test]
    fn xdg_override_preserves_trailing_slash_behavior() {
        let mut env = FakeEnv::default();
        apply_xdg_host_override(Some(PathBuf::from("/home/bob/")), &mut env);
        let (_, config) = &env.writes[0];
        // `Path::join` normalizes; the trailing slash is absorbed.
        assert_eq!(config, &OsString::from("/home/bob/.config"));
    }

    #[test]
    fn xdg_override_uses_exact_home_without_expansion() {
        // HOME may legitimately be something other than /home/<user>
        // (containers, per-user mount points, etc.) — honour it as-is.
        let mut env = FakeEnv::default();
        apply_xdg_host_override(Some(PathBuf::from("/var/home/charlie")), &mut env);
        assert_eq!(env.writes[0].1, OsString::from("/var/home/charlie/.config"));
    }

    #[test]
    fn xdg_override_prefers_host_xdg_config_home_when_set() {
        // Simulate a user with a customised XDG layout where Flatpak exposes
        // HOST_XDG_CONFIG_HOME=/data/configs.  The override must write that
        // value rather than the $HOME/.config fallback.
        let _guard = ScopedEnv::set(TEST_ENV_KEY, "dev.crosshook.CrossHook");
        let mut env = FakeEnv::default();
        env.reads.insert(
            "HOST_XDG_CONFIG_HOME".to_string(),
            OsString::from("/data/configs"),
        );
        let applied = apply_xdg_host_override(Some(PathBuf::from("/home/alice")), &mut env);
        assert!(applied);
        let config_write = env
            .writes
            .iter()
            .find(|(k, _)| k == "XDG_CONFIG_HOME")
            .expect("XDG_CONFIG_HOME must be written");
        assert_eq!(
            config_write.1,
            OsString::from("/data/configs"),
            "should use HOST_XDG_CONFIG_HOME, not $HOME/.config"
        );
        // Data and cache fall back to $HOME-derived defaults when their HOST_XDG_* vars are absent.
        let data_write = env
            .writes
            .iter()
            .find(|(k, _)| k == "XDG_DATA_HOME")
            .expect("XDG_DATA_HOME must be written");
        assert_eq!(data_write.1, OsString::from("/home/alice/.local/share"));
    }

    #[test]
    fn xdg_override_prefers_all_host_xdg_vars_when_set() {
        // When all three HOST_XDG_* vars are present, none of the $HOME-derived
        // defaults should appear in the writes.
        let mut env = FakeEnv::default();
        env.reads.insert(
            "HOST_XDG_CONFIG_HOME".to_string(),
            OsString::from("/data/configs"),
        );
        env.reads.insert(
            "HOST_XDG_DATA_HOME".to_string(),
            OsString::from("/data/share"),
        );
        env.reads.insert(
            "HOST_XDG_CACHE_HOME".to_string(),
            OsString::from("/data/cache"),
        );
        let applied = apply_xdg_host_override(Some(PathBuf::from("/home/alice")), &mut env);
        assert!(applied);
        assert_eq!(
            env.writes,
            vec![
                (
                    "XDG_CONFIG_HOME".to_string(),
                    OsString::from("/data/configs")
                ),
                ("XDG_DATA_HOME".to_string(), OsString::from("/data/share")),
                ("XDG_CACHE_HOME".to_string(), OsString::from("/data/cache")),
            ]
        );
    }
}
