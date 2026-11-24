//! Replays TypeScript-exported autosave parity fixtures
//! (`tests/fixtures/ts_parity/autosave.json`) against [`ProfileAutosave`].

use std::collections::VecDeque;

use serde::Deserialize;

use super::*;
use crate::launch::LaunchMethod;

const AUTOSAVE_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/ts_parity/autosave.json"
));

#[derive(Deserialize)]
struct Constants {
    section_debounce_ms: u64,
    environment_debounce_ms: u64,
    saved_status_hold_ms: u64,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixtureLaunchMethod {
    ProtonRun,
    SteamApplaunch,
    Native,
}

impl From<FixtureLaunchMethod> for LaunchMethod {
    fn from(m: FixtureLaunchMethod) -> Self {
        match m {
            FixtureLaunchMethod::ProtonRun => Self::ProtonRun,
            FixtureLaunchMethod::SteamApplaunch => Self::SteamApplaunch,
            FixtureLaunchMethod::Native => Self::Native,
        }
    }
}

#[derive(Deserialize)]
struct FixtureScope {
    selected_profile: String,
    draft_name: String,
    profile_exists: bool,
    launch_method: FixtureLaunchMethod,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum StepKind {
    Edit,
    AdvanceTo,
    Flush,
    HoldSave,
    ReleaseSave,
}

#[derive(Deserialize)]
struct FixtureStep {
    at_ms: u64,
    kind: StepKind,
    section: Option<AutosaveSection>,
    revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixtureTarget {
    FullProfile,
    Section,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RecordedSave {
    at_ms: u64,
    target: FixtureTarget,
    section_kind: Option<SectionSaveKind>,
    profile_name: String,
    revision: String,
}

#[derive(Deserialize)]
struct FixtureScenario {
    name: String,
    scope: FixtureScope,
    steps: Vec<FixtureStep>,
    expected_saves: Vec<RecordedSave>,
    final_value: String,
}

#[derive(Deserialize)]
struct ChipInput {
    tone: AutosaveTone,
    label: String,
}

#[derive(Deserialize)]
struct ChipCase {
    name: String,
    statuses: Vec<ChipInput>,
    expected_tone: AutosaveTone,
    expected_label: String,
    visible_at_2999ms: bool,
    visible_at_3000ms: bool,
}

#[derive(Deserialize)]
struct AutosaveFixture {
    constants: Constants,
    scenarios: Vec<FixtureScenario>,
    chip_cases: Vec<ChipCase>,
}

fn target_parts(target: SaveTarget) -> (&'static str, Option<SectionSaveKind>) {
    match target {
        SaveTarget::FullProfile => ("full_profile", None),
        SaveTarget::Section(kind) => ("section", Some(kind)),
    }
}

fn fixture_target_name(target: &FixtureTarget) -> &'static str {
    match target {
        FixtureTarget::FullProfile => "full_profile",
        FixtureTarget::Section => "section",
    }
}

#[test]
fn classic_timing_constants_match_rust_defaults() {
    let fixture: AutosaveFixture =
        serde_json::from_str(AUTOSAVE_FIXTURE).expect("autosave fixture parses");
    assert_eq!(
        u64::try_from(DEFAULT_SECTION_DEBOUNCE.as_millis()).unwrap(),
        fixture.constants.section_debounce_ms
    );
    assert_eq!(
        u64::try_from(DEFAULT_ENVIRONMENT_DEBOUNCE.as_millis()).unwrap(),
        fixture.constants.environment_debounce_ms
    );
    assert_eq!(
        u64::try_from(DEFAULT_SAVED_STATUS_HOLD.as_millis()).unwrap(),
        fixture.constants.saved_status_hold_ms
    );
}

#[test]
fn autosave_scenarios_match_typescript_hooks() {
    let fixture: AutosaveFixture =
        serde_json::from_str(AUTOSAVE_FIXTURE).expect("autosave fixture parses");
    assert!(
        fixture.scenarios.len() >= 13,
        "expected full scenario matrix"
    );

    for scenario in &fixture.scenarios {
        let scope = AutosaveScope {
            selected_profile: scenario.scope.selected_profile.clone(),
            draft_name: scenario.scope.draft_name.clone(),
            profile_exists: scenario.scope.profile_exists,
            launch_method: scenario.scope.launch_method.into(),
        };
        let mut vm = ProfileAutosave::with_policy(
            scope,
            Some(RevisionHash::new("seed")),
            AutosavePolicy::classic_parity(),
        );

        let mut actual_saves: Vec<(u64, &'static str, Option<SectionSaveKind>, String, String)> =
            Vec::new();
        let mut held = false;
        let mut held_req: Option<SaveRequest> = None;
        let mut last_persisted_rev = String::new();

        let record_and_maybe_settle =
            |vm: &mut ProfileAutosave,
             effects: Vec<AutosaveEffect>,
             now_ms: u64,
             held: bool,
             held_req: &mut Option<SaveRequest>,
             actual_saves: &mut Vec<(
                u64,
                &'static str,
                Option<SectionSaveKind>,
                String,
                String,
            )>,
             last_persisted_rev: &mut String| {
                let mut queue = VecDeque::from(effects);
                while let Some(eff) = queue.pop_front() {
                    if let AutosaveEffect::Save(req) = eff {
                        let (tname, skind) = target_parts(req.target);
                        actual_saves.push((
                            now_ms,
                            tname,
                            skind,
                            req.profile_name.clone(),
                            req.revision.as_str().to_owned(),
                        ));
                        *last_persisted_rev = req.revision.as_str().to_owned();
                        if held && held_req.is_none() {
                            *held_req = Some(req);
                        } else {
                            let next = vm.apply(
                                AutosaveEvent::SaveSucceeded {
                                    id: req.id,
                                    committed_rev: req.revision,
                                },
                                AutosaveInstant::from_millis(now_ms),
                            );
                            queue.extend(next);
                        }
                    }
                }
            };

        for step in &scenario.steps {
            match step.kind {
                StepKind::HoldSave => held = true,
                StepKind::ReleaseSave => {
                    held = false;
                    if let Some(req) = held_req.take() {
                        let eff = vm.apply(
                            AutosaveEvent::SaveSucceeded {
                                id: req.id,
                                committed_rev: req.revision,
                            },
                            AutosaveInstant::from_millis(step.at_ms),
                        );
                        record_and_maybe_settle(
                            &mut vm,
                            eff,
                            step.at_ms,
                            held,
                            &mut held_req,
                            &mut actual_saves,
                            &mut last_persisted_rev,
                        );
                    }
                }
                StepKind::Edit => {
                    let sec = step.section.expect("edit has section");
                    let rev = step.revision.clone().expect("edit has revision");
                    let eff = vm.apply(
                        AutosaveEvent::Edit {
                            section: sec,
                            revision: RevisionHash::new(rev),
                        },
                        AutosaveInstant::from_millis(step.at_ms),
                    );
                    record_and_maybe_settle(
                        &mut vm,
                        eff,
                        step.at_ms,
                        held,
                        &mut held_req,
                        &mut actual_saves,
                        &mut last_persisted_rev,
                    );
                }
                StepKind::AdvanceTo => {
                    while let Some(due) = vm.scheduled_tick() {
                        if due.as_millis() <= step.at_ms {
                            let eff = vm.apply(AutosaveEvent::Tick, due);
                            record_and_maybe_settle(
                                &mut vm,
                                eff,
                                due.as_millis(),
                                held,
                                &mut held_req,
                                &mut actual_saves,
                                &mut last_persisted_rev,
                            );
                        } else {
                            break;
                        }
                    }
                }
                StepKind::Flush => {
                    let eff = vm.apply(
                        AutosaveEvent::Flush,
                        AutosaveInstant::from_millis(step.at_ms),
                    );
                    record_and_maybe_settle(
                        &mut vm,
                        eff,
                        step.at_ms,
                        held,
                        &mut held_req,
                        &mut actual_saves,
                        &mut last_persisted_rev,
                    );
                }
            }
        }

        let expected: Vec<(u64, &'static str, Option<SectionSaveKind>, String, String)> = scenario
            .expected_saves
            .iter()
            .map(|s| {
                (
                    s.at_ms,
                    fixture_target_name(&s.target),
                    s.section_kind,
                    s.profile_name.clone(),
                    s.revision.clone(),
                )
            })
            .collect();
        assert_eq!(actual_saves, expected, "scenario `{}`", scenario.name);
        if !scenario.final_value.is_empty() {
            assert_eq!(
                last_persisted_rev, scenario.final_value,
                "final value of `{}`",
                scenario.name
            );
        }
    }
}

#[test]
fn chip_priority_and_fade_match_typescript() {
    let fixture: AutosaveFixture =
        serde_json::from_str(AUTOSAVE_FIXTURE).expect("autosave fixture parses");
    assert!(!fixture.chip_cases.is_empty());

    for case in &fixture.chip_cases {
        let converted: Vec<AutosaveStatus> = case
            .statuses
            .iter()
            .map(|s| AutosaveStatus {
                tone: s.tone,
                label: Box::leak(s.label.clone().into_boxed_str()),
                detail: None,
            })
            .collect();
        let merged = combine_statuses(&converted);
        assert_eq!(merged.tone, case.expected_tone, "case `{}`", case.name);
        assert_eq!(merged.label, case.expected_label, "case `{}`", case.name);

        if case.expected_tone == AutosaveTone::Success {
            // Verify 3s Saved -> Idle transition matches React chipVisible.
            let mut vm = ProfileAutosave::new(
                AutosaveScope::saved("Test Game", LaunchMethod::ProtonRun),
                Some(RevisionHash::new("r0")),
            );
            let _ = vm.apply(
                AutosaveEvent::Edit {
                    section: AutosaveSection::ProfileDraft,
                    revision: RevisionHash::new("r1"),
                },
                AutosaveInstant::from_millis(0),
            );
            let AutosaveEffect::Save(req) =
                &vm.apply(AutosaveEvent::Tick, AutosaveInstant::from_millis(350))[0]
            else {
                panic!("expected save");
            };
            let _ = vm.apply(
                AutosaveEvent::SaveSucceeded {
                    id: req.id,
                    committed_rev: RevisionHash::new("r1"),
                },
                AutosaveInstant::from_millis(350),
            );
            assert!(vm
                .apply(AutosaveEvent::Tick, AutosaveInstant::from_millis(3_349))
                .is_empty());
            assert_eq!(vm.status().is_chip_visible(), case.visible_at_2999ms);

            let _ = vm.apply(AutosaveEvent::Tick, AutosaveInstant::from_millis(3_350));
            assert_eq!(vm.status().is_chip_visible(), case.visible_at_3000ms);
        }
    }
}
