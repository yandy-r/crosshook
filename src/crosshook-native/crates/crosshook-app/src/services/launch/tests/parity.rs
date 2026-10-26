//! AC2 / D11: compare effective trainer inputs on the host and direct builders in Flatpak.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use crosshook_core::launch::runtime_helpers::{build_gamescope_args, host_environment_map};
use crosshook_core::launch::{LaunchRequest, SessionKind, METHOD_PROTON_RUN};
use crosshook_core::profile::GamescopeConfig;
use crosshook_core::settings::UmuPreference;

use super::super::{gamescope_required, pid_capture_required, LaunchEnv};
use super::helpers::{CapturedCommand, Harness, Metadata, Script};

fn twin_requests(harness: &Harness) -> (LaunchRequest, LaunchRequest, PathBuf) {
    let mut steam = harness.steam_request("trainer-parity");
    let working_directory = harness.root.path().join("trainer-workdir");
    fs::create_dir_all(&working_directory).unwrap();
    steam.runtime.working_directory = working_directory.to_string_lossy().into_owned();
    steam
        .custom_env_vars
        .insert("DXVK_HUD".into(), "fps".into());
    // The trainer override deliberately differs from the disabled game config.
    // allow_nested avoids reading the test runner's live gamescope environment.
    steam.trainer_gamescope = Some(GamescopeConfig {
        enabled: true,
        allow_nested: true,
        output_width: Some(800),
        output_height: Some(400),
        ..Default::default()
    });
    // Flatpak Steam trainers force UMU off. The twin must opt out explicitly.
    steam.umu_preference = UmuPreference::Proton;
    let mut proton = steam.clone();
    proton.method = METHOD_PROTON_RUN.into();
    proton.runtime.prefix_path = steam.steam.compatdata_path.clone();
    proton.runtime.proton_path = steam.steam.proton_path.clone();
    (steam, proton, working_directory)
}

/// Remove only the same live host passthrough keys as core's command-shape
/// parity test; the builders must still forward exactly the same key set.
fn stable_env(command: &CapturedCommand) -> (BTreeMap<OsString, Option<OsString>>, Vec<OsString>) {
    let mut env = command.envs.clone();
    let host_keys = host_environment_map()
        .into_keys()
        .map(OsString::from)
        .filter(|key| env.remove(key).is_some())
        .collect();
    (env, host_keys)
}

fn argument<'a>(command: &'a CapturedCommand, flag: &str) -> &'a OsStr {
    command
        .args
        .windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_os_str())
        .unwrap_or_else(|| panic!("missing helper argument {flag}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flatpak_steam_trainer_matches_proton_twin_env_directory_and_gamescope() {
    // Core builders detect the actual sandbox; injected LaunchEnv selects only
    // the service branch. On a host, path normalization is safely the identity.
    if Path::new("/.flatpak-info").exists() {
        return;
    }
    let harness = Harness::new(Metadata::Disabled);
    let env = LaunchEnv {
        is_flatpak: true,
        inside_gamescope_session: false,
    };
    let service = harness.service.clone().with_env(env);
    let (steam, proton, working_directory) = twin_requests(&harness);
    assert_eq!(
        steam.resolved_trainer_gamescope(),
        proton.resolved_trainer_gamescope()
    );
    assert_eq!(
        gamescope_required(&steam, SessionKind::Trainer, env),
        gamescope_required(&proton, SessionKind::Trainer, env)
    );
    assert!(gamescope_required(&steam, SessionKind::Trainer, env));
    assert_eq!(
        pid_capture_required(&steam, SessionKind::Trainer, env),
        pid_capture_required(&proton, SessionKind::Trainer, env)
    );
    assert!(pid_capture_required(&steam, SessionKind::Trainer, env));

    let steam_session = service.start_trainer(steam).await.unwrap();
    let proton_session = service.start_trainer(proton).await.unwrap();
    let steam_log = steam_session.log_path.clone();
    let proton_log = proton_session.log_path.clone();
    let commands = harness.spawner.captured();
    assert_eq!(commands.len(), 2);
    let steam = &commands[0];
    let proton = &commands[1];
    assert_eq!(steam.program, proton.program);
    assert_eq!(
        steam.args, proton.args,
        "same direct Proton/gamescope builder"
    );
    assert_eq!(stable_env(steam), stable_env(proton));
    assert_eq!(steam.current_dir, Some(working_directory));
    assert_eq!(steam.current_dir, proton.current_dir);
    assert_eq!(
        steam.envs.get(OsStr::new("DXVK_HUD")),
        Some(&Some(OsString::from("fps")))
    );
    assert!(
        !steam.envs.contains_key(OsStr::new("PROTON_VERB")),
        "UMU is disabled"
    );
    tokio::join!(
        Harness::collect(steam_session),
        Harness::collect(proton_session)
    );
    // Steam requests slug on the shared app id, so logs may coincide with
    // another parity test's file; only the captured commands are asserted.
    let _ = fs::remove_file(steam_log);
    let _ = fs::remove_file(proton_log);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_steam_helper_passes_equivalent_trainer_env_directory_and_gamescope() {
    if Path::new("/.flatpak-info").exists() {
        return;
    }
    let harness = Harness::new(Metadata::Disabled);
    let env = LaunchEnv {
        is_flatpak: false,
        inside_gamescope_session: false,
    };
    let service = harness.service.clone().with_env(env);
    let (steam, proton, working_directory) = twin_requests(&harness);
    let gamescope = steam.resolved_trainer_gamescope();
    assert_eq!(gamescope, proton.resolved_trainer_gamescope());
    let expected_gamescope_args: Vec<OsString> = build_gamescope_args(&gamescope)
        .into_iter()
        .map(OsString::from)
        .collect();
    // Host Steam runs a helper, not a direct gamescope child; the service's
    // watchdog predicate remains false for that branch (D8).
    assert!(!gamescope_required(&steam, SessionKind::Trainer, env));
    assert!(gamescope_required(&proton, SessionKind::Trainer, env));
    assert!(!pid_capture_required(&steam, SessionKind::Trainer, env));
    assert!(!pid_capture_required(&proton, SessionKind::Trainer, env));
    harness
        .spawner
        .push(Script::log_file("printf 'steam-helper\\n'"));
    let steam_session = service.start_trainer(steam).await.unwrap();
    let proton_session = service.start_trainer(proton).await.unwrap();
    let steam_log = steam_session.log_path.clone();
    let proton_log = proton_session.log_path.clone();
    let commands = harness.spawner.captured();
    assert_eq!(commands.len(), 2);
    let steam = &commands[0];
    let proton = &commands[1];
    assert_eq!(steam.program, OsStr::new("/bin/bash"));
    assert!(steam.args[0]
        .to_string_lossy()
        .ends_with("steam-launch-trainer.sh"));
    assert_eq!(
        argument(steam, "--working-directory"),
        working_directory.as_os_str()
    );
    assert_eq!(
        proton.current_dir.as_deref(),
        Some(working_directory.as_path())
    );
    assert_eq!(
        Some(Path::new(argument(steam, "--working-directory"))),
        proton.current_dir.as_deref()
    );
    for command in [steam, proton] {
        assert_eq!(
            command.envs.get(OsStr::new("DXVK_HUD")),
            Some(&Some(OsString::from("fps")))
        );
    }
    // These are the helper's effective Proton inputs, not its shell argv.
    for key in [
        "STEAM_COMPAT_DATA_PATH",
        "STEAM_COMPAT_CLIENT_INSTALL_PATH",
        "WINEPREFIX",
    ] {
        assert_eq!(
            steam.envs.get(OsStr::new(key)),
            proton.envs.get(OsStr::new(key)),
            "{key}"
        );
    }
    assert!(steam.args.iter().any(|arg| arg == "--gamescope-enabled"));
    assert!(steam
        .args
        .iter()
        .any(|arg| arg == "--gamescope-allow-nested"));
    let helper_gamescope_args: Vec<_> = steam
        .args
        .windows(2)
        .filter(|pair| pair[0] == "--gamescope-arg")
        .map(|pair| pair[1].clone())
        .collect();
    assert_eq!(helper_gamescope_args, expected_gamescope_args);
    assert!(
        proton
            .args
            .windows(expected_gamescope_args.len())
            .any(|args| args == expected_gamescope_args),
        "direct command carries the same gamescope config"
    );
    tokio::join!(
        Harness::collect(steam_session),
        Harness::collect(proton_session)
    );
    let _ = fs::remove_file(steam_log);
    let _ = fs::remove_file(proton_log);
}
