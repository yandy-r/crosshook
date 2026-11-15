//! AC2 / D11: compare effective trainer inputs on the host and direct builders in Flatpak.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crosshook_core::launch::runtime_helpers::{build_gamescope_args, host_environment_map};
use crosshook_core::launch::{LaunchRequest, SessionKind, METHOD_PROTON_RUN};
use crosshook_core::profile::{GamescopeConfig, HookStage, LaunchHook};
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

/// Adds a hook at `stage` that writes its sorted environment to `<dir>/<name>.env`.
fn add_env_dump_hook(
    request: &mut LaunchRequest,
    stage: HookStage,
    dir: &Path,
    name: &str,
) -> PathBuf {
    let dump = dir.join(format!("{name}.env"));
    let script = dir.join(format!("{name}-hook.sh"));
    fs::write(
        &script,
        format!("#!/bin/sh\nenv -0 > '{}'\n", dump.display()),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let hook = LaunchHook {
        id: format!("{name}-env-dump"),
        name: format!("{name} env dump"),
        path: script.to_string_lossy().into_owned(),
        stage,
        enabled: true,
        ..Default::default()
    };
    match stage {
        HookStage::PreLaunch => request.pre_launch_hooks.push(hook),
        HookStage::PostExit => request.post_exit_hooks.push(hook),
    }
    dump
}

/// Parses NUL-separated `env -0` output, so multi-line values cannot turn
/// into bogus keys; keys only ever reach assertion messages.
fn read_env_dump(path: &Path) -> BTreeMap<String, String> {
    fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("hook did not write {}", path.display()))
        .split('\0')
        .filter_map(|entry| entry.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// Key names of `left` and `right` whose values differ. Values are never
/// reported: the dumps hold the runner's whole environment.
fn differing_keys<'a>(
    left: &'a BTreeMap<String, String>,
    right: &'a BTreeMap<String, String>,
) -> Vec<&'a String> {
    left.keys()
        .chain(right.keys())
        .filter(|key| left.get(*key) != right.get(*key))
        .collect()
}

/// YAN-1611: a Flatpak Steam trainer runs as `proton_run`, so its pre-launch
/// and post-exit hooks must see the proton_run twin's environment and
/// working directory, without the game's launch optimizations.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flatpak_steam_trainer_hooks_see_the_proton_twin_environment() {
    if Path::new("/.flatpak-info").exists() {
        return;
    }
    let harness = Harness::new(Metadata::Disabled);
    let service = harness.service.clone().with_env(LaunchEnv {
        is_flatpak: true,
        inside_gamescope_session: false,
    });
    let (mut steam, mut proton, working_directory) = twin_requests(&harness);
    let hooks_dir = harness.root.path().join("hook-parity");
    fs::create_dir_all(&hooks_dir).unwrap();
    for request in [&mut steam, &mut proton] {
        // The trainer ignores game optimizations; an unresolvable one must
        // neither reach the hooks nor fail the trainer start.
        request.optimizations.enabled_option_ids = vec![
            "disable_steam_input".into(),
            "not_a_real_optimization".into(),
        ];
    }
    let steam_dump = add_env_dump_hook(&mut steam, HookStage::PreLaunch, &hooks_dir, "steam");
    let proton_dump = add_env_dump_hook(&mut proton, HookStage::PreLaunch, &hooks_dir, "proton");
    let steam_post = add_env_dump_hook(&mut steam, HookStage::PostExit, &hooks_dir, "steam-post");

    let steam_session = service.start_trainer(steam).await.unwrap();
    let proton_session = service.start_trainer(proton).await.unwrap();
    let steam_log = steam_session.log_path.clone();
    let proton_log = proton_session.log_path.clone();
    tokio::join!(
        Harness::collect(steam_session),
        Harness::collect(proton_session)
    );

    let steam_env = read_env_dump(&steam_dump);
    let proton_env = read_env_dump(&proton_dump);
    let differing = differing_keys(&steam_env, &proton_env);
    assert!(differing.is_empty(), "hook env keys differ: {differing:?}");
    let post_env = read_env_dump(&steam_post);
    let differing = differing_keys(&steam_env, &post_env);
    assert!(
        differing.is_empty(),
        "post-exit hook env keys differ: {differing:?}"
    );
    // The runner may export it itself; the hook must not add it.
    assert_eq!(
        steam_env.contains_key("PROTON_NO_STEAMINPUT"),
        std::env::var_os("PROTON_NO_STEAMINPUT").is_some(),
        "game optimization leaked into the trainer hook env"
    );
    for key in [
        "STEAM_COMPAT_DATA_PATH",
        "WINEPREFIX",
        "STEAM_COMPAT_LIBRARY_PATHS",
        "PRESSURE_VESSEL_FILESYSTEMS_RW",
    ] {
        assert!(
            steam_env.contains_key(key),
            "{key} missing from the hook env"
        );
    }
    assert_eq!(
        steam_env.get("PWD").map(String::as_str),
        Some(working_directory.to_string_lossy().as_ref()),
        "hook working directory"
    );
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
    // watchdog predicate remains false for that branch.
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
