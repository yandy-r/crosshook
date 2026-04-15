//! PR #148 non-regression smoke: under `UmuPreference::Umu`, game + trainer
//! commands can spawn concurrently and both PIDs are alive at t+500ms.
//!
//! Uses a stub `umu-run` shell script on a scoped PATH — no real umu install
//! required. Runs in <3s.
//!
//! The library crate is compiled **without** `#[cfg(test)]` for integration
//! tests, so `resolve_umu_run_path()` falls through to read the real `PATH`
//! env (the `#[cfg(test)]` test-override hook is absent). We prepend a tempdir
//! containing the stub to `PATH` before calling the builders.
//!
//! Run with:
//!   cargo test --manifest-path src/crosshook-native/Cargo.toml \
//!       -p crosshook-core --test umu_concurrent_pids

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use crosshook_core::launch::request::{LaunchRequest, RuntimeLaunchConfig};
use crosshook_core::launch::script_runner::{
    build_proton_game_command, build_proton_trainer_command,
};
use crosshook_core::settings::UmuPreference;

#[tokio::test]
async fn umu_concurrent_pids_game_and_trainer_both_alive() {
    // 1. Stub umu-run shell script on a tempdir.
    //    Sleeps 5s so we can observe both PIDs alive at t+500ms.
    let tmp = tempfile::tempdir().unwrap();
    let stub_path = tmp.path().join("umu-run");
    std::fs::write(
        &stub_path,
        "#!/bin/sh\n# Stub umu-run: sleeps so PIDs are observable.\nsleep 5\n",
    )
    .unwrap();
    std::fs::set_permissions(&stub_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    // 2. Prepend tempdir to PATH so `resolve_umu_run_path()` finds the stub.
    //    Integration test binaries are single-threaded by default; `std::env::set_var`
    //    is safe here.  We restore PATH at the end regardless.
    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let mut new_path = std::ffi::OsString::new();
    new_path.push(tmp.path());
    new_path.push(":");
    new_path.push(&old_path);
    // SAFETY: this integration test binary runs serially. No other threads read
    // PATH concurrently at this point.
    unsafe { std::env::set_var("PATH", &new_path) };

    // 3. Build a minimal game request with UmuPreference::Umu.
    //    proton_path is a string-only field; the builder does not stat it when
    //    use_umu=true (it becomes an env var, not the spawned program).
    let log_game = tmp.path().join("game.log");
    let game_request = LaunchRequest {
        method: "proton_run".to_string(),
        game_path: "/bin/true".to_string(),
        umu_preference: UmuPreference::Umu,
        network_isolation: false,
        runtime: RuntimeLaunchConfig {
            prefix_path: tmp.path().join("pfx").to_string_lossy().into_owned(),
            proton_path: tmp
                .path()
                .join("proton-fake/proton")
                .to_string_lossy()
                .into_owned(),
            working_directory: tmp.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        ..Default::default()
    };

    // 4. Build a trainer request.  trainer_loading_mode defaults to
    //    SourceDirectory so the builder does not try to copy any files.
    let log_trainer = tmp.path().join("trainer.log");
    let trainer_request = LaunchRequest {
        trainer_path: "/bin/true".to_string(),
        trainer_host_path: "/bin/true".to_string(),
        network_isolation: false,
        ..game_request.clone()
    };

    // 5. Build commands via Phase 3 builders.  The stub umu-run is on PATH so
    //    `should_use_umu` returns `(true, Some(stub_path))`.
    let mut game_cmd = build_proton_game_command(&game_request, &log_game)
        .expect("build_proton_game_command must succeed with stub umu-run on PATH");
    let mut trainer_cmd = build_proton_trainer_command(&trainer_request, &log_trainer)
        .expect("build_proton_trainer_command must succeed with stub umu-run on PATH");

    // Shed stdio so the test doesn't block on pipes.
    use std::process::Stdio;
    game_cmd.stdin(Stdio::null());
    game_cmd.stdout(Stdio::null());
    game_cmd.stderr(Stdio::null());
    trainer_cmd.stdin(Stdio::null());
    trainer_cmd.stdout(Stdio::null());
    trainer_cmd.stderr(Stdio::null());

    // 6. Spawn both processes.
    let mut game_child = game_cmd.spawn().expect("game process must spawn");
    let mut trainer_child = trainer_cmd.spawn().expect("trainer process must spawn");

    let game_pid = game_child.id();
    let trainer_pid = trainer_child.id();

    // 7. Sleep 500ms, then assert both stub processes are still alive.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let game_alive = game_child.try_wait().unwrap().is_none();
    let trainer_alive = trainer_child.try_wait().unwrap().is_none();

    // 8. Clean up regardless of assertion outcome.
    let _ = game_child.kill().await;
    let _ = trainer_child.kill().await;
    let _ = game_child.wait().await;
    let _ = trainer_child.wait().await;

    // 9. Restore PATH.
    unsafe { std::env::set_var("PATH", &old_path) };

    assert!(
        game_alive,
        "game PID {game_pid:?} exited before t+500ms — PR #148 regression? \
         Check that build_proton_game_command uses the stub umu-run and not a \
         direct /bin/true (which exits immediately)."
    );
    assert!(
        trainer_alive,
        "trainer PID {trainer_pid:?} exited before t+500ms — PR #148 regression? \
         Check that build_proton_trainer_command uses the stub umu-run and not a \
         direct /bin/true (which exits immediately)."
    );
}
