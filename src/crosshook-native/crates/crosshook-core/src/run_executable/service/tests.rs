use super::command_builder::build_run_executable_command_with;
use super::{
    is_throwaway_prefix_path, is_throwaway_prefix_path_in, prepare_run_executable,
    resolve_adhoc_prefix_path, resolve_default_adhoc_prefix_path, run_executable,
    validate_run_executable_request,
};
use crate::run_executable::{
    RunExecutableError, RunExecutableRequest, RunExecutableValidationError,
};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use tokio::process::Command;

fn argv(command: &Command) -> Vec<String> {
    let std_command = command.as_std();
    std::iter::once(std_command.get_program())
        .chain(std_command.get_args())
        .map(|value: &OsStr| value.to_string_lossy().into_owned())
        .collect()
}

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
fn build_run_executable_command_msi_argv_is_unchanged() {
    let temp_dir = tempdir().expect("temp dir");
    let mut request = valid_request(temp_dir.path());
    let msi_path = temp_dir.path().join("installer.msi");
    fs::write(&msi_path, b"msi").expect("msi file");
    request.executable_path = msi_path.to_string_lossy().into_owned();
    let prefix_path = PathBuf::from(&request.prefix_path);

    let command = build_run_executable_command_with(&request, &prefix_path, false);

    assert_eq!(
        argv(&command),
        vec![
            request.proton_path.clone(),
            "run".to_string(),
            "msiexec".to_string(),
            "/i".to_string(),
            request.executable_path.clone(),
            "/qb".to_string(),
        ]
    );
    assert_eq!(
        command.as_std().get_current_dir(),
        Some(temp_dir.path()),
        "native run should start in the installer's directory"
    );
}

#[test]
fn build_run_executable_command_exe_argv_is_proton_run_exe() {
    let temp_dir = tempdir().expect("temp dir");
    let request = valid_request(temp_dir.path());
    let prefix_path = PathBuf::from(&request.prefix_path);

    let command = build_run_executable_command_with(&request, &prefix_path, false);

    assert_eq!(
        argv(&command),
        vec![
            request.proton_path.clone(),
            "run".to_string(),
            request.executable_path.clone(),
        ]
    );
    let wineprefix = command
        .as_std()
        .get_envs()
        .find(|(key, _)| *key == "WINEPREFIX")
        .and_then(|(_, value)| value)
        .map(|value| value.to_string_lossy().into_owned());
    assert_eq!(wineprefix.as_deref(), Some(request.prefix_path.as_str()));
}

#[test]
fn build_run_executable_command_flatpak_argv_is_lifecycle_bound() {
    let temp_dir = tempdir().expect("temp dir");
    let mut request = valid_request(temp_dir.path());
    let msi_path = temp_dir.path().join("installer.msi");
    fs::write(&msi_path, b"msi").expect("msi file");
    request.executable_path = msi_path.to_string_lossy().into_owned();
    let prefix_path = PathBuf::from(&request.prefix_path);

    let command = build_run_executable_command_with(&request, &prefix_path, true);
    let args = argv(&command);

    assert_eq!(
        &args[..4],
        ["flatpak-spawn", "--host", "--watch-bus", "--clear-env"]
    );
    assert!(
        args.contains(&format!("--directory={}", temp_dir.path().display())),
        "flatpak argv should carry the working directory, got: {args:?}"
    );
    assert!(
        args.contains(&format!("--env=WINEPREFIX={}", request.prefix_path)),
        "flatpak argv should forward WINEPREFIX, got: {args:?}"
    );
    assert_eq!(
        &args[args.len() - 6..],
        [
            request.proton_path.as_str(),
            "run",
            "msiexec",
            "/i",
            request.executable_path.as_str(),
            "/qb",
        ]
    );
}

#[test]
fn build_run_executable_command_normalizes_flatpak_host_mounted_paths() {
    let temp_dir = tempdir().expect("temp dir");
    let mut request = valid_request(temp_dir.path());
    let normalized_executable_path = request.executable_path.clone();
    let normalized_proton_path = request.proton_path.clone();
    request.executable_path = format!("/run/host{}", request.executable_path);
    request.proton_path = format!("/run/host{}", request.proton_path);
    let prefix_path = PathBuf::from(&request.prefix_path);

    let command = build_run_executable_command_with(&request, &prefix_path, false);

    assert_eq!(
        argv(&command),
        vec![
            normalized_proton_path,
            "run".to_string(),
            normalized_executable_path,
        ]
    );
}

#[test]
fn prepare_run_executable_resolves_unique_throwaway_prefixes() {
    let temp_dir = tempdir().expect("temp dir");
    let mut request = valid_request(temp_dir.path());
    request.prefix_path = String::new();
    let adhoc_root = temp_dir.path().join("prefixes").join("_run-adhoc");

    let first = prepare_run_executable(&request, &adhoc_root, "aaaaaaaaaaaa").expect("first");
    let second = prepare_run_executable(&request, &adhoc_root, "bbbbbbbbbbbb").expect("second");

    assert!(first.is_throwaway && second.is_throwaway);
    assert_eq!(first.prefix_path, adhoc_root.join("installer-aaaaaaaaaaaa"));
    assert_eq!(
        second.prefix_path,
        adhoc_root.join("installer-bbbbbbbbbbbb")
    );
    assert!(first.prefix_path.is_dir() && second.prefix_path.is_dir());
    assert!(is_throwaway_prefix_path_in(&first.prefix_path, &adhoc_root));
    assert!(is_throwaway_prefix_path_in(
        &second.prefix_path,
        &adhoc_root
    ));
    assert!(
        format!("{:?}", first.command).contains(first.prefix_path.to_string_lossy().as_ref()),
        "command should target the resolved prefix"
    );
}

#[test]
fn prepare_run_executable_keeps_explicit_prefix_and_is_not_throwaway() {
    let temp_dir = tempdir().expect("temp dir");
    let request = valid_request(temp_dir.path());
    let adhoc_root = temp_dir.path().join("_run-adhoc");

    let prepared = prepare_run_executable(&request, &adhoc_root, "aaaaaaaaaaaa").expect("prepare");

    assert!(!prepared.is_throwaway);
    assert_eq!(prepared.prefix_path, PathBuf::from(&request.prefix_path));
    assert!(
        !adhoc_root.exists(),
        "explicit prefix must not touch the adhoc root"
    );
}

#[test]
fn prepare_run_executable_rejects_invalid_request_without_provisioning() {
    let temp_dir = tempdir().expect("temp dir");
    let adhoc_root = temp_dir.path().join("_run-adhoc");

    let result = prepare_run_executable(
        &RunExecutableRequest::default(),
        &adhoc_root,
        "aaaaaaaaaaaa",
    );

    assert!(matches!(
        result,
        Err(RunExecutableError::Validation(
            RunExecutableValidationError::ExecutablePathRequired
        ))
    ));
    assert!(!adhoc_root.exists());
}

#[test]
fn resolve_adhoc_prefix_path_slugifies_executable_stem() {
    let root = PathBuf::from("/data/_run-adhoc");
    let resolved = resolve_adhoc_prefix_path(&root, Path::new("/x/Setup Wizard.exe"), "abc123");

    assert_eq!(resolved, root.join("setup-wizard-abc123"));
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
