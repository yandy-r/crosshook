//! Integration tests for umu_database CSV coverage lookup.
//!
//! Exercises `check_coverage` end-to-end: fixture CSV staging, XDG env
//! wiring, Found/Missing/Unknown outcomes, and mtime-based cache invalidation.
//!
//! Each test uses a unique `TempDir`, so the `(path, mtime)` cache key
//! naturally differs between tests — no `clear_cache_for_test` needed.
//!
//! Run with:
//!   cargo test --manifest-path src/crosshook-native/Cargo.toml \
//!       -p crosshook-core --test umu_database_coverage

use crosshook_core::umu_database::{check_coverage, CsvCoverage};
use std::{env, fs, sync::Mutex, time::Duration};
use tempfile::TempDir;

// Serialize all env mutations across tests in this binary.
static ENV_LOCK: Mutex<()> = Mutex::new(());

// Fixture CSV — mirrors the upstream schema (CODENAME holds the numeric app id).
const FIXTURE_CSV: &str = "\
TITLE,STORE,CODENAME,UMU_ID,COMMON ACRONYM (Optional),NOTE (Optional),EXE_STRINGS (Optional)
Ghost of Tsushima,steam,546590,umu-546590,GoT,,ghostoftsushima.exe
Resident Evil 4 Remake,steam,2050650,umu-2050650,RE4R,,re4.exe
";

// Second fixture used by the mtime-invalidation test (replaces v1 entirely).
const FIXTURE_CSV_V2: &str = "\
TITLE,STORE,CODENAME,UMU_ID,COMMON ACRONYM (Optional),NOTE (Optional),EXE_STRINGS (Optional)
Death Stranding,steam,100001,umu-100001,,,deathstranding.exe
";

// ── helpers ───────────────────────────────────────────────────────────────────

/// Stage `content` as `<tmp>/umu-protonfixes/umu-database.csv`.
/// The returned `TempDir` must stay alive for the duration of the test.
fn stage_csv(content: &str) -> TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("umu-protonfixes");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("umu-database.csv"), content).unwrap();
    tmp
}

/// Point `HOME`, `XDG_DATA_HOME`, and `XDG_DATA_DIRS` at `tmp` so that
/// `resolve_umu_database_path()` finds the staged CSV via `XDG_DATA_DIRS`.
///
/// # Safety
/// Caller must hold `ENV_LOCK` for the duration of the test.
fn point_env_at(tmp: &TempDir) {
    // SAFETY: ENV_LOCK is held — no other test mutates these vars concurrently.
    unsafe {
        env::set_var("HOME", tmp.path());
        env::set_var("XDG_DATA_HOME", tmp.path().join("local/share"));
        env::set_var("XDG_DATA_DIRS", tmp.path().display().to_string());
    }
}

/// Returns true if any hardcoded umu-database CSV path exists on the host.
/// Tests that require no CSV source must skip when this returns true.
fn host_has_umu_csv() -> bool {
    [
        "/usr/share/umu-protonfixes/umu-database.csv",
        "/usr/share/umu/umu-database.csv",
        "/opt/umu-launcher/umu-protonfixes/umu-database.csv",
    ]
    .iter()
    .any(|p| fs::metadata(p).map(|m| m.is_file()).unwrap_or(false))
}

// ── Test 1: known app id present → Found ─────────────────────────────────────

/// Stages the fixture CSV and verifies that app id `546590` (Ghost of Tsushima)
/// resolves to `CsvCoverage::Found`.
#[test]
fn coverage_found_for_known_app_id() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = stage_csv(FIXTURE_CSV);
    point_env_at(&tmp);
    assert_eq!(
        check_coverage("546590", Some("steam")),
        CsvCoverage::Found,
        "546590 (Ghost of Tsushima) must be Found in the fixture CSV"
    );
}

// ── Test 2: app id absent → Missing ──────────────────────────────────────────

/// Same fixture CSV; `292030` (Witcher 3) is not listed — the motivating case
/// from issue #262 where Missing triggers a STEAM_COMPAT_APP_ID override.
#[test]
fn coverage_missing_for_absent_app_id() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = stage_csv(FIXTURE_CSV);
    point_env_at(&tmp);
    assert_eq!(
        check_coverage("292030", Some("steam")),
        CsvCoverage::Missing,
        "292030 (Witcher 3) must be Missing — the fixture CSV does not include it"
    );
}

// ── Test 3: no CSV source reachable → Unknown ─────────────────────────────────

/// Points all env vars at an empty tempdir so `resolve_umu_database_path()`
/// returns `None`, which must yield `CsvCoverage::Unknown`.
///
/// Skipped when the host has umu-launcher installed at a hardcoded path, since
/// those paths cannot be overridden via env vars.
#[test]
fn coverage_unknown_when_no_csv_source() {
    if host_has_umu_csv() {
        // Host has a packaged umu-database at a hardcoded path — we cannot
        // override those candidates, so this test cannot be conclusive.
        return;
    }
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    // SAFETY: ENV_LOCK is held — no other test mutates these vars concurrently.
    unsafe {
        env::set_var("HOME", tmp.path());
        env::set_var("XDG_DATA_HOME", tmp.path().join("local/share"));
        // Point XDG_DATA_DIRS at a sub-path that does not exist.
        env::set_var(
            "XDG_DATA_DIRS",
            tmp.path().join("nonexistent").display().to_string(),
        );
    }
    assert_eq!(
        check_coverage("546590", Some("steam")),
        CsvCoverage::Unknown,
        "when no CSV source is reachable, coverage must be Unknown"
    );
}

// ── Test 4: mtime-based cache invalidation ────────────────────────────────────

/// Verifies that stale cache entries are evicted when the CSV file's mtime
/// advances.  Stages CSV v1 (contains 546590), calls `check_coverage` (caches),
/// overwrites with v2 (only 100001), sleeps 1s to bump mtime, then asserts
/// that 100001 is now Found and 546590 is Missing.
#[test]
fn coverage_respects_mtime_cache_invalidation() {
    let _guard = ENV_LOCK.lock().unwrap();

    // Stage CSV v1 and wire env.
    let tmp = stage_csv(FIXTURE_CSV);
    point_env_at(&tmp);

    // First call — loads and caches CSV v1 at path P with mtime t1.
    assert_eq!(
        check_coverage("546590", Some("steam")),
        CsvCoverage::Found,
        "546590 must be Found in CSV v1 before cache invalidation"
    );

    // Overwrite the CSV at the same path with v2 (100001 only, no 546590).
    let csv_path = tmp.path().join("umu-protonfixes/umu-database.csv");
    fs::write(&csv_path, FIXTURE_CSV_V2).unwrap();

    // Sleep 1 s so the filesystem mtime is strictly greater than t1 (ext4
    // and most Linux filesystems have 1-second mtime resolution).
    std::thread::sleep(Duration::from_secs(1));

    // Second call — mtime differs → cache evicted → CSV v2 loaded.
    assert_eq!(
        check_coverage("100001", Some("steam")),
        CsvCoverage::Found,
        "100001 must be Found after mtime-triggered cache reload to CSV v2"
    );

    // Third call — same path + same mtime → served from re-warmed cache.
    assert_eq!(
        check_coverage("546590", Some("steam")),
        CsvCoverage::Missing,
        "546590 must be Missing after CSV v2 replaces v1 (no re-read needed)"
    );
}
