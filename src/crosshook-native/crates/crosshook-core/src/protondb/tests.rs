use chrono::{Duration, Utc};
use tokio::runtime::Builder;

use super::aggregation::{
    normalize_report_feed, ProtonDbReportEntry, ProtonDbReportFeedResponse, ProtonDbReportNotes,
    ProtonDbReportResponses,
};
use super::models::{
    cache_key_for_app_id, ProtonDbLookupResult, ProtonDbLookupState, ProtonDbSnapshot,
    ProtonDbTier,
};
use crate::metadata::MetadataStore;

fn runtime() -> tokio::runtime::Runtime {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

fn lookup_result(app_id: &str) -> ProtonDbLookupResult {
    runtime()
        .block_on(super::lookup_protondb(
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

    let result = runtime()
        .block_on(super::lookup_protondb(&store, app_id, false));

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
