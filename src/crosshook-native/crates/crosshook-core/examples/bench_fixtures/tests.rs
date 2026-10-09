//! Example-level regression tests. `[[example]] test = true` makes `cargo test -p crosshook-core`
//! (and `--example bench_fixtures`) run them.
//!
//! The generator guards against the real `$HOME`/XDG stores via `check_output_dir_real_env`;
//! temp dirs live under the system temp dir, which is never inside those stores.

use std::path::{Path, PathBuf};

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;

use crate::generate::{generate, BenchConfig};
use crate::guard;
use crate::layout::{snapshot_tree, Layout, TOKEN};
use crate::materialize::materialize;

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

fn gen_into(dir: &Path, cfg: &BenchConfig) -> PathBuf {
    let out = dir.join("canonical");
    generate(&out, cfg).expect("generate");
    out
}

fn assert_fk_clean(c: &rusqlite::Connection) {
    let violations = c
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query_map([], |_| Ok(()))
        .unwrap()
        .count();
    assert_eq!(violations, 0, "foreign_key_check reported violations");
}

#[test]
fn same_seed_is_byte_identical_across_out_paths() {
    let (a, b) = (tmp(), tmp());
    let cfg = BenchConfig::reduced(42);
    let (ta, tb) = (
        gen_into(a.path(), &cfg),
        gen_into(&b.path().join("nested/deeper"), &cfg),
    );
    let (sa, sb) = (snapshot_tree(&ta), snapshot_tree(&tb));
    assert_eq!(
        sa.iter().map(|(p, _)| p).collect::<Vec<_>>(),
        sb.iter().map(|(p, _)| p).collect::<Vec<_>>(),
        "file lists differ"
    );
    for ((path, x), (_, y)) in sa.iter().zip(sb.iter()) {
        assert!(x == y, "{} differs between runs", path.display());
    }
}

#[test]
fn full_fixture_metadata_is_byte_identical() {
    // Regression: reduced fixtures missed wall-clock collisions present at full scale.
    let (a, b) = (tmp(), tmp());
    let cfg = BenchConfig::full(42);
    let (ta, tb) = (gen_into(a.path(), &cfg), gen_into(b.path(), &cfg));
    let db = "data/crosshook/metadata.db";
    assert_eq!(
        std::fs::read(ta.join(db)).unwrap(),
        std::fs::read(tb.join(db)).unwrap()
    );
}

#[test]
fn different_seed_differs() {
    let (a, b) = (tmp(), tmp());
    let ta = gen_into(a.path(), &BenchConfig::reduced(1));
    let tb = gen_into(b.path(), &BenchConfig::reduced(2));
    assert_ne!(snapshot_tree(&ta), snapshot_tree(&tb));
}

#[test]
fn counts_match_config_via_public_apis() {
    let d = tmp();
    let cfg = BenchConfig::reduced(7);
    let out = gen_into(d.path(), &cfg);
    let layout = Layout::new(&out);

    let profiles = ProfileStore::with_base_path(layout.profiles_dir.clone());
    assert_eq!(profiles.list().unwrap().len(), cfg.profiles);

    let store = MetadataStore::with_path(&layout.metadata_db).unwrap();
    store
        .with_sqlite_conn("count", |c| {
            assert_fk_clean(c);
            let n = |sql: &str| -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap() };
            assert_eq!(n("SELECT COUNT(*) FROM profiles"), cfg.profiles as i64);
            assert_eq!(
                n("SELECT COUNT(*) FROM health_snapshots"),
                cfg.profiles as i64
            );
            assert_eq!(
                n("SELECT COUNT(*) FROM game_image_cache"),
                cfg.profiles as i64
            );
            assert_eq!(
                n("SELECT COUNT(*) FROM proton_release_catalog"),
                cfg.proton_rows as i64
            );
            assert_eq!(
                n("SELECT COUNT(*) FROM community_profiles"),
                cfg.community_profiles as i64
            );
            assert!(n("SELECT COUNT(*) FROM collections") >= 2);
            assert!(n("SELECT COUNT(*) FROM launch_operations WHERE status='failed'") > 0);
            Ok(())
        })
        .unwrap();
    assert!(!store.list_favorite_profiles().unwrap().is_empty());

    // Portrait lookup resolves offline: row exists, far-future expiry, file exists.
    let row = store
        .get_game_image("9900000", "portrait")
        .unwrap()
        .expect("portrait row");
    assert_eq!(row.source, "steam_cdn");
    assert!(row.expires_at.as_deref().unwrap().starts_with("2099"));
    assert!(Path::new(&row.file_path).exists() || row.file_path.starts_with(TOKEN));
}

#[test]
fn normalization_leaves_no_real_path_or_wall_clock_leaks() {
    let d = tmp();
    let out = gen_into(d.path(), &BenchConfig::reduced(3));
    let layout = Layout::new(&out);
    // Before any reopen: normalisation must leave no WAL/SHM sidecars behind.
    for (path, _) in snapshot_tree(&out) {
        let name = path.to_string_lossy().into_owned();
        assert!(
            !name.ends_with("-wal") && !name.ends_with("-shm"),
            "sidecar left: {name}"
        );
    }
    let real = out.to_string_lossy().into_owned();
    let year = chrono::Utc::now().format("%Y").to_string();

    let store = MetadataStore::with_path(&layout.metadata_db).unwrap();
    store
        .with_sqlite_conn("scan", |c| {
            let tables: Vec<String> = c
                .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
                .unwrap()
                .query_map([], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            for t in tables {
                let cols: Vec<String> = c
                    .prepare(&format!("PRAGMA table_info(\"{t}\")"))
                    .unwrap()
                    .query_map([], |r| r.get(1))
                    .unwrap()
                    .collect::<Result<_, _>>()
                    .unwrap();
                for col in cols {
                    let leaked_path: i64 = c
                        .query_row(
                            &format!("SELECT COUNT(*) FROM \"{t}\" WHERE CAST(\"{col}\" AS TEXT) LIKE ?1"),
                            [format!("%{real}%")],
                            |r| r.get(0),
                        )
                        .unwrap();
                    assert_eq!(leaked_path, 0, "{t}.{col} leaks real out path");
                    // Wall-clock year only matters while it differs from the fixed 2024 epoch.
                    if year != "2024" {
                        let leaked_ts: i64 = c
                            .query_row(
                                &format!(
                                    "SELECT COUNT(*) FROM \"{t}\" WHERE CAST(\"{col}\" AS TEXT) LIKE ?1 AND \"{col}\" NOT LIKE '2099%'"
                                ),
                                [format!("{year}-%")],
                                |r| r.get(0),
                            )
                            .unwrap();
                        assert_eq!(leaked_ts, 0, "{t}.{col} leaks wall-clock timestamp");
                    }
                }
            }
            Ok(())
        })
        .unwrap();

    // Text files carry the token, never the real path.
    for (path, bytes) in snapshot_tree(&out) {
        if path.extension().is_some_and(|e| e == "toml" || e == "json") {
            let text = String::from_utf8_lossy(&bytes);
            assert!(
                !text.contains(&real),
                "{} leaks real out path",
                path.display()
            );
        }
    }
}

#[test]
fn empty_variant_is_valid_and_migrated() {
    let d = tmp();
    let out = gen_into(d.path(), &BenchConfig::empty(42));
    let layout = Layout::new(&out);
    let store = MetadataStore::with_path(&layout.metadata_db).unwrap();
    store
        .with_sqlite_conn("empty", |c| {
            let n: i64 = c
                .query_row("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 0);
            Ok(())
        })
        .unwrap();
    assert!(ProfileStore::with_base_path(layout.profiles_dir)
        .list()
        .unwrap()
        .is_empty());
}

#[test]
fn materialize_swaps_token_in_db_and_text_and_db_opens() {
    let d = tmp();
    let out = gen_into(d.path(), &BenchConfig::reduced(5));
    let root = d.path().join("isolated");
    materialize(&out, &root).expect("materialize");

    let layout = Layout::new(&root);
    let store = MetadataStore::with_path(&layout.metadata_db).expect("reopen via with_path");
    let real = root.to_string_lossy().into_owned();
    store
        .with_sqlite_conn("verify", |c| {
            assert_fk_clean(c);
            let with_token: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM launch_operations WHERE log_path LIKE ?1",
                    [format!("%{TOKEN}%")],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(with_token, 0, "token left in db");
            let rooted: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM launch_operations WHERE log_path LIKE ?1",
                    [format!("{real}/%")],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(rooted > 0, "db paths not rewritten to isolated root");
            Ok(())
        })
        .unwrap();

    let loaded = ProfileStore::with_base_path(layout.profiles_dir.clone())
        .load("bench-game-0000")
        .unwrap();
    assert!(loaded.game.executable_path.starts_with(&real));
    assert!(!loaded.game.executable_path.contains(TOKEN));
    // Portrait image file exists where the DB row points.
    let row = store
        .get_game_image("9900000", "portrait")
        .unwrap()
        .unwrap();
    assert!(Path::new(&row.file_path).exists(), "{}", row.file_path);
}

// ── Guard regressions: refuse BEFORE any mkdir ─────────────────────────────

#[test]
fn generate_refuses_real_home_and_creates_nothing() {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return;
    };
    for target in [
        home.clone(),
        home.join(".config/crosshook"),
        home.join(".local/share/crosshook/bench"),
        home.join(".var/app/dev.crosshook.CrossHook"),
    ] {
        let existed = target.exists();
        let err = generate(&target, &BenchConfig::reduced(1)).unwrap_err();
        assert!(err.contains("refusing"), "{err}");
        assert_eq!(
            target.exists(),
            existed,
            "guard must not create {}",
            target.display()
        );
    }
}

#[test]
fn materialize_refuses_protected_root_and_symlink_and_creates_nothing() {
    let d = tmp();
    let out = gen_into(d.path(), &BenchConfig::reduced(1));

    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let target = home.join(".config/crosshook");
        let existed = target.exists();
        let err = materialize(&out, &target).unwrap_err();
        assert!(err.contains("refusing"), "{err}");
        assert_eq!(target.exists(), existed);
    }

    let real = d.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let link = d.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let err = materialize(&out, &link.join("root")).unwrap_err();
    assert!(err.contains("symlink"), "{err}");
    assert!(!real.join("root").exists());
}

#[test]
fn guard_accepts_ordinary_temp_dir() {
    let d = tmp();
    assert!(guard::check_output_dir_real_env(&d.path().join("ok")).is_ok());
}
