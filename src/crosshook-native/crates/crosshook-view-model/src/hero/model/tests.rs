use serde::Deserialize;

use super::*;

const FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/ts_parity/hero.json"
));

#[derive(Deserialize)]
struct Tab {
    id: String,
    label: String,
}

#[derive(Deserialize)]
struct TabId {
    id: String,
    valid: bool,
}

#[derive(Deserialize)]
struct DisplayPath {
    value: Option<String>,
    expected: String,
}

#[derive(Deserialize)]
struct ImageCase<'a> {
    #[serde(borrow)]
    input: HeroImageInputs<'a>,
    #[serde(borrow)]
    expected: HeroImage<'a>,
}

#[derive(Deserialize)]
struct Fixture<'a> {
    tabs: Vec<Tab>,
    default_tab: HeroTab,
    tab_ids: Vec<TabId>,
    display_path: Vec<DisplayPath>,
    #[serde(borrow)]
    hero_image: Vec<ImageCase<'a>>,
}

fn fixture() -> Fixture<'static> {
    serde_json::from_str(FIXTURE).expect("hero fixture parses")
}

#[test]
fn tabs_match_typescript_order_ids_and_labels() {
    let got: Vec<_> = HeroTab::ALL.iter().map(|t| (t.id(), t.label())).collect();
    let fixture = fixture();
    let want: Vec<_> = fixture
        .tabs
        .iter()
        .map(|t| (t.id.as_str(), t.label.as_str()))
        .collect();
    assert_eq!(got, want);
    assert_eq!(HeroTab::default(), fixture.default_tab);
}

#[test]
fn tab_id_parsing_matches_typescript() {
    for case in fixture().tab_ids {
        assert_eq!(
            HeroTab::from_id(&case.id).is_some(),
            case.valid,
            "id {:?}",
            case.id
        );
    }
    for tab in HeroTab::ALL {
        assert_eq!(HeroTab::from_id(tab.id()), Some(tab));
    }
}

#[test]
fn display_path_matches_typescript() {
    for case in fixture().display_path {
        assert_eq!(
            display_path(case.value.as_deref()),
            case.expected,
            "value {:?}",
            case.value
        );
    }
}

#[test]
fn hero_image_precedence_matches_typescript() {
    for (i, case) in fixture().hero_image.iter().enumerate() {
        assert_eq!(
            resolve_hero_image(&case.input),
            case.expected,
            "hero image case {i}"
        );
    }
}

const METHODS: [LaunchMethod; 3] = [
    LaunchMethod::Native,
    LaunchMethod::ProtonRun,
    LaunchMethod::SteamApplaunch,
];

#[test]
fn every_method_shows_all_tabs_with_or_without_trainer() {
    for method in METHODS {
        for has_trainer in [false, true] {
            assert_eq!(
                visible_tabs(method, has_trainer),
                HeroTab::ALL,
                "{method:?} trainer={has_trainer}"
            );
        }
    }
}

#[test]
fn resolve_falls_back_to_overview() {
    for method in METHODS {
        for has_trainer in [false, true] {
            assert_eq!(
                HeroTab::resolve(None, method, has_trainer),
                HeroTab::Overview
            );
            assert_eq!(
                HeroTab::resolve(Some("nope"), method, has_trainer),
                HeroTab::Overview
            );
            assert_eq!(
                HeroTab::resolve(Some("Trainer"), method, has_trainer),
                HeroTab::Overview
            );
            assert_eq!(
                HeroTab::resolve(Some("trainer"), method, has_trainer),
                HeroTab::Trainer
            );
            assert_eq!(
                HeroTab::resolve(Some("launch-options"), method, has_trainer),
                HeroTab::LaunchOptions
            );
        }
    }
}
