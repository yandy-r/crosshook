//! Core capability derivation logic.

use super::formatting::format_tool_list;
use super::tool_check::{collect_install_hints, resolve_tool_check};
use super::types::{Capability, CapabilityMap, CapabilityState};
use crate::onboarding::{HostToolCheckResult, ReadinessCatalog, ReadinessCheckResult};

pub(super) fn derive_capabilities_with_map(
    result: &ReadinessCheckResult,
    capability_map: &CapabilityMap,
    readiness_catalog: &ReadinessCatalog,
) -> Vec<Capability> {
    capability_map
        .entries
        .iter()
        .map(|definition| {
            let missing_required = definition
                .required_tools
                .iter()
                .filter_map(|tool_id| {
                    let check = resolve_tool_check(result, readiness_catalog, tool_id, true);
                    (!check.is_available).then_some(check)
                })
                .collect::<Vec<_>>();

            let missing_optional = definition
                .optional_tools
                .iter()
                .filter_map(|tool_id| {
                    let check = resolve_tool_check(result, readiness_catalog, tool_id, false);
                    (!check.is_available).then_some(check)
                })
                .collect::<Vec<_>>();

            let state = if !missing_required.is_empty() {
                CapabilityState::Unavailable
            } else if !missing_optional.is_empty() {
                CapabilityState::Degraded
            } else {
                CapabilityState::Available
            };

            Capability {
                id: definition.id.clone(),
                label: definition.label.clone(),
                category: definition.category.clone(),
                state,
                rationale: capability_rationale(
                    &definition.label,
                    state,
                    &missing_required,
                    &missing_optional,
                ),
                required_tool_ids: definition.required_tools.clone(),
                optional_tool_ids: definition.optional_tools.clone(),
                install_hints: collect_install_hints(
                    missing_required.iter().chain(missing_optional.iter()),
                ),
                missing_required,
                missing_optional,
            }
        })
        .collect()
}

fn capability_rationale(
    label: &str,
    state: CapabilityState,
    missing_required: &[HostToolCheckResult],
    missing_optional: &[HostToolCheckResult],
) -> Option<String> {
    match state {
        CapabilityState::Available => None,
        CapabilityState::Unavailable => {
            let tools = format_tool_list(missing_required);
            let noun = if missing_required.len() == 1 {
                "is"
            } else {
                "are"
            };
            Some(format!(
                "{label} is unavailable because {tools} {noun} not available on the host."
            ))
        }
        CapabilityState::Degraded => {
            let tools = format_tool_list(missing_optional);
            let (tool_word, verb) = if missing_optional.len() == 1 {
                ("optional tool", "is")
            } else {
                ("optional tools", "are")
            };
            Some(format!(
                "{label} is degraded because {tool_word} {tools} {verb} not available on the host."
            ))
        }
    }
}
