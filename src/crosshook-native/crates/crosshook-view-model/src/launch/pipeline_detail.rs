//! Pipeline detail panel: environment groups, the wrapper chain and the
//! summary line (`derivePipelineDetail.ts`, `launchPreviewPresentation.ts`).

use super::preview::{last_path_segment, EnvVarSource, LaunchPreview, UmuDecision, WrapperSource};
use super::LaunchMethod;

/// Group id: an environment source or the cleared-variables group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvGroupId {
    /// Variables set from this source.
    Source(EnvVarSource),
    /// Variables cleared before launch.
    Cleared,
}

/// One variable of an [`EnvGroup`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvGroupVar {
    /// Name.
    pub key: String,
    /// Value; `None` in the cleared group.
    pub value: Option<String>,
}

/// Environment variables of one source, in preview order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvGroup {
    /// Group id.
    pub id: EnvGroupId,
    /// Display label.
    pub label: &'static str,
    /// Variables.
    pub vars: Vec<EnvGroupVar>,
}

impl EnvGroup {
    /// Whether this is the cleared-variables group.
    #[must_use]
    pub fn is_cleared(&self) -> bool {
        self.id == EnvGroupId::Cleared
    }

    /// Clipboard text: `KEY=value` lines, or `unset KEY` lines for the
    /// cleared group (`formatEnvGroupClipboard`).
    #[must_use]
    pub fn clipboard_text(&self) -> String {
        let lines: Vec<String> = self
            .vars
            .iter()
            .map(|var| {
                if self.is_cleared() {
                    format!("unset {}", var.key)
                } else {
                    format!("{}={}", var.key, var.value.as_deref().unwrap_or(""))
                }
            })
            .collect();
        lines.join("\n")
    }
}

/// Display order of the source groups.
const ENV_GROUP_ORDER: [EnvVarSource; 5] = [
    EnvVarSource::Host,
    EnvVarSource::SteamProton,
    EnvVarSource::ProtonRuntime,
    EnvVarSource::LaunchOptimization,
    EnvVarSource::ProfileCustom,
];

/// Environment grouped by source in display order, empty groups omitted,
/// then the cleared group when any variable is cleared. Empty when the
/// preview has no environment.
#[must_use]
pub fn env_groups(preview: &LaunchPreview) -> Vec<EnvGroup> {
    let Some(environment) = preview.environment.as_ref() else {
        return Vec::new();
    };
    let mut groups: Vec<EnvGroup> = ENV_GROUP_ORDER
        .iter()
        .filter_map(|&source| {
            let vars: Vec<EnvGroupVar> = environment
                .iter()
                .filter(|var| var.source == source)
                .map(|var| EnvGroupVar {
                    key: var.key.clone(),
                    value: Some(var.value.clone()),
                })
                .collect();
            (!vars.is_empty()).then_some(EnvGroup {
                id: EnvGroupId::Source(source),
                label: source.label(),
                vars,
            })
        })
        .collect();
    if !preview.cleared_variables.is_empty() {
        groups.push(EnvGroup {
            id: EnvGroupId::Cleared,
            label: "Cleared before launch",
            vars: preview
                .cleared_variables
                .iter()
                .map(|key| EnvGroupVar {
                    key: key.clone(),
                    value: None,
                })
                .collect(),
        });
    }
    groups
}

/// Kind of a [`WrapperChainItem`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WrapperKind {
    /// Network isolation.
    Isolation,
    /// Launch optimization.
    Optimization,
    /// Gamescope.
    Gamescope,
    /// The runtime (`umu-run` or Proton).
    Runtime,
}

impl WrapperKind {
    const fn from_source(source: WrapperSource) -> Self {
        match source {
            WrapperSource::Optimization => Self::Optimization,
            WrapperSource::NetworkIsolation => Self::Isolation,
            WrapperSource::Gamescope => Self::Gamescope,
        }
    }

    /// Key prefix of chain item ids.
    const fn key(self) -> &'static str {
        match self {
            Self::Isolation => "isolation",
            Self::Optimization => "optimization",
            Self::Gamescope => "gamescope",
            Self::Runtime => "runtime",
        }
    }
}

/// One row of the wrapper chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperChainItem {
    /// Stable key: `{kind}-{index}`, `runtime-umu` or `runtime-proton`.
    pub id: String,
    /// Kind.
    pub kind: WrapperKind,
    /// Display token (`unshare --net`, `umu-run`, `proton`, ...).
    pub token: String,
    /// Whether it runs.
    pub active: bool,
    /// Why.
    pub reason: String,
    /// Secondary line (umu coverage and GAMEID summary).
    pub detail: Option<String>,
    /// Invocation that absorbs this wrapper.
    pub folded_into: Option<String>,
}

/// The wrapper chain in invocation order, followed by the runtime rows, or
/// `None` when the preview has no wrapper attribution.
///
/// With a umu decision: an `umu-run` row, plus an active direct-Proton row
/// when umu is not used. Without one: a Proton row when a Proton setup exists
/// (none for native launches).
#[must_use]
pub fn wrapper_chain(preview: &LaunchPreview) -> Option<Vec<WrapperChainItem>> {
    let details = preview.wrapper_details.as_ref()?;
    let mut chain: Vec<WrapperChainItem> = details
        .iter()
        .enumerate()
        .map(|(index, detail)| {
            let kind = WrapperKind::from_source(detail.source);
            WrapperChainItem {
                id: format!("{}-{index}", kind.key()),
                kind,
                token: detail.command.join(" "),
                active: detail.active,
                reason: detail.reason.clone(),
                detail: None,
                folded_into: detail.folded_into.clone(),
            }
        })
        .collect();

    let proton_executable = preview
        .proton_setup
        .as_ref()
        .map_or("", |setup| setup.proton_executable.as_str());
    let proton_token = match last_path_segment(proton_executable) {
        "" => "proton",
        segment => segment,
    };
    let proton_row = |reason: &str| WrapperChainItem {
        id: "runtime-proton".to_owned(),
        kind: WrapperKind::Runtime,
        token: proton_token.to_owned(),
        active: true,
        reason: reason.to_owned(),
        detail: None,
        folded_into: None,
    };

    if let Some(umu) = preview.umu_decision.as_ref() {
        chain.push(WrapperChainItem {
            id: "runtime-umu".to_owned(),
            kind: WrapperKind::Runtime,
            token: "umu-run".to_owned(),
            active: umu.will_use_umu,
            reason: umu.reason.clone(),
            detail: Some(umu_detail(umu)),
            folded_into: None,
        });
        if !umu.will_use_umu {
            chain.push(proton_row("Direct Proton launch"));
        }
    } else if preview.proton_setup.is_some() {
        chain.push(proton_row(
            if preview.resolved_method == LaunchMethod::SteamApplaunch {
                "Launched through Steam"
            } else {
                "Direct Proton launch"
            },
        ));
    }
    Some(chain)
}

fn umu_detail(umu: &UmuDecision) -> String {
    let coverage = format!("csv coverage: {}", umu.csv_coverage.as_str());
    match umu.gameid_resolution.as_ref() {
        Some(resolution) => format!(
            "{coverage} · GAMEID {} ({})",
            resolution.game_id,
            resolution.source.label()
        ),
        None => coverage,
    }
}

/// Summary line: env var count (cleared excluded), active standalone wrapper
/// count (when a chain exists) and the umu choice (`detailSummaryMeta`).
#[must_use]
pub fn summary_meta(
    groups: &[EnvGroup],
    chain: Option<&[WrapperChainItem]>,
    umu: Option<&UmuDecision>,
) -> String {
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    let env_count: usize = groups
        .iter()
        .filter(|g| !g.is_cleared())
        .map(|g| g.vars.len())
        .sum();
    let mut parts = vec![format!("{env_count} env var{}", plural(env_count))];
    if let Some(chain) = chain {
        let wrappers = chain
            .iter()
            .filter(|item| {
                item.kind != WrapperKind::Runtime
                    && item.active
                    && item.folded_into.as_deref().is_none_or(str::is_empty)
            })
            .count();
        parts.push(format!("{wrappers} wrapper{}", plural(wrappers)));
    }
    if let Some(umu) = umu {
        parts.push(
            if umu.will_use_umu {
                "umu: umu-run"
            } else {
                "umu: direct Proton"
            }
            .to_owned(),
        );
    }
    parts.join(" · ")
}
