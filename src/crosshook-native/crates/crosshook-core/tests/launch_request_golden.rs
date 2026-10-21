//! Golden `GameProfile` → `LaunchRequest` cases shared with the TypeScript
//! builders (`src/utils/__tests__/launchRequestGolden.test.ts`).

use std::fs;
use std::path::{Path, PathBuf};

use crosshook_core::launch::{LaunchRequest, LaunchRequestError};
use crosshook_core::profile::{resolve_launch_method, resolve_trainer_launch_method, GameProfile};
use crosshook_core::settings::UmuPreference;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct GoldenCase {
    input: GoldenInput,
    expected: GoldenExpected,
}

#[derive(Deserialize)]
struct GoldenInput {
    profile: Value,
    steam_client_path: String,
    profile_name: String,
    umu_default: UmuPreference,
    is_flatpak: bool,
}

#[derive(Deserialize)]
struct GoldenExpected {
    method: String,
    trainer_method: String,
    #[serde(default)]
    request: Option<Value>,
    #[serde(default)]
    error: Option<String>,
}

fn fixture_paths() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/launch_request");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| entry.expect("fixture dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths
}

fn error_code(error: &LaunchRequestError) -> &'static str {
    match error {
        LaunchRequestError::MissingGamePath => "missing_game_path",
        LaunchRequestError::UnsupportedMethod(_) => "unsupported_method",
    }
}

fn check_case(path: &Path) {
    let name = path.display();
    let raw = fs::read_to_string(path).unwrap_or_else(|e| panic!("{name}: read: {e}"));
    let case: GoldenCase =
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{name}: parse case: {e}"));
    let profile: GameProfile = serde_json::from_value(case.input.profile)
        .unwrap_or_else(|e| panic!("{name}: parse profile: {e}"));

    let method = resolve_launch_method(&profile);
    assert_eq!(method, case.expected.method, "{name}: method");
    assert_eq!(
        resolve_trainer_launch_method(method, case.input.is_flatpak),
        case.expected.trainer_method,
        "{name}: trainer_method"
    );

    let result = LaunchRequest::from_profile(
        &profile,
        method,
        &case.input.steam_client_path,
        &case.input.profile_name,
        case.input.umu_default,
    );
    match (result, case.expected.request, case.expected.error) {
        (Ok(request), Some(expected), None) => {
            let actual = serde_json::to_value(&request).expect("serialize request");
            assert_eq!(actual, expected, "{name}: request");
        }
        (Err(error), None, Some(expected)) => {
            assert_eq!(error_code(&error), expected, "{name}: error");
        }
        (result, request, error) => panic!(
            "{name}: outcome mismatch: got {result:?}, expected request={} error={error:?}",
            request.is_some()
        ),
    }
}

#[test]
fn launch_request_golden_fixtures_match_from_profile() {
    let paths = fixture_paths();
    assert!(!paths.is_empty(), "no golden launch_request fixtures found");
    for path in &paths {
        check_case(path);
    }
}
