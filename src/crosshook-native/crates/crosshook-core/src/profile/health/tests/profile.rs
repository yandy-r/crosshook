use std::fs;

use tempfile::tempdir;

use crate::profile::toml_store::ProfileStore;
use crate::profile::{
    GameProfile, GameSection, InjectionSection, LaunchSection, RuntimeSection, SteamSection,
    TrainerSection,
};

use super::super::{batch_check_health, check_profile_health, HealthStatus};
use super::fixtures::{healthy_steam_profile, make_executable};

#[test]
fn healthy_profile_reports_healthy_status() {
    let tmp = tempdir().expect("tempdir");
    let profile = healthy_steam_profile(tmp.path());
    let report = check_profile_health("test-game", &profile);

    assert!(
        matches!(report.status, HealthStatus::Healthy),
        "expected Healthy, got {:?}; issues: {:?}",
        report.status,
        report.issues
    );
    assert!(report.issues.is_empty());
    assert_eq!(report.name, "test-game");
    assert_eq!(report.launch_method, "steam_applaunch");
}

#[test]
fn missing_game_exe_reports_stale() {
    let tmp = tempdir().expect("tempdir");
    let mut profile = healthy_steam_profile(tmp.path());
    profile.game.executable_path = tmp
        .path()
        .join("nonexistent.exe")
        .to_string_lossy()
        .to_string();

    let report = check_profile_health("stale-game", &profile);

    assert!(
        matches!(report.status, HealthStatus::Stale),
        "expected Stale, got {:?}",
        report.status
    );
    assert!(report
        .issues
        .iter()
        .any(|i| i.field == "game.executable_path"));
}

#[test]
fn game_exe_is_directory_reports_broken() {
    let tmp = tempdir().expect("tempdir");
    let dir_path = tmp.path().join("itsadir");
    fs::create_dir_all(&dir_path).expect("mkdir");

    let mut profile = healthy_steam_profile(tmp.path());
    profile.game.executable_path = dir_path.to_string_lossy().to_string();

    let report = check_profile_health("broken-game", &profile);

    assert!(
        matches!(report.status, HealthStatus::Broken),
        "expected Broken, got {:?}",
        report.status
    );
    assert!(report
        .issues
        .iter()
        .any(|i| i.field == "game.executable_path"));
}

#[test]
fn unconfigured_profile_reports_broken() {
    let profile = GameProfile::default();
    let report = check_profile_health("empty-profile", &profile);

    // game.executable_path is required for all methods — empty → Broken
    assert!(
        matches!(report.status, HealthStatus::Broken),
        "expected Broken for empty profile, got {:?}",
        report.status
    );
    assert!(report
        .issues
        .iter()
        .any(|i| i.field == "game.executable_path"));
}

#[test]
fn missing_proton_reports_stale_for_steam_applaunch() {
    let tmp = tempdir().expect("tempdir");
    let mut profile = healthy_steam_profile(tmp.path());
    // Point proton_path at a nonexistent path
    profile.steam.proton_path = tmp.path().join("gone_proton").to_string_lossy().to_string();

    let report = check_profile_health("stale-steam", &profile);

    assert!(
        matches!(report.status, HealthStatus::Stale),
        "expected Stale (missing proton), got {:?}",
        report.status
    );
    assert!(report.issues.iter().any(|i| i.field == "steam.proton_path"));
}

#[cfg(unix)]
#[test]
fn proton_path_not_executable_reports_broken() {
    let tmp = tempdir().expect("tempdir");
    let mut profile = healthy_steam_profile(tmp.path());

    // Create a non-executable file as proton
    let non_exec = tmp.path().join("proton_no_exec");
    fs::write(&non_exec, b"data").expect("write");
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(&non_exec).expect("meta").permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&non_exec, perms).expect("chmod");
    profile.steam.proton_path = non_exec.to_string_lossy().to_string();

    let report = check_profile_health("broken-proton", &profile);

    assert!(
        matches!(report.status, HealthStatus::Broken),
        "expected Broken (non-executable proton), got {:?}",
        report.status
    );
    assert!(report.issues.iter().any(|i| i.field == "steam.proton_path"));
}

#[test]
fn batch_check_health_returns_all_profiles() {
    let tmp = tempdir().expect("tempdir");
    let store = ProfileStore::with_base_path(tmp.path().join("profiles"));

    let profile = healthy_steam_profile(tmp.path());
    store.save("game-a", &profile).expect("save game-a");
    store.save("game-b", &profile).expect("save game-b");

    let summary = batch_check_health(&store);

    assert_eq!(summary.total_count, 2);
    assert_eq!(summary.profiles.len(), 2);
}

#[test]
fn batch_check_health_isolates_toml_parse_error() {
    let tmp = tempdir().expect("tempdir");
    let store = ProfileStore::with_base_path(tmp.path().join("profiles"));

    // Save a valid profile
    let profile = healthy_steam_profile(tmp.path());
    store.save("valid-profile", &profile).expect("save valid");

    // Write an intentionally malformed TOML file
    let bad_path = store.base_path.join("broken-toml.toml");
    fs::create_dir_all(&store.base_path).expect("mkdir profiles");
    fs::write(&bad_path, b"[invalid toml content %%% @@").expect("write bad toml");

    let summary = batch_check_health(&store);

    // Should have 2 profiles total: 1 valid, 1 broken
    assert_eq!(summary.total_count, 2);
    assert_eq!(summary.broken_count, 1);

    let broken = summary
        .profiles
        .iter()
        .find(|r| r.name == "broken-toml")
        .expect("broken-toml report missing");
    assert!(matches!(broken.status, HealthStatus::Broken));
    assert!(!broken.issues.is_empty());
}

#[test]
fn batch_check_health_empty_store_returns_empty_summary() {
    let tmp = tempdir().expect("tempdir");
    let store = ProfileStore::with_base_path(tmp.path().join("profiles"));

    let summary = batch_check_health(&store);

    assert_eq!(summary.total_count, 0);
    assert_eq!(summary.healthy_count, 0);
    assert_eq!(summary.stale_count, 0);
    assert_eq!(summary.broken_count, 0);
}

#[test]
fn proton_run_method_checks_runtime_prefix_not_steam() {
    let tmp = tempdir().expect("tempdir");

    let game_exe = tmp.path().join("game.exe");
    make_executable(&game_exe);

    let prefix = tmp.path().join("pfx");
    fs::create_dir_all(&prefix).expect("mkdir prefix");

    let proton = tmp.path().join("proton");
    make_executable(&proton);

    let profile = GameProfile {
        game: GameSection {
            name: "Proton Game".to_string(),
            executable_path: game_exe.to_string_lossy().to_string(),
            custom_cover_art_path: String::new(),
            custom_portrait_art_path: String::new(),
            custom_background_art_path: String::new(),
        },
        trainer: TrainerSection::default(),
        injection: InjectionSection::default(),
        steam: SteamSection::default(),
        runtime: RuntimeSection {
            prefix_path: prefix.to_string_lossy().to_string(),
            proton_path: proton.to_string_lossy().to_string(),
            working_directory: String::new(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
            umu_preference: None,
        },
        launch: LaunchSection {
            method: "proton_run".to_string(),
            ..Default::default()
        },
        local_override: crate::profile::LocalOverrideSection::default(),
    };

    let report = check_profile_health("proton-run-game", &profile);

    assert!(
        matches!(report.status, HealthStatus::Healthy),
        "expected Healthy for proton_run profile with all paths present, got {:?}; issues: {:?}",
        report.status,
        report.issues
    );
}

#[test]
fn host_mounted_runtime_proton_path_is_healthy() {
    let tmp = tempdir().expect("tempdir");

    let game_exe = tmp.path().join("game.exe");
    make_executable(&game_exe);

    let prefix = tmp.path().join("pfx");
    fs::create_dir_all(&prefix).expect("mkdir prefix");

    let proton = tmp.path().join("proton");
    make_executable(&proton);

    let profile = GameProfile {
        game: GameSection {
            name: "Proton Game".to_string(),
            executable_path: game_exe.to_string_lossy().to_string(),
            custom_cover_art_path: String::new(),
            custom_portrait_art_path: String::new(),
            custom_background_art_path: String::new(),
        },
        trainer: TrainerSection::default(),
        injection: InjectionSection::default(),
        steam: SteamSection::default(),
        runtime: RuntimeSection {
            prefix_path: format!("/run/host{}", prefix.to_string_lossy()),
            proton_path: format!("/run/host{}", proton.to_string_lossy()),
            working_directory: String::new(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
            umu_preference: None,
        },
        launch: LaunchSection {
            method: "proton_run".to_string(),
            ..Default::default()
        },
        local_override: crate::profile::LocalOverrideSection::default(),
    };

    let report = check_profile_health("proton-run-host-mounted", &profile);

    assert!(
        matches!(report.status, HealthStatus::Healthy),
        "expected Healthy for host-mounted proton_run profile, got {:?}; issues: {:?}",
        report.status,
        report.issues
    );
}
