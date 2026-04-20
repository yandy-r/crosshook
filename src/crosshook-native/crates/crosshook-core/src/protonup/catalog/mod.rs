//! Catalog retrieval for Proton compatibility-tool releases (GE-Proton, Proton-CachyOS, …)
//! with cache-first / live-refresh / stale-fallback.

mod cache;
mod client;
mod config;
mod fetch;

use chrono::{Duration as ChronoDuration, Utc};

use crate::metadata::MetadataStore;
use crate::protonup::{ProtonUpCacheMeta, ProtonUpCatalogResponse, ProtonUpProvider};

pub(crate) use cache::{
    build_response_from_rows, load_catalog_rows, persist_catalog, scoped_cache_key,
    stale_fallback_or_offline,
};
pub use config::{catalog_config, CatalogProviderConfig};
use fetch::fetch_live_catalog_by_id;

pub(crate) const CACHE_TTL_HOURS: i64 = 6;

/// Fetch available versions by provider id with cache-live-stale fallback.
///
/// Dispatch goes through the [`providers::registry`], so any provider id the
/// registry knows about (including `proton-em` and experimental providers)
/// works. Unknown ids surface stale cache when possible; otherwise an empty
/// offline response.
///
/// 1. If `force_refresh` is false, return a valid (non-expired) cache hit immediately.
/// 2. Attempt a live fetch via the provider trait.
/// 3. On network failure **or** empty/unknown live results, fall back to a stale cache entry.
/// 4. If no cache exists at all, return an empty offline response.
pub async fn list_available_versions_by_id(
    metadata_store: &MetadataStore,
    force_refresh: bool,
    provider_id: &str,
    include_prereleases: bool,
) -> ProtonUpCatalogResponse {
    let cache_key = scoped_cache_key(provider_id, include_prereleases);

    // Step 1: cache-first (skip on force_refresh)
    if !force_refresh {
        let rows = load_catalog_rows(metadata_store, &cache_key);
        if let Some(response) = build_response_from_rows(&rows, false) {
            return response;
        }
    }

    // Step 2: live fetch via the provider trait
    match fetch_live_catalog_by_id(provider_id, include_prereleases).await {
        Ok(versions) if !versions.is_empty() => {
            let fetched_at = Utc::now().to_rfc3339();
            let expires_at = (Utc::now() + ChronoDuration::hours(CACHE_TTL_HOURS)).to_rfc3339();

            persist_catalog(
                metadata_store,
                &cache_key,
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

        Ok(_) => {
            tracing::warn!(
                provider_id,
                "ProtonUp live catalog returned empty — trying stale cache"
            );
            stale_fallback_or_offline(metadata_store, &cache_key)
        }

        Err(error) => {
            tracing::warn!(%error, provider_id, "ProtonUp live catalog fetch failed — trying stale cache");
            stale_fallback_or_offline(metadata_store, &cache_key)
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

#[cfg(test)]
mod tests;
