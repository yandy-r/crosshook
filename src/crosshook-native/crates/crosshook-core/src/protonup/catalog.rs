//! Catalog retrieval for GE-Proton releases with cache-first/live-refresh/stale-fallback.

use std::sync::OnceLock;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use reqwest::StatusCode;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;

use crate::metadata::{MetadataStore, MetadataStoreError};
use crate::protonup::{ProtonUpAvailableVersion, ProtonUpCacheMeta, ProtonUpCatalogResponse};

const CACHE_KEY: &str = "protonup:catalog:ge-proton";
const CACHE_TTL_HOURS: i64 = 6;
const GH_RELEASES_URL: &str =
    "https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases";
const REQUEST_TIMEOUT_SECS: u64 = 10;
const MAX_RELEASES: usize = 30;
const PROVIDER_GE_PROTON: &str = "ge-proton";

static PROTONUP_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

// ----- GitHub Release API response types (private) -----

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

// ----- Internal cache row -----

#[derive(Debug)]
struct CachedCatalogRow {
    payload_json: String,
    fetched_at: String,
    expires_at: Option<String>,
}

// ----- HTTP client -----

fn protonup_http_client() -> Result<&'static reqwest::Client, reqwest::Error> {
    if let Some(client) = PROTONUP_HTTP_CLIENT.get() {
        return Ok(client);
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let _ = PROTONUP_HTTP_CLIENT.set(client);
    Ok(PROTONUP_HTTP_CLIENT
        .get()
        .expect("ProtonUp HTTP client should be initialized before use"))
}

// ----- Public entry point -----

/// Fetch available GE-Proton versions with cache-live-stale fallback.
///
/// 1. If `force_refresh` is false, return a valid (non-expired) cache hit immediately.
/// 2. Attempt a live fetch from GitHub Releases.
/// 3. On network failure, fall back to a stale cache entry.
/// 4. If no cache exists at all, return an empty offline response.
pub async fn list_available_versions(
    metadata_store: &MetadataStore,
    force_refresh: bool,
) -> ProtonUpCatalogResponse {
    // Step 1: cache-first (skip on force_refresh)
    if !force_refresh {
        if let Some(cached) = load_cached_row(metadata_store, false) {
            if let Some(response) = parse_cached_row(cached, false) {
                return response;
            }
        }
    }

    // Step 2: live fetch
    match fetch_live_catalog().await {
        Ok(versions) => {
            let fetched_at = Utc::now().to_rfc3339();
            let expires_at =
                (Utc::now() + ChronoDuration::hours(CACHE_TTL_HOURS)).to_rfc3339();

            let response = ProtonUpCatalogResponse {
                versions,
                cache: ProtonUpCacheMeta {
                    stale: false,
                    offline: false,
                    fetched_at: Some(fetched_at),
                    expires_at: Some(expires_at.clone()),
                },
            };

            persist_catalog(metadata_store, &response, &expires_at);
            response
        }

        Err(error) => {
            tracing::warn!(%error, "ProtonUp live catalog fetch failed — trying stale cache");

            // Step 3: stale fallback
            if let Some(stale) = load_cached_row(metadata_store, true) {
                if let Some(response) = parse_cached_row(stale, true) {
                    return response;
                }
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

// ----- Live fetch -----

async fn fetch_live_catalog() -> Result<Vec<ProtonUpAvailableVersion>, reqwest::Error> {
    let client = protonup_http_client()?;

    let response = client
        .get(GH_RELEASES_URL)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?;

    if response.status() == StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }

    let releases = response
        .error_for_status()?
        .json::<Vec<GhRelease>>()
        .await?;

    Ok(parse_releases(releases))
}

fn gh_release_to_version(release: GhRelease) -> Option<ProtonUpAvailableVersion> {
    // Require a .tar.gz asset — skip releases without one
    let tarball = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".tar.gz"))?;

    let checksum = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".sha512sum"));

    Some(ProtonUpAvailableVersion {
        provider: PROVIDER_GE_PROTON.to_string(),
        version: release.tag_name,
        release_url: Some(release.html_url),
        download_url: Some(tarball.browser_download_url.clone()),
        asset_size: Some(tarball.size),
        checksum_url: checksum.map(|a| a.browser_download_url.clone()),
        checksum_kind: checksum.map(|_| "sha512".to_string()),
    })
}

// ----- Cache helpers -----

fn load_cached_row(
    metadata_store: &MetadataStore,
    allow_expired: bool,
) -> Option<CachedCatalogRow> {
    if !metadata_store.is_available() {
        return None;
    }

    let now = Utc::now().to_rfc3339();
    let action = if allow_expired {
        "load a cached ProtonUp catalog row"
    } else {
        "load a valid cached ProtonUp catalog row"
    };

    metadata_store
        .with_sqlite_conn(action, |conn| {
            let sql = if allow_expired {
                "SELECT payload_json, fetched_at, expires_at \
                 FROM external_cache_entries WHERE cache_key = ?1 LIMIT 1"
            } else {
                "SELECT payload_json, fetched_at, expires_at \
                 FROM external_cache_entries \
                 WHERE cache_key = ?1 AND (expires_at IS NULL OR expires_at > ?2) LIMIT 1"
            };

            let row_params = if allow_expired {
                params![CACHE_KEY]
            } else {
                params![CACHE_KEY, now]
            };

            conn.query_row(sql, row_params, |row| {
                Ok(CachedCatalogRow {
                    payload_json: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    fetched_at: row.get::<_, String>(1)?,
                    expires_at: row.get::<_, Option<String>>(2)?,
                })
            })
            .optional()
            .map_err(|source| MetadataStoreError::Database {
                action: "query a ProtonUp catalog cache row",
                source,
            })
        })
        .ok()
        .flatten()
}

fn parse_cached_row(row: CachedCatalogRow, is_stale: bool) -> Option<ProtonUpCatalogResponse> {
    if row.payload_json.trim().is_empty() {
        return None;
    }

    let mut response =
        serde_json::from_str::<ProtonUpCatalogResponse>(&row.payload_json).ok()?;

    response.cache = ProtonUpCacheMeta {
        stale: is_stale,
        offline: is_stale,
        fetched_at: Some(row.fetched_at),
        expires_at: row.expires_at,
    };

    Some(response)
}

/// Convert a list of GitHub releases into `ProtonUpAvailableVersion` entries,
/// applying the same filtering rules used during live fetches: skip drafts,
/// pre-releases, and releases without a `.tar.gz` asset. Cap at
/// `MAX_RELEASES`.
///
/// Extracted to allow unit-testing without network I/O.
fn parse_releases(releases: Vec<GhRelease>) -> Vec<ProtonUpAvailableVersion> {
    releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .take(MAX_RELEASES)
        .filter_map(gh_release_to_version)
        .collect()
}

fn persist_catalog(
    metadata_store: &MetadataStore,
    response: &ProtonUpCatalogResponse,
    expires_at: &str,
) {
    let Ok(payload) = serde_json::to_string(response) else {
        tracing::warn!(cache_key = CACHE_KEY, "failed to serialize ProtonUp catalog payload");
        return;
    };

    if let Err(error) =
        metadata_store.put_cache_entry(GH_RELEASES_URL, CACHE_KEY, &payload, Some(expires_at))
    {
        tracing::warn!(cache_key = CACHE_KEY, %error, "failed to persist ProtonUp catalog cache");
    }
}

// ----- Tests -----

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal `GhRelease` for testing.
    fn make_release(
        tag_name: &str,
        draft: bool,
        prerelease: bool,
        assets: Vec<GhAsset>,
    ) -> GhRelease {
        GhRelease {
            tag_name: tag_name.to_string(),
            html_url: format!("https://github.com/GloriousEggroll/proton-ge-custom/releases/tag/{tag_name}"),
            draft,
            prerelease,
            assets,
        }
    }

    fn tar_gz_asset(tag_name: &str) -> GhAsset {
        GhAsset {
            name: format!("{tag_name}.tar.gz"),
            browser_download_url: format!(
                "https://github.com/releases/download/{tag_name}/{tag_name}.tar.gz"
            ),
            size: 1_234_567,
        }
    }

    fn sha512_asset(tag_name: &str) -> GhAsset {
        GhAsset {
            name: format!("{tag_name}.sha512sum"),
            browser_download_url: format!(
                "https://github.com/releases/download/{tag_name}/{tag_name}.sha512sum"
            ),
            size: 128,
        }
    }

    #[test]
    fn parse_gh_release_extracts_version_and_urls() {
        let releases = vec![make_release(
            "GE-Proton9-21",
            false,
            false,
            vec![tar_gz_asset("GE-Proton9-21"), sha512_asset("GE-Proton9-21")],
        )];

        let versions = parse_releases(releases);
        assert_eq!(versions.len(), 1);
        let v = &versions[0];
        assert_eq!(v.version, "GE-Proton9-21");
        assert_eq!(v.provider, "ge-proton");
        assert!(v.download_url.as_deref().unwrap().ends_with(".tar.gz"));
        assert!(v.release_url.as_deref().unwrap().contains("GE-Proton9-21"));
        assert!(v.checksum_url.is_some());
        assert_eq!(v.checksum_kind.as_deref(), Some("sha512"));
        assert_eq!(v.asset_size, Some(1_234_567));
    }

    #[test]
    fn skips_draft_releases() {
        let releases = vec![
            make_release("GE-Proton9-21", true, false, vec![tar_gz_asset("GE-Proton9-21")]),
            make_release("GE-Proton9-20", false, false, vec![tar_gz_asset("GE-Proton9-20")]),
        ];

        let versions = parse_releases(releases);
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, "GE-Proton9-20");
    }

    #[test]
    fn skips_prerelease_releases() {
        let releases = vec![
            make_release("GE-Proton9-21-rc1", false, true, vec![tar_gz_asset("GE-Proton9-21-rc1")]),
            make_release("GE-Proton9-20", false, false, vec![tar_gz_asset("GE-Proton9-20")]),
        ];

        let versions = parse_releases(releases);
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, "GE-Proton9-20");
    }

    #[test]
    fn skips_releases_without_tar_gz() {
        let releases = vec![make_release(
            "GE-Proton9-21",
            false,
            false,
            // Only a checksum — no .tar.gz present.
            vec![sha512_asset("GE-Proton9-21")],
        )];

        let versions = parse_releases(releases);
        assert!(versions.is_empty());
    }

    #[test]
    fn extracts_checksum_url_when_present() {
        let releases = vec![make_release(
            "GE-Proton9-21",
            false,
            false,
            vec![tar_gz_asset("GE-Proton9-21"), sha512_asset("GE-Proton9-21")],
        )];

        let versions = parse_releases(releases);
        let v = &versions[0];
        assert!(v.checksum_url.is_some());
        assert!(v.checksum_url.as_deref().unwrap().ends_with(".sha512sum"));
        assert_eq!(v.checksum_kind.as_deref(), Some("sha512"));
    }

    #[test]
    fn checksum_url_is_none_when_no_sha512sum_asset() {
        let releases = vec![make_release(
            "GE-Proton9-21",
            false,
            false,
            vec![tar_gz_asset("GE-Proton9-21")],
        )];

        let versions = parse_releases(releases);
        let v = &versions[0];
        assert!(v.checksum_url.is_none());
        assert!(v.checksum_kind.is_none());
    }

    #[test]
    fn caps_at_max_releases() {
        // Generate MAX_RELEASES + 5 valid releases.
        let releases: Vec<GhRelease> = (0..MAX_RELEASES + 5)
            .map(|i| {
                let tag = format!("GE-Proton9-{i}");
                make_release(&tag, false, false, vec![tar_gz_asset(&tag)])
            })
            .collect();

        let versions = parse_releases(releases);
        assert_eq!(versions.len(), MAX_RELEASES);
    }

    #[test]
    fn empty_release_list_returns_empty() {
        let versions = parse_releases(vec![]);
        assert!(versions.is_empty());
    }
}
