use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;

use super::dismissals::{apply_install_nag_dismissal, apply_steam_deck_caveats_dismissal};
use super::system::evaluate_checks_inner;
use crate::profile::health::HealthIssueSeverity;
use crate::steam::ProtonInstall;

fn make_proton_exe(dir: &Path) {
    let proton = dir.join("proton");
    fs::write(&proton, b"#!/bin/sh\n").expect("write proton");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&proton).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&proton, perms).expect("chmod");
    }
}

fn setup_steam_root(base: &Path) -> PathBuf {
    let steam_root = base.to_path_buf();
    fs::create_dir_all(steam_root.join("steamapps")).expect("steamapps");
    steam_root
}

fn make_proton_install(steam_root: &Path) -> ProtonInstall {
    use std::collections::BTreeSet;
    let proton_dir = steam_root.join("steamapps/common/Proton-9");
    fs::create_dir_all(&proton_dir).expect("proton dir");
    make_proton_exe(&proton_dir);
    ProtonInstall {
        name: "Proton-9".to_string(),
        path: proton_dir.join("proton"),
        is_official: true,
        aliases: vec!["Proton-9".to_string()],
        normalized_aliases: BTreeSet::from(["proton9".to_string()]),
    }
}

fn add_compatdata(steam_root: &Path) {
    let pfx = steam_root.join("steamapps/compatdata/12345/pfx");
    fs::create_dir_all(&pfx).expect("pfx dir");
}

#[test]
fn all_pass_case() {
    let tmp = tempdir().expect("tempdir");
    let steam_root = setup_steam_root(tmp.path());
    let proton = make_proton_install(&steam_root);
    add_compatdata(&steam_root);

    let result = evaluate_checks_inner(
        &[steam_root],
        &[proton],
        Some("umu-run".to_string()),
        false,
        false,
    );

    assert!(
        result.all_passed,
        "expected all_passed; checks: {:?}",
        result.checks
    );
    assert_eq!(result.critical_failures, 0);
    assert_eq!(result.warnings, 0);
    assert_eq!(result.checks.len(), 5);
}

#[test]
fn no_steam_case() {
    let result = evaluate_checks_inner(&[], &[], None, false, false);

    assert!(!result.all_passed);

    let steam_check = result
        .checks
        .iter()
        .find(|c| c.field == "steam_installed")
        .expect("steam_installed check missing");
    assert!(
        matches!(steam_check.severity, HealthIssueSeverity::Error),
        "expected Error for steam_installed"
    );

    let proton_check = result
        .checks
        .iter()
        .find(|c| c.field == "proton_available")
        .expect("proton_available check missing");
    assert!(
        matches!(proton_check.severity, HealthIssueSeverity::Error),
        "expected Error for proton_available"
    );

    assert_eq!(result.critical_failures, 2);
    assert_eq!(result.warnings, 2);
}

#[test]
fn no_proton_case() {
    let tmp = tempdir().expect("tempdir");
    let steam_root = setup_steam_root(tmp.path());
    add_compatdata(&steam_root);

    let result = evaluate_checks_inner(&[steam_root], &[], None, false, false);

    assert!(!result.all_passed);

    let steam_check = result
        .checks
        .iter()
        .find(|c| c.field == "steam_installed")
        .expect("steam_installed check missing");
    assert!(
        matches!(steam_check.severity, HealthIssueSeverity::Info),
        "expected Info for steam_installed"
    );

    let proton_check = result
        .checks
        .iter()
        .find(|c| c.field == "proton_available")
        .expect("proton_available check missing");
    assert!(
        matches!(proton_check.severity, HealthIssueSeverity::Error),
        "expected Error for proton_available"
    );

    assert_eq!(result.critical_failures, 1);
    assert_eq!(result.warnings, 1);
}

#[test]
fn no_compatdata_case() {
    let tmp = tempdir().expect("tempdir");
    let steam_root = setup_steam_root(tmp.path());
    let proton = make_proton_install(&steam_root);

    let result = evaluate_checks_inner(
        &[steam_root],
        &[proton],
        Some("umu-run".to_string()),
        false,
        false,
    );

    assert!(!result.all_passed);
    assert_eq!(result.critical_failures, 0);
    assert_eq!(result.warnings, 1);

    let game_check = result
        .checks
        .iter()
        .find(|c| c.field == "game_launched_once")
        .expect("game_launched_once check missing");
    assert!(
        matches!(game_check.severity, HealthIssueSeverity::Warning),
        "expected Warning for game_launched_once"
    );
}

#[test]
fn native_missing_umu_keeps_info_path() {
    let tmp = tempdir().expect("tempdir");
    let steam_root = setup_steam_root(tmp.path());
    let proton = make_proton_install(&steam_root);
    add_compatdata(&steam_root);

    let result = evaluate_checks_inner(&[steam_root], &[proton], None, false, false);

    let umu_check = result
        .checks
        .iter()
        .find(|c| c.field == "umu_run_available")
        .expect("umu_run_available check missing");
    assert!(
        matches!(umu_check.severity, HealthIssueSeverity::Warning),
        "expected Warning severity for native missing umu; got {:?}",
        umu_check.severity
    );
    assert!(
        umu_check.remediation.is_empty(),
        "native missing umu must not emit Flatpak remediation text"
    );
    assert!(
        result.umu_install_guidance.is_none(),
        "native missing umu must not produce a guidance payload"
    );
    assert!(
        !result.all_passed,
        "missing umu (Warning) should set all_passed=false; checks: {:?}",
        result.checks
    );
}

#[test]
fn flatpak_missing_umu_reports_actionable_guidance() {
    let _catalog = crate::onboarding::load_readiness_catalog(None);
    crate::onboarding::initialize_readiness_catalog(_catalog);

    let tmp = tempdir().expect("tempdir");
    let steam_root = setup_steam_root(tmp.path());
    let proton = make_proton_install(&steam_root);
    add_compatdata(&steam_root);

    let result = evaluate_checks_inner(&[steam_root], &[proton], None, true, false);

    let umu_check = result
        .checks
        .iter()
        .find(|c| c.field == "umu_run_available")
        .expect("umu_run_available check missing");
    assert!(
        matches!(umu_check.severity, HealthIssueSeverity::Warning),
        "Flatpak missing umu must use Warning severity for amber visual; got {:?}",
        umu_check.severity
    );
    assert!(
        !umu_check.remediation.is_empty(),
        "Flatpak missing umu must have non-empty remediation text"
    );
    assert!(
        umu_check.message.contains("Flatpak"),
        "Flatpak missing umu message should mention Flatpak host environment"
    );

    let guidance = result
        .umu_install_guidance
        .as_ref()
        .expect("umu_install_guidance payload must be present for Flatpak+missing umu");
    assert!(
        !guidance.install_command.is_empty(),
        "guidance install_command must be non-empty"
    );
    assert!(
        !guidance.docs_url.is_empty(),
        "guidance docs_url must be non-empty"
    );
    assert!(
        !guidance.description.is_empty(),
        "guidance description must be non-empty"
    );

    assert!(
        !result.all_passed,
        "Flatpak missing umu (Warning) should set all_passed=false; checks: {:?}",
        result.checks
    );
}

#[test]
fn install_nag_dismissal_clears_flatpak_umu_guidance() {
    let cat = crate::onboarding::load_readiness_catalog(None);
    crate::onboarding::initialize_readiness_catalog(cat);

    let tmp = tempdir().expect("tempdir");
    let steam_root = setup_steam_root(tmp.path());
    let proton = make_proton_install(&steam_root);
    add_compatdata(&steam_root);

    let mut result = evaluate_checks_inner(&[steam_root], &[proton], None, true, false);
    assert!(
        result.umu_install_guidance.is_some(),
        "precondition: missing umu on Flatpak must emit guidance"
    );
    apply_install_nag_dismissal(&mut result, &Some("2026-04-15T12:00:00Z".to_string()));
    assert!(
        result.umu_install_guidance.is_none(),
        "dismissed install nag must strip umu_install_guidance on subsequent readiness"
    );
}

#[test]
fn caveats_absent_when_not_steam_deck() {
    let tmp = tempdir().expect("tempdir");
    let steam_root = setup_steam_root(tmp.path());
    let proton = make_proton_install(&steam_root);
    add_compatdata(&steam_root);

    let result = evaluate_checks_inner(&[steam_root], &[proton], None, false, false);

    assert!(
        result.steam_deck_caveats.is_none(),
        "non-Deck system must not populate steam_deck_caveats"
    );
}

#[test]
fn caveats_present_when_steam_deck_and_not_flatpak() {
    let tmp = tempdir().expect("tempdir");
    let steam_root = setup_steam_root(tmp.path());
    let proton = make_proton_install(&steam_root);
    add_compatdata(&steam_root);

    let result = evaluate_checks_inner(
        &[steam_root],
        &[proton],
        Some("umu-run".to_string()),
        false,
        true,
    );

    let caveats = result
        .steam_deck_caveats
        .as_ref()
        .expect("steam_deck_caveats must be present on Deck (non-Flatpak)");
    assert_eq!(caveats.items.len(), 3, "expected 3 caveat items");
    assert!(
        !caveats.description.is_empty(),
        "caveats description must be non-empty"
    );
    assert!(
        !caveats.docs_url.is_empty(),
        "caveats docs_url must be non-empty"
    );
    assert!(
        result.all_passed,
        "caveats alone must not flip all_passed to false; checks: {:?}",
        result.checks
    );
}

#[test]
fn caveats_present_when_steam_deck_and_flatpak() {
    let tmp = tempdir().expect("tempdir");
    let steam_root = setup_steam_root(tmp.path());
    let proton = make_proton_install(&steam_root);
    add_compatdata(&steam_root);

    let result = evaluate_checks_inner(
        &[steam_root],
        &[proton],
        Some("/usr/bin/umu-run".to_string()),
        true,
        true,
    );

    let caveats = result
        .steam_deck_caveats
        .as_ref()
        .expect("steam_deck_caveats must be present on Deck (Flatpak)");
    assert_eq!(caveats.items.len(), 3, "expected 3 caveat items");
    assert!(
        result.all_passed,
        "caveats alone must not flip all_passed to false; checks: {:?}",
        result.checks
    );
}

#[test]
fn caveats_cleared_after_apply_dismissal() {
    let tmp = tempdir().expect("tempdir");
    let steam_root = setup_steam_root(tmp.path());
    let proton = make_proton_install(&steam_root);
    add_compatdata(&steam_root);

    let mut result = evaluate_checks_inner(&[steam_root], &[proton], None, false, true);
    assert!(
        result.steam_deck_caveats.is_some(),
        "precondition: Deck must populate caveats"
    );

    apply_steam_deck_caveats_dismissal(&mut result, &Some("2026-04-15T12:00:00Z".to_string()));

    assert!(
        result.steam_deck_caveats.is_none(),
        "dismissed caveats must be cleared after apply_steam_deck_caveats_dismissal"
    );
}

#[test]
fn apply_dismissal_noop_when_none() {
    let tmp = tempdir().expect("tempdir");
    let steam_root = setup_steam_root(tmp.path());
    let proton = make_proton_install(&steam_root);
    add_compatdata(&steam_root);

    let mut result = evaluate_checks_inner(&[steam_root], &[proton], None, false, true);
    assert!(
        result.steam_deck_caveats.is_some(),
        "precondition: Deck must populate caveats"
    );

    apply_steam_deck_caveats_dismissal(&mut result, &None);

    assert!(
        result.steam_deck_caveats.is_some(),
        "None dismissed_at must leave caveats intact"
    );
}

#[test]
fn caveats_present_even_when_umu_absent_and_flatpak() {
    let cat = crate::onboarding::load_readiness_catalog(None);
    crate::onboarding::initialize_readiness_catalog(cat);

    let tmp = tempdir().expect("tempdir");
    let steam_root = setup_steam_root(tmp.path());
    let proton = make_proton_install(&steam_root);
    add_compatdata(&steam_root);

    let result = evaluate_checks_inner(&[steam_root], &[proton], None, true, true);

    assert!(
        result.umu_install_guidance.is_some(),
        "Flatpak + missing umu must emit umu_install_guidance"
    );
    assert!(
        result.steam_deck_caveats.is_some(),
        "Deck + Flatpak + missing umu must also emit steam_deck_caveats"
    );
}

#[test]
fn all_passed_stays_true_with_caveats_only() {
    let tmp = tempdir().expect("tempdir");
    let steam_root = setup_steam_root(tmp.path());
    let proton = make_proton_install(&steam_root);
    add_compatdata(&steam_root);

    let result = evaluate_checks_inner(
        &[steam_root],
        &[proton],
        Some("/usr/bin/umu-run".to_string()),
        false,
        true,
    );

    assert!(
        result.steam_deck_caveats.is_some(),
        "Steam Deck caveats must be present"
    );
    assert_eq!(
        result.critical_failures, 0,
        "caveats must not add critical failures"
    );
    assert_eq!(result.warnings, 0, "caveats must not add warnings");
    assert!(
        result.all_passed,
        "all_passed must remain true when only caveats are present; checks: {:?}",
        result.checks
    );
}
