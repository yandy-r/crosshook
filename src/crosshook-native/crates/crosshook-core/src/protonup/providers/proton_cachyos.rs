//! Proton-CachyOS provider — fetches releases from CachyOS/proton-cachyos.

use async_trait::async_trait;

use crate::protonup::ProtonUpAvailableVersion;

use super::{
    fetch_github_releases, parse_releases, ChecksumKind, ProtonReleaseProvider, ProviderError,
};

const GH_RELEASES_URL: &str = "https://api.github.com/repos/CachyOS/proton-cachyos/releases";
const MAX_RELEASES: usize = 30;

/// Provider for Proton-CachyOS (CachyOS-optimised Proton fork).
pub struct ProtonCachyOsProvider;

impl ProtonCachyOsProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProtonCachyOsProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtonReleaseProvider for ProtonCachyOsProvider {
    fn id(&self) -> &'static str {
        "proton-cachyos"
    }

    fn display_name(&self) -> &'static str {
        "Proton-CachyOS"
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
        include_prereleases: bool,
    ) -> Result<Vec<ProtonUpAvailableVersion>, ProviderError> {
        let releases = fetch_github_releases(client, GH_RELEASES_URL).await?;
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
    "protonup:catalog:proton-cachyos"
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
            published_at: Some("2026-03-30T12:00:00Z".to_string()),
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

        let versions = parse_releases(releases, "proton-cachyos", MAX_RELEASES, false);
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].provider, "proton-cachyos");
        assert_eq!(versions[0].version, "cachyos-10.0-20260330-slr");
        assert!(versions[0]
            .download_url
            .as_deref()
            .unwrap()
            .ends_with(".tar.xz"));
        assert!(versions[0]
            .checksum_url
            .as_deref()
            .unwrap()
            .contains("x86_64.sha512sum"));
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

        let versions = parse_releases(releases, "proton-cachyos", MAX_RELEASES, false);
        assert_eq!(versions.len(), 1);
        let url = versions[0].download_url.as_deref().unwrap();
        assert!(
            url.contains("x86_64.tar.xz") && !url.contains("x86_64_v3"),
            "expected baseline x86_64, got {url}"
        );
    }

    #[test]
    fn proton_cachyos_skip_draft() {
        let releases = vec![
            make_release(
                "cachyos-draft",
                true,
                false,
                vec![tar_xz_named("proton-cachyos-draft-x86_64.tar.xz")],
            ),
            make_release(
                "cachyos-stable",
                false,
                false,
                vec![tar_xz_named("proton-cachyos-stable-x86_64.tar.xz")],
            ),
        ];

        let versions = parse_releases(releases, "proton-cachyos", MAX_RELEASES, false);
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, "cachyos-stable");
    }

    #[test]
    fn proton_cachyos_caps_at_max() {
        let releases: Vec<GhRelease> = (0..MAX_RELEASES + 5)
            .map(|i| {
                make_release(
                    &format!("cachyos-{i}"),
                    false,
                    false,
                    vec![tar_xz_named(&format!("proton-cachyos-{i}-x86_64.tar.xz"))],
                )
            })
            .collect();

        let versions = parse_releases(releases, "proton-cachyos", MAX_RELEASES, false);
        assert_eq!(versions.len(), MAX_RELEASES);
    }
}
