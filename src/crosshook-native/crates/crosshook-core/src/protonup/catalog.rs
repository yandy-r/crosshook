//! Catalog retrieval for Proton compatibility-tool releases (GE-Proton, Proton-CachyOS, …)
//! with cache-first / live-refresh / stale-fallback.

use std::sync::OnceLock;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use reqwest::StatusCode;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;

use crate::metadata::{MetadataStore, MetadataStoreError};
use crate::protonup::{ProtonUpAvailableVersion, ProtonUpCacheMeta, ProtonUpCatalogResponse, ProtonUpProvider};

const CACHE_TTL_HOURS: i64 = 6;
const REQUEST_TIMEOUT_SECS: u64 = 10;
const MAX_RELEASES: usize = 30;

static PROTONUP_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Per-provider GitHub Releases API URL, SQLite cache key, and `provider` string on rows.
#[derive(Debug, Clone, Copy)]
pub struct CatalogProviderConfig {
    pub cache_key: &'static str,
    pub gh_releases_url: &'static str,
    pub provider_id: &'static str,
}

pub fn catalog_config(provider: ProtonUpProvider) -> CatalogProviderConfig {
    match provider {
        ProtonUpProvider::GeProton => CatalogProviderConfig {
            cache_key: "protonup:catalog:ge-proton",
            gh_releases_url:
                "https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases",
            provider_id: "ge-proton",
        },
        ProtonUpProvider::ProtonCachyos => CatalogProviderConfig {
            cache_key: "protonup:catalog:proton-cachyos",
            gh_releases_url: "https://api.github.com/repos/CachyOS/proton-cachyos/releases",
            provider_id: "proton-cachyos",
        },
    }
}

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

/// Fetch available versions for `provider` with cache-live-stale fallback.
///
/// 1. If `force_refresh` is false, return a valid (non-expired) cache hit immediately.
/// 2. Attempt a live fetch from GitHub Releases.
/// 3. On network failure, fall back to a stale cache entry.
/// 4. If no cache exists at all, return an empty offline response.
pub async fn list_available_versions(
    metadata_store: &MetadataStore,
    force_refresh: bool,
    provider: ProtonUpProvider,
) -> ProtonUpCatalogResponse {
    let cfg = catalog_config(provider);

    // Step 1: cache-first (skip on force_refresh)
    if !force_refresh {
        if let Some(cached) = load_cached_row(metadata_store, false, cfg.cache_key) {
            if let Some(response) = parse_cached_row(cached, false) {
                return response;
            }
        }
    }

    // Step 2: live fetch
    match fetch_live_catalog(&cfg).await {
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

            persist_catalog(metadata_store, &response, &expires_at, &cfg);
            response
        }

        Err(error) => {
            tracing::warn!(%error, cache_key = cfg.cache_key, "ProtonUp live catalog fetch failed — trying stale cache");

            // Step 3: stale fallback
            if let Some(stale) = load_cached_row(metadata_store, true, cfg.cache_key) {
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

async fn fetch_live_catalog(
    cfg: &CatalogProviderConfig,
) -> Result<Vec<ProtonUpAvailableVersion>, reqwest::Error> {
    let client = protonup_http_client()?;

    let response = client
        .get(cfg.gh_releases_url)
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

    Ok(parse_releases(releases, cfg.provider_id))
}

/// Strip `.tar.gz` / `.tar.xz` from an asset filename for matching sidecar `.sha512sum` files.
fn tarball_stem(name: &str) -> Option<&str> {
    name.strip_suffix(".tar.gz")
        .or_else(|| name.strip_suffix(".tar.xz"))
}

/// Pick the primary downloadable archive for a release.
///
/// GE-Proton publishes `.tar.gz`. Proton-CachyOS publishes `.tar.xz` for several
/// architectures; we prefer Linux x86_64 baseline, then x86_64_v3, then any non-ARM `.tar.xz`.
fn pick_tarball_asset<'a>(assets: &'a [GhAsset], provider_id: &str) -> Option<&'a GhAsset> {
    if provider_id == "proton-cachyos" {
        let xz: Vec<&GhAsset> = assets.iter().filter(|a| a.name.ends_with(".tar.xz")).collect();

        let baseline_x86 = xz.iter().find(|a| {
            let n = a.name.as_str();
            n.contains("x86_64") && !n.contains("x86_64_v3")
        });
        let v3 = xz.iter().find(|a| a.name.contains("x86_64_v3"));
        let non_arm = xz.iter().find(|a| !a.name.to_lowercase().contains("arm"));

        baseline_x86
            .or(v3)
            .or(non_arm)
            .copied()
            .or_else(|| xz.first().copied())
    } else {
        assets.iter().find(|a| a.name.ends_with(".tar.gz"))
    }
}

fn find_matching_sha512sum<'a>(assets: &'a [GhAsset], tarball_name: &str) -> Option<&'a GhAsset> {
    let stem = tarball_stem(tarball_name)?;
    let expected = format!("{stem}.sha512sum");
    assets.iter().find(|a| a.name == expected)
}

fn gh_release_to_version(release: GhRelease, provider_id: &str) -> Option<ProtonUpAvailableVersion> {
    let tarball = pick_tarball_asset(&release.assets, provider_id)?;

    let checksum = find_matching_sha512sum(&release.assets, &tarball.name);

    Some(ProtonUpAvailableVersion {
        provider: provider_id.to_string(),
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
    cache_key: &str,
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
                params![cache_key]
            } else {
                params![cache_key, now]
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
/// pre-releases, and releases without a supported tarball (`.tar.gz` for GE-Proton,
/// `.tar.xz` for Proton-CachyOS). Cap at `MAX_RELEASES`.
///
/// Extracted to allow unit-testing without network I/O.
fn parse_releases(releases: Vec<GhRelease>, provider_id: &str) -> Vec<ProtonUpAvailableVersion> {
    releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .take(MAX_RELEASES)
        .filter_map(|r| gh_release_to_version(r, provider_id))
        .collect()
}

fn persist_catalog(
    metadata_store: &MetadataStore,
    response: &ProtonUpCatalogResponse,
    expires_at: &str,
    cfg: &CatalogProviderConfig,
) {
    let Ok(payload) = serde_json::to_string(response) else {
        tracing::warn!(cache_key = cfg.cache_key, "failed to serialize ProtonUp catalog payload");
        return;
    };

    if let Err(error) = metadata_store.put_cache_entry(
        cfg.gh_releases_url,
        cfg.cache_key,
        &payload,
        Some(expires_at),
    ) {
        tracing::warn!(cache_key = cfg.cache_key, %error, "failed to persist ProtonUp catalog cache");
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
            html_url: format!("https://github.com/example/releases/tag/{tag_name}"),
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

    fn tar_xz_named(name: &str) -> GhAsset {
        GhAsset {
            name: name.to_string(),
            browser_download_url: format!("https://github.com/releases/download/x/{name}"),
            size: 300_000_000,
        }
    }

    fn sha512_named(name: &str) -> GhAsset {
        GhAsset {
            name: name.to_string(),
            browser_download_url: format!("https://github.com/releases/download/x/{name}"),
            size: 128,
        }
    }

    #[test]
    fn catalog_configs_have_distinct_cache_keys_and_urls() {
        let ge = catalog_config(ProtonUpProvider::GeProton);
        let cachy = catalog_config(ProtonUpProvider::ProtonCachyos);
        assert_ne!(ge.cache_key, cachy.cache_key);
        assert_ne!(ge.gh_releases_url, cachy.gh_releases_url);
        assert_ne!(ge.provider_id, cachy.provider_id);
        assert!(ge.gh_releases_url.contains("GloriousEggroll"));
        assert!(cachy.gh_releases_url.contains("CachyOS/proton-cachyos"));
    }

    #[test]
    fn parse_gh_release_extracts_version_and_urls() {
        let releases = vec![make_release(
            "GE-Proton9-21",
            false,
            false,
            vec![tar_gz_asset("GE-Proton9-21"), sha512_asset("GE-Proton9-21")],
        )];

        let versions = parse_releases(releases, "ge-proton");
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
    fn parse_releases_stamps_proton_cachyos_provider() {
        let tb = "proton-cachyos-10.0-20260330-slr-x86_64.tar.xz";
        let releases = vec![make_release(
            "cachyos-10.0-20260330-slr",
            false,
            false,
            vec![
                tar_xz_named(tb),
                sha512_named("proton-cachyos-10.0-20260330-slr-x86_64.sha512sum"),
            ],
        )];

        let versions = parse_releases(releases, "proton-cachyos");
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].provider, "proton-cachyos");
        assert_eq!(versions[0].version, "cachyos-10.0-20260330-slr");
        assert!(versions[0].download_url.as_deref().unwrap().ends_with(".tar.xz"));
        assert!(versions[0].checksum_url.as_deref().unwrap().contains("x86_64.sha512sum"));
    }

    #[test]
    fn proton_cachyos_prefers_baseline_x86_64_over_v3_and_arm() {
        let releases = vec![make_release(
            "cachyos-test",
            false,
            false,
            vec![
                tar_xz_named("proton-cachyos-test-arm64.tar.xz"),
                tar_xz_named("proton-cachyos-test-x86_64_v3.tar.xz"),
                tar_xz_named("proton-cachyos-test-x86_64.tar.xz"),
                sha512_named("proton-cachyos-test-x86_64.sha512sum"),
                sha512_named("proton-cachyos-test-x86_64_v3.sha512sum"),
            ],
        )];

        let versions = parse_releases(releases, "proton-cachyos");
        assert_eq!(versions.len(), 1);
        let url = versions[0].download_url.as_deref().unwrap();
        assert!(
            url.contains("x86_64.tar.xz") && !url.contains("x86_64_v3"),
            "expected baseline x86_64, got {url}"
        );
    }

    #[test]
    fn skips_draft_releases() {
        let releases = vec![
            make_release("GE-Proton9-21", true, false, vec![tar_gz_asset("GE-Proton9-21")]),
            make_release("GE-Proton9-20", false, false, vec![tar_gz_asset("GE-Proton9-20")]),
        ];

        let versions = parse_releases(releases, "ge-proton");
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, "GE-Proton9-20");
    }

    #[test]
    fn skips_prerelease_releases() {
        let releases = vec![
            make_release("GE-Proton9-21-rc1", false, true, vec![tar_gz_asset("GE-Proton9-21-rc1")]),
            make_release("GE-Proton9-20", false, false, vec![tar_gz_asset("GE-Proton9-20")]),
        ];

        let versions = parse_releases(releases, "ge-proton");
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

        let versions = parse_releases(releases, "ge-proton");
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

        let versions = parse_releases(releases, "ge-proton");
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

        let versions = parse_releases(releases, "ge-proton");
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

        let versions = parse_releases(releases, "ge-proton");
        assert_eq!(versions.len(), MAX_RELEASES);
    }

    #[test]
    fn empty_release_list_returns_empty() {
        let versions = parse_releases(vec![], "ge-proton");
        assert!(versions.is_empty());
    }
}
