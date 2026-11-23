//! Replays the TypeScript parity fixtures (`tests/fixtures/ts_parity/`,
//! written by `scripts/export-ts-parity-fixtures.sh`) against the Rust port.

use serde::Deserialize;

use super::pipeline::{derive_pipeline, PipelineNodeId, PipelineNodeStatus, PipelineTone};
use super::pipeline_detail::{
    env_groups, summary_meta, wrapper_chain, EnvGroup, EnvGroupId, WrapperKind,
};
use super::preview::{EnvVarSource, GameIdSource, LaunchPreview, PipelineProfile};
use super::validation_map::node_for_code;
use super::{LaunchMethod, LaunchPhase};

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ts_parity/",
            $name,
            ".json"
        ))
    };
}

fn parse<'a, T: Deserialize<'a>>(text: &'a str) -> T {
    serde_json::from_str(text).expect("parity fixture parses")
}

#[derive(Deserialize)]
struct ExpectedNode {
    id: PipelineNodeId,
    label: String,
    status: PipelineNodeStatus,
    detail: Option<String>,
    tone: Option<PipelineTone>,
}

#[derive(Deserialize)]
struct PipelineCase {
    name: String,
    method: LaunchMethod,
    profile: Option<PipelineProfile>,
    preview: Option<LaunchPreview>,
    phase: LaunchPhase,
    omit: Vec<PipelineNodeId>,
    expected: Vec<ExpectedNode>,
}

#[test]
fn pipeline_matches_typescript() {
    let cases: Vec<PipelineCase> = parse(fixture!("pipeline"));
    assert!(cases.len() >= 20, "fixture covers the state matrix");
    for case in cases {
        let nodes = derive_pipeline(
            case.method,
            case.profile.as_ref(),
            case.preview.as_ref(),
            case.phase,
            &case.omit,
        );
        let got: Vec<_> = nodes
            .iter()
            .map(|n| (n.id, n.id.label(), n.status, n.detail.as_deref(), n.tone))
            .collect();
        let want: Vec<_> = case
            .expected
            .iter()
            .map(|n| {
                (
                    n.id,
                    n.label.as_str(),
                    n.status,
                    n.detail.as_deref(),
                    n.tone,
                )
            })
            .collect();
        assert_eq!(got, want, "pipeline case `{}`", case.name);
    }
}

#[derive(Deserialize)]
struct ExpectedVar {
    key: String,
    value: Option<String>,
}

#[derive(Deserialize)]
struct ExpectedGroup {
    id: String,
    label: String,
    vars: Vec<ExpectedVar>,
    cleared: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedChainItem {
    id: String,
    kind: String,
    token: String,
    active: bool,
    reason: String,
    detail: Option<String>,
    folded_into: Option<String>,
}

#[derive(Deserialize)]
struct ExpectedDetail {
    env_groups: Vec<ExpectedGroup>,
    clipboard: Vec<String>,
    wrapper_chain: Option<Vec<ExpectedChainItem>>,
    summary_meta: String,
}

#[derive(Deserialize)]
struct DetailCase {
    name: String,
    preview: LaunchPreview,
    expected: ExpectedDetail,
}

#[derive(Deserialize)]
struct SourceLabel<S> {
    source: S,
    label: String,
}

#[derive(Deserialize)]
struct DetailFixture {
    cases: Vec<DetailCase>,
    empty_summary_meta: String,
    env_source_labels: Vec<SourceLabel<EnvVarSource>>,
    gameid_source_labels: Vec<SourceLabel<GameIdSource>>,
}

fn group_id(id: EnvGroupId) -> &'static str {
    match id {
        EnvGroupId::Cleared => "cleared",
        EnvGroupId::Source(source) => match source {
            EnvVarSource::ProtonRuntime => "proton_runtime",
            EnvVarSource::LaunchOptimization => "launch_optimization",
            EnvVarSource::Host => "host",
            EnvVarSource::SteamProton => "steam_proton",
            EnvVarSource::ProfileCustom => "profile_custom",
        },
    }
}

fn kind_name(kind: WrapperKind) -> &'static str {
    match kind {
        WrapperKind::Isolation => "isolation",
        WrapperKind::Optimization => "optimization",
        WrapperKind::Gamescope => "gamescope",
        WrapperKind::Runtime => "runtime",
    }
}

#[test]
fn pipeline_detail_matches_typescript() {
    let fixture: DetailFixture = parse(fixture!("pipeline_detail"));
    for case in &fixture.cases {
        let name = &case.name;
        let groups = env_groups(&case.preview);
        let got: Vec<_> = groups
            .iter()
            .map(|g| {
                let vars: Vec<_> = g
                    .vars
                    .iter()
                    .map(|v| (v.key.as_str(), v.value.as_deref()))
                    .collect();
                (group_id(g.id), g.label, vars, g.is_cleared())
            })
            .collect();
        let want: Vec<_> = case
            .expected
            .env_groups
            .iter()
            .map(|g| {
                let vars: Vec<_> = g
                    .vars
                    .iter()
                    .map(|v| (v.key.as_str(), v.value.as_deref()))
                    .collect();
                (g.id.as_str(), g.label.as_str(), vars, g.cleared)
            })
            .collect();
        assert_eq!(got, want, "env groups of `{name}`");

        let clipboard: Vec<String> = groups.iter().map(EnvGroup::clipboard_text).collect();
        assert_eq!(clipboard, case.expected.clipboard, "clipboard of `{name}`");

        let chain = wrapper_chain(&case.preview);
        let got = chain.as_ref().map(|items| {
            items
                .iter()
                .map(|i| {
                    let item = (i.id.as_str(), kind_name(i.kind), i.token.as_str(), i.active);
                    (
                        item,
                        i.reason.as_str(),
                        i.detail.as_deref(),
                        i.folded_into.as_deref(),
                    )
                })
                .collect::<Vec<_>>()
        });
        let want = case.expected.wrapper_chain.as_ref().map(|items| {
            items
                .iter()
                .map(|i| {
                    let item = (i.id.as_str(), i.kind.as_str(), i.token.as_str(), i.active);
                    (
                        item,
                        i.reason.as_str(),
                        i.detail.as_deref(),
                        i.folded_into.as_deref(),
                    )
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(got, want, "wrapper chain of `{name}`");

        let meta = summary_meta(
            &groups,
            chain.as_deref(),
            case.preview.umu_decision.as_ref(),
        );
        assert_eq!(meta, case.expected.summary_meta, "summary meta of `{name}`");
    }
    assert_eq!(summary_meta(&[], None, None), fixture.empty_summary_meta);
    for entry in &fixture.env_source_labels {
        assert_eq!(entry.source.label(), entry.label);
    }
    for entry in &fixture.gameid_source_labels {
        assert_eq!(entry.source.label(), entry.label);
    }
}

#[derive(Deserialize)]
struct ValidationCase {
    code: Option<String>,
    expected: PipelineNodeId,
}

#[test]
fn validation_map_matches_typescript() {
    let cases: Vec<ValidationCase> = parse(fixture!("validation_map"));
    for case in cases {
        assert_eq!(
            node_for_code(case.code.as_deref()),
            case.expected,
            "code {:?}",
            case.code
        );
    }
}
