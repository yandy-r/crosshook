//! Capability type definitions.

use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::onboarding::HostToolCheckResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/onboarding.ts"))]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Available,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/onboarding.ts"))]
pub struct Capability {
    pub id: String,
    pub label: String,
    pub category: String,
    pub state: CapabilityState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    #[serde(default)]
    pub required_tool_ids: Vec<String>,
    #[serde(default)]
    pub optional_tool_ids: Vec<String>,
    #[serde(default)]
    pub missing_required: Vec<HostToolCheckResult>,
    #[serde(default)]
    pub missing_optional: Vec<HostToolCheckResult>,
    #[serde(default)]
    pub install_hints: Vec<crate::onboarding::HostToolInstallCommand>,
}

/// Validated capability definition loaded from TOML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDefinition {
    pub id: String,
    pub label: String,
    pub category: String,
    pub required_tools: Vec<String>,
    pub optional_tools: Vec<String>,
}

/// Validated in-memory capability map.
#[derive(Debug, Clone)]
pub struct CapabilityMap {
    pub catalog_version: u32,
    pub entries: Vec<CapabilityDefinition>,
}

impl CapabilityMap {
    pub fn from_entries(catalog_version: u32, entries: Vec<CapabilityDefinition>) -> Self {
        Self {
            catalog_version: catalog_version.max(1),
            entries,
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&CapabilityDefinition> {
        self.entries.iter().find(|entry| entry.id == id)
    }
}
