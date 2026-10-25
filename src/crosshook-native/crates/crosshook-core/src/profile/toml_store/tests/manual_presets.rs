use crate::profile::toml_store::ProfileStoreError;

use super::fixtures::sample_profile;

#[test]
fn apply_manual_optimization_preset_rejects_bundled_prefix_without_mutating() {
    let mut profile = sample_profile();
    let before = profile.clone();

    let result = profile
        .apply_manual_optimization_preset("  bundled/foo ", vec!["use_gamemode".to_string()]);

    assert!(matches!(
        result,
        Err(ProfileStoreError::ReservedLaunchPresetName(name)) if name == "bundled/foo"
    ));
    assert_eq!(profile, before);
}

#[test]
fn apply_manual_optimization_preset_rejects_blank_name() {
    let mut profile = sample_profile();

    assert!(matches!(
        profile.apply_manual_optimization_preset("   ", vec![]),
        Err(ProfileStoreError::InvalidLaunchPresetName(_))
    ));
}

#[test]
fn apply_manual_optimization_preset_rejects_unknown_option_ids() {
    let mut profile = sample_profile();
    let before = profile.clone();

    assert!(matches!(
        profile.apply_manual_optimization_preset("Mine", vec!["nope".into()]),
        Err(ProfileStoreError::InvalidLaunchOptimizationId(id)) if id == "nope"
    ));
    assert_eq!(profile, before);
}
