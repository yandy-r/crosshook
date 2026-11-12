//! Provider trait and shared GitHub Releases parsing infrastructure.
//!
//! Each provider (GE-Proton, Proton-CachyOS, …) implements [`ProtonReleaseProvider`]
//! and is registered in [`registry`]. `catalog.rs` iterates the registry rather
//! than hard-coding per-provider branches.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use super::ProtonUpAvailableVersion;

pub mod ge_proton;
pub mod proton_cachyos;
pub mod proton_em;

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors that a provider's `fetch` implementation may surface.
#[derive(Debug)]
pub enum ProviderError {
    Http(reqwest::Error),
    Parse(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "http error: {e}"),
            Self::Parse(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for ProviderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Http(e) => Some(e),
            Self::Parse(_) => None,
        }
    }
}

impl From<reqwest::Error> for ProviderError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e)
    }
}

// ── Checksum kind ─────────────────────────────────────────────────────────────

/// How a provider publishes checksums alongside its release archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChecksumKind {
    Sha512Sidecar,
    Sha256Manifest,
    None,
}

// ── Trait ─────────────────────────────────────────────────────────────────────

/// A Proton compatibility-tool release provider.
///
/// Implementations are stateless; they carry only static configuration.
#[async_trait]
pub trait ProtonReleaseProvider: Send + Sync {
    /// Stable machine-readable identifier (e.g. `"ge-proton"`).
    fn id(&self) -> &'static str;
    /// Human-readable display name.
    fn display_name(&self) -> &'static str;
    /// Whether CrossHook's native install path supports this provider.
    fn supports_install(&self) -> bool;
    /// Checksum strategy used by this provider's releases.
    fn checksum_kind(&self) -> ChecksumKind;
    /// Fetch available releases from upstream.
    async fn fetch(
        &self,
        client: &reqwest::Client,
        include_prereleases: bool,
    ) -> Result<Vec<ProtonUpAvailableVersion>, ProviderError>;
}

// ── Registry ──────────────────────────────────────────────────────────────────

/// All active providers in priority order.
///
pub fn registry() -> Vec<Arc<dyn ProtonReleaseProvider>> {
    vec![
        Arc::new(ge_proton::GeProtonProvider::new()),
        Arc::new(proton_cachyos::ProtonCachyOsProvider::new()),
        Arc::new(proton_em::ProtonEmProvider::new()),
    ]
}

/// Look up a provider by its stable machine-readable id.
pub fn find_provider_by_id(id: &str) -> Option<Arc<dyn ProtonReleaseProvider>> {
    registry().into_iter().find(|p| p.id() == id)
}

// ── Provider descriptor ───────────────────────────────────────────────────────

/// Serialisable snapshot of a provider's static metadata, for IPC/UI use.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtonUpProviderDescriptor {
    pub id: String,
    pub display_name: String,
    pub supports_install: bool,
    pub checksum_kind: ChecksumKind,
}

impl ProtonUpProviderDescriptor {
    pub fn from_provider(provider: &dyn ProtonReleaseProvider) -> Self {
        Self {
            id: provider.id().to_string(),
            display_name: provider.display_name().to_string(),
            supports_install: provider.supports_install(),
            checksum_kind: provider.checksum_kind(),
        }
    }
}

/// Describe all registered providers as serialisable DTOs.
///
/// Batch 4 will consume this from a Tauri handler.
pub fn describe_providers() -> Vec<ProtonUpProviderDescriptor> {
    registry()
        .iter()
        .map(|p| ProtonUpProviderDescriptor::from_provider(p.as_ref()))
        .collect()
}

// ── Shared GitHub fetch helper ────────────────────────────────────────────────

/// Maximum response body size accepted from the GitHub Releases API.
///
/// A trickle attack can still buffer a large payload even with a 10 s timeout;
/// this ceiling caps the byte volume we are willing to deserialize.
const MAX_CATALOG_BYTES: u64 = 10 * 1024 * 1024; // 10 MiB

/// Fetch and deserialize a GitHub Releases API response.
///
/// Enforces a [`MAX_CATALOG_BYTES`] ceiling on the `Content-Length` header before
/// deserializing. Every provider shares this helper so the guard is applied
/// consistently.
pub(super) async fn fetch_github_releases(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<GhRelease>, ProviderError> {
    use reqwest::StatusCode;

    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?;

    if response.status() == StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }

    let response = response.error_for_status()?;

    if let Some(len) = response.content_length() {
        if len > MAX_CATALOG_BYTES {
            return Err(ProviderError::Parse(format!(
                "GitHub Releases response too large: {len} bytes (limit {MAX_CATALOG_BYTES})"
            )));
        }
    }

    let releases = response.json::<Vec<GhRelease>>().await?;
    Ok(releases)
}

/// Fetch GitHub releases and convert them into provider versions using the
/// shared archive-selection rules.
pub(super) async fn fetch_github_versions(
    client: &reqwest::Client,
    url: &str,
    provider_id: &str,
    max: usize,
    include_prereleases: bool,
) -> Result<Vec<ProtonUpAvailableVersion>, ProviderError> {
    let releases = fetch_github_releases(client, url).await?;
    Ok(parse_releases(
        releases,
        provider_id,
        max,
        include_prereleases,
    ))
}

// ── Shared GitHub Releases API types (used by both providers) ─────────────────

/// A single release from the GitHub Releases API.
#[derive(Debug, Deserialize)]
pub(super) struct GhRelease {
    pub tag_name: String,
    pub html_url: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<GhAsset>,
    /// ISO-8601 UTC publication timestamp from GitHub. Absent on some drafts.
    #[serde(default)]
    pub published_at: Option<String>,
}

/// A single asset attached to a GitHub release.
#[derive(Debug, Deserialize)]
pub(super) struct GhAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

// ── Shared parsing helpers ────────────────────────────────────────────────────

/// Strip `.tar.gz` / `.tar.xz` from an asset filename so we can match
/// sidecar `.sha512sum` files.
pub(super) fn tarball_stem(name: &str) -> Option<&str> {
    name.strip_suffix(".tar.gz")
        .or_else(|| name.strip_suffix(".tar.xz"))
}

/// Find the `.sha512sum` sidecar asset for `tarball_name`.
pub(super) fn find_matching_sha512sum<'a>(
    assets: &'a [GhAsset],
    tarball_name: &str,
) -> Option<&'a GhAsset> {
    let stem = tarball_stem(tarball_name)?;
    let expected = format!("{stem}.sha512sum");
    assets.iter().find(|a| a.name == expected)
}

/// True when an archive name targets an ARM host: one of its `-`/`_`/`.`
/// separated tokens is `aarch64`, `arm64`, `armhf`, `armv7` or `arm`.
pub(crate) fn is_arm_archive(name: &str) -> bool {
    name.to_ascii_lowercase()
        .split(['-', '_', '.'])
        .any(|token| matches!(token, "aarch64" | "arm64" | "armhf" | "armv7" | "arm"))
}

/// True for microarchitecture-optimized builds (`x86_64_v2`, `_v3`, `_v4`, …),
/// which need CPU features the baseline `x86_64` build does not.
fn is_optimized_x86_64(name: &str) -> bool {
    name.match_indices("x86_64_v")
        .any(|(i, m)| name[i + m.len()..].starts_with(|c: char| c.is_ascii_digit()))
}

/// Pick the primary downloadable archive for a release.
///
/// Proton-CachyOS publishes `.tar.xz` only; every other provider publishes
/// `.tar.gz` or `.tar.xz`. Providers that publish one archive per architecture
/// (GE-Proton from 11-5, Proton-CachyOS) list them in no useful order, so the
/// pick never trusts upload order: it prefers the Linux x86_64 baseline, then
/// x86_64_v3, then any non-ARM archive (never another optimized variant first). Proton itself is x86_64-only, so an ARM
/// archive is only chosen when it is the sole candidate.
pub(super) fn pick_tarball_asset<'a>(
    assets: &'a [GhAsset],
    provider_id: &str,
) -> Option<&'a GhAsset> {
    let candidates: Vec<&GhAsset> = assets
        .iter()
        .filter(|a| {
            a.name.ends_with(".tar.xz")
                || (provider_id != "proton-cachyos" && a.name.ends_with(".tar.gz"))
        })
        .collect();

    let baseline_x86 = candidates
        .iter()
        .find(|a| a.name.contains("x86_64") && !is_optimized_x86_64(&a.name));
    let v3 = candidates.iter().find(|a| a.name.contains("x86_64_v3"));
    let non_arm = candidates.iter().find(|a| !is_arm_archive(&a.name));

    baseline_x86
        .or(v3)
        .or(non_arm)
        .copied()
        .or_else(|| candidates.first().copied())
}

/// Convert a single GitHub release into a `ProtonUpAvailableVersion`.
///
/// Returns `None` when no supported archive asset is found.
pub(super) fn gh_release_to_version(
    release: GhRelease,
    provider_id: &str,
) -> Option<ProtonUpAvailableVersion> {
    let tarball = pick_tarball_asset(&release.assets, provider_id)?;
    // Proton is x86_64-only: a release with nothing but an ARM archive can
    // never install, so it is not offered (and never cached).
    if is_arm_archive(&tarball.name) {
        return None;
    }
    let checksum = find_matching_sha512sum(&release.assets, &tarball.name);

    Some(ProtonUpAvailableVersion {
        provider: provider_id.to_string(),
        version: release.tag_name,
        release_url: Some(release.html_url),
        download_url: Some(tarball.browser_download_url.clone()),
        asset_size: Some(tarball.size),
        checksum_url: checksum.map(|a| a.browser_download_url.clone()),
        checksum_kind: checksum.map(|_| "sha512".to_string()),
        published_at: release.published_at,
    })
}

/// Convert a list of GitHub releases into `ProtonUpAvailableVersion` entries,
/// skipping drafts, pre-releases, and releases without a supported tarball.
/// Caps at `max` entries.
pub(super) fn parse_releases(
    releases: Vec<GhRelease>,
    provider_id: &str,
    max: usize,
    include_prereleases: bool,
) -> Vec<ProtonUpAvailableVersion> {
    releases
        .into_iter()
        .filter(|r| !r.draft && (include_prereleases || !r.prerelease))
        .filter_map(|r| gh_release_to_version(r, provider_id))
        .take(max)
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
            published_at: Some("2026-04-17T12:00:00Z".to_string()),
        }
    }

    fn tar_gz_asset(tag_name: &str) -> GhAsset {
        GhAsset {
            name: format!("{tag_name}.tar.gz"),
            browser_download_url: format!(
                "https://github.com/example/releases/download/{tag_name}/{tag_name}.tar.gz"
            ),
            size: 1234,
        }
    }

    fn checksum_only_asset(tag_name: &str) -> GhAsset {
        GhAsset {
            name: format!("{tag_name}.sha512sum"),
            browser_download_url: format!(
                "https://github.com/example/releases/download/{tag_name}/{tag_name}.sha512sum"
            ),
            size: 128,
        }
    }

    #[test]
    fn registry_contains_ge_proton_and_proton_cachyos() {
        let ids: Vec<&str> = registry().iter().map(|p| p.id()).collect();
        assert!(
            ids.contains(&"ge-proton"),
            "registry must include ge-proton"
        );
        assert!(
            ids.contains(&"proton-cachyos"),
            "registry must include proton-cachyos"
        );
        assert!(
            ids.contains(&"proton-em"),
            "registry must include proton-em"
        );
    }

    #[test]
    fn find_provider_by_id_returns_known_provider() {
        let p = find_provider_by_id("ge-proton").expect("ge-proton must be found");
        assert_eq!(p.id(), "ge-proton");
    }

    #[test]
    fn find_provider_by_id_returns_none_for_unknown() {
        assert!(
            find_provider_by_id("nonexistent-provider").is_none(),
            "unknown id must return None"
        );
    }

    #[test]
    fn describe_providers_includes_at_least_two_entries() {
        let descriptors = describe_providers();
        assert!(
            descriptors.len() >= 2,
            "expected at least two provider descriptors, got {}",
            descriptors.len()
        );
    }

    #[test]
    fn descriptor_round_trips_via_serde_json() {
        let descriptors = describe_providers();
        let first = descriptors.first().expect("at least one descriptor");
        let json = serde_json::to_string(first).expect("serialisation must succeed");
        let parsed: ProtonUpProviderDescriptor =
            serde_json::from_str(&json).expect("deserialisation must succeed");
        assert_eq!(parsed.id, first.id);
        assert_eq!(parsed.display_name, first.display_name);
        assert_eq!(parsed.supports_install, first.supports_install);
        assert_eq!(parsed.checksum_kind, first.checksum_kind);
    }

    #[test]
    fn parse_releases_caps_successful_versions_instead_of_attempted_releases() {
        let releases = vec![
            make_release(
                "invalid-1",
                false,
                false,
                vec![checksum_only_asset("invalid-1")],
            ),
            make_release(
                "invalid-2",
                false,
                false,
                vec![checksum_only_asset("invalid-2")],
            ),
            make_release("valid-1", false, false, vec![tar_gz_asset("valid-1")]),
            make_release("valid-2", false, false, vec![tar_gz_asset("valid-2")]),
        ];

        let versions = parse_releases(releases, "ge-proton", 2, false);
        let tags: Vec<&str> = versions
            .iter()
            .map(|version| version.version.as_str())
            .collect();

        assert_eq!(tags, vec!["valid-1", "valid-2"]);
    }

    fn named_asset(name: &str) -> GhAsset {
        GhAsset {
            name: name.to_string(),
            browser_download_url: format!("https://github.com/example/releases/download/x/{name}"),
            size: 1,
        }
    }

    #[test]
    fn ge_proton_prefers_x86_64_when_aarch64_is_listed_first() {
        let assets = vec![
            named_asset("GE-Proton11-5-aarch64.sha512sum"),
            named_asset("GE-Proton11-5-aarch64.tar.gz"),
            named_asset("GE-Proton11-5-x86_64.sha512sum"),
            named_asset("GE-Proton11-5-x86_64.tar.gz"),
        ];
        let picked = pick_tarball_asset(&assets, "ge-proton").expect("an archive");
        assert_eq!(picked.name, "GE-Proton11-5-x86_64.tar.gz");
        let checksum = find_matching_sha512sum(&assets, &picked.name).expect("paired checksum");
        assert_eq!(checksum.name, "GE-Proton11-5-x86_64.sha512sum");
    }

    #[test]
    fn single_arch_archive_is_still_picked() {
        let assets = vec![
            named_asset("GE-Proton9-21.sha512sum"),
            named_asset("GE-Proton9-21.tar.gz"),
        ];
        let picked = pick_tarball_asset(&assets, "ge-proton").expect("an archive");
        assert_eq!(picked.name, "GE-Proton9-21.tar.gz");
    }

    #[test]
    fn arm_only_release_is_not_offered() {
        let release = make_release(
            "GE-Proton11-5",
            false,
            false,
            vec![named_asset("GE-Proton11-5-aarch64.tar.gz")],
        );
        assert!(gh_release_to_version(release, "ge-proton").is_none());
    }

    #[test]
    fn arm_detection_matches_tokens_not_substrings() {
        assert!(is_arm_archive("GE-Proton11-5-aarch64.tar.gz"));
        assert!(is_arm_archive("proton-cachyos-11.0-arm64.tar.xz"));
        assert!(is_arm_archive("tool_armhf.tar.gz"));
        assert!(!is_arm_archive("GE-Proton11-5-x86_64.tar.gz"));
        assert!(!is_arm_archive("tool-warm-start.tar.gz"));
    }

    #[test]
    fn optimized_variants_are_not_taken_as_the_baseline() {
        let assets = vec![
            named_asset("proton-cachyos-11.0-x86_64_v4.tar.xz"),
            named_asset("proton-cachyos-11.0-x86_64_v3.tar.xz"),
            named_asset("proton-cachyos-11.0-x86_64.tar.xz"),
        ];
        let picked = pick_tarball_asset(&assets, "proton-cachyos").expect("an archive");
        assert_eq!(picked.name, "proton-cachyos-11.0-x86_64.tar.xz");
        assert!(is_optimized_x86_64("tool-x86_64_v2.tar.gz"));
        assert!(!is_optimized_x86_64("tool-x86_64.tar.gz"));
    }
}
