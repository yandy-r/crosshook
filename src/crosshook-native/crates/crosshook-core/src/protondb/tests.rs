use std::collections::HashSet;

use chrono::{Duration, Utc};
use tokio::runtime::Builder;

use super::aggregation::{
    normalize_report_feed, ProtonDbReportEntry, ProtonDbReportFeedResponse, ProtonDbReportNotes,
    ProtonDbReportResponses,
};
use super::models::{
    cache_key_for_app_id, ProtonDbCacheState, ProtonDbEnvVarSuggestion,
    ProtonDbLookupResult, ProtonDbLookupState, ProtonDbRecommendationGroup, ProtonDbSnapshot,
    ProtonDbTier,
};
use super::suggestions::{derive_suggestions, SuggestionStatus};
use crate::launch::catalog::OptimizationEntry;
use crate::metadata::MetadataStore;
use crate::profile::GameProfile;

fn runtime() -> tokio::runtime::Runtime {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

fn lookup_result(app_id: &str) -> ProtonDbLookupResult {
    runtime().block_on(super::lookup_protondb(
        &MetadataStore::disabled(),
        app_id,
        false,
    ))
}

fn feed(reports: Vec<ProtonDbReportEntry>) -> ProtonDbReportFeedResponse {
    ProtonDbReportFeedResponse { reports }
}

#[test]
fn empty_app_id_short_circuits_to_default_result() {
    let result = lookup_result("   ");

    assert_eq!(result, ProtonDbLookupResult::default());
}

#[test]
fn stale_cache_is_returned_when_live_lookup_fails() {
    let app_id = "invalid app id";
    let store = MetadataStore::open_in_memory().expect("open metadata store");
    let stale_payload = ProtonDbLookupResult {
        app_id: String::new(),
        state: ProtonDbLookupState::Ready,
        cache: None,
        snapshot: Some(ProtonDbSnapshot {
            app_id: String::new(),
            tier: ProtonDbTier::Gold,
            source_url: "https://www.protondb.com/app/old".to_string(),
            fetched_at: String::new(),
            ..ProtonDbSnapshot::default()
        }),
    };
    let expires_at = (Utc::now() - Duration::hours(1)).to_rfc3339();
    let payload = serde_json::to_string(&stale_payload).expect("serialize stale payload");

    store
        .put_cache_entry(
            "https://www.protondb.com/app/old",
            &cache_key_for_app_id(app_id),
            &payload,
            Some(&expires_at),
        )
        .expect("seed expired cache entry");

    let result = runtime().block_on(super::lookup_protondb(&store, app_id, false));

    let cache = result.cache.as_ref().expect("cache metadata");
    let snapshot = result.snapshot.as_ref().expect("cached snapshot");

    assert_eq!(result.app_id, app_id);
    assert_eq!(result.state, ProtonDbLookupState::Stale);
    assert_eq!(cache.cache_key, cache_key_for_app_id(app_id));
    assert!(cache.from_cache);
    assert!(cache.is_stale);
    assert!(cache.is_offline);
    assert!(!cache.fetched_at.is_empty());
    assert_eq!(snapshot.app_id, app_id);
    assert_eq!(snapshot.tier, ProtonDbTier::Gold);
    assert_eq!(snapshot.fetched_at, cache.fetched_at);
}

#[test]
fn safe_env_suggestion_parsing_accepts_supported_key_value_fragments() {
    let groups = normalize_report_feed(feed(vec![ProtonDbReportEntry {
        id: "safe-env".to_string(),
        timestamp: 1,
        responses: ProtonDbReportResponses {
            concluding_notes: String::new(),
            launch_options:
                "PROTON_USE_WINED3D=1 foo=bar BAD-NAME=2 WINEPREFIX=/tmp/prefix %command%"
                    .to_string(),
            proton_version: "9.0-4".to_string(),
            variant: String::new(),
            notes: ProtonDbReportNotes::default(),
        },
    }]));

    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    assert_eq!(group.title, "Suggested environment variables");
    assert_eq!(group.env_vars.len(), 1);
    assert_eq!(group.env_vars[0].key, "PROTON_USE_WINED3D");
    assert_eq!(group.env_vars[0].value, "1");
    assert_eq!(group.env_vars[0].source_label, "Proton 9.0-4");
    assert!(group.launch_options.is_empty());
}

#[test]
fn ld_preload_is_rejected_as_env_suggestion() {
    let groups = normalize_report_feed(feed(vec![ProtonDbReportEntry {
        id: "ld-preload-test".to_string(),
        timestamp: 1,
        responses: ProtonDbReportResponses {
            launch_options: "LD_PRELOAD=/evil.so DXVK_ASYNC=1 %command%".to_string(),
            ..ProtonDbReportResponses::default()
        },
    }]));
    let keys: Vec<&str> = groups
        .iter()
        .flat_map(|g| g.env_vars.iter())
        .map(|e| e.key.as_str())
        .collect();
    assert!(!keys.contains(&"LD_PRELOAD"), "LD_PRELOAD must be blocked");
    assert!(keys.contains(&"DXVK_ASYNC"), "safe key must pass through");
}

#[test]
fn path_is_rejected_as_env_suggestion() {
    let groups = normalize_report_feed(feed(vec![ProtonDbReportEntry {
        id: "path-test".to_string(),
        timestamp: 1,
        responses: ProtonDbReportResponses {
            launch_options: "PATH=/usr/bin PROTON_USE_WINED3D=1 %command%".to_string(),
            ..ProtonDbReportResponses::default()
        },
    }]));
    let keys: Vec<&str> = groups
        .iter()
        .flat_map(|g| g.env_vars.iter())
        .map(|e| e.key.as_str())
        .collect();
    assert!(!keys.contains(&"PATH"), "PATH must be blocked");
    assert!(keys.contains(&"PROTON_USE_WINED3D"), "safe key must pass through");
}

#[test]
fn ld_prefix_keys_are_rejected() {
    let groups = normalize_report_feed(feed(vec![ProtonDbReportEntry {
        id: "ld-prefix-test".to_string(),
        timestamp: 1,
        responses: ProtonDbReportResponses {
            launch_options: "LD_LIBRARY_PATH=/tmp LD_AUDIT=foo DXVK_ASYNC=1 %command%".to_string(),
            ..ProtonDbReportResponses::default()
        },
    }]));
    let keys: Vec<&str> = groups
        .iter()
        .flat_map(|g| g.env_vars.iter())
        .map(|e| e.key.as_str())
        .collect();
    assert!(!keys.contains(&"LD_LIBRARY_PATH"), "LD_LIBRARY_PATH must be blocked via prefix");
    assert!(!keys.contains(&"LD_AUDIT"), "LD_AUDIT must be blocked via prefix");
    assert!(keys.contains(&"DXVK_ASYNC"), "safe key must pass through");
}

#[test]
fn unsupported_raw_launch_strings_stay_copy_only() {
    let raw_launch = r#"env WINEPREFIX="/tmp/prefix" %command%"#;
    let groups = normalize_report_feed(feed(vec![ProtonDbReportEntry {
        id: "raw-launch".to_string(),
        timestamp: 2,
        responses: ProtonDbReportResponses {
            concluding_notes: String::new(),
            launch_options: raw_launch.to_string(),
            proton_version: String::new(),
            variant: String::new(),
            notes: ProtonDbReportNotes::default(),
        },
    }]));

    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    assert_eq!(group.title, "Copy-only launch string");
    assert!(group.env_vars.is_empty());
    assert_eq!(group.launch_options.len(), 1);
    assert_eq!(group.launch_options[0].text, raw_launch);
    assert_eq!(group.launch_options[0].source_label, "Launch option");
    assert_eq!(group.launch_options[0].supporting_report_count, Some(1));
}

// --- Helpers for derive_suggestions tests ---

fn test_catalog_entry(id: &str, env: Vec<[String; 2]>) -> OptimizationEntry {
    OptimizationEntry {
        id: id.to_string(),
        applies_to_method: "proton_run".to_string(),
        env,
        wrappers: Vec::new(),
        conflicts_with: Vec::new(),
        required_binary: String::new(),
        label: format!("Test: {id}"),
        description: format!("Test optimization {id}"),
        help_text: String::new(),
        category: "graphics".to_string(),
        target_gpu_vendor: String::new(),
        advanced: false,
        community: false,
        applicable_methods: Vec::new(),
    }
}

fn test_lookup_with_env_vars(env_vars: Vec<(&str, &str, u32)>) -> ProtonDbLookupResult {
    ProtonDbLookupResult {
        app_id: "12345".to_string(),
        state: ProtonDbLookupState::Ready,
        cache: Some(ProtonDbCacheState {
            is_stale: false,
            ..ProtonDbCacheState::default()
        }),
        snapshot: Some(ProtonDbSnapshot {
            app_id: "12345".to_string(),
            tier: ProtonDbTier::Gold,
            total_reports: Some(100),
            recommendation_groups: vec![ProtonDbRecommendationGroup {
                group_id: "test-group".to_string(),
                env_vars: env_vars
                    .into_iter()
                    .map(|(k, v, count)| ProtonDbEnvVarSuggestion {
                        key: k.to_string(),
                        value: v.to_string(),
                        source_label: "test".to_string(),
                        supporting_report_count: Some(count),
                    })
                    .collect(),
                ..ProtonDbRecommendationGroup::default()
            }],
            ..ProtonDbSnapshot::default()
        }),
    }
}

// --- Catalog bridge tests ---

#[test]
fn catalog_match_maps_known_optimization() {
    let catalog = vec![test_catalog_entry(
        "enable_dxvk_async",
        vec![["DXVK_ASYNC".to_string(), "1".to_string()]],
    )];
    let lookup = test_lookup_with_env_vars(vec![("DXVK_ASYNC", "1", 50)]);
    let profile = GameProfile::default();
    let dismissed = HashSet::new();

    let result = derive_suggestions(&lookup, &profile, &catalog, &dismissed);

    assert_eq!(result.catalog_suggestions.len(), 1);
    assert_eq!(result.catalog_suggestions[0].catalog_entry_id, "enable_dxvk_async");
    assert_eq!(result.env_var_suggestions.len(), 0);
}

#[test]
fn catalog_match_value_mismatch_stays_tier2() {
    let catalog = vec![test_catalog_entry(
        "enable_dxvk_async",
        vec![["DXVK_ASYNC".to_string(), "1".to_string()]],
    )];
    let lookup = test_lookup_with_env_vars(vec![("DXVK_ASYNC", "0", 30)]);
    let profile = GameProfile::default();
    let dismissed = HashSet::new();

    let result = derive_suggestions(&lookup, &profile, &catalog, &dismissed);

    assert_eq!(result.catalog_suggestions.len(), 0);
    assert_eq!(result.env_var_suggestions.len(), 1);
    assert_eq!(result.env_var_suggestions[0].key, "DXVK_ASYNC");
    assert_eq!(result.env_var_suggestions[0].value, "0");
}

#[test]
fn catalog_match_unmapped_key_stays_tier2() {
    let catalog = vec![test_catalog_entry(
        "some_entry",
        vec![["OTHER_KEY".to_string(), "1".to_string()]],
    )];
    let lookup = test_lookup_with_env_vars(vec![("PROTON_NO_ESYNC", "1", 20)]);
    let profile = GameProfile::default();
    let dismissed = HashSet::new();

    let result = derive_suggestions(&lookup, &profile, &catalog, &dismissed);

    assert_eq!(result.catalog_suggestions.len(), 0);
    assert_eq!(result.env_var_suggestions.len(), 1);
    assert_eq!(result.env_var_suggestions[0].key, "PROTON_NO_ESYNC");
}

// --- Status computation tests ---

#[test]
fn already_applied_when_key_matches_profile() {
    let mut profile = GameProfile::default();
    profile
        .launch
        .custom_env_vars
        .insert("PROTON_NO_ESYNC".to_string(), "1".to_string());

    let lookup = test_lookup_with_env_vars(vec![("PROTON_NO_ESYNC", "1", 40)]);
    let result = derive_suggestions(&lookup, &profile, &[], &HashSet::new());

    assert_eq!(result.env_var_suggestions[0].status, SuggestionStatus::AlreadyApplied);
}

#[test]
fn conflict_when_key_present_with_different_value() {
    let mut profile = GameProfile::default();
    profile
        .launch
        .custom_env_vars
        .insert("PROTON_NO_ESYNC".to_string(), "0".to_string());

    let lookup = test_lookup_with_env_vars(vec![("PROTON_NO_ESYNC", "1", 40)]);
    let result = derive_suggestions(&lookup, &profile, &[], &HashSet::new());

    assert_eq!(result.env_var_suggestions[0].status, SuggestionStatus::Conflict);
}

#[test]
fn new_when_key_absent() {
    let profile = GameProfile::default();
    let lookup = test_lookup_with_env_vars(vec![("PROTON_NO_ESYNC", "1", 40)]);
    let result = derive_suggestions(&lookup, &profile, &[], &HashSet::new());

    assert_eq!(result.env_var_suggestions[0].status, SuggestionStatus::New);
}

#[test]
fn dismissed_status_overrides() {
    let profile = GameProfile::default();
    let lookup = test_lookup_with_env_vars(vec![("PROTON_NO_ESYNC", "1", 40)]);
    let mut dismissed = HashSet::new();
    dismissed.insert("PROTON_NO_ESYNC".to_string());

    let result = derive_suggestions(&lookup, &profile, &[], &dismissed);

    assert_eq!(result.env_var_suggestions[0].status, SuggestionStatus::Dismissed);
}

// --- Sorting test ---

#[test]
fn suggestions_sorted_by_report_count_descending() {
    let lookup = test_lookup_with_env_vars(vec![
        ("VAR_A", "1", 10),
        ("VAR_B", "1", 50),
        ("VAR_C", "1", 30),
    ]);
    let profile = GameProfile::default();
    let result = derive_suggestions(&lookup, &profile, &[], &HashSet::new());

    let counts: Vec<u32> = result
        .env_var_suggestions
        .iter()
        .map(|s| s.supporting_report_count)
        .collect();
    assert_eq!(counts, vec![50, 30, 10], "suggestions must be sorted by report count descending");
}
