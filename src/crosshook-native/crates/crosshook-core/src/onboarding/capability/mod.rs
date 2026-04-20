//! Host capability map and derived capability state.
//!
//! This module provides the capability derivation system that evaluates which
//! gaming features (Gamescope, MangoHud, etc.) are available, degraded, or
//! unavailable based on host tool readiness.
//!
//! Public types ([`Capability`], [`CapabilityState`], [`CapabilityMap`]) and
//! the main entry point ([`derive_capabilities`]) are re-exported from submodules.
//! TOML parsing, map loading, and the process-global singleton are in
//! [`super::capability_loader`].

mod derive;
mod formatting;
mod tool_check;
mod types;

#[cfg(test)]
mod tests;

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
