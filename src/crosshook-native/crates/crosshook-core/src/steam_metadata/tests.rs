use chrono::{Duration, Utc};
use tokio::runtime::Builder;

use super::models::{
    cache_key_for_app_id, SteamAppDetails, SteamGenre, SteamMetadataLookupResult,
    SteamMetadataLookupState,
};
use crate::metadata::MetadataStore;

fn runtime() -> tokio::runtime::Runtime {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

fn seed_cache_entry(
    store: &MetadataStore,
    app_id: &str,
    result: &SteamMetadataLookupResult,
    expires_at: &str,
) {
    let cache_key = cache_key_for_app_id(app_id).expect("valid app id should produce a cache key");
    let payload = serde_json::to_string(result).expect("serialize cache payload");
    store
        .put_cache_entry(
            "https://store.steampowered.com/api/appdetails",
            &cache_key,
            &payload,
            Some(expires_at),
        )
        .expect("seed cache entry");
}

fn ready_result(app_id: &str) -> SteamMetadataLookupResult {
    SteamMetadataLookupResult {
        app_id: app_id.to_string(),
        state: SteamMetadataLookupState::Ready,
        app_details: Some(SteamAppDetails {
            name: Some("Half-Life 2".to_string()),
            short_description: Some("A classic game".to_string()),
            header_image: Some("https://cdn.example.com/header.jpg".to_string()),
            genres: vec![SteamGenre {
                id: "1".to_string(),
                description: "Action".to_string(),
            }],
        }),
        from_cache: false,
        is_stale: false,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[test]
fn empty_app_id_returns_default_result() {
    let store = MetadataStore::disabled();
    let result = runtime().block_on(super::lookup_steam_metadata(&store, "   ", false));

    assert_eq!(result, SteamMetadataLookupResult::default());
    assert_eq!(result.state, SteamMetadataLookupState::Idle);
}

#[test]
fn non_numeric_app_id_returns_default_result() {
    let store = MetadataStore::disabled();
    let result = runtime().block_on(super::lookup_steam_metadata(&store, "not-a-number", false));

    assert_eq!(result, SteamMetadataLookupResult::default());
    assert_eq!(result.state, SteamMetadataLookupState::Idle);
}

#[test]
fn valid_cache_hit_returns_ready_from_cache() {
    let app_id = "220";
    let store = MetadataStore::open_in_memory().expect("open metadata store");
    let expires_at = (Utc::now() + Duration::hours(12)).to_rfc3339();
    seed_cache_entry(&store, app_id, &ready_result(app_id), &expires_at);

    let result = runtime().block_on(super::lookup_steam_metadata(&store, app_id, false));

    assert_eq!(result.app_id, app_id);
    assert_eq!(result.state, SteamMetadataLookupState::Ready);
    assert!(result.from_cache, "expected from_cache=true for cache hit");
    assert!(!result.is_stale);

    let details = result.app_details.as_ref().expect("app_details present");
    assert_eq!(details.name.as_deref(), Some("Half-Life 2"));
    assert_eq!(details.genres.len(), 1);
    assert_eq!(details.genres[0].description, "Action");
}

#[test]
fn expired_cache_returns_stale_when_network_unavailable() {
    // Use a clearly invalid app_id so the live fetch will fail quickly (no real network call).
    // We use a valid numeric id but the store is in-memory with an expired entry and no network
    // connectivity in tests — the client will error out and fall back to stale cache.
    let app_id = "0000000001";
    let store = MetadataStore::open_in_memory().expect("open metadata store");
    // Seed an already-expired entry.
    let expires_at = (Utc::now() - Duration::hours(1)).to_rfc3339();
    seed_cache_entry(&store, app_id, &ready_result(app_id), &expires_at);

    let result = runtime().block_on(super::lookup_steam_metadata(&store, app_id, false));

    assert_eq!(result.state, SteamMetadataLookupState::Stale);
    assert!(result.from_cache);
    assert!(result.is_stale);
    assert!(result.app_details.is_some(), "stale result should have details");
}

#[test]
fn missing_cache_without_network_returns_unavailable() {
    // Numeric-looking app id with no cache entry; live fetch will fail (no real network in tests).
    let app_id = "9999999999";
    let store = MetadataStore::open_in_memory().expect("open metadata store");

    let result = runtime().block_on(super::lookup_steam_metadata(&store, app_id, false));

    // Live fetch will fail; no cache entry exists → Unavailable.
    assert_eq!(
        result.state,
        SteamMetadataLookupState::Unavailable,
        "expected Unavailable when no cache and no network"
    );
    assert!(!result.from_cache);
    assert!(!result.is_stale);
    assert!(result.app_details.is_none());
}

#[test]
fn force_refresh_bypasses_valid_cache() {
    // Use a numeric app id that does not exist in the Steam catalog.
    // The live fetch will fail (Steam returns success:false), triggering the stale fallback path.
    let app_id = "9000000001";
    let store = MetadataStore::open_in_memory().expect("open metadata store");
    let expires_at = (Utc::now() + Duration::hours(12)).to_rfc3339();
    seed_cache_entry(&store, app_id, &ready_result(app_id), &expires_at);

    // force_refresh=true skips the valid cache. The live fetch fails for this non-existent app id.
    // Fallback to stale cache → Stale. If stale fallback also fails → Unavailable.
    let result = runtime().block_on(super::lookup_steam_metadata(&store, app_id, true));

    assert!(
        result.state == SteamMetadataLookupState::Stale
            || result.state == SteamMetadataLookupState::Unavailable,
        "force_refresh should skip fresh cache and fall back to Stale or Unavailable, got {:?}",
        result.state
    );
}
