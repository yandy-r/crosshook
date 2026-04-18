use std::collections::BTreeMap;
use std::path::PathBuf;

use super::super::gateway::{
    flatpak_custom_env_directory_with, host_command_with,
    host_command_with_env_and_directory_inner, host_command_with_env_inner, host_std_command_with,
    host_std_command_with_env_inner,
};

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
    let envs = BTreeMap::from([
        ("WINEPREFIX".to_string(), "/home/alice/.wine".to_string()),
        ("DXVK_ASYNC".to_string(), "1".to_string()),
    ]);
    let cmd = host_command_with_env_inner("wine", &envs, &BTreeMap::new(), true);
    let std_cmd = cmd.as_std();
    assert_eq!(std_cmd.get_program(), "flatpak-spawn");
    let args: Vec<&std::ffi::OsStr> = std_cmd.get_args().collect();
    assert_eq!(args[0], std::ffi::OsStr::new("--host"));
    assert!(args
        .iter()
        .any(|arg| *arg == std::ffi::OsStr::new("--env=DXVK_ASYNC=1")));
    assert!(args
        .iter()
        .any(|arg| *arg == std::ffi::OsStr::new("--env=WINEPREFIX=/home/alice/.wine")));
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
    assert!(args
        .iter()
        .any(|arg| { *arg == std::ffi::OsStr::new("--directory=/mnt/games/The Witcher 3") }));
    assert!(args
        .iter()
        .any(|arg| *arg == std::ffi::OsStr::new("--env=DXVK_ASYNC=1")));
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
    let envs = BTreeMap::from([("DXVK_ASYNC".to_string(), "1".to_string())]);
    let cmd = host_command_with_env_inner("wine", &envs, &BTreeMap::new(), false);
    let std_cmd = cmd.as_std();
    assert_eq!(std_cmd.get_program(), "wine");
    let args: Vec<&std::ffi::OsStr> = std_cmd.get_args().collect();
    assert!(args.is_empty(), "expected no extra args for non-flatpak");
    let envs_on_cmd: Vec<(&std::ffi::OsStr, Option<&std::ffi::OsStr>)> =
        std_cmd.get_envs().collect();
    assert!(envs_on_cmd.iter().any(|(key, value)| {
        *key == std::ffi::OsStr::new("DXVK_ASYNC") && *value == Some(std::ffi::OsStr::new("1"))
    }));
}

#[test]
fn flatpak_custom_env_directory_prefers_xdg_cache_home() {
    let directory = flatpak_custom_env_directory_with(
        Some(PathBuf::from("/home/alice/.cache")),
        Some(PathBuf::from("/home/alice")),
    );

    assert_eq!(directory, PathBuf::from("/home/alice/.cache/crosshook"));
}

#[test]
fn flatpak_custom_env_directory_falls_back_to_home_cache() {
    let directory = flatpak_custom_env_directory_with(None, Some(PathBuf::from("/home/alice")));

    assert_eq!(directory, PathBuf::from("/home/alice/.cache/crosshook"));
}

#[test]
fn flatpak_custom_env_directory_falls_back_to_temp_dir() {
    let directory = flatpak_custom_env_directory_with(None, None);
    assert_eq!(directory, std::env::temp_dir().join("crosshook"));
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
    assert!(args
        .iter()
        .any(|arg| *arg == std::ffi::OsStr::new("--env=DXVK_ASYNC=1")));
    assert!(args
        .iter()
        .any(|arg| *arg == std::ffi::OsStr::new("--env=WINEPREFIX=/home/alice/.wine")));
    assert_eq!(*args.last().unwrap(), std::ffi::OsStr::new("wine"));
}

#[test]
fn host_std_command_with_env_uses_envs_method_when_not_flatpak() {
    let envs = BTreeMap::from([("DXVK_ASYNC".to_string(), "1".to_string())]);
    let cmd = host_std_command_with_env_inner("wine", &envs, &BTreeMap::new(), false);
    assert_eq!(cmd.get_program(), "wine");
    let args: Vec<&std::ffi::OsStr> = cmd.get_args().collect();
    assert!(args.is_empty(), "expected no extra args for non-flatpak");
    let envs_on_cmd: Vec<(&std::ffi::OsStr, Option<&std::ffi::OsStr>)> = cmd.get_envs().collect();
    assert!(envs_on_cmd.iter().any(|(key, value)| {
        *key == std::ffi::OsStr::new("DXVK_ASYNC") && *value == Some(std::ffi::OsStr::new("1"))
    }));
}
