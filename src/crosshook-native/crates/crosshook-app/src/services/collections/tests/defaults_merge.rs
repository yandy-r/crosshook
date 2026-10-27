//! `CollectionsService` defaults merge tests (D9): the policy ported from the
//! Tauri shell helper plus the per-field merge table.

use crosshook_core::profile::{
    CollectionDefaultsSection, GameProfile, GamescopeConfig, LaunchOptimizationsSection,
    MangoHudConfig,
};

use super::helpers::{profile_with_env, Harness};
use crate::services::collections::CollectionsService;
use crate::AppErrorKind;

fn extra_table(key: &str, value: &str) -> toml::Table {
    let mut table = toml::Table::new();
    table.insert(key.to_string(), toml::Value::String(value.to_string()));
    table
}

fn gamescope(width: u32) -> GamescopeConfig {
    GamescopeConfig {
        enabled: true,
        internal_width: Some(width),
        ..GamescopeConfig::default()
    }
}

// ── apply_defaults policy (ported) ──────────────────────────────────────

#[tokio::test]
async fn none_collection_id_returns_profile_unchanged() {
    let h = Harness::new();
    let profile = profile_with_env("PROFILE_ONLY", "keep-me");

    let result = h
        .service
        .apply_defaults(profile.clone(), None)
        .await
        .unwrap();

    assert_eq!(result, profile);
}

#[tokio::test]
async fn whitespace_collection_id_returns_profile_unchanged() {
    let h = Harness::new();
    let profile = profile_with_env("PROFILE_ONLY", "keep-me");

    let result = h
        .service
        .apply_defaults(profile.clone(), Some("   "))
        .await
        .unwrap();

    assert_eq!(result, profile);
}

#[tokio::test]
async fn valid_defaults_merge_into_profile() {
    let h = Harness::new();
    let mut defaults = CollectionDefaultsSection {
        method: Some("proton_run".to_string()),
        ..CollectionDefaultsSection::default()
    };
    defaults
        .custom_env_vars
        .insert("CROSSHOOK_PROBE".to_string(), "1".to_string());
    let id = h.collection_with_defaults("Speedrun Tools", &defaults);

    let profile = profile_with_env("PROFILE_ONLY", "keep-me");
    let result = h
        .service
        .apply_defaults(profile, Some(id.as_str()))
        .await
        .unwrap();

    assert_eq!(result.launch.method, "proton_run");
    assert_eq!(
        result
            .launch
            .custom_env_vars
            .get("CROSSHOOK_PROBE")
            .map(String::as_str),
        Some("1"),
        "collection env vars merge on top of profile env vars"
    );
    assert_eq!(
        result
            .launch
            .custom_env_vars
            .get("PROFILE_ONLY")
            .map(String::as_str),
        Some("keep-me"),
        "profile env vars without a collision are preserved"
    );
}

#[tokio::test]
async fn unknown_collection_id_fails_open_with_unmodified_profile() {
    let h = Harness::new();
    let profile = profile_with_env("PROFILE_ONLY", "keep-me");

    let result = h
        .service
        .apply_defaults(profile.clone(), Some("no-such-id"))
        .await
        .unwrap();

    assert_eq!(result, profile);
}

#[tokio::test]
async fn corrupt_defaults_bubble_error_to_caller() {
    let h = Harness::new();
    let id = h.metadata.create_collection("Broken").unwrap();
    h.corrupt_defaults(&id);

    let err = h
        .service
        .apply_defaults(
            profile_with_env("PROFILE_ONLY", "keep-me"),
            Some(id.as_str()),
        )
        .await
        .expect_err("corrupt defaults must bubble up");

    assert_eq!(err.kind(), AppErrorKind::Internal);
    assert_eq!(err.message(), "metadata database is corrupt");
}

#[tokio::test]
async fn database_error_fails_open() {
    let h = Harness::new();
    let id = h.metadata.create_collection("Soon broken").unwrap();
    h.metadata
        .with_sqlite_conn("break schema", |conn| {
            conn.execute_batch("ALTER TABLE collections RENAME TO collections_gone")
                .expect("rename table");
            Ok(())
        })
        .unwrap();
    let profile = profile_with_env("PROFILE_ONLY", "keep-me");

    let result = h
        .service
        .apply_defaults(profile.clone(), Some(id.as_str()))
        .await
        .expect("a non-corrupt store failure must fail open");

    assert_eq!(result, profile);
}

#[tokio::test]
async fn null_defaults_leave_profile_unchanged() {
    let h = Harness::new();
    let id = h.metadata.create_collection("No defaults").unwrap();
    let profile = profile_with_env("PROFILE_ONLY", "keep-me");

    let result = h
        .service
        .apply_defaults(profile.clone(), Some(id.as_str()))
        .await
        .unwrap();

    assert_eq!(result, profile);
}

#[tokio::test]
async fn disabled_store_is_identity() {
    let h = Harness::disabled();
    let profile = profile_with_env("PROFILE_ONLY", "keep-me");

    let result = h
        .service
        .apply_defaults(profile.clone(), Some("any-id"))
        .await
        .unwrap();

    assert_eq!(result, profile);
}

// ── per-field merge table (pure `resolve`) ──────────────────────────────

#[test]
fn resolve_without_defaults_is_identity() {
    let profile = profile_with_env("A", "1");
    assert_eq!(CollectionsService::resolve(profile.clone(), None), profile);
}

#[test]
fn network_isolation_some_true_and_some_false_replace() {
    let mut profile = GameProfile::default();
    profile.launch.network_isolation = false;
    let on = CollectionDefaultsSection {
        network_isolation: Some(true),
        ..CollectionDefaultsSection::default()
    };
    assert!(
        CollectionsService::resolve(profile.clone(), Some(&on))
            .launch
            .network_isolation
    );

    profile.launch.network_isolation = true;
    let off = CollectionDefaultsSection {
        network_isolation: Some(false),
        ..CollectionDefaultsSection::default()
    };
    assert!(
        !CollectionsService::resolve(profile.clone(), Some(&off))
            .launch
            .network_isolation
    );

    let unset = CollectionDefaultsSection::default();
    assert!(
        CollectionsService::resolve(profile, Some(&unset))
            .launch
            .network_isolation
    );
}

#[test]
fn trainer_gamescope_replaces_while_gamescope_is_untouched() {
    let mut profile = GameProfile::default();
    profile.launch.gamescope = gamescope(800);
    profile.launch.trainer_gamescope = gamescope(1);
    let defaults = CollectionDefaultsSection {
        trainer_gamescope: Some(gamescope(1920)),
        ..CollectionDefaultsSection::default()
    };

    let merged = CollectionsService::resolve(profile, Some(&defaults));

    assert_eq!(merged.launch.trainer_gamescope.internal_width, Some(1920));
    assert_eq!(merged.launch.gamescope.internal_width, Some(800));
}

#[test]
fn gamescope_replaces_and_keeps_profile_extras() {
    let mut profile = GameProfile::default();
    profile.launch.gamescope = gamescope(800);
    profile.launch.gamescope.extra = extra_table("future", "profile");
    let mut incoming = gamescope(2560);
    incoming.extra = extra_table("future", "collection");
    let defaults = CollectionDefaultsSection {
        gamescope: Some(incoming),
        ..CollectionDefaultsSection::default()
    };

    let merged = CollectionsService::resolve(profile, Some(&defaults));

    assert_eq!(merged.launch.gamescope.internal_width, Some(2560));
    assert_eq!(
        merged.launch.gamescope.extra,
        extra_table("future", "profile")
    );
}

#[test]
fn mangohud_replaces_with_profile_extra_retained() {
    let mut profile = GameProfile::default();
    profile.launch.mangohud = MangoHudConfig {
        enabled: false,
        fps_limit: Some(30),
        extra: extra_table("keep", "me"),
        ..MangoHudConfig::default()
    };
    let defaults = CollectionDefaultsSection {
        mangohud: Some(MangoHudConfig {
            enabled: true,
            fps_limit: Some(144),
            extra: extra_table("drop", "me"),
            ..MangoHudConfig::default()
        }),
        ..CollectionDefaultsSection::default()
    };

    let merged = CollectionsService::resolve(profile, Some(&defaults));

    assert!(merged.launch.mangohud.enabled);
    assert_eq!(merged.launch.mangohud.fps_limit, Some(144));
    assert_eq!(merged.launch.mangohud.extra, extra_table("keep", "me"));
}

#[test]
fn optimizations_replace_keeping_profile_extras() {
    let mut profile = GameProfile::default();
    profile.launch.optimizations = LaunchOptimizationsSection {
        enabled_option_ids: vec!["old".to_string()],
        extra: extra_table("keep", "me"),
    };
    let defaults = CollectionDefaultsSection {
        optimizations: Some(LaunchOptimizationsSection {
            enabled_option_ids: vec!["new_a".to_string(), "new_b".to_string()],
            extra: extra_table("drop", "me"),
        }),
        ..CollectionDefaultsSection::default()
    };

    let merged = CollectionsService::resolve(profile, Some(&defaults));

    assert_eq!(
        merged.launch.optimizations.enabled_option_ids,
        vec!["new_a", "new_b"]
    );
    assert_eq!(merged.launch.optimizations.extra, extra_table("keep", "me"));
}

#[test]
fn whitespace_method_is_ignored() {
    let mut profile = GameProfile::default();
    profile.launch.method = "native".to_string();
    let defaults = CollectionDefaultsSection {
        method: Some("   ".to_string()),
        ..CollectionDefaultsSection::default()
    };

    let merged = CollectionsService::resolve(profile, Some(&defaults));

    assert_eq!(merged.launch.method, "native");
}

#[test]
fn empty_custom_env_vars_is_a_no_op() {
    let profile = profile_with_env("PROFILE_ONLY", "keep-me");
    let defaults = CollectionDefaultsSection::default();

    let merged = CollectionsService::resolve(profile.clone(), Some(&defaults));

    assert_eq!(merged, profile);
}

#[test]
fn colliding_env_key_takes_collection_value() {
    let profile = profile_with_env("SHARED", "profile");
    let mut defaults = CollectionDefaultsSection::default();
    defaults
        .custom_env_vars
        .insert("SHARED".to_string(), "collection".to_string());

    let merged = CollectionsService::resolve(profile, Some(&defaults));

    assert_eq!(
        merged
            .launch
            .custom_env_vars
            .get("SHARED")
            .map(String::as_str),
        Some("collection")
    );
}

#[test]
fn mixed_all_fields_merge() {
    let mut profile = profile_with_env("PROFILE_ONLY", "keep-me");
    profile.launch.method = "native".to_string();
    profile.launch.network_isolation = false;
    profile.launch.gamescope = gamescope(100);
    let mut defaults = CollectionDefaultsSection {
        method: Some("proton_run".to_string()),
        optimizations: Some(LaunchOptimizationsSection {
            enabled_option_ids: vec!["opt".to_string()],
            extra: toml::Table::new(),
        }),
        network_isolation: Some(true),
        gamescope: Some(gamescope(200)),
        trainer_gamescope: Some(gamescope(300)),
        mangohud: Some(MangoHudConfig {
            enabled: true,
            ..MangoHudConfig::default()
        }),
        ..CollectionDefaultsSection::default()
    };
    defaults
        .custom_env_vars
        .insert("COLLECTION".to_string(), "1".to_string());

    let merged = CollectionsService::resolve(profile, Some(&defaults));

    assert_eq!(merged.launch.method, "proton_run");
    assert_eq!(merged.launch.optimizations.enabled_option_ids, vec!["opt"]);
    assert!(merged.launch.network_isolation);
    assert_eq!(merged.launch.gamescope.internal_width, Some(200));
    assert_eq!(merged.launch.trainer_gamescope.internal_width, Some(300));
    assert!(merged.launch.mangohud.enabled);
    assert_eq!(merged.launch.custom_env_vars.len(), 2);
}

// ── resolve_defaults_sync branches (AC2) ────────────────────────────────

#[test]
fn sync_resolver_none_and_blank_ids_yield_none() {
    let h = Harness::new();
    assert_eq!(h.service.resolve_defaults_sync(None).unwrap(), None);
    assert_eq!(h.service.resolve_defaults_sync(Some("  ")).unwrap(), None);
}

#[test]
fn sync_resolver_returns_stored_defaults() {
    let h = Harness::new();
    let defaults = CollectionDefaultsSection {
        method: Some("proton_run".to_string()),
        ..CollectionDefaultsSection::default()
    };
    let id = h.collection_with_defaults("C", &defaults);

    // The id is trimmed before lookup.
    let resolved = h
        .service
        .resolve_defaults_sync(Some(&format!("  {id} ")))
        .unwrap();

    assert_eq!(resolved, Some(defaults));
}

#[test]
fn sync_resolver_unknown_id_and_null_defaults_yield_none() {
    let h = Harness::new();
    let id = h.metadata.create_collection("Empty").unwrap();

    assert_eq!(
        h.service.resolve_defaults_sync(Some("no-such-id")).unwrap(),
        None
    );
    assert_eq!(h.service.resolve_defaults_sync(Some(&id)).unwrap(), None);
}

#[test]
fn sync_resolver_corrupt_defaults_error() {
    let h = Harness::new();
    let id = h.metadata.create_collection("Broken").unwrap();
    h.corrupt_defaults(&id);

    let err = h.service.resolve_defaults_sync(Some(&id)).unwrap_err();

    assert_eq!(err.kind(), AppErrorKind::Internal);
}

#[test]
fn sync_resolver_disabled_store_yields_none() {
    let h = Harness::disabled();
    assert_eq!(h.service.resolve_defaults_sync(Some("any")).unwrap(), None);
}

// ── get/set defaults ────────────────────────────────────────────────────

#[tokio::test]
async fn get_defaults_clears_extras() {
    let h = Harness::new();
    let mut seeded = CollectionDefaultsSection {
        method: Some("proton_run".to_string()),
        ..CollectionDefaultsSection::default()
    };
    seeded.extra = extra_table("future", "x");
    let id = h.collection_with_defaults("C", &seeded);

    let got = h.service.get_defaults(id).await.unwrap().unwrap();

    assert_eq!(got.method.as_deref(), Some("proton_run"));
    assert!(got.extra.is_empty());
}

#[tokio::test]
async fn set_defaults_ignores_incoming_extras_and_preserves_stored_ones() {
    let h = Harness::new();
    let mut seeded = CollectionDefaultsSection {
        method: Some("native".to_string()),
        ..CollectionDefaultsSection::default()
    };
    seeded.extra = extra_table("stored", "keep");
    let id = h.collection_with_defaults("C", &seeded);

    let mut incoming = CollectionDefaultsSection {
        method: Some("proton_run".to_string()),
        ..CollectionDefaultsSection::default()
    };
    incoming.extra = extra_table("injected", "nope");
    h.service
        .set_defaults(id.clone(), Some(incoming))
        .await
        .unwrap();

    let stored = h.metadata.get_collection_defaults(&id).unwrap().unwrap();
    assert_eq!(stored.method.as_deref(), Some("proton_run"));
    assert_eq!(stored.extra, extra_table("stored", "keep"));
}

#[tokio::test]
async fn set_defaults_none_clears() {
    let h = Harness::new();
    let defaults = CollectionDefaultsSection {
        method: Some("native".to_string()),
        ..CollectionDefaultsSection::default()
    };
    let id = h.collection_with_defaults("C", &defaults);

    h.service.set_defaults(id.clone(), None).await.unwrap();

    assert_eq!(h.service.get_defaults(id).await.unwrap(), None);
}

#[tokio::test]
async fn set_defaults_fails_on_corrupt_existing_row_but_none_clears_it() {
    let h = Harness::new();
    let id = h.metadata.create_collection("Broken").unwrap();
    h.corrupt_defaults(&id);
    let incoming = CollectionDefaultsSection {
        method: Some("native".to_string()),
        ..CollectionDefaultsSection::default()
    };

    let err = h
        .service
        .set_defaults(id.clone(), Some(incoming))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), AppErrorKind::Internal);

    h.service.set_defaults(id.clone(), None).await.unwrap();
    assert_eq!(h.service.get_defaults(id).await.unwrap(), None);
}

#[tokio::test]
async fn defaults_unknown_collection_keeps_not_found_text() {
    let h = Harness::new();
    let err = h
        .service
        .get_defaults("ghost".to_string())
        .await
        .unwrap_err();
    assert_eq!(
        err.kind(),
        AppErrorKind::Validation {
            field: "collection"
        }
    );
    assert_eq!(
        err.message(),
        "metadata validation error: collection not found: ghost"
    );
    let err = h
        .service
        .set_defaults("ghost".to_string(), None)
        .await
        .unwrap_err();
    assert_eq!(
        err.message(),
        "metadata validation error: collection not found: ghost"
    );
}

#[tokio::test]
async fn defaults_disabled_store_is_unavailable() {
    let h = Harness::disabled();
    let err = h.service.get_defaults("id".to_string()).await.unwrap_err();
    assert_eq!(err.kind(), AppErrorKind::Unavailable);
    let err = h
        .service
        .set_defaults("id".to_string(), None)
        .await
        .unwrap_err();
    assert_eq!(err.kind(), AppErrorKind::Unavailable);
}
