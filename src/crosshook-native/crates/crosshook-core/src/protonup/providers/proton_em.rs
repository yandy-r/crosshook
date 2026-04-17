//! Proton-EM provider — fetches releases from Etaash-mathamsetty/Proton.
//!
//! Canonical upstream repo (community-accepted): https://github.com/Etaash-mathamsetty/Proton
//! If this fork moves, GloriousEggroll/proton-em is a possible fallback to verify.

use async_trait::async_trait;

use crate::protonup::ProtonUpAvailableVersion;

use super::{parse_releases, ChecksumKind, GhRelease, ProtonReleaseProvider, ProviderError};

const GH_RELEASES_URL: &str = "https://api.github.com/repos/Etaash-mathamsetty/Proton/releases";
const MAX_RELEASES: usize = 30;

/// Provider for Proton-EM (community Proton fork by Etaash-mathamsetty).
pub struct ProtonEmProvider;

impl ProtonEmProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProtonEmProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtonReleaseProvider for ProtonEmProvider {
    fn id(&self) -> &'static str {
        "proton-em"
    }

    fn display_name(&self) -> &'static str {
        "Proton-EM"
    }

    fn supports_install(&self) -> bool {
        true
    }

    /// Most Proton-EM releases do not ship checksum sidecars.
    /// `install.rs` handles the `None` case by emitting a warning and a
    /// `Phase::Verifying` progress message without failing the install.
    fn checksum_kind(&self) -> ChecksumKind {
        ChecksumKind::None
    }

    async fn fetch(
        &self,
        client: &reqwest::Client,
        include_prereleases: bool,
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

        Ok(parse_releases(
            releases,
            self.id(),
            MAX_RELEASES,
            include_prereleases,
        ))
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
    "protonup:catalog:proton-em"
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
                "https://github.com/Etaash-mathamsetty/Proton/releases/tag/{tag_name}"
            ),
            draft,
            prerelease,
            assets,
            published_at: Some("2025-12-01T08:00:00Z".to_string()),
        }
    }

    /// Realistic Proton-EM asset shape: `proton-<tag>.tar.xz` (upstream ships
    /// `.tar.xz`, not `.tar.gz`). Earlier tests used `.tar.gz` which passed
    /// only because the picker's non-CachyOS branch was limited to `.tar.gz`
    /// — that shortcut silently dropped every real EM release.
    fn tar_xz_asset(tag_name: &str) -> GhAsset {
        GhAsset {
            name: format!("proton-{tag_name}.tar.xz"),
            browser_download_url: format!(
                "https://github.com/Etaash-mathamsetty/Proton/releases/download/{tag_name}/proton-{tag_name}.tar.xz"
            ),
            size: 2_100_000,
        }
    }

    #[test]
    fn parse_releases_proton_em_skips_drafts_and_prereleases() {
        let releases = vec![
            make_release(
                "proton-em-9.0-draft",
                true,
                false,
                vec![tar_xz_asset("proton-em-9.0-draft")],
            ),
            make_release(
                "proton-em-9.0-rc1",
                false,
                true,
                vec![tar_xz_asset("proton-em-9.0-rc1")],
            ),
            make_release(
                "proton-em-9.0",
                false,
                false,
                vec![tar_xz_asset("proton-em-9.0")],
            ),
        ];

        let versions = parse_releases(releases, "proton-em", MAX_RELEASES, false);
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, "proton-em-9.0");
    }

    #[test]
    fn parse_releases_proton_em_picks_tarball() {
        let releases = vec![make_release(
            "EM-10.0-37-HDR",
            false,
            false,
            vec![tar_xz_asset("EM-10.0-37-HDR")],
        )];

        let versions = parse_releases(releases, "proton-em", MAX_RELEASES, false);
        assert_eq!(versions.len(), 1);
        let v = &versions[0];
        assert_eq!(v.provider, "proton-em");
        assert_eq!(v.version, "EM-10.0-37-HDR");
        // Upstream ships `.tar.xz`; picker must not limit itself to `.tar.gz`.
        assert!(v.download_url.as_deref().unwrap().ends_with(".tar.xz"));
        // `.sha512sum` sidecars don't exist for EM (it ships `.sha256sum`),
        // and the provider declares `ChecksumKind::None` so no checksum URL.
        assert!(v.checksum_url.is_none());
    }

    #[test]
    fn proton_em_reports_supports_install_true() {
        let provider = ProtonEmProvider::new();
        assert!(provider.supports_install());
    }

    #[test]
    fn proton_em_checksum_kind_is_none() {
        let provider = ProtonEmProvider::new();
        assert_eq!(provider.checksum_kind(), ChecksumKind::None);
    }
}
