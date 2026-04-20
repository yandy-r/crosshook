use chrono::{Duration as ChronoDuration, Utc};

use crate::metadata::{MetadataStore, ProtonCatalogRow};
use crate::protonup::{ProtonUpAvailableVersion, ProtonUpProvider};

use super::cache::logical_provider_id_for_registry;
use super::*;

#[test]
fn catalog_configs_have_distinct_cache_keys_and_urls() {
    let ge = catalog_config(ProtonUpProvider::GeProton);
    let cachy = catalog_config(ProtonUpProvider::ProtonCachyos);
    let em = catalog_config(ProtonUpProvider::ProtonEm);
    assert_ne!(ge.cache_key, cachy.cache_key);
    assert_ne!(ge.cache_key, em.cache_key);
    assert_ne!(cachy.cache_key, em.cache_key);
    assert_ne!(ge.gh_releases_url, cachy.gh_releases_url);
    assert_ne!(ge.gh_releases_url, em.gh_releases_url);
    assert_ne!(cachy.gh_releases_url, em.gh_releases_url);
    assert_ne!(ge.provider_id, cachy.provider_id);
    assert_ne!(ge.provider_id, em.provider_id);
    assert_ne!(cachy.provider_id, em.provider_id);
    assert!(ge.gh_releases_url.contains("GloriousEggroll"));
    assert!(cachy.gh_releases_url.contains("CachyOS/proton-cachyos"));
    assert!(em.gh_releases_url.contains("Etaash-mathamsetty/Proton"));
}

#[test]
fn scoped_cache_key_parts() {
    assert_eq!(scoped_cache_key("ge-proton", false), "ge-proton:stable");
    assert_eq!(scoped_cache_key("ge-proton", true), "ge-proton:prereleases");
}

#[test]
fn logical_provider_id_for_registry_strips_suffix() {
    assert_eq!(
        logical_provider_id_for_registry("ge-proton:stable"),
        "ge-proton"
    );
    assert_eq!(
        logical_provider_id_for_registry("proton-cachyos:prereleases"),
        "proton-cachyos"
    );
}

// ── Integration: v22 DB + catalog read/write path ─────────────────────────

fn make_version(version: &str) -> ProtonUpAvailableVersion {
    ProtonUpAvailableVersion {
        provider: "ge-proton".to_string(),
        version: version.to_string(),
        release_url: Some(format!("https://example.com/release/{version}")),
        download_url: Some(format!("https://example.com/{version}.tar.gz")),
        checksum_url: Some(format!("https://example.com/{version}.sha512sum")),
        checksum_kind: Some("sha512".to_string()),
        asset_size: Some(500_000_000),
        published_at: Some("2024-06-01T00:00:00Z".to_string()),
    }
}

fn make_row(
    provider_id: &str,
    version_tag: &str,
    fetched_at: &str,
    expires_at: Option<&str>,
    version: &ProtonUpAvailableVersion,
) -> ProtonCatalogRow {
    ProtonCatalogRow {
        provider_id: provider_id.to_string(),
        version_tag: version_tag.to_string(),
        payload_json: serde_json::to_string(version).unwrap(),
        release_url: version.release_url.clone(),
        download_url: version.download_url.clone(),
        checksum_url: version.checksum_url.clone(),
        checksum_kind: version.checksum_kind.clone(),
        asset_size: version.asset_size.map(|s| s as i64),
        fetched_at: fetched_at.to_string(),
        expires_at: expires_at.map(str::to_owned),
    }
}

/// Fresh v22 in-memory store (runs all migrations including 21→22).
fn open_store() -> MetadataStore {
    MetadataStore::open_in_memory().expect("open in-memory metadata store")
}

#[test]
fn empty_store_returns_no_response() {
    let store = open_store();
    let rows = load_catalog_rows(&store, "ge-proton:stable");
    assert!(rows.is_empty());
    let resp = build_response_from_rows(&rows, false);
    assert!(resp.is_none(), "empty store should produce no response");
}

#[test]
fn fresh_rows_return_non_stale_response() {
    let store = open_store();

    // Both rows fetched recently (well within TTL) and not yet expired.
    let future = (Utc::now() + ChronoDuration::hours(5)).to_rfc3339();
    let fetched = Utc::now().to_rfc3339();

    let v1 = make_version("GE-Proton9-1");
    let v2 = make_version("GE-Proton9-2");
    let scoped = "ge-proton:stable";
    let rows = vec![
        make_row(scoped, "GE-Proton9-1", &fetched, Some(&future), &v1),
        make_row(scoped, "GE-Proton9-2", &fetched, Some(&future), &v2),
    ];

    store.put_proton_catalog(&rows).expect("put rows");

    let loaded = load_catalog_rows(&store, scoped);
    assert_eq!(loaded.len(), 2);

    let resp = build_response_from_rows(&loaded, false)
        .expect("should produce a response from fresh rows");
    assert_eq!(resp.versions.len(), 2);
    assert!(!resp.cache.stale, "fresh rows should not be stale");
    assert!(!resp.cache.offline);
    assert!(resp.cache.fetched_at.is_some());
}

#[test]
fn stable_and_prerelease_snapshots_coexist() {
    let store = open_store();
    let future = (Utc::now() + ChronoDuration::hours(5)).to_rfc3339();
    let fetched = Utc::now().to_rfc3339();

    let v_stable = make_version("GE-Proton9-1");
    let v_pre = make_version("GE-Proton9-0-rc1");

    store
        .put_proton_catalog(&[make_row(
            "ge-proton:stable",
            "GE-Proton9-1",
            &fetched,
            Some(&future),
            &v_stable,
        )])
        .expect("put stable");
    store
        .put_proton_catalog(&[make_row(
            "ge-proton:prereleases",
            "GE-Proton9-0-rc1",
            &fetched,
            Some(&future),
            &v_pre,
        )])
        .expect("put prerelease");

    let stable_loaded = load_catalog_rows(&store, "ge-proton:stable");
    let pre_loaded = load_catalog_rows(&store, "ge-proton:prereleases");
    assert_eq!(stable_loaded.len(), 1);
    assert_eq!(pre_loaded.len(), 1);
    assert_ne!(stable_loaded[0].version_tag, pre_loaded[0].version_tag);
}

#[test]
fn stale_oldest_row_makes_response_stale() {
    let store = open_store();

    // One row older than 6 h (stale), one fresh.
    let stale_fetched = (Utc::now() - ChronoDuration::hours(7)).to_rfc3339();
    let fresh_fetched = Utc::now().to_rfc3339();
    let future = (Utc::now() + ChronoDuration::hours(5)).to_rfc3339();

    let v1 = make_version("GE-Proton9-1");
    let v2 = make_version("GE-Proton9-2");
    let scoped = "ge-proton:stable";
    let rows = vec![
        // Stale row: no expires_at, fetched_at older than TTL.
        make_row(scoped, "GE-Proton9-1", &stale_fetched, None, &v1),
        // Fresh row: expires in future.
        make_row(scoped, "GE-Proton9-2", &fresh_fetched, Some(&future), &v2),
    ];

    store.put_proton_catalog(&rows).expect("put rows");

    let loaded = load_catalog_rows(&store, scoped);
    assert_eq!(loaded.len(), 2);

    // Calling build_response_from_rows with is_stale=false:
    // all_expired check: v1 has no expires_at → uses fetched_at + TTL → expired;
    // v2 has a future expires_at → not expired. So not all_expired → proceeds.
    // Staleness check: oldest fetched_at (stale_fetched, 7h ago) < ttl_cutoff → stale=true.
    let resp = build_response_from_rows(&loaded, false)
        .expect("should produce a response even with one stale row");
    assert_eq!(resp.versions.len(), 2);
    assert!(
        resp.cache.stale,
        "oldest row older than TTL should mark response as stale"
    );
}

#[test]
fn all_expired_rows_return_none_for_fresh_check() {
    let store = open_store();

    // Both rows have expires_at in the past.
    let past = (Utc::now() - ChronoDuration::hours(1)).to_rfc3339();
    let fetched = (Utc::now() - ChronoDuration::hours(7)).to_rfc3339();

    let v1 = make_version("GE-Proton9-1");
    let v2 = make_version("GE-Proton9-2");
    let scoped = "ge-proton:stable";
    let rows = vec![
        make_row(scoped, "GE-Proton9-1", &fetched, Some(&past), &v1),
        make_row(scoped, "GE-Proton9-2", &fetched, Some(&past), &v2),
    ];

    store.put_proton_catalog(&rows).expect("put rows");

    let loaded = load_catalog_rows(&store, scoped);
    let resp = build_response_from_rows(&loaded, false);
    assert!(
        resp.is_none(),
        "all-expired rows should return None for a non-stale (fresh-only) check"
    );

    // But stale check should still return them.
    let stale_resp = build_response_from_rows(&loaded, true);
    assert!(
        stale_resp.is_some(),
        "stale=true should return expired rows as fallback"
    );
}

#[test]
fn no_network_offline_path_returns_empty_not_error() {
    // A disabled store (no DB) should yield empty rows → no panic.
    let store = MetadataStore::disabled();
    let rows = load_catalog_rows(&store, "ge-proton:stable");
    assert!(rows.is_empty());

    // build_response_from_rows on empty gives None, which maps to offline empty.
    let resp = build_response_from_rows(&rows, false);
    assert!(resp.is_none());
}

#[tokio::test]
async fn unknown_provider_force_refresh_falls_back_to_stale_scoped_cache() {
    let store = open_store();
    let future = (Utc::now() + ChronoDuration::hours(5)).to_rfc3339();
    let fetched = Utc::now().to_rfc3339();
    let v1 = make_version("GE-Proton9-1");
    let key = scoped_cache_key("totally-unknown-xyz", false);
    let row = make_row(&key, "GE-Proton9-1", &fetched, Some(&future), &v1);
    store.put_proton_catalog(&[row]).expect("seed cache");

    let resp = list_available_versions_by_id(&store, true, "totally-unknown-xyz", false).await;
    assert_eq!(resp.versions.len(), 1);
    assert_eq!(resp.versions[0].version, "GE-Proton9-1");
}
