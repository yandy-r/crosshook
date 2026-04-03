//! Steam Store metadata lookup with cache-first pattern.

mod client;
pub mod models;

pub use client::lookup_steam_metadata;
pub use models::{
    cache_key_for_app_id, normalize_app_id, SteamAppDetails, SteamGenre, SteamMetadataLookupResult,
    SteamMetadataLookupState, STEAM_METADATA_CACHE_NAMESPACE,
};

#[cfg(test)]
mod tests;
