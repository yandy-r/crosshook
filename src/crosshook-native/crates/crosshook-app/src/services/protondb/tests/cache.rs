use crosshook_core::metadata::MetadataStore;
use crosshook_core::protondb::{ProtonDbLookupResult, ProtonDbLookupState, ProtonDbTier};

use super::helpers::Harness;
use crate::AppErrorKind;

#[tokio::test]
async fn lookup_and_suggestions_preserve_cached_data() {
    let harness = Harness::new(MetadataStore::open_in_memory().unwrap());
    harness.profile(true);
    harness.seed_cache("123", false);
    let lookup = harness.service.lookup(" 123 ".into(), false).await.unwrap();
    assert_eq!(lookup.state, ProtonDbLookupState::Ready);
    let cache = lookup.cache.unwrap();
    assert!(cache.from_cache);
    assert!(!cache.is_stale);
    let suggestions = harness
        .service
        .suggestions("123".into(), "Game".into(), false)
        .await
        .unwrap();
    assert_eq!(suggestions.tier, ProtonDbTier::Gold);
    assert_eq!(suggestions.total_reports, 12);
    assert!(!suggestions.is_stale);
}

#[tokio::test]
async fn disabled_metadata_keeps_empty_lookup_and_suggestions_usable() {
    let harness = Harness::new(MetadataStore::disabled());
    harness.profile(false);
    assert_eq!(
        harness.service.lookup("  ".into(), false).await.unwrap(),
        ProtonDbLookupResult::default()
    );
    let result = harness
        .service
        .suggestions("  ".into(), "Game".into(), false)
        .await
        .unwrap();
    assert!(result.catalog_suggestions.is_empty());
    assert!(result.env_var_suggestions.is_empty());
    assert_eq!(result.tier, ProtonDbTier::Unknown);
}

#[tokio::test]
async fn stale_cache_falls_back_when_live_lookup_fails() {
    let harness = Harness::new(MetadataStore::open_in_memory().unwrap());
    // Matches core's existing stale regression: this is not a valid Steam id.
    let app_id = "invalid app id";
    harness.seed_cache(app_id, true);
    let result = harness.service.lookup(app_id.into(), false).await.unwrap();
    assert_eq!(result.state, ProtonDbLookupState::Stale);
    let cache = result.cache.unwrap();
    assert!(cache.from_cache && cache.is_stale && cache.is_offline);
    assert_eq!(result.snapshot.unwrap().tier, ProtonDbTier::Gold);
}

#[tokio::test]
async fn dismiss_preserves_validation_texts_and_thirty_day_retention_without_eviction() {
    let harness = Harness::new(MetadataStore::open_in_memory().unwrap());
    let empty = harness
        .service
        .dismiss_suggestion("  ".into(), "123".into(), "key".into())
        .await
        .unwrap_err();
    assert_eq!(
        empty.kind(),
        AppErrorKind::Validation {
            field: "profile_name"
        }
    );
    assert_eq!(empty.message(), "profile_name is required");
    harness.profile(false);
    let missing = harness
        .service
        .dismiss_suggestion("Game".into(), "123".into(), "key".into())
        .await
        .unwrap_err();
    assert_eq!(missing.kind(), AppErrorKind::NotFound);
    assert_eq!(
        missing.message(),
        "profile 'Game' has no metadata record — cannot dismiss suggestion"
    );
    harness.profile(true);
    let id = harness.metadata.lookup_profile_id("Game").unwrap().unwrap();
    harness
        .metadata
        .dismiss_suggestion(&id, "123", "old", 0)
        .unwrap();
    harness.metadata.with_sqlite_conn("expire fixture dismissal", |conn| {
        conn.execute("UPDATE suggestion_dismissals SET expires_at = '2000-01-01T00:00:00Z' WHERE suggestion_key = 'old'", []).unwrap();
        Ok(())
    }).unwrap();
    harness
        .service
        .dismiss_suggestion(" Game ".into(), "123".into(), "key".into())
        .await
        .unwrap();
    assert!(harness
        .metadata
        .get_dismissed_keys(&id, "123")
        .unwrap()
        .contains("key"));
    harness.metadata.with_sqlite_conn("inspect retained dismissals", |conn| {
        let old: i64 = conn.query_row("SELECT COUNT(*) FROM suggestion_dismissals WHERE suggestion_key = 'old'", [], |row| row.get(0)).unwrap();
        assert_eq!(old, 1);
        let days: f64 = conn.query_row("SELECT julianday(expires_at) - julianday(dismissed_at) FROM suggestion_dismissals WHERE suggestion_key = 'key'", [], |row| row.get(0)).unwrap();
        assert!((days - 30.0).abs() < 0.001);
        Ok(())
    }).unwrap();
}
