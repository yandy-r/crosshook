//! umu database refresh, CSV coverage and GAMEID cache operations.

mod coverage;
mod refresh;
pub mod seams;

use std::sync::Arc;

use crosshook_core::metadata::MetadataStore;

use seams::{CoreUmuRefresher, UmuRefresher};

/// Toolkit-independent umu database operations over the shared metadata store.
#[derive(Clone)]
pub struct UmuService {
    metadata: MetadataStore,
    refresher: Arc<dyn UmuRefresher>,
}

impl UmuService {
    /// Builds the service with the production umu database refresher.
    pub fn new(metadata: MetadataStore) -> Self {
        Self {
            metadata,
            refresher: Arc::new(CoreUmuRefresher),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_refresher(
        metadata: MetadataStore,
        refresher: Arc<dyn UmuRefresher>,
    ) -> Self {
        Self {
            metadata,
            refresher,
        }
    }
}

static_assertions::assert_impl_all!(UmuService: Send, Sync);

#[cfg(test)]
mod tests;
