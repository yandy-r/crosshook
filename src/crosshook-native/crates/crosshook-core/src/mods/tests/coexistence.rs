use crate::launch::request::{LaunchValidationIssue, ValidationSeverity};
use crate::profile::{InjectionMethod, TrainerLoadingMode};

use super::super::coexistence::{
    analyze_mod_coexistence, CoexistenceAdvisory, CoexistenceContext, RULES,
};
use super::super::model::{ModCategory, ModProvenance, ProfileModRecord};

fn mod_record(
    name: &str,
    category: ModCategory,
    paths: &[&str],
    enabled: bool,
) -> ProfileModRecord {
    ProfileModRecord {
        mod_id: format!("mod-{name}"),
        profile_id: "profile-1".to_string(),
        name: name.to_string(),
        category,
        paths: paths.iter().map(ToString::to_string).collect(),
        enabled,
        provenance: ModProvenance::Manual,
        source_url: None,
        created_at: "2026-01-01T00:00:00+00:00".to_string(),
        updated_at: "2026-01-01T00:00:00+00:00".to_string(),
    }
}

fn ctx() -> CoexistenceContext {
    CoexistenceContext {
        trainer_configured: false,
        trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
        injection_method: InjectionMethod::Disabled,
        injection_dll_names: Vec::new(),
        mangohud_enabled: false,
        gamescope_enabled: false,
    }
}

fn codes(advisories: &[CoexistenceAdvisory]) -> Vec<&'static str> {
    advisories.iter().map(|advisory| advisory.code).collect()
}

#[test]
fn injection_vector_copy_to_prefix_without_dll_overlap_is_info_with_softened_wording() {
    let mods = [mod_record("SKSE64", ModCategory::ScriptExtender, &[], true)];
    let context = CoexistenceContext {
        trainer_configured: true,
        trainer_loading_mode: TrainerLoadingMode::CopyToPrefix,
        ..ctx()
    };

    let advisories = analyze_mod_coexistence(&mods, &context);
    let matching: Vec<_> = advisories
        .iter()
        .filter(|a| a.code == "mod_coexistence_injection_vector")
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "loading-mode heuristic must fire exactly once"
    );
    assert_eq!(matching[0].severity, ValidationSeverity::Info);
    assert!(matching[0].message.contains("SKSE64"));
    assert!(matching[0]
        .message
        .contains("may share the injection vector"));
}

#[test]
fn injection_vector_fires_warning_for_proxy_overlay_with_dll_overlap() {
    let mods = [mod_record(
        "ReShade",
        ModCategory::OverlayInjection,
        &["dxgi.dll"],
        true,
    )];
    let context = CoexistenceContext {
        trainer_configured: true,
        injection_method: InjectionMethod::LoadLibrary,
        injection_dll_names: vec!["dxgi.dll".to_string()],
        ..ctx()
    };

    let advisories = analyze_mod_coexistence(&mods, &context);
    let matching: Vec<_> = advisories
        .iter()
        .filter(|a| a.code == "mod_coexistence_injection_vector")
        .collect();
    assert_eq!(matching.len(), 1);
    assert_eq!(matching[0].severity, ValidationSeverity::Warning);
}

#[test]
fn injection_vector_dll_overlap_with_copy_to_prefix_fires_single_warning() {
    let mods = [mod_record(
        "SKSE64",
        ModCategory::ScriptExtender,
        &["skse64_loader.dll"],
        true,
    )];
    let context = CoexistenceContext {
        trainer_configured: true,
        trainer_loading_mode: TrainerLoadingMode::CopyToPrefix,
        injection_method: InjectionMethod::LoadLibrary,
        injection_dll_names: vec!["skse64_loader.dll".to_string()],
        ..ctx()
    };

    let advisories = analyze_mod_coexistence(&mods, &context);
    let matching: Vec<_> = advisories
        .iter()
        .filter(|a| a.code == "mod_coexistence_injection_vector")
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "overlap + heuristic must not double-fire: {matching:?}"
    );
    assert_eq!(matching[0].severity, ValidationSeverity::Warning);
}

#[test]
fn injection_vector_silent_without_trainer() {
    let mods = [mod_record("SKSE64", ModCategory::ScriptExtender, &[], true)];
    let context = CoexistenceContext {
        trainer_loading_mode: TrainerLoadingMode::CopyToPrefix,
        ..ctx()
    };

    let advisories = analyze_mod_coexistence(&mods, &context);
    assert!(!codes(&advisories).contains(&"mod_coexistence_injection_vector"));
}

#[test]
fn injection_vector_silent_with_source_directory_and_no_dll_overlap() {
    let mods = [mod_record(
        "ReShade",
        ModCategory::OverlayInjection,
        &["dxgi.dll"],
        true,
    )];
    let context = CoexistenceContext {
        trainer_configured: true,
        injection_method: InjectionMethod::LoadLibrary,
        injection_dll_names: vec!["trainer-hook.dll".to_string()],
        ..ctx()
    };

    let advisories = analyze_mod_coexistence(&mods, &context);
    assert!(!codes(&advisories).contains(&"mod_coexistence_injection_vector"));
}

#[test]
fn overlay_conflict_fires_with_mangohud() {
    let mods = [mod_record(
        "ReShade",
        ModCategory::OverlayInjection,
        &[],
        true,
    )];
    let context = CoexistenceContext {
        mangohud_enabled: true,
        ..ctx()
    };

    let advisories = analyze_mod_coexistence(&mods, &context);
    let advisory = advisories
        .iter()
        .find(|a| a.code == "mod_coexistence_overlay_conflict")
        .unwrap();
    assert_eq!(advisory.severity, ValidationSeverity::Warning);
    assert!(advisory.message.contains("MangoHud"));
    assert!(!advisory.message.contains("gamescope"));
}

#[test]
fn overlay_conflict_fires_with_gamescope() {
    let mods = [mod_record(
        "ReShade",
        ModCategory::OverlayInjection,
        &[],
        true,
    )];
    let context = CoexistenceContext {
        gamescope_enabled: true,
        ..ctx()
    };

    let advisories = analyze_mod_coexistence(&mods, &context);
    let advisory = advisories
        .iter()
        .find(|a| a.code == "mod_coexistence_overlay_conflict")
        .unwrap();
    assert!(advisory.message.contains("gamescope"));
    assert!(!advisory.message.contains("MangoHud"));
}

#[test]
fn overlay_conflict_names_both_overlays() {
    let mods = [mod_record(
        "ReShade",
        ModCategory::OverlayInjection,
        &[],
        true,
    )];
    let context = CoexistenceContext {
        mangohud_enabled: true,
        gamescope_enabled: true,
        ..ctx()
    };

    let advisories = analyze_mod_coexistence(&mods, &context);
    let advisory = advisories
        .iter()
        .find(|a| a.code == "mod_coexistence_overlay_conflict")
        .unwrap();
    assert!(advisory.message.contains("MangoHud and gamescope"));
}

#[test]
fn overlay_conflict_silent_without_overlays() {
    let mods = [mod_record(
        "ReShade",
        ModCategory::OverlayInjection,
        &[],
        true,
    )];
    let advisories = analyze_mod_coexistence(&mods, &ctx());
    assert!(!codes(&advisories).contains(&"mod_coexistence_overlay_conflict"));
}

#[test]
fn file_replacement_notice_fires_info() {
    let mods = [mod_record(
        "HD Texture Pack",
        ModCategory::FileReplacement,
        &[],
        true,
    )];
    let advisories = analyze_mod_coexistence(&mods, &ctx());
    let advisory = advisories
        .iter()
        .find(|a| a.code == "mod_coexistence_file_replacement_notice")
        .unwrap();
    assert_eq!(advisory.severity, ValidationSeverity::Info);
    assert!(advisory.message.contains("HD Texture Pack"));
}

#[test]
fn file_replacement_notice_silent_for_other_category() {
    let mods = [mod_record("Misc Tool", ModCategory::Other, &[], true)];
    let advisories = analyze_mod_coexistence(&mods, &ctx());
    assert!(advisories.is_empty());
}

#[test]
fn script_extender_launch_order_fires_with_trainer() {
    let mods = [mod_record("SKSE64", ModCategory::ScriptExtender, &[], true)];
    let context = CoexistenceContext {
        trainer_configured: true,
        ..ctx()
    };

    let advisories = analyze_mod_coexistence(&mods, &context);
    let advisory = advisories
        .iter()
        .find(|a| a.code == "mod_coexistence_script_extender_launch_order")
        .unwrap();
    assert_eq!(advisory.severity, ValidationSeverity::Info);
}

#[test]
fn script_extender_launch_order_silent_without_trainer() {
    let mods = [mod_record("SKSE64", ModCategory::ScriptExtender, &[], true)];
    let advisories = analyze_mod_coexistence(&mods, &ctx());
    assert!(!codes(&advisories).contains(&"mod_coexistence_script_extender_launch_order"));
}

#[test]
fn disabled_mods_are_ignored() {
    let mods = [mod_record(
        "HD Texture Pack",
        ModCategory::FileReplacement,
        &[],
        false,
    )];
    let advisories = analyze_mod_coexistence(&mods, &ctx());
    assert!(
        advisories.is_empty(),
        "disabled mods must be filtered defensively"
    );
}

#[test]
fn empty_mods_short_circuits() {
    assert!(analyze_mod_coexistence(&[], &ctx()).is_empty());
}

#[test]
fn no_rule_is_fatal() {
    for rule in RULES {
        assert_ne!(
            rule.severity,
            ValidationSeverity::Fatal,
            "rule {} must never be Fatal — advisories cannot block launch",
            rule.code
        );
    }
}

#[test]
fn advisory_maps_to_issue_with_code() {
    let mods = [mod_record(
        "HD Texture Pack",
        ModCategory::FileReplacement,
        &[],
        true,
    )];
    let advisories = analyze_mod_coexistence(&mods, &ctx());
    let issue = LaunchValidationIssue::mod_coexistence_advisory(&advisories[0]);

    assert_eq!(
        issue.code.as_deref(),
        Some("mod_coexistence_file_replacement_notice")
    );
    assert_eq!(issue.severity, ValidationSeverity::Info);
    assert_eq!(issue.message, advisories[0].message);
    assert_eq!(issue.help, advisories[0].help);
    assert!(issue.trainer_hash_stored.is_none());
    assert!(issue.hook_id.is_none());
}
