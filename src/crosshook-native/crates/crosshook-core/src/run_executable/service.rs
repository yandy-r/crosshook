use std::fs;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use tokio::process::Command;

use crate::launch::runtime_helpers::{
    apply_host_environment, apply_runtime_proton_environment, apply_working_directory,
    attach_log_stdio, new_direct_proton_command,
};

use super::models::{
    RunExecutableError, RunExecutableRequest, RunExecutableResult, RunExecutableValidationError,
};

/// Root namespace under `~/.local/share/crosshook/prefixes/` for ad-hoc runs.
///
/// Underscore prefix sorts above alphanumeric profile prefixes in `ls`/`tree`,
/// making throwaway runner prefixes visually distinct from real game prefixes.
const ADHOC_PREFIX_ROOT_SEGMENT: &str = "crosshook/prefixes/_run-adhoc";

/// Default fallback slug used when an executable file stem cannot be slugified.
const ADHOC_FALLBACK_SLUG: &str = "adhoc";

/// Returns `true` when `prefix_path` is a direct child of the throwaway
/// `_run-adhoc/` namespace under the platform data-local directory — i.e. it
/// looks exactly like something [`resolve_default_adhoc_prefix_path`] would
/// have produced.
///
/// Used by the Tauri layer as a defense-in-depth guard before any
/// `remove_dir_all` / `rm -rf` against the prefix path. The check is strict:
/// the parent must be the canonical adhoc namespace root, the path must
/// have a non-empty file name, and there must be no `..` traversal in the
/// resolved chain.
pub fn is_throwaway_prefix_path(prefix_path: &Path) -> bool {
    let Some(base_dirs) = BaseDirs::new() else {
        return false;
    };
    let expected_parent = base_dirs.data_local_dir().join(ADHOC_PREFIX_ROOT_SEGMENT);

    // Reject any `..` components — a malicious or buggy slug could otherwise
    // synthesize a path that *looks* rooted under `_run-adhoc/` but actually
    // escapes via traversal once symlinks resolve.
    if prefix_path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return false;
    }

    let Some(parent) = prefix_path.parent() else {
        return false;
    };
    let Some(file_name) = prefix_path.file_name() else {
        return false;
    };

    // The slug must be non-empty and must not itself contain a separator,
    // both of which `slugify` already guarantees but we re-verify here so
    // the guard is independently sound.
    if file_name.is_empty() {
        return false;
    }

    parent == expected_parent.as_path()
}

pub fn validate_run_executable_request(
    request: &RunExecutableRequest,
) -> Result<(), RunExecutableValidationError> {
    validate_executable_path(request.executable_path.trim())?;
    validate_proton_path(request.proton_path.trim())?;
    validate_optional_prefix_path(request.prefix_path.trim())?;

    Ok(())
}

pub fn build_run_executable_command(
    request: &RunExecutableRequest,
    prefix_path: &Path,
    log_path: &Path,
) -> Result<Command, RunExecutableError> {
    let executable_path = request.executable_path.trim();
    let mut command = new_direct_proton_command(request.proton_path.trim());

    if is_msi_path(Path::new(executable_path)) {
        // `msiexec` ships with every Proton/Wine prefix; the `/qb` flag asks for
        // basic UI (progress without modal prompts) so the user still sees what
        // is happening but does not have to babysit a fully interactive run.
        command.arg("msiexec");
        command.arg("/i");
        command.arg(executable_path);
        command.arg("/qb");
    } else {
        command.arg(executable_path);
    }

    apply_host_environment(&mut command);
    apply_runtime_proton_environment(
        &mut command,
        prefix_path.to_string_lossy().as_ref(),
        request.steam_client_install_path.trim(),
    );
    apply_working_directory(
        &mut command,
        request.working_directory.trim(),
        Path::new(executable_path),
    );
    attach_log_stdio(&mut command, log_path).map_err(|error| {
        RunExecutableError::LogAttachmentFailed {
            path: log_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;

    Ok(command)
}

pub fn run_executable(
    request: &RunExecutableRequest,
    log_path: &Path,
) -> Result<(RunExecutableResult, tokio::process::Child), RunExecutableError> {
    validate_run_executable_request(request)?;

    let prefix_path = if request.prefix_path.trim().is_empty() {
        resolve_default_adhoc_prefix_path(Path::new(request.executable_path.trim()))?
    } else {
        PathBuf::from(request.prefix_path.trim())
    };

    provision_prefix(&prefix_path)?;

    let mut command = build_run_executable_command(request, &prefix_path, log_path)?;
    let child = command
        .spawn()
        .map_err(|error| RunExecutableError::RunnerSpawnFailed {
            message: error.to_string(),
        })?;

    let result = RunExecutableResult {
        succeeded: true,
        message: "Executable launched.".to_string(),
        helper_log_path: log_path.display().to_string(),
        resolved_prefix_path: prefix_path.display().to_string(),
    };

    Ok((result, child))
}

/// Resolves the default `_run-adhoc/<slug>` prefix path for an executable.
///
/// Returns [`RunExecutableError::HomeDirectoryUnavailable`] when no platform
/// home directory can be located (e.g. headless CI without `$HOME`).
pub fn resolve_default_adhoc_prefix_path(
    executable_path: &Path,
) -> Result<PathBuf, RunExecutableError> {
    let base_dirs = BaseDirs::new().ok_or(RunExecutableError::HomeDirectoryUnavailable)?;
    Ok(resolve_default_adhoc_prefix_path_from_data_local_dir(
        base_dirs.data_local_dir(),
        executable_path,
    ))
}

fn resolve_default_adhoc_prefix_path_from_data_local_dir(
    data_local_dir: &Path,
    executable_path: &Path,
) -> PathBuf {
    let stem = executable_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    data_local_dir
        .join(ADHOC_PREFIX_ROOT_SEGMENT)
        .join(slugify(stem))
}

fn provision_prefix(prefix_path: &Path) -> Result<(), RunExecutableError> {
    if let Ok(metadata) = fs::metadata(prefix_path) {
        if !metadata.is_dir() {
            return Err(RunExecutableError::PrefixCreationFailed {
                path: prefix_path.to_path_buf(),
                message: "Path exists but is not a directory.".to_string(),
            });
        }
        return Ok(());
    }

    fs::create_dir_all(prefix_path).map_err(|error| RunExecutableError::PrefixCreationFailed {
        path: prefix_path.to_path_buf(),
        message: error.to_string(),
    })
}

fn validate_executable_path(path: &str) -> Result<(), RunExecutableValidationError> {
    if path.is_empty() {
        return Err(RunExecutableValidationError::ExecutablePathRequired);
    }

    let path = Path::new(path);
    if !path.exists() {
        return Err(RunExecutableValidationError::ExecutablePathMissing);
    }
    if !path.is_file() {
        return Err(RunExecutableValidationError::ExecutablePathNotFile);
    }
    if !is_windows_runnable_executable(path) {
        return Err(RunExecutableValidationError::ExecutablePathNotWindowsExecutable);
    }

    Ok(())
}

fn validate_proton_path(path: &str) -> Result<(), RunExecutableValidationError> {
    if path.is_empty() {
        return Err(RunExecutableValidationError::ProtonPathRequired);
    }

    let path = Path::new(path);
    if !path.exists() {
        return Err(RunExecutableValidationError::ProtonPathMissing);
    }
    if !is_executable_file(path) {
        return Err(RunExecutableValidationError::ProtonPathNotExecutable);
    }

    Ok(())
}

fn validate_optional_prefix_path(path: &str) -> Result<(), RunExecutableValidationError> {
    if path.is_empty() {
        return Ok(());
    }

    let path = Path::new(path);
    if !path.exists() {
        return Err(RunExecutableValidationError::PrefixPathMissing);
    }
    if !path.is_dir() {
        return Err(RunExecutableValidationError::PrefixPathNotDirectory);
    }

    Ok(())
}

fn is_windows_runnable_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("msi")
        })
}

fn is_msi_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("msi"))
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

fn slugify(name: &str) -> String {
    let slug: String = name
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        ADHOC_FALLBACK_SLUG.to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    fn write_executable_script(path: &Path, body: &str) {
        fs::write(path, body).expect("write executable script");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path).expect("script metadata").permissions();
            permissions.set_mode(permissions.mode() | 0o111);
            fs::set_permissions(path, permissions).expect("set executable permissions");
        }
    }

    fn valid_request(temp_dir: &Path) -> RunExecutableRequest {
        let executable_path = temp_dir.join("installer.exe");
        let proton_path = temp_dir.join("proton");
        let prefix_path = temp_dir.join("prefix");

        fs::write(&executable_path, b"installer").expect("executable file");
        write_executable_script(&proton_path, "#!/bin/sh\nexit 0\n");
        fs::create_dir_all(&prefix_path).expect("prefix directory");

        RunExecutableRequest {
            executable_path: executable_path.to_string_lossy().into_owned(),
            proton_path: proton_path.to_string_lossy().into_owned(),
            prefix_path: prefix_path.to_string_lossy().into_owned(),
            working_directory: String::new(),
            steam_client_install_path: String::new(),
        }
    }

    #[test]
    fn validate_run_executable_request_accepts_valid_request() {
        let temp_dir = tempdir().expect("temp dir");
        let request = valid_request(temp_dir.path());

        assert!(validate_run_executable_request(&request).is_ok());
    }

    #[test]
    fn validate_run_executable_request_accepts_msi_executable() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());

        let msi_path = temp_dir.path().join("installer.msi");
        fs::write(&msi_path, b"msi").expect("msi file");
        request.executable_path = msi_path.to_string_lossy().into_owned();

        assert!(validate_run_executable_request(&request).is_ok());
    }

    #[test]
    fn validate_run_executable_request_accepts_uppercase_exe_and_msi_extensions() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());

        let exe_path = temp_dir.path().join("Setup.EXE");
        fs::write(&exe_path, b"exe").expect("exe file");
        request.executable_path = exe_path.to_string_lossy().into_owned();
        assert!(
            validate_run_executable_request(&request).is_ok(),
            "Uppercase .EXE should be accepted"
        );

        let msi_path = temp_dir.path().join("Setup.MSI");
        fs::write(&msi_path, b"msi").expect("msi file");
        request.executable_path = msi_path.to_string_lossy().into_owned();
        assert!(
            validate_run_executable_request(&request).is_ok(),
            "Uppercase .MSI should be accepted"
        );
    }

    #[test]
    fn validate_run_executable_request_rejects_empty_executable_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        request.executable_path = String::new();

        assert!(matches!(
            validate_run_executable_request(&request),
            Err(RunExecutableValidationError::ExecutablePathRequired)
        ));
    }

    #[test]
    fn validate_run_executable_request_rejects_missing_executable_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        request.executable_path = temp_dir
            .path()
            .join("nonexistent.exe")
            .to_string_lossy()
            .into_owned();

        assert!(matches!(
            validate_run_executable_request(&request),
            Err(RunExecutableValidationError::ExecutablePathMissing)
        ));
    }

    #[test]
    fn validate_run_executable_request_rejects_directory_as_executable_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        let dir_path = temp_dir.path().join("setup-dir.exe");
        fs::create_dir_all(&dir_path).expect("setup dir");
        request.executable_path = dir_path.to_string_lossy().into_owned();

        assert!(matches!(
            validate_run_executable_request(&request),
            Err(RunExecutableValidationError::ExecutablePathNotFile)
        ));
    }

    #[test]
    fn validate_run_executable_request_rejects_non_exe_or_msi_extension() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        let txt_path = temp_dir.path().join("setup.txt");
        fs::write(&txt_path, b"not an executable").expect("txt file");
        request.executable_path = txt_path.to_string_lossy().into_owned();

        assert!(matches!(
            validate_run_executable_request(&request),
            Err(RunExecutableValidationError::ExecutablePathNotWindowsExecutable)
        ));
    }

    #[test]
    fn validate_run_executable_request_rejects_empty_proton_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        request.proton_path = String::new();

        assert!(matches!(
            validate_run_executable_request(&request),
            Err(RunExecutableValidationError::ProtonPathRequired)
        ));
    }

    #[test]
    fn validate_run_executable_request_rejects_missing_proton_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        request.proton_path = temp_dir
            .path()
            .join("nonexistent-proton")
            .to_string_lossy()
            .into_owned();

        assert!(matches!(
            validate_run_executable_request(&request),
            Err(RunExecutableValidationError::ProtonPathMissing)
        ));
    }

    #[test]
    fn validate_run_executable_request_rejects_non_executable_proton_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        let non_exec_path = temp_dir.path().join("proton-no-exec");
        fs::write(&non_exec_path, b"not executable").expect("non-exec file");
        request.proton_path = non_exec_path.to_string_lossy().into_owned();

        assert!(matches!(
            validate_run_executable_request(&request),
            Err(RunExecutableValidationError::ProtonPathNotExecutable)
        ));
    }

    #[test]
    fn validate_run_executable_request_allows_empty_prefix_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        request.prefix_path = String::new();

        assert!(validate_run_executable_request(&request).is_ok());
    }

    #[test]
    fn validate_run_executable_request_rejects_missing_prefix_path_when_provided() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        request.prefix_path = temp_dir
            .path()
            .join("nonexistent-prefix")
            .to_string_lossy()
            .into_owned();

        assert!(matches!(
            validate_run_executable_request(&request),
            Err(RunExecutableValidationError::PrefixPathMissing)
        ));
    }

    #[test]
    fn validate_run_executable_request_rejects_file_as_prefix_path() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        let file_path = temp_dir.path().join("prefix-file");
        fs::write(&file_path, b"not a directory").expect("prefix file");
        request.prefix_path = file_path.to_string_lossy().into_owned();

        assert!(matches!(
            validate_run_executable_request(&request),
            Err(RunExecutableValidationError::PrefixPathNotDirectory)
        ));
    }

    #[test]
    fn build_run_executable_command_uses_msiexec_for_msi() {
        let temp_dir = tempdir().expect("temp dir");
        let mut request = valid_request(temp_dir.path());
        let msi_path = temp_dir.path().join("installer.msi");
        fs::write(&msi_path, b"msi").expect("msi file");
        request.executable_path = msi_path.to_string_lossy().into_owned();

        let prefix_path = PathBuf::from(request.prefix_path.clone());
        let log_path = temp_dir.path().join("test-msi.log");
        std::fs::File::create(&log_path).unwrap();

        let command = build_run_executable_command(&request, &prefix_path, &log_path).unwrap();
        let debug_output = format!("{command:?}");

        assert!(
            debug_output.contains("msiexec"),
            "Command should invoke msiexec for .msi inputs, got: {debug_output}"
        );
        assert!(
            debug_output.contains("/i"),
            "Command should pass /i to msiexec, got: {debug_output}"
        );
        assert!(
            debug_output.contains(&msi_path.display().to_string()),
            "Command should reference the msi path, got: {debug_output}"
        );
    }

    #[test]
    fn build_run_executable_command_uses_direct_arg_for_exe() {
        let temp_dir = tempdir().expect("temp dir");
        let request = valid_request(temp_dir.path());
        let prefix_path = PathBuf::from(request.prefix_path.clone());
        let log_path = temp_dir.path().join("test-exe.log");
        std::fs::File::create(&log_path).unwrap();

        let command = build_run_executable_command(&request, &prefix_path, &log_path).unwrap();
        let debug_output = format!("{command:?}");

        assert!(
            !debug_output.contains("msiexec"),
            "Command must not invoke msiexec for .exe inputs, got: {debug_output}"
        );
        assert!(
            debug_output.contains(&request.executable_path),
            "Command should reference the executable path, got: {debug_output}"
        );
    }

    #[test]
    fn build_run_executable_command_references_proton_path() {
        let temp_dir = tempdir().expect("temp dir");
        let request = valid_request(temp_dir.path());
        let prefix_path = PathBuf::from(request.prefix_path.clone());
        let log_path = temp_dir.path().join("test-proton.log");
        std::fs::File::create(&log_path).unwrap();

        let command = build_run_executable_command(&request, &prefix_path, &log_path).unwrap();
        let debug_output = format!("{command:?}");

        assert!(
            debug_output.contains(&request.proton_path),
            "Command should reference the proton path, got: {debug_output}"
        );
    }

    #[test]
    fn resolve_default_adhoc_prefix_path_slugifies_executable_stem() {
        let temp_dir = tempdir().expect("temp dir");
        let resolved = resolve_default_adhoc_prefix_path_from_data_local_dir(
            temp_dir.path(),
            Path::new("/x/Setup Wizard.exe"),
        );

        let expected = temp_dir
            .path()
            .join("crosshook/prefixes/_run-adhoc/setup-wizard");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn resolve_default_adhoc_prefix_path_falls_back_when_stem_is_unprintable() {
        let temp_dir = tempdir().expect("temp dir");
        let resolved = resolve_default_adhoc_prefix_path_from_data_local_dir(
            temp_dir.path(),
            Path::new("/x/---.exe"),
        );

        let expected = temp_dir.path().join("crosshook/prefixes/_run-adhoc/adhoc");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn is_throwaway_prefix_path_accepts_real_adhoc_prefix() {
        // The path produced by resolve_default_adhoc_prefix_path is by
        // definition a child of the adhoc namespace root, so this should
        // always validate as a safe throwaway target.
        let resolved = resolve_default_adhoc_prefix_path(Path::new("/x/Setup Wizard.exe"))
            .expect("resolve adhoc prefix path");
        assert!(
            is_throwaway_prefix_path(&resolved),
            "resolved adhoc prefix should be classified as throwaway: {}",
            resolved.display()
        );
    }

    #[test]
    fn is_throwaway_prefix_path_rejects_paths_outside_namespace() {
        let candidates = [
            "/tmp/_run-adhoc/foo",
            "/home/user/Documents/_run-adhoc/setup",
            "/var/lib/crosshook/prefixes/_run-adhoc/setup",
            "/home/user/.local/share/crosshook/prefixes/keepme",
        ];
        for candidate in candidates {
            assert!(
                !is_throwaway_prefix_path(Path::new(candidate)),
                "path outside the canonical adhoc root must NOT be classified as throwaway: {candidate}"
            );
        }
    }

    #[test]
    fn is_throwaway_prefix_path_rejects_parent_dir_traversal() {
        // Build a path that nominally lives under the adhoc root but uses
        // `..` to walk back out before terminating elsewhere.
        let resolved =
            resolve_default_adhoc_prefix_path(Path::new("/x/setup.exe")).expect("resolve adhoc");
        let traversal = resolved
            .parent()
            .unwrap()
            .join("..")
            .join("..")
            .join("escape");
        assert!(
            !is_throwaway_prefix_path(&traversal),
            "path containing `..` traversal must NOT be classified as throwaway: {}",
            traversal.display()
        );
    }

    #[test]
    fn is_throwaway_prefix_path_rejects_namespace_root_itself() {
        // Even the namespace root directory must be rejected — deleting it
        // would wipe every other in-flight or recently-finished adhoc run.
        let resolved =
            resolve_default_adhoc_prefix_path(Path::new("/x/setup.exe")).expect("resolve adhoc");
        let namespace_root = resolved.parent().unwrap().to_path_buf();
        assert!(
            !is_throwaway_prefix_path(&namespace_root),
            "namespace root must NOT be classified as throwaway: {}",
            namespace_root.display()
        );
    }

    #[test]
    fn run_executable_rejects_invalid_request() {
        let temp_dir = tempdir().expect("temp dir");
        let log_path = temp_dir.path().join("run.log");
        std::fs::File::create(&log_path).unwrap();

        let request = RunExecutableRequest::default();
        let result = run_executable(&request, &log_path);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RunExecutableError::Validation(RunExecutableValidationError::ExecutablePathRequired)
        ));
    }
}
