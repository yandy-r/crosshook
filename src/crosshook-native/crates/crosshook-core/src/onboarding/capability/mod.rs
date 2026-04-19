//! Host capability map and derived capability state.
//!
//! Struct definitions, [`CapabilityState`], and [`derive_capabilities`] live here.
//! TOML parsing, map loading, and the process-global singleton are in
//! [`super::capability_loader`].

mod derive;
mod formatting;
#[cfg(test)]
mod tests;
mod tool_check;
mod types;

pub use types::{Capability, CapabilityDefinition, CapabilityMap, CapabilityState};

use super::capability_loader::global_capability_map;
use super::{global_readiness_catalog, ReadinessCheckResult};

/// Derives capabilities from a readiness check result using the global capability map.
pub fn derive_capabilities(result: &ReadinessCheckResult) -> Vec<Capability> {
    derive::derive_capabilities_with_map(
        result,
        global_capability_map(),
        global_readiness_catalog(),
    )
}
