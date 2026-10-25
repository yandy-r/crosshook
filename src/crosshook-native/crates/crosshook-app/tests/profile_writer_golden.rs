//! Golden byte compatibility for `ProfileWriter` (YAN-811 D15).
//!
//! Each scenario seeds `<profiles>/Alpha.toml` from `fixtures/profile_writer/seed.toml`,
//! drives the writer facade, and compares the written TOML byte-for-byte with
//! the fixture captured from the pre-refactor Tauri commands. The fixtures are
//! frozen: a mismatch is a writer regression, never a reason to regenerate them.

use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use crosshook_app::{EventBus, ProfileWriter};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::SettingsStore;
use tempfile::TempDir;

const SEED: &[u8] = include_bytes!("fixtures/profile_writer/seed.toml");
const SAVE: &[u8] = include_bytes!("fixtures/profile_writer/save.toml");
const RENAME: &[u8] = include_bytes!("fixtures/profile_writer/rename.toml");
const DUPLICATE: &[u8] = include_bytes!("fixtures/profile_writer/duplicate.toml");
const ROLLBACK: &[u8] = include_bytes!("fixtures/profile_writer/rollback.toml");

/// A process-wide tempdir installed as `HOME` on first use. The seed is a
/// `proton_run` profile without a Steam compatdata path, so rename launcher
/// cleanup resolves the launcher directory from `$HOME`; this keeps it away
/// from the real user's launchers.
static ISOLATED_HOME: LazyLock<TempDir> = LazyLock::new(|| {
    let home = tempfile::tempdir().expect("home tempdir");
    std::env::set_var("HOME", home.path());
    home
});

struct Scenario {
    _root: TempDir,
    profiles: ProfileStore,
    writer: ProfileWriter,
}

impl Scenario {
    /// Tempdir stores, in-memory metadata, and `Alpha.toml` copied from the seed.
    fn seeded() -> Self {
        LazyLock::force(&ISOLATED_HOME);
        let root = tempfile::tempdir().expect("scenario tempdir");
        let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
        let settings = SettingsStore::with_base_path(root.path().join("config"));
        let metadata = MetadataStore::open_in_memory().expect("in-memory metadata");
        fs::create_dir_all(&profiles.base_path).expect("profiles dir");
        fs::write(profiles.base_path.join("Alpha.toml"), SEED).expect("seed profile");
        let writer = ProfileWriter::new(profiles.clone(), metadata, settings, EventBus::default());
        Self {
            _root: root,
            profiles,
            writer,
        }
    }

    /// Full editor save of `Alpha` with `game.name` changed. The editor's
    /// copy crosses IPC, which drops unknown keys (`clear_extra`).
    async fn save_alpha_named(&self, game_name: &str) -> Option<i64> {
        let mut profile = self.profiles.load("Alpha").expect("load Alpha");
        profile.clear_extra();
        profile.game.name = game_name.to_string();
        self.writer
            .save("Alpha".to_string(), profile)
            .await
            .expect("save Alpha")
            .new_revision_id
    }

    fn assert_profile_bytes(&self, name: &str, expected: &[u8]) {
        let path = self.profiles.base_path.join(format!("{name}.toml"));
        assert_golden(&path, expected);
    }
}

fn assert_golden(path: &Path, expected: &[u8]) {
    let actual = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(
        actual == expected,
        "{} differs from the frozen golden fixture\n--- expected ---\n{}\n--- actual ---\n{}",
        path.display(),
        String::from_utf8_lossy(expected),
        String::from_utf8_lossy(&actual),
    );
}

#[tokio::test]
async fn save_matches_pre_refactor_bytes() {
    let scenario = Scenario::seeded();
    scenario.save_alpha_named("Alpha Updated").await;
    scenario.assert_profile_bytes("Alpha", SAVE);
}

#[tokio::test]
async fn rename_matches_pre_refactor_bytes() {
    let scenario = Scenario::seeded();
    scenario
        .writer
        .rename("Alpha".to_string(), "Alpha Renamed".to_string())
        .await
        .expect("rename Alpha");
    assert!(!scenario.profiles.base_path.join("Alpha.toml").exists());
    scenario.assert_profile_bytes("Alpha Renamed", RENAME);
}

#[tokio::test]
async fn duplicate_matches_pre_refactor_bytes() {
    let scenario = Scenario::seeded();
    let result = scenario
        .writer
        .duplicate("Alpha".to_string())
        .await
        .expect("duplicate Alpha")
        .into_output();
    assert_eq!(result.name, "Alpha (Copy)");
    scenario.assert_profile_bytes("Alpha (Copy)", DUPLICATE);
    scenario.assert_profile_bytes("Alpha", SEED);
}

#[tokio::test]
async fn rollback_matches_pre_refactor_bytes() {
    let scenario = Scenario::seeded();
    let saved_revision = scenario
        .save_alpha_named("Alpha Updated")
        .await
        .expect("save captures a revision");
    scenario.save_alpha_named("Alpha Broken").await;

    scenario
        .writer
        .config_rollback("Alpha".to_string(), saved_revision)
        .await
        .expect("rollback Alpha");
    scenario.assert_profile_bytes("Alpha", ROLLBACK);
}
