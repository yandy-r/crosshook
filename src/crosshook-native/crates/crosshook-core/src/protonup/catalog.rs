//! Catalog retrieval for Proton compatibility-tool releases (GE-Proton, Proton-CachyOS, …)
//! with cache-first / live-refresh / stale-fallback.

use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use tokio::sync::OnceCell;

use crate::metadata::{MetadataStore, ProtonCatalogRow};
use crate::protonup::{
    providers, ProtonUpAvailableVersion, ProtonUpCacheMeta, ProtonUpCatalogResponse,
    ProtonUpProvider,
};

const CACHE_TTL_HOURS: i64 = 6;
const REQUEST_TIMEOUT_SECS: u64 = 10;

static PROTONUP_HTTP_CLIENT: OnceCell<reqwest::Client> = OnceCell::const_new();

/// Per-provider GitHub Releases API URL and `provider_id` string on rows.
///
/// Retained for use by `fetch_live_catalog`'s fallback HTTP path and the
/// existing `catalog_configs_have_distinct_cache_keys_and_urls` test.
#[derive(Debug, Clone, Copy)]
pub struct CatalogProviderConfig {
    pub cache_key: &'static str,
    pub gh_releases_url: &'static str,
    pub provider_id: &'static str,
}

pub fn catalog_config(provider: ProtonUpProvider) -> CatalogProviderConfig {
    match provider {
        ProtonUpProvider::GeProton => CatalogProviderConfig {
            cache_key: providers::ge_proton::cache_key(),
            gh_releases_url: providers::ge_proton::gh_releases_url(),
            provider_id: "ge-proton",
        },
        ProtonUpProvider::ProtonCachyos => CatalogProviderConfig {
            cache_key: providers::proton_cachyos::cache_key(),
            gh_releases_url: providers::proton_cachyos::gh_releases_url(),
            provider_id: "proton-cachyos",
        },
        ProtonUpProvider::ProtonEm => CatalogProviderConfig {
            cache_key: providers::proton_em::cache_key(),
            gh_releases_url: providers::proton_em::gh_releases_url(),
            provider_id: "proton-em",
        },
    }
}

// ----- HTTP client -----

async fn protonup_http_client() -> Result<&'static reqwest::Client, reqwest::Error> {
    PROTONUP_HTTP_CLIENT
        .get_or_try_init(|| async {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
                .build()
        })
        .await
}

// ----- Public entry point -----

/// Fetch available versions by provider id with cache-live-stale fallback.
///
/// Dispatch goes through the [`providers::registry`], so any provider id the
/// registry knows about (including `proton-em` and experimental providers)
/// works. Unknown ids yield an empty catalog rather than silently falling
/// back to GE-Proton.
///
/// 1. If `force_refresh` is false, return a valid (non-expired) cache hit immediately.
/// 2. Attempt a live fetch via the provider trait.
/// 3. On network failure, fall back to a stale cache entry.
/// 4. If no cache exists at all, return an empty offline response.
pub async fn list_available_versions_by_id(
    metadata_store: &MetadataStore,
    force_refresh: bool,
    provider_id: &str,
    include_prereleases: bool,
) -> ProtonUpCatalogResponse {
    // Step 1: cache-first (skip on force_refresh)
    if !force_refresh {
        let rows = load_catalog_rows(metadata_store, provider_id);
        if let Some(response) = build_response_from_rows(&rows, false) {
            return response;
        }
    }

    // Step 2: live fetch via the provider trait
    match fetch_live_catalog_by_id(provider_id, include_prereleases).await {
        Ok(versions) => {
            let fetched_at = Utc::now().to_rfc3339();
            let expires_at = (Utc::now() + ChronoDuration::hours(CACHE_TTL_HOURS)).to_rfc3339();

            persist_catalog(
                metadata_store,
                provider_id,
                &versions,
                &fetched_at,
                &expires_at,
            );

            ProtonUpCatalogResponse {
                versions,
                cache: ProtonUpCacheMeta {
                    stale: false,
                    offline: false,
                    fetched_at: Some(fetched_at),
                    expires_at: Some(expires_at),
                },
            }
        }

        Err(error) => {
            tracing::warn!(%error, provider_id, "ProtonUp live catalog fetch failed — trying stale cache");

            // Step 3: stale fallback
            let stale_rows = load_catalog_rows(metadata_store, provider_id);
            if let Some(response) = build_response_from_rows(&stale_rows, true) {
                return response;
            }

            // Step 4: completely offline, no cache
            ProtonUpCatalogResponse {
                versions: Vec::new(),
                cache: ProtonUpCacheMeta {
                    stale: false,
                    offline: true,
                    fetched_at: None,
                    expires_at: None,
                },
            }
        }
    }
}

/// Back-compat shim: dispatches the legacy [`ProtonUpProvider`] enum via its
/// kebab-case id. Prefer [`list_available_versions_by_id`] for new callers.
pub async fn list_available_versions(
    metadata_store: &MetadataStore,
    force_refresh: bool,
    provider: ProtonUpProvider,
    include_prereleases: bool,
) -> ProtonUpCatalogResponse {
    list_available_versions_by_id(
        metadata_store,
        force_refresh,
        &provider.to_string(),
        include_prereleases,
    )
    .await
}

// ----- Live fetch -----

async fn fetch_live_catalog_by_id(
    provider_id: &str,
    include_prereleases: bool,
) -> Result<Vec<ProtonUpAvailableVersion>, providers::ProviderError> {
    let client = protonup_http_client()
        .await
        .map_err(providers::ProviderError::Http)?;

    // Resolve the matching provider implementation from the registry.
    let registry = providers::registry();
    match registry.iter().find(|p| p.id() == provider_id).cloned() {
        Some(provider_impl) => provider_impl.fetch(client, include_prereleases).await,
        None => {
            tracing::warn!(
                provider_id,
                "Unknown Proton provider id — returning empty catalog"
            );
            Ok(Vec::new())
        }
    }
}

// ----- Cache helpers -----

/// Load all rows for `provider_id` from the v22 `proton_release_catalog` table.
///
/// Returns an empty vec when the store is unavailable or has no rows for this provider.
fn load_catalog_rows(metadata_store: &MetadataStore, provider_id: &str) -> Vec<ProtonCatalogRow> {
    if !metadata_store.is_available() {
        return Vec::new();
    }

    match metadata_store.get_proton_catalog(provider_id) {
        Ok(rows) => rows,
        Err(error) => {
            tracing::warn!(%error, provider_id, "failed to load ProtonUp catalog rows from DB");
            Vec::new()
        }
    }
}

/// Convert a set of `ProtonCatalogRow`s into a `ProtonUpCatalogResponse`.
///
/// When `is_stale` is false the caller should only pass rows whose `expires_at`
/// has not yet elapsed — this function treats whatever is given as authoritative.
/// When `is_stale` is true the rows may be expired; mark the response accordingly.
///
/// Returns `None` when `rows` is empty or every row fails to deserialize.
fn build_response_from_rows(
    rows: &[ProtonCatalogRow],
    is_stale: bool,
) -> Option<ProtonUpCatalogResponse> {
    if rows.is_empty() {
        return None;
    }

    let now = Utc::now();
    let ttl_cutoff = now - ChronoDuration::hours(CACHE_TTL_HOURS);

    // When not forcing stale, skip if all rows are expired.
    if !is_stale {
        let all_expired = rows.iter().all(|r| {
            // Prefer expires_at if present; fall back to fetched_at + TTL.
            if let Some(ref exp) = r.expires_at {
                DateTime::parse_from_rfc3339(exp)
                    .map(|dt| dt.with_timezone(&Utc) <= now)
                    .unwrap_or(true)
            } else {
                DateTime::parse_from_rfc3339(&r.fetched_at)
                    .map(|dt| dt.with_timezone(&Utc) <= ttl_cutoff)
                    .unwrap_or(true)
            }
        });
        if all_expired {
            return None;
        }
    }

    // Deserialize each row's payload_json into a ProtonUpAvailableVersion.
    let versions: Vec<ProtonUpAvailableVersion> = rows
        .iter()
        .filter_map(|r| match serde_json::from_str(&r.payload_json) {
            Ok(v) => Some(v),
            Err(err) => {
                tracing::warn!(
                    provider_id = %r.provider_id,
                    version_tag = %r.version_tag,
                    %err,
                    "failed to parse ProtonUp catalog row payload — treating as missing"
                );
                None
            }
        })
        .collect();

    if versions.is_empty() {
        return None;
    }

    // cache_meta.fetched_at: the oldest fetched_at across all rows defines the age of cached data.
    let oldest_fetched_at = rows
        .iter()
        .map(|r| r.fetched_at.as_str())
        .min()
        .map(str::to_owned);

    // Determine staleness from the oldest row's age when not already forced stale.
    let actually_stale = is_stale || {
        oldest_fetched_at
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc) <= ttl_cutoff)
            .unwrap_or(false)
    };

    // expires_at for the response: use the minimum expires_at across rows that have one.
    let min_expires_at = rows
        .iter()
        .filter_map(|r| r.expires_at.as_deref())
        .min()
        .map(str::to_owned);

    Some(ProtonUpCatalogResponse {
        versions,
        cache: ProtonUpCacheMeta {
            stale: actually_stale,
            offline: is_stale,
            fetched_at: oldest_fetched_at,
            expires_at: min_expires_at,
        },
    })
}

/// Persist fetched versions into the v22 `proton_release_catalog` table.
///
/// Each `ProtonUpAvailableVersion` becomes one row keyed on `(provider_id, version_tag)`.
/// The provider-level `ChecksumKind` from the registry is serialized into `checksum_kind`
/// so the install path can choose the right verification strategy without re-fetching.
fn persist_catalog(
    metadata_store: &MetadataStore,
    provider_id: &str,
    versions: &[ProtonUpAvailableVersion],
    fetched_at: &str,
    expires_at: &str,
) {
    // Derive the provider's ChecksumKind from the registry for the checksum_kind column.
    let registry_checksum_kind: Option<String> = {
        let registry = providers::registry();
        registry.iter().find(|p| p.id() == provider_id).map(|p| {
            serde_json::to_string(&p.checksum_kind())
                .unwrap_or_default()
                .trim_matches('"')
                .to_owned()
        })
    };

    let rows: Vec<ProtonCatalogRow> = versions
        .iter()
        .filter_map(|v| {
            let payload_json = match serde_json::to_string(v) {
                Ok(s) => s,
                Err(err) => {
                    tracing::warn!(
                        provider_id,
                        version = %v.version,
                        %err,
                        "failed to serialize ProtonUp version for catalog row — skipping"
                    );
                    return None;
                }
            };

            Some(ProtonCatalogRow {
                provider_id: provider_id.to_owned(),
                version_tag: v.version.clone(),
                payload_json,
                release_url: v.release_url.clone(),
                download_url: v.download_url.clone(),
                checksum_url: v.checksum_url.clone(),
                // Use the row's own checksum_kind when present; otherwise fall back to
                // the provider-level registry value.
                checksum_kind: v
                    .checksum_kind
                    .clone()
                    .or_else(|| registry_checksum_kind.clone()),
                asset_size: v.asset_size.map(|s| s as i64),
                fetched_at: fetched_at.to_owned(),
                expires_at: Some(expires_at.to_owned()),
            })
        })
        .collect();

    if let Err(error) = metadata_store.replace_proton_catalog(provider_id, &rows) {
        tracing::warn!(
            provider_id,
            %error,
            "failed to atomically replace ProtonUp catalog snapshot in DB"
        );
    }
}

// ----- Tests -----

#[cfg(test)]
mod tests {
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
        let rows = load_catalog_rows(&store, "ge-proton");
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
        let rows = vec![
            make_row("ge-proton", "GE-Proton9-1", &fetched, Some(&future), &v1),
            make_row("ge-proton", "GE-Proton9-2", &fetched, Some(&future), &v2),
        ];

        store.put_proton_catalog(&rows).expect("put rows");

        let loaded = load_catalog_rows(&store, "ge-proton");
        assert_eq!(loaded.len(), 2);

        let resp = build_response_from_rows(&loaded, false)
            .expect("should produce a response from fresh rows");
        assert_eq!(resp.versions.len(), 2);
        assert!(!resp.cache.stale, "fresh rows should not be stale");
        assert!(!resp.cache.offline);
        assert!(resp.cache.fetched_at.is_some());
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
        let rows = vec![
            // Stale row: no expires_at, fetched_at older than TTL.
            make_row("ge-proton", "GE-Proton9-1", &stale_fetched, None, &v1),
            // Fresh row: expires in future.
            make_row(
                "ge-proton",
                "GE-Proton9-2",
                &fresh_fetched,
                Some(&future),
                &v2,
            ),
        ];

        store.put_proton_catalog(&rows).expect("put rows");

        let loaded = load_catalog_rows(&store, "ge-proton");
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
        let rows = vec![
            make_row("ge-proton", "GE-Proton9-1", &fetched, Some(&past), &v1),
            make_row("ge-proton", "GE-Proton9-2", &fetched, Some(&past), &v2),
        ];

        store.put_proton_catalog(&rows).expect("put rows");

        let loaded = load_catalog_rows(&store, "ge-proton");
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
        let rows = load_catalog_rows(&store, "ge-proton");
        assert!(rows.is_empty());

        // build_response_from_rows on empty gives None, which maps to offline empty.
        let resp = build_response_from_rows(&rows, false);
        assert!(resp.is_none());
    }
}
