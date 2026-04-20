use crate::protonup::{providers, ProtonUpAvailableVersion};

use super::client::protonup_http_client;

pub(crate) async fn fetch_live_catalog_by_id(
    provider_id: &str,
    include_prereleases: bool,
) -> Result<Vec<ProtonUpAvailableVersion>, providers::ProviderError> {
    let client = protonup_http_client()
        .await
        .map_err(providers::ProviderError::Http)?;

    // Resolve the matching provider implementation from the registry.
    let registry = providers::registry();
    match registry.iter().find(|p| p.id() == provider_id).cloned() {
        Some(provider_impl) => provider_impl.fetch(client, include_prereleases).await,
        None => {
            tracing::warn!(
                provider_id,
                "Unknown Proton provider id — treating as failed live fetch so stale cache is preserved"
            );
            Err(providers::ProviderError::Parse(format!(
                "unknown provider id: {provider_id}"
            )))
        }
    }
}
