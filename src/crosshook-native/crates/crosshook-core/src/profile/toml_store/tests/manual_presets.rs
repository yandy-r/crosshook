use tempfile::tempdir;

use crate::profile::toml_store::{ProfileStore, ProfileStoreError};

use super::fixtures::sample_profile;

#[test]
fn save_manual_launch_optimization_preset_rejects_bundled_prefix() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();
    store.save("p", &profile).unwrap();

    let result = store.save_manual_launch_optimization_preset(
        "p",
        "bundled/foo",
        vec!["use_gamemode".to_string()],
    );
    assert!(matches!(
        result,
        Err(ProfileStoreError::ReservedLaunchPresetName(_))
    ));
}
