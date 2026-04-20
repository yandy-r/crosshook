use chrono::{DateTime, Duration as ChronoDuration, Utc};

use crate::metadata::{MetadataStore, ProtonCatalogRow};
use crate::protonup::{
    providers, ProtonUpAvailableVersion, ProtonUpCacheMeta, ProtonUpCatalogResponse,
};

use super::CACHE_TTL_HOURS;

/// SQLite `provider_id` column stores this scoped key so stable and prerelease
/// catalog snapshots do not overwrite each other.
pub(crate) fn scoped_cache_key(logical_provider_id: &str, include_prereleases: bool) -> String {
    format!(
        "{}:{}",
        logical_provider_id,
        if include_prereleases {
            "prereleases"
        } else {
            "stable"
        }
    )
}

/// Strip `:stable` / `:prereleases` for [`providers::registry`] lookup (checksum kind, etc.).
pub(crate) fn logical_provider_id_for_registry(scoped_provider_id: &str) -> &str {
    scoped_provider_id
        .strip_suffix(":stable")
        .or_else(|| scoped_provider_id.strip_suffix(":prereleases"))
        .unwrap_or(scoped_provider_id)
}

/// Load all rows for `cache_key` from the v22 `proton_release_catalog` table.
///
/// `cache_key` is the scoped id from [`scoped_cache_key`].
///
/// Returns an empty vec when the store is unavailable or has no rows for this key.
pub(crate) fn load_catalog_rows(
    metadata_store: &MetadataStore,
    cache_key: &str,
) -> Vec<ProtonCatalogRow> {
    if !metadata_store.is_available() {
        return Vec::new();
    }

    match metadata_store.get_proton_catalog(cache_key) {
        Ok(rows) => rows,
        Err(error) => {
            tracing::warn!(%error, cache_key, "failed to load ProtonUp catalog rows from DB");
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
pub(crate) fn build_response_from_rows(
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
/// Each `ProtonUpAvailableVersion` becomes one row keyed on `(scoped_cache_key, version_tag)`.
/// The provider-level `ChecksumKind` from the registry is serialized into `checksum_kind`
/// so the install path can choose the right verification strategy without re-fetching.
///
/// `scoped_provider_id` is the result of [`scoped_cache_key`]; row `provider_id` matches it.
pub(crate) fn persist_catalog(
    metadata_store: &MetadataStore,
    scoped_provider_id: &str,
    versions: &[ProtonUpAvailableVersion],
    fetched_at: &str,
    expires_at: &str,
) {
    let logical = logical_provider_id_for_registry(scoped_provider_id);

    // Derive the provider's ChecksumKind from the registry for the checksum_kind column.
    let registry_checksum_kind: Option<String> = {
        let registry = providers::registry();
        registry.iter().find(|p| p.id() == logical).map(|p| {
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
                        scoped_provider_id,
                        version = %v.version,
                        %err,
                        "failed to serialize ProtonUp version for catalog row — skipping"
                    );
                    return None;
                }
            };

            Some(ProtonCatalogRow {
                provider_id: scoped_provider_id.to_owned(),
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

    if let Err(error) = metadata_store.replace_proton_catalog(scoped_provider_id, &rows) {
        tracing::warn!(
            scoped_provider_id,
            %error,
            "failed to atomically replace ProtonUp catalog snapshot in DB"
        );
    }
}

pub(crate) fn stale_fallback_or_offline(
    metadata_store: &MetadataStore,
    cache_key: &str,
) -> ProtonUpCatalogResponse {
    let stale_rows = load_catalog_rows(metadata_store, cache_key);
    if let Some(response) = build_response_from_rows(&stale_rows, true) {
        return response;
    }

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
