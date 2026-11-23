//! Launch pipeline derivation (`derivePipelineNodes.ts`).
//!
//! Tier 1 (no preview) checks only the profile config; tier 2 (with a
//! preview) uses the dry run's resolved paths and validation. A non-idle
//! [`LaunchPhase`] overlays live status on either tier without changing it.

use super::preview::{last_path_segment, non_blank, LaunchPreview, PipelineProfile};
use super::validation_map::first_fatal;
use super::{LaunchMethod, LaunchPhase};

/// Stable pipeline node id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "kebab-case"))]
pub enum PipelineNodeId {
    /// Game executable.
    Game,
    /// Wine prefix.
    WinePrefix,
    /// Proton.
    Proton,
    /// Steam.
    Steam,
    /// Trainer.
    Trainer,
    /// Launch optimizations.
    Optimizations,
    /// Summary node.
    Launch,
}

impl PipelineNodeId {
    /// Display label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Game => "Game",
            Self::WinePrefix => "Wine Prefix",
            Self::Proton => "Proton",
            Self::Steam => "Steam",
            Self::Trainer => "Trainer",
            Self::Optimizations => "Optimizations",
            Self::Launch => "Launch",
        }
    }
}

/// Status a node renders with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "kebab-case"))]
pub enum PipelineNodeStatus {
    /// Ready.
    Configured,
    /// Missing input.
    NotConfigured,
    /// Blocked by a fatal issue.
    Error,
    /// Running now (live overlay).
    Active,
    /// Done (live overlay).
    Complete,
}

/// Live presentation hint on an [`PipelineNodeStatus::Active`] node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "kebab-case"))]
pub enum PipelineTone {
    /// Waiting for the game before the trainer starts.
    Waiting,
}

/// One pipeline node. The label is [`PipelineNodeId::label`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineNode {
    /// Node id.
    pub id: PipelineNodeId,
    /// Status.
    pub status: PipelineNodeStatus,
    /// Detail line (resolved name, error message, ...).
    pub detail: Option<String>,
    /// Live tone.
    pub tone: Option<PipelineTone>,
}

impl PipelineNode {
    fn new(id: PipelineNodeId, status: PipelineNodeStatus, detail: Option<String>) -> Self {
        Self {
            id,
            status,
            detail,
            tone: None,
        }
    }
}

/// Node ids of a launch method, in display order.
#[must_use]
pub const fn method_nodes(method: LaunchMethod) -> &'static [PipelineNodeId] {
    use PipelineNodeId as N;
    match method {
        LaunchMethod::ProtonRun => &[
            N::Game,
            N::WinePrefix,
            N::Proton,
            N::Trainer,
            N::Optimizations,
            N::Launch,
        ],
        LaunchMethod::SteamApplaunch => {
            &[N::Game, N::Steam, N::Trainer, N::Optimizations, N::Launch]
        }
        LaunchMethod::Native => &[N::Game, N::Trainer, N::Launch],
    }
}

/// Derives the pipeline of `method`, dropping the `omit` nodes (which the
/// launch node also ignores).
///
/// Tier 1 when `preview` is `None` (a `None` profile is not configured), tier
/// 2 otherwise; `phase` then overlays live status.
#[must_use]
pub fn derive_pipeline(
    method: LaunchMethod,
    profile: Option<&PipelineProfile>,
    preview: Option<&LaunchPreview>,
    phase: LaunchPhase,
    omit: &[PipelineNodeId],
) -> Vec<PipelineNode> {
    let ids = method_nodes(method)
        .iter()
        .copied()
        .filter(|id| !omit.contains(id));
    let mut nodes: Vec<PipelineNode> = Vec::with_capacity(method_nodes(method).len());
    for id in ids {
        let node = match (preview, id) {
            (Some(preview), PipelineNodeId::Launch) => launch_node(&nodes, preview),
            (Some(preview), _) => tier2_node(id, preview),
            (None, PipelineNodeId::Launch) => {
                let ready = nodes
                    .iter()
                    .all(|n| n.status == PipelineNodeStatus::Configured);
                PipelineNode::new(id, configured_if(ready), None)
            }
            (None, _) => PipelineNode::new(id, configured_if(tier1_configured(id, profile)), None),
        };
        nodes.push(node);
    }
    apply_phase(phase, &mut nodes);
    nodes
}

const fn configured_if(ready: bool) -> PipelineNodeStatus {
    if ready {
        PipelineNodeStatus::Configured
    } else {
        PipelineNodeStatus::NotConfigured
    }
}

fn tier1_configured(id: PipelineNodeId, profile: Option<&PipelineProfile>) -> bool {
    let Some(p) = profile else { return false };
    let set = |value: &str| !value.trim().is_empty();
    match id {
        PipelineNodeId::Game => set(&p.game_executable_path),
        PipelineNodeId::WinePrefix => set(&p.prefix_path),
        PipelineNodeId::Proton => set(&p.proton_path),
        PipelineNodeId::Steam => set(&p.steam_app_id),
        PipelineNodeId::Trainer => set(&p.trainer_path),
        PipelineNodeId::Optimizations => p.enabled_optimizations > 0,
        PipelineNodeId::Launch => false,
    }
}

/// Tier-2 detail and whether the node is resolved. Never called for
/// [`PipelineNodeId::Launch`].
fn tier2_state(id: PipelineNodeId, preview: &LaunchPreview) -> (Option<String>, bool) {
    let proton = preview.proton_setup.as_ref();
    match id {
        PipelineNodeId::Game => {
            let name = non_blank(Some(&preview.game_executable_name));
            let exe = non_blank(Some(&preview.game_executable));
            let detail = name
                .map(str::to_owned)
                .or_else(|| exe.map(|e| last_path_segment(e).to_owned()));
            (detail, name.is_some() || exe.is_some())
        }
        PipelineNodeId::WinePrefix => {
            let path = proton.map(|s| s.wine_prefix_path.as_str());
            let detail = segment(path);
            (detail, non_blank(path).is_some())
        }
        PipelineNodeId::Proton => {
            let path = proton.map(|s| s.proton_executable.as_str());
            let detail = segment(path);
            (detail, non_blank(path).is_some())
        }
        PipelineNodeId::Steam => {
            let detail = if preview
                .steam_launch_options
                .as_deref()
                .is_some_and(|o| !o.is_empty())
            {
                "Launch options set"
            } else {
                "Ready"
            };
            (
                Some(detail.to_owned()),
                preview.resolved_method == LaunchMethod::SteamApplaunch,
            )
        }
        PipelineNodeId::Trainer => {
            let path = preview.trainer.as_ref().map(|t| t.path.as_str());
            let detail = segment(path);
            (detail, preview.trainer.is_some())
        }
        PipelineNodeId::Optimizations => {
            let resolved = preview.environment.is_some() || preview.wrappers.is_some();
            let detail = resolved.then(|| match preview.environment.as_ref().map_or(0, Vec::len) {
                0 => "No optimizations".to_owned(),
                n => format!("{n} env vars"),
            });
            (detail, resolved)
        }
        PipelineNodeId::Launch => (None, false),
    }
}

/// Last path segment of a non-empty path. A whitespace-only path gives
/// `Some("")`, as the TypeScript `p ? lastPathSegment(p) : undefined` does.
fn segment(path: Option<&str>) -> Option<String> {
    path.filter(|p| !p.is_empty())
        .map(|p| last_path_segment(p).to_owned())
}

fn tier2_node(id: PipelineNodeId, preview: &LaunchPreview) -> PipelineNode {
    // Only fatal issues mark a node as an error; warnings are not shown in the
    // pipeline yet (Classic parity).
    if let Some(issue) = first_fatal(&preview.validation.issues, id) {
        return PipelineNode::new(id, PipelineNodeStatus::Error, Some(issue.message.clone()));
    }
    if id == PipelineNodeId::Optimizations {
        if let Some(error) = preview.directives_error.as_ref().filter(|e| !e.is_empty()) {
            return PipelineNode::new(id, PipelineNodeStatus::Error, Some(error.clone()));
        }
    }
    let (detail, resolved) = tier2_state(id, preview);
    if resolved {
        PipelineNode::new(id, PipelineNodeStatus::Configured, detail)
    } else {
        let detail = detail.unwrap_or_else(|| "Not configured".to_owned());
        PipelineNode::new(id, PipelineNodeStatus::NotConfigured, Some(detail))
    }
}

fn launch_node(prior: &[PipelineNode], preview: &LaunchPreview) -> PipelineNode {
    let node = |status, detail: &str| {
        PipelineNode::new(PipelineNodeId::Launch, status, Some(detail.to_owned()))
    };
    if let Some(issue) = first_fatal(&preview.validation.issues, PipelineNodeId::Launch) {
        return node(PipelineNodeStatus::Error, &issue.message);
    }
    if prior.iter().any(|n| n.status == PipelineNodeStatus::Error) {
        return node(PipelineNodeStatus::Error, "Resolve errors above");
    }
    if prior
        .iter()
        .any(|n| n.status == PipelineNodeStatus::NotConfigured)
    {
        return node(PipelineNodeStatus::NotConfigured, "Complete steps above");
    }
    if non_blank(preview.effective_command.as_deref()).is_none() {
        return node(PipelineNodeStatus::NotConfigured, "Not ready");
    }
    node(PipelineNodeStatus::Configured, "Command ready")
}

fn apply_phase(phase: LaunchPhase, nodes: &mut [PipelineNode]) {
    let mut patch = |id: PipelineNodeId,
                     status: PipelineNodeStatus,
                     tone: Option<PipelineTone>,
                     waiting: bool| {
        if let Some(node) = nodes.iter_mut().find(|n| n.id == id) {
            node.status = status;
            node.tone = tone;
            if waiting {
                node.detail = Some("Waiting".to_owned());
            }
        }
    };
    match phase {
        LaunchPhase::Idle => {}
        LaunchPhase::GameLaunching => patch(
            PipelineNodeId::Game,
            PipelineNodeStatus::Active,
            None,
            false,
        ),
        LaunchPhase::WaitingForTrainer => {
            patch(
                PipelineNodeId::Game,
                PipelineNodeStatus::Complete,
                None,
                false,
            );
            patch(
                PipelineNodeId::Trainer,
                PipelineNodeStatus::Active,
                Some(PipelineTone::Waiting),
                true,
            );
        }
        LaunchPhase::TrainerLaunching => {
            patch(
                PipelineNodeId::Game,
                PipelineNodeStatus::Complete,
                None,
                false,
            );
            patch(
                PipelineNodeId::Trainer,
                PipelineNodeStatus::Active,
                None,
                false,
            );
        }
        LaunchPhase::SessionActive => {
            for node in nodes.iter_mut() {
                node.status = if node.id == PipelineNodeId::Launch {
                    PipelineNodeStatus::Active
                } else {
                    PipelineNodeStatus::Complete
                };
                node.tone = None;
            }
        }
    }
}
