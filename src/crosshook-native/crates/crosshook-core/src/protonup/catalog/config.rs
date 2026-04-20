use crate::protonup::{providers, ProtonUpProvider};

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
