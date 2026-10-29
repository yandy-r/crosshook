use super::*;
use std::ffi::OsStr;
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

/// Fake `proton`: refuses anything but `run <...>/setup.exe`, then lays down a
/// game executable inside `$WINEPREFIX`.
const FAKE_PROTON_SCRIPT: &str = "#!/bin/sh\n\
[ \"$1\" = run ] || exit 9\n\
case \"$2\" in */setup.exe) ;; *) exit 10 ;; esac\n\
prefix_path=\"$WINEPREFIX\"\n\
mkdir -p \"$prefix_path/drive_c/Games/Example Game\"\n\
cat <<'EOF' > \"$prefix_path/drive_c/Games/Example Game/ExampleGame.exe\"\n\
game\n\
EOF\n\
exit 0\n";

fn valid_request(temp_dir: &Path) -> InstallGameRequest {
    let installer_path = temp_dir.join("setup.exe");
    let trainer_path = temp_dir.join("trainer.exe");
    let proton_path = temp_dir.join("proton");
    let prefix_path = temp_dir.join("prefix");

    fs::write(&installer_path, b"installer").expect("installer file");
    fs::write(&trainer_path, b"trainer").expect("trainer file");
    write_executable_script(&proton_path, FAKE_PROTON_SCRIPT);

    InstallGameRequest {
        profile_name: "example-game".to_string(),
        display_name: "Example Game".to_string(),
        installer_path: installer_path.to_string_lossy().into_owned(),
        trainer_path: trainer_path.to_string_lossy().into_owned(),
        proton_path: proton_path.to_string_lossy().into_owned(),
        prefix_path: prefix_path.to_string_lossy().into_owned(),
        installed_game_executable_path: String::new(),
        launcher_icon_path: String::new(),
        custom_cover_art_path: String::new(),
        runner_method: String::new(),
        steam_app_id: String::new(),
        custom_portrait_art_path: String::new(),
        custom_background_art_path: String::new(),
        working_directory: String::new(),
    }
}

/// Today's `install_game` expectations for the fake installer in `valid_request`.
fn assert_discovered_example_game(
    result: &InstallGameResult,
    temp_dir: &Path,
    prefix_path: &Path,
    log_path: &Path,
) {
    assert!(prefix_path.is_dir());
    assert!(result.succeeded);
    assert_eq!(
        result.message,
        "Installer completed. Review the generated profile."
    );
    assert_eq!(result.helper_log_path, log_path.to_string_lossy());
    assert_eq!(result.profile_name, "example-game");
    assert!(result.needs_executable_confirmation);
    let expected_game_path = prefix_path
        .join("drive_c/Games/Example Game/ExampleGame.exe")
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        result
            .discovered_game_executable_candidates
            .first()
            .map(std::string::String::as_str),
        Some(expected_game_path.as_str())
    );
    assert_eq!(result.profile.game.executable_path, expected_game_path);
    assert_eq!(
        result.profile.runtime.working_directory,
        prefix_path
            .join("drive_c/Games/Example Game")
            .to_string_lossy()
            .into_owned()
    );
    assert_eq!(result.profile.launch.method, "proton_run");
    assert_eq!(result.profile.game.name, "Example Game");
    assert_eq!(
        result.profile.trainer.path,
        temp_dir.join("trainer.exe").to_string_lossy().into_owned()
    );
}

fn command_argv(command: &Command) -> Vec<String> {
    let std_command = command.as_std();
    std::iter::once(std_command.get_program())
        .chain(std_command.get_args())
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn resolve_default_prefix_path_uses_data_local_root_and_slugifies_name() {
    let temp_dir = tempdir().expect("temp dir");
    let profile_name = "God of War Ragnarok";

    let prefix_root = resolve_default_prefix_path_from_data_local_dir(temp_dir.path());
    let prefix_path = prefix_root.join(slugify_profile_name(profile_name));

    assert_eq!(
        prefix_path,
        temp_dir
            .path()
            .join(DEFAULT_PREFIX_ROOT_SEGMENT)
            .join("god-of-war-ragnarok")
    );
}

#[test]
fn validate_install_request_returns_specific_field_errors() {
    let temp_dir = tempdir().expect("temp dir");
    let base_request = valid_request(temp_dir.path());

    assert!(validate_install_request(&base_request).is_ok());

    let mut request = base_request.clone();
    request.profile_name = String::new();
    assert!(matches!(
        validate_install_request(&request),
        Err(InstallGameValidationError::ProfileNameRequired)
    ));

    let mut request = base_request.clone();
    request.installer_path = temp_dir
        .path()
        .join("setup.txt")
        .to_string_lossy()
        .into_owned();
    fs::write(&request.installer_path, b"installer").expect("installer txt");
    assert!(matches!(
        validate_install_request(&request),
        Err(InstallGameValidationError::InstallerPathNotWindowsExecutable)
    ));

    let mut request = base_request.clone();
    request.proton_path = temp_dir
        .path()
        .join("proton-dir")
        .to_string_lossy()
        .into_owned();
    fs::create_dir_all(&request.proton_path).expect("proton dir");
    assert!(matches!(
        validate_install_request(&request),
        Err(InstallGameValidationError::ProtonPathNotExecutable)
    ));

    let mut request = base_request.clone();
    request.installed_game_executable_path = temp_dir
        .path()
        .join("candidate")
        .to_string_lossy()
        .into_owned();
    fs::create_dir_all(&request.installed_game_executable_path).expect("candidate dir");
    assert!(matches!(
        validate_install_request(&request),
        Err(InstallGameValidationError::InstalledGameExecutablePathNotFile)
    ));

    let mut request = base_request;
    request.custom_cover_art_path = temp_dir.path().to_string_lossy().into_owned();
    assert!(matches!(
        validate_install_request(&request),
        Err(InstallGameValidationError::CustomCoverArtPathNotFile)
    ));
}

#[test]
fn validate_install_request_accepts_flatpak_host_mounted_paths() {
    let temp_dir = tempdir().expect("temp dir");
    let mut request = valid_request(temp_dir.path());
    request.installer_path = format!("/run/host{}", request.installer_path);
    request.trainer_path = format!("/run/host{}", request.trainer_path);
    request.proton_path = format!("/run/host{}", request.proton_path);
    request.prefix_path = format!("/run/host{}", request.prefix_path);

    assert!(validate_install_request(&request).is_ok());
}

#[test]
fn install_game_creates_prefix_and_prefers_discovered_executable() {
    let temp_dir = tempdir().expect("temp dir");
    let request = valid_request(temp_dir.path());
    let prefix_path = PathBuf::from(&request.prefix_path);
    let log_path = temp_dir.path().join("install.log");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    let task_log_path = log_path.clone();
    let result = runtime
        .block_on(async move {
            tokio::task::spawn_blocking(move || install_game(&request, &task_log_path))
                .await
                .expect("join install task")
        })
        .expect("install game");

    assert_discovered_example_game(&result, temp_dir.path(), &prefix_path, &log_path);
}

#[test]
fn prepare_install_provisions_prefix_and_builds_run_installer_command() {
    let temp_dir = tempdir().expect("temp dir");
    let request = valid_request(temp_dir.path());
    let expected_prefix = PathBuf::from(&request.prefix_path);
    assert!(!expected_prefix.exists());

    let prepared = prepare_install(&request).expect("prepare install");

    assert_eq!(prepared.prefix_path, expected_prefix);
    assert!(expected_prefix.is_dir(), "prefix must be provisioned");
    let debug_output = format!("{:?}", prepared.command);
    let expected_tail = format!("\"run\" \"{}\"", request.installer_path);
    assert!(
        debug_output.contains(&expected_tail),
        "expected `{expected_tail}` in command, got: {debug_output}"
    );
    assert!(
        debug_output.contains(&request.proton_path),
        "expected proton path in command, got: {debug_output}"
    );
}

#[test]
fn prepare_install_rejects_invalid_request_without_provisioning() {
    let temp_dir = tempdir().expect("temp dir");
    let mut request = valid_request(temp_dir.path());
    request.installer_path = String::new();

    let error = prepare_install(&request).expect_err("invalid request");

    assert!(matches!(
        error,
        InstallGameError::Validation(InstallGameValidationError::InstallerPathRequired)
    ));
    assert!(!Path::new(&request.prefix_path).exists());
}

#[test]
fn prepare_install_rejects_prefix_path_that_is_a_file() {
    let temp_dir = tempdir().expect("temp dir");
    let mut request = valid_request(temp_dir.path());
    let file_prefix = temp_dir.path().join("prefix-file");
    fs::write(&file_prefix, b"not a dir").expect("prefix file");
    request.prefix_path = file_prefix.to_string_lossy().into_owned();

    assert!(matches!(
        prepare_install(&request),
        Err(InstallGameError::Validation(
            InstallGameValidationError::PrefixPathNotDirectory
        ))
    ));
    assert!(file_prefix.is_file(), "the file must be left untouched");
}

#[tokio::test]
async fn finish_install_after_prepared_command_matches_install_game_result() {
    let temp_dir = tempdir().expect("temp dir");
    let request = valid_request(temp_dir.path());
    let log_path = temp_dir.path().join("install.log");

    let PreparedInstall {
        mut command,
        prefix_path,
    } = prepare_install(&request).expect("prepare install");
    let status = command
        .spawn()
        .expect("spawn fake installer")
        .wait()
        .await
        .expect("wait fake installer");
    assert!(status.success(), "fake installer failed: {status:?}");

    let result = finish_install(&request, &prefix_path, &log_path);

    assert_discovered_example_game(&result, temp_dir.path(), &prefix_path, &log_path);
}

#[test]
fn finish_install_prefers_configured_executable_over_discovery() {
    let temp_dir = tempdir().expect("temp dir");
    let mut request = valid_request(temp_dir.path());
    let configured = temp_dir.path().join("Configured/Game.exe");
    request.installed_game_executable_path = configured.to_string_lossy().into_owned();
    let prefix_path = PathBuf::from(&request.prefix_path);
    fs::create_dir_all(&prefix_path).expect("prefix dir");

    let result = finish_install(&request, &prefix_path, Path::new("/tmp/install.log"));

    assert_eq!(
        result.profile.game.executable_path,
        configured.to_string_lossy()
    );
    assert_eq!(
        result.profile.runtime.working_directory,
        temp_dir.path().join("Configured").to_string_lossy()
    );
    assert!(result.discovered_game_executable_candidates.is_empty());
}

#[test]
fn install_command_flatpak_argv_is_lifecycle_bound_proton_run() {
    let temp_dir = tempdir().expect("temp dir");
    let request = valid_request(temp_dir.path());
    let prefix_path = PathBuf::from(&request.prefix_path);

    let command = build_install_command(&request, &prefix_path, true);
    let argv = command_argv(&command);

    assert_eq!(
        &argv[..4],
        ["flatpak-spawn", "--host", "--watch-bus", "--clear-env"]
    );
    let expected_directory = format!("--directory={}", temp_dir.path().display());
    assert!(
        argv.contains(&expected_directory),
        "expected {expected_directory} in {argv:?}"
    );
    let expected_prefix_env = format!(
        "--env=WINEPREFIX={}",
        crate::launch::runtime_helpers::resolve_wine_prefix_path(&prefix_path).display()
    );
    assert!(
        argv.contains(&expected_prefix_env),
        "expected {expected_prefix_env} in {argv:?}"
    );
    assert_eq!(
        &argv[argv.len() - 3..],
        [
            request.proton_path.as_str(),
            "run",
            request.installer_path.as_str()
        ]
    );
    assert!(command.as_std().get_current_dir().is_none());
}

#[test]
fn install_command_native_runs_proton_in_installer_directory() {
    let temp_dir = tempdir().expect("temp dir");
    let request = valid_request(temp_dir.path());
    let prefix_path = PathBuf::from(&request.prefix_path);

    let command = build_install_command(&request, &prefix_path, false);

    assert_eq!(
        command_argv(&command),
        [
            request.proton_path.as_str(),
            "run",
            request.installer_path.as_str()
        ]
    );
    assert_eq!(command.as_std().get_current_dir(), Some(temp_dir.path()));
    let has_compat_env = command
        .as_std()
        .get_envs()
        .any(|(key, value)| key == OsStr::new("STEAM_COMPAT_DATA_PATH") && value.is_some());
    assert!(has_compat_env, "native command must carry Proton env");
}

#[test]
fn resolve_prefix_root_with_host_override_returns_override_path() {
    let override_path = PathBuf::from("/h/.local/share/crosshook/prefixes");
    let result =
        resolve_prefix_root_with(Some(override_path.clone())).expect("resolve with override");
    assert_eq!(result, override_path);
}

#[test]
fn resolve_prefix_root_without_override_falls_through_to_base_dirs() {
    let result = resolve_prefix_root_with(None).expect("resolve without override");
    assert!(
        result.ends_with(DEFAULT_PREFIX_ROOT_SEGMENT),
        "expected path ending with '{}', got '{}'",
        DEFAULT_PREFIX_ROOT_SEGMENT,
        result.display()
    );
}
