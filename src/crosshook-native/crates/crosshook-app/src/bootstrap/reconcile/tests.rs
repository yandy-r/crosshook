use super::*;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{GameProfile, ProfileStore};
use crosshook_core::settings::SettingsStore;
use tempfile::tempdir;

fn store_pair() -> (SettingsStore, ProfileStore) {
    let temp_dir = tempdir().unwrap();
    let settings_store =
        SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"));
    let profile_store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    (settings_store, profile_store)
}

// ---- auto-load resolution (moved from src-tauri/src/startup.rs) -----------

#[test]
fn returns_none_when_auto_load_is_disabled() {
    let (settings_store, profile_store) = store_pair();
    settings_store
        .save(&AppSettingsData {
            auto_load_last_profile: false,
            last_used_profile: "elden-ring".to_string(),
            community_taps: Vec::new(),
            onboarding_completed: false,
            offline_mode: false,
            ..Default::default()
        })
        .unwrap();
    profile_store
        .save("elden-ring", &GameProfile::default())
        .unwrap();

    let resolved = resolve_auto_load_profile(
        &settings_store.load().unwrap(),
        &profile_store.list().unwrap(),
    );

    assert_eq!(resolved, None);
}

#[test]
fn returns_some_when_last_used_profile_exists() {
    let (settings_store, profile_store) = store_pair();
    settings_store
        .save(&AppSettingsData {
            auto_load_last_profile: true,
            last_used_profile: "elden-ring".to_string(),
            community_taps: Vec::new(),
            onboarding_completed: false,
            offline_mode: false,
            ..Default::default()
        })
        .unwrap();
    profile_store
        .save("elden-ring", &GameProfile::default())
        .unwrap();

    let resolved = resolve_auto_load_profile(
        &settings_store.load().unwrap(),
        &profile_store.list().unwrap(),
    );

    assert_eq!(resolved.as_deref(), Some("elden-ring"));
}

#[test]
fn returns_none_when_last_used_profile_is_missing() {
    let (settings_store, profile_store) = store_pair();
    settings_store
        .save(&AppSettingsData {
            auto_load_last_profile: true,
            last_used_profile: "missing-profile".to_string(),
            community_taps: Vec::new(),
            onboarding_completed: false,
            offline_mode: false,
            ..Default::default()
        })
        .unwrap();

    let resolved = resolve_auto_load_profile(
        &settings_store.load().unwrap(),
        &profile_store.list().unwrap(),
    );

    assert_eq!(resolved, None);
}

#[test]
fn returns_none_when_last_used_profile_is_blank() {
    let (settings_store, profile_store) = store_pair();
    settings_store
        .save(&AppSettingsData {
            auto_load_last_profile: true,
            last_used_profile: "   ".to_string(),
            community_taps: Vec::new(),
            onboarding_completed: false,
            offline_mode: false,
            ..Default::default()
        })
        .unwrap();

    let resolved = resolve_auto_load_profile(
        &settings_store.load().unwrap(),
        &profile_store.list().unwrap(),
    );

    assert_eq!(resolved, None);
}

// ---- pure-settings variant used by live snapshots -------------------------

#[test]
fn resolve_auto_load_profile_matches_store_variant() {
    let (settings_store, profile_store) = store_pair();
    let settings = AppSettingsData {
        auto_load_last_profile: true,
        last_used_profile: "elden-ring".to_string(),
        ..Default::default()
    };
    settings_store.save(&settings).unwrap();
    profile_store
        .save("elden-ring", &GameProfile::default())
        .unwrap();

    assert_eq!(
        resolve_auto_load_profile(&settings, &profile_store.list().unwrap()).as_deref(),
        Some("elden-ring")
    );
}

#[test]
fn resolve_auto_load_profile_none_when_disabled() {
    let settings = AppSettingsData {
        auto_load_last_profile: false,
        last_used_profile: "anything".to_string(),
        ..Default::default()
    };
    assert_eq!(resolve_auto_load_profile(&settings, &[]), None);
}

// ---- onboarding derivation -------------------------------------------------

#[test]
fn onboarding_shows_when_not_completed_and_no_profiles() {
    let (_, profile_store) = store_pair();
    let settings = AppSettingsData {
        onboarding_completed: false,
        ..Default::default()
    };

    let onboarding = derive_onboarding(&settings, &profile_store.list().unwrap());

    assert!(onboarding.show);
    assert!(!onboarding.has_profiles);
}

#[test]
fn onboarding_hidden_when_completed_with_profiles() {
    let (_, profile_store) = store_pair();
    profile_store
        .save("elden-ring", &GameProfile::default())
        .unwrap();
    let settings = AppSettingsData {
        onboarding_completed: true,
        ..Default::default()
    };

    let onboarding = derive_onboarding(&settings, &profile_store.list().unwrap());

    assert!(!onboarding.show);
    assert!(onboarding.has_profiles);
}

#[test]
fn onboarding_reports_profiles_even_when_prompt_shows() {
    let (_, profile_store) = store_pair();
    profile_store
        .save("elden-ring", &GameProfile::default())
        .unwrap();
    let settings = AppSettingsData {
        onboarding_completed: false,
        ..Default::default()
    };

    let onboarding = derive_onboarding(&settings, &profile_store.list().unwrap());

    assert!(onboarding.show);
    assert!(onboarding.has_profiles);
}

// ---- nag sync --------------------------------------------------------------

#[test]
fn sync_nags_persists_legacy_dismissals() {
    let metadata = MetadataStore::open_in_memory().unwrap();
    let settings = AppSettingsData {
        install_nag_dismissed_at: Some("2025-01-01T00:00:00Z".to_string()),
        steam_deck_caveats_dismissed_at: Some("2025-01-01T00:00:00Z".to_string()),
        ..Default::default()
    };

    sync_nags(&settings, &metadata);

    let dismissed = metadata.get_dismissed_readiness_nags().unwrap();
    assert!(dismissed.contains("umu_run"));
    assert!(dismissed.contains("steam_deck_caveats"));
}

#[test]
fn sync_nags_noop_without_legacy_dismissals() {
    let metadata = MetadataStore::open_in_memory().unwrap();
    let settings = AppSettingsData::default();

    sync_nags(&settings, &metadata);

    let dismissed = metadata.get_dismissed_readiness_nags().unwrap();
    assert!(dismissed.is_empty());
}

#[test]
fn sync_nags_noop_on_disabled_metadata() {
    let metadata = MetadataStore::disabled();
    let settings = AppSettingsData {
        install_nag_dismissed_at: Some("2025-01-01T00:00:00Z".to_string()),
        ..Default::default()
    };

    // Must not panic or attempt a write.
    sync_nags(&settings, &metadata);
}

// ---- reconciliation --------------------------------------------------------

#[test]
fn reconciliation_syncs_profiles_into_metadata() {
    let (_, profile_store) = store_pair();
    profile_store
        .save("elden-ring", &GameProfile::default())
        .unwrap();
    let metadata = MetadataStore::open_in_memory().unwrap();

    run_metadata_reconciliation(&metadata, &profile_store).unwrap();

    let ids = metadata
        .query_profile_ids_for_names(&["elden-ring".to_string()])
        .unwrap();
    assert_eq!(ids.len(), 1);
}

#[test]
fn reconciliation_is_idempotent() {
    let (_, profile_store) = store_pair();
    profile_store
        .save("elden-ring", &GameProfile::default())
        .unwrap();
    let metadata = MetadataStore::open_in_memory().unwrap();

    run_metadata_reconciliation(&metadata, &profile_store).unwrap();
    run_metadata_reconciliation(&metadata, &profile_store).unwrap();

    let ids = metadata
        .query_profile_ids_for_names(&["elden-ring".to_string()])
        .unwrap();
    assert_eq!(ids.len(), 1);
}
