//! Golden byte compatibility for `ProfileWriter` (YAN-811 D15).
//!
//! Each scenario seeds `<profiles>/Alpha.toml` from `fixtures/profile_writer/seed.toml`,
//! drives the writer facade, and compares the written TOML byte-for-byte with
//! the fixture captured from the pre-refactor Tauri commands. The fixtures are
//! frozen: a mismatch is a writer regression, never a reason to regenerate them.
//!
//! This is a `harness = false` binary. The seed is a `proton_run` profile without
//! a Steam compatdata path, so rename launcher cleanup resolves the launcher
//! directory from `$HOME`. `main` points `HOME` at a tempdir before any runtime
//! or other thread exists, then runs the scenarios sequentially, so no thread can
//! observe the environment while it changes and the real user's launchers are
//! never touched.

use std::fs;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::pin::Pin;

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

struct Scenario {
    _root: TempDir,
    profiles: ProfileStore,
    writer: ProfileWriter,
}

impl Scenario {
    /// Tempdir stores, in-memory metadata, and `Alpha.toml` copied from the seed.
    fn seeded() -> Self {
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

async fn save_matches_pre_refactor_bytes() {
    let scenario = Scenario::seeded();
    scenario.save_alpha_named("Alpha Updated").await;
    scenario.assert_profile_bytes("Alpha", SAVE);
}

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

type GoldenFuture = Pin<Box<dyn Future<Output = ()>>>;

/// Every golden scenario, by libtest-style name.
const TESTS: [(&str, fn() -> GoldenFuture); 4] = [
    ("save_matches_pre_refactor_bytes", || {
        Box::pin(save_matches_pre_refactor_bytes())
    }),
    ("rename_matches_pre_refactor_bytes", || {
        Box::pin(rename_matches_pre_refactor_bytes())
    }),
    ("duplicate_matches_pre_refactor_bytes", || {
        Box::pin(duplicate_matches_pre_refactor_bytes())
    }),
    ("rollback_matches_pre_refactor_bytes", || {
        Box::pin(rollback_matches_pre_refactor_bytes())
    }),
];

/// The subset of the libtest command line cargo, nextest and IDEs pass.
struct Selection {
    list: bool,
    ignored: bool,
    exact: bool,
    filters: Vec<String>,
    skips: Vec<String>,
}

impl Selection {
    fn parse(args: &[String]) -> Self {
        let mut selection = Self {
            list: false,
            ignored: false,
            exact: false,
            filters: Vec::new(),
            skips: Vec::new(),
        };
        let mut rest = args.iter().skip(1);
        while let Some(arg) = rest.next() {
            match arg.as_str() {
                "--list" => selection.list = true,
                "--ignored" => selection.ignored = true,
                "--exact" => selection.exact = true,
                "--skip" => selection.skips.extend(rest.next().cloned()),
                "--test-threads" | "--format" | "--color" | "--logfile" | "--report-time"
                | "--shuffle-seed" | "--ensure-time" | "-Z" => {
                    rest.next();
                }
                flag if flag.starts_with('-') => {}
                filter => selection.filters.push(filter.to_owned()),
            }
        }
        selection
    }

    fn matches(&self, pattern: &str, name: &str) -> bool {
        if self.exact {
            pattern == name
        } else {
            name.contains(pattern)
        }
    }

    /// No scenario is `#[ignore]`d, so `--ignored` selects nothing.
    fn selects(&self, name: &str) -> bool {
        !self.ignored
            && (self.filters.is_empty() || self.filters.iter().any(|f| self.matches(f, name)))
            && !self.skips.iter().any(|s| self.matches(s, name))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let selection = Selection::parse(&args);
    let selected: Vec<_> = TESTS
        .iter()
        .filter(|(name, _)| selection.selects(name))
        .collect();

    if selection.list {
        for (name, _) in &selected {
            println!("{name}: test");
        }
        return;
    }

    let home = tempfile::tempdir().expect("home tempdir");
    // SAFETY: harness = false; no runtime or other thread exists yet, and the
    // scenarios below run sequentially on a runtime built after this write.
    unsafe {
        std::env::set_var("HOME", home.path());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    println!("\nrunning {} tests", selected.len());
    let mut failed = Vec::new();
    for (name, test) in &selected {
        let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| runtime.block_on(test())));
        match outcome {
            Ok(()) => println!("test {name} ... ok"),
            Err(_) => {
                println!("test {name} ... FAILED");
                failed.push(*name);
            }
        }
    }
    drop(runtime);
    drop(home);

    let passed = selected.len() - failed.len();
    if failed.is_empty() {
        println!("\ntest result: ok. {passed} passed; 0 failed\n");
        return;
    }
    println!("\nfailures:");
    for name in &failed {
        println!("    {name}");
    }
    println!(
        "\ntest result: FAILED. {passed} passed; {} failed\n",
        failed.len()
    );
    std::process::exit(1);
}
