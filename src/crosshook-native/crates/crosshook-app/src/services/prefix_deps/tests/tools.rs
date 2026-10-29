//! Tool selection and Steam App ID tests (S1–S5, D13).

use std::cell::Cell;

use crosshook_core::prefix_deps::PrefixDepsTool;
use crosshook_core::profile::GameProfile;

use super::super::{profile_steam_app_id, resolve_tool, ResolvedTool, ToolSelectionError};
use super::fakes::detection;

const PROTONTRICKS: &str = "/opt/tools/protontricks";
const WINETRICKS_ON_PATH: &str = "winetricks";

fn protontricks() -> crosshook_core::prefix_deps::BinaryDetectionResult {
    detection(Some(PROTONTRICKS), Some(PrefixDepsTool::Protontricks))
}

#[test]
fn s1_app_id_parse_rejects_invalid_and_canonicalizes_valid_ids() {
    for invalid in [
        "",
        "  ",
        "0",
        "00",
        "abc",
        "-1",
        "+5",
        "--help",
        "4294967296",
    ] {
        let lookups = Cell::new(0);
        let result = resolve_tool(&protontricks(), Some(invalid), || {
            lookups.set(lookups.get() + 1);
            None
        });
        assert_eq!(
            result,
            Err(ToolSelectionError::ProtontricksNeedsAppId),
            "accepted {invalid:?}"
        );
        assert_eq!(lookups.get(), 1, "fallback not consulted for {invalid:?}");
    }
    for (raw, canonical) in [
        (" 12345 ", "12345"),
        ("007", "7"),
        ("4294967295", "4294967295"),
    ] {
        let resolved = resolve_tool(&protontricks(), Some(raw), || {
            panic!("fallback consulted for a valid id {raw:?}")
        })
        .unwrap();
        assert_eq!(
            resolved,
            ResolvedTool {
                binary_path: PROTONTRICKS.to_string(),
                tool: PrefixDepsTool::Protontricks,
                steam_app_id: Some(canonical.to_string()),
            }
        );
    }
}

#[test]
fn s2_profile_app_id_prefers_steam_then_runtime() {
    let mut profile = GameProfile::default();
    profile.steam.app_id = "not-a-number".to_string();
    profile.runtime.steam_app_id = "570".to_string();
    let app_id = profile_steam_app_id(&profile);
    assert_eq!(app_id.as_deref(), Some("570"));
    let resolved = resolve_tool(&protontricks(), app_id.as_deref(), || None).unwrap();
    assert_eq!(resolved.tool, PrefixDepsTool::Protontricks);
    assert_eq!(resolved.steam_app_id.as_deref(), Some("570"));

    profile.steam.app_id = " 0440 ".to_string();
    assert_eq!(profile_steam_app_id(&profile).as_deref(), Some("440"));

    profile.steam.app_id = "0".to_string();
    profile.runtime.steam_app_id = String::new();
    assert_eq!(profile_steam_app_id(&profile), None);
}

#[test]
fn s3_protontricks_without_app_id_falls_back_to_winetricks_on_path() {
    let resolved = resolve_tool(&protontricks(), None, || {
        Some(WINETRICKS_ON_PATH.to_string())
    })
    .unwrap();
    assert_eq!(
        resolved,
        ResolvedTool {
            binary_path: WINETRICKS_ON_PATH.to_string(),
            tool: PrefixDepsTool::Winetricks,
            steam_app_id: None,
        }
    );
}

#[test]
fn s4_protontricks_without_app_id_or_winetricks_is_rejected() {
    assert_eq!(
        resolve_tool(&protontricks(), None, || None),
        Err(ToolSelectionError::ProtontricksNeedsAppId)
    );
}

#[test]
fn s5_winetricks_ignores_the_app_id_and_never_looks_up_path() {
    let found = detection(
        Some("/opt/tools/winetricks"),
        Some(PrefixDepsTool::Winetricks),
    );
    let resolved = resolve_tool(&found, Some("570"), || {
        panic!("winetricks must not consult the PATH fallback")
    })
    .unwrap();
    assert_eq!(
        resolved,
        ResolvedTool {
            binary_path: "/opt/tools/winetricks".to_string(),
            tool: PrefixDepsTool::Winetricks,
            steam_app_id: None,
        }
    );
}

#[test]
fn missing_tool_is_not_found() {
    assert_eq!(
        resolve_tool(&detection(None, None), Some("570"), || {
            Some(WINETRICKS_ON_PATH.to_string())
        }),
        Err(ToolSelectionError::NotFound)
    );
    let mut found_without_path = protontricks();
    found_without_path.binary_path = None;
    assert_eq!(
        resolve_tool(&found_without_path, Some("570"), || None),
        Err(ToolSelectionError::NotFound)
    );
}

#[test]
fn missing_tool_type_falls_back_to_the_binary_name() {
    let unnamed = detection(Some(PROTONTRICKS), None);
    let resolved = resolve_tool(&unnamed, Some("570"), || None).unwrap();
    assert_eq!(resolved.tool, PrefixDepsTool::Protontricks);

    let other = detection(Some("/opt/tools/tricks"), None);
    let resolved = resolve_tool(&other, Some("570"), || None).unwrap();
    assert_eq!(resolved.tool, PrefixDepsTool::Winetricks);
    assert_eq!(resolved.steam_app_id, None);
}
