//! Profile TOML persistence — store, error type, and related utilities.

mod error;
mod store;
mod utils;

#[cfg(test)]
mod tests;

pub use error::ProfileStoreError;
pub use store::{DuplicateProfileResult, ProfileStore};
pub use utils::{bundled_optimization_preset_toml_key, profile_to_shareable_toml};
