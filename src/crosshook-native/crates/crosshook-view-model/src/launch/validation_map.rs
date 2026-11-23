//! Maps launch validation issues onto pipeline nodes (`mapValidationToNode.ts`).

use super::pipeline::PipelineNodeId;
use super::preview::ValidationIssue;

/// Code prefixes per node, checked in this order; the first match wins.
///
/// Codes come from core `ValidationError::code()`; a new variant there needs
/// a prefix here (otherwise it lands on [`PipelineNodeId::Launch`]).
const NODE_PREFIXES: &[(PipelineNodeId, &[&str])] = &[
    (
        PipelineNodeId::Game,
        &["game_path", "native_windows_executable"],
    ),
    (PipelineNodeId::WinePrefix, &["runtime_prefix"]),
    (PipelineNodeId::Proton, &["runtime_proton"]),
    (PipelineNodeId::Steam, &["steam_"]),
    (
        PipelineNodeId::Trainer,
        &[
            "trainer_",
            "native_trainer",
            "unshare_net",
            "mod_coexistence_",
        ],
    ),
    (
        PipelineNodeId::Optimizations,
        &[
            "unknown_launch_optimization",
            "duplicate_launch_optimization",
            "incompatible_launch_optimization",
            "launch_optimization",
            "gamescope_",
            "custom_env_var",
        ],
    ),
];

/// Node an issue belongs to: by its trimmed code's prefix, or
/// [`PipelineNodeId::Launch`] (the summary node) for unknown or missing codes.
#[must_use]
pub fn node_for_issue(issue: &ValidationIssue) -> PipelineNodeId {
    node_for_code(issue.code.as_deref())
}

/// [`node_for_issue`] on a bare code.
#[must_use]
pub fn node_for_code(code: Option<&str>) -> PipelineNodeId {
    let Some(code) = code.map(str::trim).filter(|c| !c.is_empty()) else {
        return PipelineNodeId::Launch;
    };
    NODE_PREFIXES
        .iter()
        .find(|(_, prefixes)| prefixes.iter().any(|p| code.starts_with(p)))
        .map_or(PipelineNodeId::Launch, |(node, _)| *node)
}

/// First fatal issue of `node`, in backend order (Classic sorts each node's
/// issues fatal-first, stably, and takes the first fatal one).
pub(crate) fn first_fatal(
    issues: &[ValidationIssue],
    node: PipelineNodeId,
) -> Option<&ValidationIssue> {
    issues.iter().find(|issue| {
        issue.severity == super::preview::Severity::Fatal && node_for_issue(issue) == node
    })
}
