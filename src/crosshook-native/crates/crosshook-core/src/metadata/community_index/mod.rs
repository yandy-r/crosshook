//! Community profile index operations.
//!
//! This module handles indexing of community tap profiles and trainer sources
//! into the metadata database. It enforces A6 string length bounds and validates
//! trainer source URLs (HTTPS-only).

mod constants;
mod helpers;
mod indexing;
mod queries;
mod trainer_sources;

#[cfg(test)]
mod tests;

// Re-export public API (preserving existing import paths)
pub use indexing::index_community_tap_result_with_trainers;
pub use queries::list_community_tap_profiles;

// Re-export parent types for internal use
use super::{db, MetadataStoreError};
