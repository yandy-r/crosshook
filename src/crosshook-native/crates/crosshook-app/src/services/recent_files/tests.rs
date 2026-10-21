//! Tests for recent-files push ordering, dedupe, and cap.

use std::path::Path;

use crosshook_core::settings::{RecentFilesData, RecentFilesStore, SettingsStore};
use tempfile::{tempdir, TempDir};

use super::{RecentFileKind, RecentFilesService};
use crate::AppErrorKind;

struct Fixture {
    temp: TempDir,
    service: RecentFilesService,
    store: RecentFilesStore,
}

/// Service with `recent_files_limit = limit`.
fn fixture(limit: u32) -> Fixture {
    let temp = tempdir().unwrap();
    let settings = SettingsStore::with_base_path(temp.path().join("config"));
    settings
        .update(|s| {
            s.recent_files_limit = limit;
            Ok::<_, ()>(())
        })
        .unwrap()
        .unwrap();
    let store = RecentFilesStore::with_path(temp.path().join("recent.toml"));
    Fixture {
        service: RecentFilesService::new(settings, store.clone()),
        store,
        temp,
    }
}

/// Creates an existing file (load drops missing paths) and returns its path.
fn touch(dir: &Path, name: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, b"").unwrap();
    path.display().to_string()
}

#[tokio::test]
async fn push_moves_to_front_dedupes_and_caps() {
    let f = fixture(3);
    let [a, b, c, d] = ["a.exe", "b.exe", "c.exe", "d.exe"].map(|n| touch(f.temp.path(), n));

    for path in [&a, &b, &c] {
        f.service.push(RecentFileKind::Game, path).await.unwrap();
    }
    assert_eq!(
        f.service.load().await.unwrap().game_paths,
        vec![c.clone(), b.clone(), a.clone()]
    );

    // Re-pushing an existing path (with whitespace) moves it to the front once.
    let data = f
        .service
        .push(RecentFileKind::Game, &format!("  {a} "))
        .await
        .unwrap();
    assert_eq!(data.game_paths, vec![a.clone(), c.clone(), b.clone()]);

    // A fourth distinct path evicts the oldest entry.
    let data = f.service.push(RecentFileKind::Game, &d).await.unwrap();
    assert_eq!(data.game_paths, vec![d.clone(), a.clone(), c.clone()]);
    assert_eq!(f.service.load().await.unwrap().game_paths, data.game_paths);
}

/// Concurrent pushes through service clones must not lose updates.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_pushes_through_clones_keep_every_path() {
    let f = fixture(100);
    let paths: Vec<String> = (0..24)
        .map(|i| touch(f.temp.path(), &format!("g{i}.exe")))
        .collect();
    let tasks: Vec<_> = paths
        .iter()
        .cloned()
        .map(|path| {
            let service = f.service.clone();
            tokio::spawn(async move { service.push(RecentFileKind::Game, &path).await })
        })
        .collect();
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    let mut stored = f.service.load().await.unwrap().game_paths;
    stored.sort();
    let mut expected = paths;
    expected.sort();
    assert_eq!(stored, expected);
}

#[tokio::test]
async fn push_targets_only_the_requested_list() {
    let f = fixture(10);
    let trainer = touch(f.temp.path(), "t.exe");
    let dll = touch(f.temp.path(), "x.dll");
    f.service
        .push(RecentFileKind::Trainer, &trainer)
        .await
        .unwrap();
    let data = f.service.push(RecentFileKind::Dll, &dll).await.unwrap();
    assert!(data.game_paths.is_empty());
    assert_eq!(data.trainer_paths, vec![trainer]);
    assert_eq!(data.dll_paths, vec![dll]);
}

#[tokio::test]
async fn push_rejects_empty_path() {
    let f = fixture(10);
    let error = f
        .service
        .push(RecentFileKind::Game, "   ")
        .await
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Validation { field: "path" });
    assert!(!f.store.path.exists());
}

#[tokio::test]
async fn save_preserves_unknown_keys_and_load_hides_them() {
    let f = fixture(2);
    std::fs::write(&f.store.path, "future = 1\n").unwrap();
    let paths = ["a", "b", "c"].map(|n| touch(f.temp.path(), n));
    let incoming = RecentFilesData {
        game_paths: paths.to_vec(),
        extra: toml::Table::from_iter([("injected".to_string(), true.into())]),
        ..Default::default()
    };
    f.service.save(incoming).await.unwrap();

    let on_disk = std::fs::read_to_string(&f.store.path).unwrap();
    assert!(on_disk.contains("future = 1"));
    assert!(!on_disk.contains("injected"));

    let loaded = f.service.load().await.unwrap();
    assert!(loaded.extra.is_empty());
    assert_eq!(loaded.game_paths, paths[..2].to_vec());
}
