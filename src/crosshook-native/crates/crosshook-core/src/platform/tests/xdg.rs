use std::ffi::OsString;
use std::path::PathBuf;

use super::super::xdg::apply_xdg_host_override;
use super::common::{FakeEnv, ScopedEnv, TEST_ENV_KEY};

#[test]
fn xdg_override_sets_all_four_paths_from_home() {
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
            (
                "XDG_STATE_HOME".to_string(),
                OsString::from("/home/alice/.local/state")
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
    assert_eq!(config, &OsString::from("/home/bob/.config"));
}

#[test]
fn xdg_override_uses_exact_home_without_expansion() {
    let mut env = FakeEnv::default();
    apply_xdg_host_override(Some(PathBuf::from("/var/home/charlie")), &mut env);
    assert_eq!(env.writes[0].1, OsString::from("/var/home/charlie/.config"));
}

#[test]
fn xdg_override_prefers_host_xdg_config_home_when_set() {
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
        .find(|(key, _)| key == "XDG_CONFIG_HOME")
        .expect("XDG_CONFIG_HOME must be written");
    assert_eq!(config_write.1, OsString::from("/data/configs"));
    let data_write = env
        .writes
        .iter()
        .find(|(key, _)| key == "XDG_DATA_HOME")
        .expect("XDG_DATA_HOME must be written");
    assert_eq!(data_write.1, OsString::from("/home/alice/.local/share"));
    let state_write = env
        .writes
        .iter()
        .find(|(key, _)| key == "XDG_STATE_HOME")
        .expect("XDG_STATE_HOME must be written");
    assert_eq!(state_write.1, OsString::from("/home/alice/.local/state"));
}

#[test]
fn xdg_override_prefers_all_host_xdg_vars_when_set() {
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
            (
                "XDG_STATE_HOME".to_string(),
                OsString::from("/home/alice/.local/state")
            ),
        ]
    );
}
