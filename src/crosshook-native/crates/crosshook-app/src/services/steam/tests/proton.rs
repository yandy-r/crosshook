use std::ffi::OsString;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::steam::{SteamAutoPopulateFieldState, SteamAutoPopulateRequest};

use super::super::client::default_client_install_path_with_env;
use super::helpers::{create_proton, service};

#[test]
fn resolver_preserves_override_and_handles_absent_home() {
    let override_path = default_client_install_path_with_env(|key| {
        (key == "STEAM_COMPAT_CLIENT_INSTALL_PATH").then(|| OsString::from("  /not/installed  "))
    });
    assert_eq!(override_path, "/not/installed");
    assert_eq!(default_client_install_path_with_env(|_| None), "");
    assert_eq!(
        default_client_install_path_with_env(|key| {
            (key == "STEAM_COMPAT_CLIENT_INSTALL_PATH").then(|| OsString::from(" \t "))
        }),
        ""
    );
}

#[test]
fn resolver_probes_directory_candidates_in_frozen_order() {
    let temp = tempfile::tempdir().unwrap();
    let candidates = [
        ".local/share/Steam",
        ".steam/root",
        ".var/app/com.valvesoftware.Steam/data/Steam",
    ];
    let resolve = || {
        default_client_install_path_with_env(|key| {
            (key == "HOME").then(|| temp.path().as_os_str().to_owned())
        })
    };
    assert_eq!(resolve(), "");
    // A regular file named steamapps is not an installation.
    let first = temp.path().join(candidates[0]);
    std::fs::create_dir_all(&first).unwrap();
    std::fs::write(first.join("steamapps"), b"").unwrap();
    assert_eq!(resolve(), "");
    std::fs::remove_file(first.join("steamapps")).unwrap();
    for candidate in candidates.iter().rev() {
        let path = temp.path().join(candidate);
        std::fs::create_dir_all(path.join("steamapps")).unwrap();
        assert_eq!(resolve(), path.to_string_lossy());
    }
}

#[tokio::test]
async fn proton_discovery_is_fresh_and_uses_fixture_root() {
    let temp = tempfile::tempdir().unwrap();
    let service = service(temp.path(), MetadataStore::disabled());
    let root = temp.path().join("Steam");
    create_proton(&root, "Fixture-Proton-A");
    let scan = || Some(root.to_string_lossy().into_owned());
    let installs = service.list_proton_installs(scan()).await.unwrap();
    assert!(installs.iter().any(|tool| tool.name == "Fixture-Proton-A"));
    create_proton(&root, "Fixture-Proton-B");
    let installs = service.list_proton_installs(scan()).await.unwrap();
    assert!(installs.iter().any(|tool| tool.name == "Fixture-Proton-B"));
}

#[test]
fn proton_scan_yields_while_blocking_pool_is_gated() {
    let temp = tempfile::tempdir().unwrap();
    create_proton(temp.path(), "Fixture-Proton");
    let service = service(temp.path(), MetadataStore::disabled());
    let root = temp.path().to_string_lossy().into_owned();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let gate = runtime.spawn_blocking(move || {
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    entered_rx.recv().unwrap();
    let release = Arc::new(Mutex::new(Some(release_tx)));
    let unblock = release.clone();
    let (done_tx, done_rx) = mpsc::channel();
    // Watchdog only releases the gate if the executor stops making progress;
    // no sleeps or runtime timers are needed to detect a synchronous regression.
    let watchdog = std::thread::spawn(move || {
        if done_rx.recv_timeout(Duration::from_secs(5)).is_err() {
            if let Some(sender) = unblock.lock().unwrap().take() {
                let _ = sender.send(());
            }
        }
    });
    runtime.block_on(async {
        let scan = tokio::spawn(async move { service.list_proton_installs(Some(root)).await });
        tokio::task::yield_now().await;
        let queued_on_blocking_pool = !scan.is_finished();
        if let Some(sender) = release.lock().unwrap().take() {
            sender.send(()).unwrap();
        }
        done_tx.send(()).unwrap();
        gate.await.unwrap();
        let installs = scan.await.unwrap().unwrap();
        assert!(
            queued_on_blocking_pool,
            "scan ran synchronously on the caller thread"
        );
        assert!(installs.iter().any(|tool| tool.name == "Fixture-Proton"));
    });
    watchdog.join().unwrap();
}

#[tokio::test]
async fn auto_populate_empty_game_retains_manual_hints() {
    let temp = tempfile::tempdir().unwrap();
    let result = service(temp.path(), MetadataStore::disabled())
        .auto_populate(SteamAutoPopulateRequest::default())
        .await
        .unwrap();
    assert_eq!(result.app_id_state, SteamAutoPopulateFieldState::NotFound);
    assert!(!result.has_any_match());
    assert!(!result.manual_hints.is_empty());
}

#[tokio::test]
async fn auto_populate_finds_game_prefix_and_proton_from_fixture() {
    let temp = tempfile::tempdir().unwrap();
    let steamapps = temp.path().join("steamapps");
    let game = steamapps.join("common/Test Game/game.exe");
    let prefix = steamapps.join("compatdata/12345");
    let proton = steamapps.join("common/Fixture-Proton/proton");
    std::fs::create_dir_all(game.parent().unwrap()).unwrap();
    std::fs::create_dir_all(&prefix).unwrap();
    std::fs::create_dir_all(proton.parent().unwrap()).unwrap();
    std::fs::create_dir_all(temp.path().join("config")).unwrap();
    std::fs::write(&game, b"test").unwrap();
    std::fs::write(&proton, b"#!/bin/sh\n").unwrap();
    std::fs::write(
        steamapps.join("appmanifest_12345.acf"),
        r#""AppState" { "appid" "12345" "installdir" "Test Game" }"#,
    )
    .unwrap();
    std::fs::write(
        temp.path().join("config/config.vdf"),
        r#""root" { "CompatToolMapping" { "12345" { "name" "Fixture-Proton" } } }"#,
    )
    .unwrap();
    let result = service(temp.path(), MetadataStore::disabled())
        .auto_populate(SteamAutoPopulateRequest {
            game_path: game,
            steam_client_install_path: temp.path().to_path_buf(),
        })
        .await
        .unwrap();
    assert_eq!(result.app_id_state, SteamAutoPopulateFieldState::Found);
    assert_eq!(result.app_id, "12345");
    assert_eq!(result.compatdata_state, SteamAutoPopulateFieldState::Found);
    assert_eq!(result.compatdata_path, prefix);
    assert_eq!(result.proton_state, SteamAutoPopulateFieldState::Found);
    assert_eq!(result.proton_path, proton);
}
