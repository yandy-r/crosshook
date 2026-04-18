#![cfg(test)]

use super::super::*;
use super::fixtures::*;
use crate::launch::request::{LaunchRequest, METHOD_PROTON_RUN};

// Serialize all tests that mutate process-global env vars (HOME, XDG_DATA_HOME, XDG_DATA_DIRS).
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// Minimal fixture CSV — Ghost of Tsushima (546590) present; Witcher 3 (292030) absent.
const FIXTURE_CSV: &str = "\
TITLE,STORE,CODENAME,UMU_ID,COMMON ACRONYM (Optional),NOTE (Optional),EXE_STRINGS (Optional)
Ghost of Tsushima,steam,546590,umu-546590,GoT,,ghostoftsushima.exe
";

#[test]
fn preview_reports_csv_coverage_found_when_app_id_matches() {
    let _env = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    // Place the fixture at data_local_dir()/crosshook/umu-database.csv — priority 1 in
    // resolve_umu_database_path, found before any system /usr/share/... paths.
    let xdg_data_home = tmp.path().join("local_share");
    let csv_dir = xdg_data_home.join("crosshook");
    std::fs::create_dir_all(&csv_dir).unwrap();
    std::fs::write(csv_dir.join("umu-database.csv"), FIXTURE_CSV).unwrap();
    std::env::set_var("HOME", tmp.path());
    std::env::set_var("XDG_DATA_HOME", &xdg_data_home);
    std::env::set_var("XDG_DATA_DIRS", "");
    crate::umu_database::coverage::clear_cache_for_test();

    let (_td, mut request) = proton_request();
    request.steam.app_id = "546590".to_string();
    let preview = build_launch_preview(&request).unwrap();
    let umu = preview
        .umu_decision
        .as_ref()
        .expect("umu_decision populated");
    assert_eq!(umu.csv_coverage, crate::umu_database::CsvCoverage::Found);
}

#[test]
fn preview_reports_csv_coverage_missing_when_app_id_absent() {
    let _env = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    // Same fixture CSV as the Found test — Witcher 3 (292030) is the motivating missing case
    // from issue #262 (proton-cachyos STEAM_COMPAT_APP_ID override side-effect).
    let xdg_data_home = tmp.path().join("local_share");
    let csv_dir = xdg_data_home.join("crosshook");
    std::fs::create_dir_all(&csv_dir).unwrap();
    std::fs::write(csv_dir.join("umu-database.csv"), FIXTURE_CSV).unwrap();
    std::env::set_var("HOME", tmp.path());
    std::env::set_var("XDG_DATA_HOME", &xdg_data_home);
    std::env::set_var("XDG_DATA_DIRS", "");
    crate::umu_database::coverage::clear_cache_for_test();

    let (_td, mut request) = proton_request();
    request.steam.app_id = "292030".to_string(); // Witcher 3 — absent from fixture
    let preview = build_launch_preview(&request).unwrap();
    let umu = preview
        .umu_decision
        .as_ref()
        .expect("umu_decision populated");
    assert_eq!(umu.csv_coverage, crate::umu_database::CsvCoverage::Missing);
}

#[test]
fn preview_reports_csv_coverage_unknown_when_no_csv_source() {
    // Skip on hosts where a system-level CSV exists and cannot be overridden —
    // resolve_umu_database_path checks hardcoded /usr/share/... paths before XDG_DATA_DIRS,
    // and we cannot redirect those.
    let system_csvs = [
        "/usr/share/umu-protonfixes/umu-database.csv",
        "/usr/share/umu/umu-database.csv",
        "/opt/umu-launcher/umu-protonfixes/umu-database.csv",
    ];
    if system_csvs
        .iter()
        .any(|p| std::fs::metadata(p).map(|m| m.is_file()).unwrap_or(false))
    {
        eprintln!("skip: host has a system umu-database CSV — cannot isolate Unknown case");
        return;
    }

    let _env = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    // Point HOME and XDG_DATA_HOME at an empty tempdir — no CSV anywhere reachable.
    std::env::set_var("HOME", tmp.path());
    std::env::set_var("XDG_DATA_HOME", tmp.path().join("local_share"));
    std::env::set_var("XDG_DATA_DIRS", "");
    crate::umu_database::coverage::clear_cache_for_test();

    let (_td, mut request) = proton_request();
    request.steam.app_id = "546590".to_string();
    let preview = build_launch_preview(&request).unwrap();
    let umu = preview
        .umu_decision
        .as_ref()
        .expect("umu_decision populated");
    assert_eq!(umu.csv_coverage, crate::umu_database::CsvCoverage::Unknown);
}

#[test]
fn auto_preference_preview_reports_using_umu_when_umu_run_present() {
    let dir = tempfile::tempdir().unwrap();
    let umu_stub = dir.path().join("umu-run");
    std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    let mut request = LaunchRequest {
        method: METHOD_PROTON_RUN.to_string(),
        game_path: "/tmp/game.exe".to_string(),
        umu_preference: crate::settings::UmuPreference::Auto,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let preview = build_launch_preview(&request).unwrap();
    let decision = preview.umu_decision.as_ref().unwrap();
    assert!(decision.will_use_umu, "Auto + umu-run present must use umu");
    assert!(
        decision.reason.starts_with("using umu-run at "),
        "expected reason starting with 'using umu-run at ', got: {}",
        decision.reason
    );
}

#[test]
fn auto_preference_preview_explains_fallback_when_umu_missing() {
    let dir = tempfile::tempdir().unwrap();
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

    let mut request = LaunchRequest {
        method: METHOD_PROTON_RUN.to_string(),
        game_path: "/tmp/game.exe".to_string(),
        umu_preference: crate::settings::UmuPreference::Auto,
        ..Default::default()
    };
    request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

    let preview = build_launch_preview(&request).unwrap();
    let decision = preview.umu_decision.as_ref().unwrap();
    assert!(!decision.will_use_umu, "Auto + no umu-run must not use umu");
    assert_eq!(
        decision.reason,
        "preference = Auto but umu-run was not found on the backend PATH — falling back to direct Proton"
    );
}
