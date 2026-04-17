//! GE-Proton provider — fetches releases from GloriousEggroll/proton-ge-custom.

use async_trait::async_trait;

use crate::protonup::ProtonUpAvailableVersion;

use super::{parse_releases, ChecksumKind, GhRelease, ProtonReleaseProvider, ProviderError};

const GH_RELEASES_URL: &str =
    "https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases";
const MAX_RELEASES: usize = 30;

/// Provider for GE-Proton (community-patched Proton by GloriousEggroll).
pub struct GeProtonProvider;

impl GeProtonProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GeProtonProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtonReleaseProvider for GeProtonProvider {
    fn id(&self) -> &'static str {
        "ge-proton"
    }

    fn display_name(&self) -> &'static str {
        "GE-Proton"
    }

    fn supports_install(&self) -> bool {
        true
    }

    fn checksum_kind(&self) -> ChecksumKind {
        ChecksumKind::Sha512Sidecar
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

        Ok(parse_releases(releases, self.id(), MAX_RELEASES))
    }
}

/// The GitHub Releases API URL used by this provider.
///
/// Exposed for `catalog.rs` to use as the SQLite cache key URL.
pub fn gh_releases_url() -> &'static str {
    GH_RELEASES_URL
}

/// SQLite cache key for this provider's catalog.
pub fn cache_key() -> &'static str {
    "protonup:catalog:ge-proton"
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
            html_url: format!("https://github.com/example/releases/tag/{tag_name}"),
            draft,
            prerelease,
            assets,
            published_at: Some("2025-04-10T22:16:05Z".to_string()),
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

        let versions = parse_releases(releases, "ge-proton", MAX_RELEASES);
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
            make_release(
                "GE-Proton9-21",
                true,
                false,
                vec![tar_gz_asset("GE-Proton9-21")],
            ),
            make_release(
                "GE-Proton9-20",
                false,
                false,
                vec![tar_gz_asset("GE-Proton9-20")],
            ),
        ];

        let versions = parse_releases(releases, "ge-proton", MAX_RELEASES);
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, "GE-Proton9-20");
    }

    #[test]
    fn skips_prerelease_releases() {
        let releases = vec![
            make_release(
                "GE-Proton9-21-rc1",
                false,
                true,
                vec![tar_gz_asset("GE-Proton9-21-rc1")],
            ),
            make_release(
                "GE-Proton9-20",
                false,
                false,
                vec![tar_gz_asset("GE-Proton9-20")],
            ),
        ];

        let versions = parse_releases(releases, "ge-proton", MAX_RELEASES);
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

        let versions = parse_releases(releases, "ge-proton", MAX_RELEASES);
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

        let versions = parse_releases(releases, "ge-proton", MAX_RELEASES);
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

        let versions = parse_releases(releases, "ge-proton", MAX_RELEASES);
        let v = &versions[0];
        assert!(v.checksum_url.is_none());
        assert!(v.checksum_kind.is_none());
    }

    #[test]
    fn caps_at_max_releases() {
        let releases: Vec<GhRelease> = (0..MAX_RELEASES + 5)
            .map(|i| {
                let tag = format!("GE-Proton9-{i}");
                make_release(&tag, false, false, vec![tar_gz_asset(&tag)])
            })
            .collect();

        let versions = parse_releases(releases, "ge-proton", MAX_RELEASES);
        assert_eq!(versions.len(), MAX_RELEASES);
    }

    #[test]
    fn empty_release_list_returns_empty() {
        let versions = parse_releases(vec![], "ge-proton", MAX_RELEASES);
        assert!(versions.is_empty());
    }

    #[test]
    fn propagates_published_at_from_release() {
        let releases = vec![make_release(
            "GE-Proton9-21",
            false,
            false,
            vec![tar_gz_asset("GE-Proton9-21")],
        )];

        let versions = parse_releases(releases, "ge-proton", MAX_RELEASES);
        assert_eq!(
            versions[0].published_at.as_deref(),
            Some("2025-04-10T22:16:05Z")
        );
    }
}
