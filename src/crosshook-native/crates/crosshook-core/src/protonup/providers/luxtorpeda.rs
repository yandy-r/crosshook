//! Luxtorpeda provider — catalog-only, fetches releases from luxtorpeda-dev/luxtorpeda.
//!
//! Install support is disabled because the folder-name-vs-tag invariant required
//! for safe extraction into the compatibility tools directory has not yet been
//! verified for Luxtorpeda releases.

use async_trait::async_trait;

use crate::protonup::ProtonUpAvailableVersion;

use super::{ChecksumKind, GhRelease, ProtonReleaseProvider, ProviderError};

const GH_RELEASES_URL: &str = "https://api.github.com/repos/luxtorpeda-dev/luxtorpeda/releases";
const MAX_RELEASES: usize = 30;

/// Provider for Luxtorpeda (catalog-only).
pub struct LuxtorpedaProvider;

impl LuxtorpedaProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LuxtorpedaProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtonReleaseProvider for LuxtorpedaProvider {
    fn id(&self) -> &'static str {
        "luxtorpeda"
    }

    fn display_name(&self) -> &'static str {
        "Luxtorpeda"
    }

    /// Catalog-only: folder-name-vs-tag invariant not yet verified.
    fn supports_install(&self) -> bool {
        false
    }

    fn checksum_kind(&self) -> ChecksumKind {
        ChecksumKind::Sha256Manifest
    }

    async fn fetch(
        &self,
        client: &reqwest::Client,
        _include_prereleases: bool,
    ) -> Result<Vec<ProtonUpAvailableVersion>, ProviderError> {
        use reqwest::StatusCode;

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

        Ok(releases_to_versions(releases))
    }
}

fn releases_to_versions(releases: Vec<GhRelease>) -> Vec<ProtonUpAvailableVersion> {
    releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .take(MAX_RELEASES)
        .filter_map(|r| {
            // Luxtorpeda publishes a per-release SHA256SUMS manifest.
            let checksum_url = format!(
                "https://github.com/luxtorpeda-dev/luxtorpeda/releases/download/{}/SHA256SUMS",
                r.tag_name
            );
            // Pick the primary .tar.gz asset (catalog-only, so we still record download_url).
            let tarball = r
                .assets
                .iter()
                .find(|a| a.name.ends_with(".tar.gz") && !a.name.contains(".sha"))?;

            Some(ProtonUpAvailableVersion {
                provider: "luxtorpeda".to_string(),
                version: r.tag_name,
                release_url: Some(r.html_url),
                download_url: Some(tarball.browser_download_url.clone()),
                asset_size: Some(tarball.size),
                checksum_url: Some(checksum_url),
                checksum_kind: Some("sha256".to_string()),
                published_at: r.published_at,
            })
        })
        .collect()
}

/// The GitHub Releases API URL used by this provider.
///
/// Exposed for `catalog.rs` to use as the SQLite cache key URL.
pub fn gh_releases_url() -> &'static str {
    GH_RELEASES_URL
}

/// SQLite cache key for this provider's catalog.
pub fn cache_key() -> &'static str {
    "protonup:catalog:luxtorpeda"
}

#[cfg(test)]
mod tests {
    use super::super::{GhAsset, GhRelease};
    use super::*;

    fn make_release(
        tag_name: &str,
        draft: bool,
        prerelease: bool,
        assets: Vec<GhAsset>,
    ) -> GhRelease {
        GhRelease {
            tag_name: tag_name.to_string(),
            html_url: format!(
                "https://github.com/luxtorpeda-dev/luxtorpeda/releases/tag/{tag_name}"
            ),
            draft,
            prerelease,
            assets,
            published_at: Some("2024-09-12T10:00:00Z".to_string()),
        }
    }

    fn tar_gz_asset(tag_name: &str) -> GhAsset {
        GhAsset {
            name: format!("luxtorpeda-{tag_name}.tar.gz"),
            browser_download_url: format!(
                "https://github.com/luxtorpeda-dev/luxtorpeda/releases/download/{tag_name}/luxtorpeda-{tag_name}.tar.gz"
            ),
            size: 50_000_000,
        }
    }

    #[test]
    fn parse_releases_luxtorpeda_shapes_tags() {
        let releases = vec![make_release("10", false, false, vec![tar_gz_asset("10")])];

        let versions = releases_to_versions(releases);
        assert_eq!(versions.len(), 1);
        let v = &versions[0];
        assert_eq!(v.provider, "luxtorpeda");
        assert_eq!(v.version, "10");
        assert!(v.download_url.as_deref().unwrap().ends_with(".tar.gz"));
        assert_eq!(
            v.checksum_url.as_deref(),
            Some("https://github.com/luxtorpeda-dev/luxtorpeda/releases/download/10/SHA256SUMS")
        );
        assert_eq!(v.checksum_kind.as_deref(), Some("sha256"));
    }

    #[test]
    fn luxtorpeda_is_catalog_only() {
        let provider = LuxtorpedaProvider::new();
        assert!(!provider.supports_install());
    }

    #[test]
    fn luxtorpeda_checksum_kind_is_sha256_manifest() {
        let provider = LuxtorpedaProvider::new();
        assert_eq!(provider.checksum_kind(), ChecksumKind::Sha256Manifest);
    }
}
