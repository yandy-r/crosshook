use tempfile::tempdir;

use super::super::super::models::SteamAutoPopulateFieldState;
use super::super::resolution::resolve_proton_path;
use super::{create_tool, write_steam_config};

#[test]
fn resolves_exact_normalized_and_heuristic_proton_mappings() {
    let steam_root = tempdir().expect("steam root");
    let exact_tool = steam_root.path().join("steamapps/common/GE-Proton-9-4");
    let normalized_tool = steam_root
        .path()
        .join("steamapps/common/Proton-Experimental");
    let heuristic_tool = steam_root.path().join("steamapps/common/Custom-Tool");

    create_tool(&exact_tool, None);
    create_tool(&normalized_tool, None);
    create_tool(
        &heuristic_tool,
        Some(
            r#"
            "compat_tools"
            {
                "exp-beta"
                {
                    "display_name" "Proton Experimental Beta"
                }
            }
            "#,
        ),
    );

    write_steam_config(
        steam_root.path(),
        r#"
        "root"
        {
            "CompatToolMapping"
            {
                "111"
                {
                    "name" "GE-Proton-9-4"
                }
                "222"
                {
                    "name" "Proton  Experimental"
                }
                "333"
                {
                    "name" "Beta"
                }
            }
        }
        "#,
    );

    let mut diagnostics = Vec::new();
    let exact = resolve_proton_path("111", &[steam_root.path().to_path_buf()], &mut diagnostics);
    assert_eq!(exact.state, SteamAutoPopulateFieldState::Found);
    assert_eq!(exact.proton_path, exact_tool.join("proton"));

    let normalized =
        resolve_proton_path("222", &[steam_root.path().to_path_buf()], &mut diagnostics);
    assert_eq!(normalized.state, SteamAutoPopulateFieldState::Found);
    assert_eq!(normalized.proton_path, normalized_tool.join("proton"));

    let heuristic =
        resolve_proton_path("333", &[steam_root.path().to_path_buf()], &mut diagnostics);
    assert_eq!(heuristic.state, SteamAutoPopulateFieldState::Found);
    assert_eq!(heuristic.proton_path, heuristic_tool.join("proton"));
}

#[test]
fn resolves_ambiguous_and_missing_proton_mappings() {
    let steam_root = tempdir().expect("steam root");
    let tool_one = steam_root.path().join("steamapps/common/Proton-A");
    let tool_two = steam_root.path().join("steamapps/common/Proton-B");

    let shared_alias_definition = Some(
        r#"
        "compat_tools"
        {
            "Shared Proton"
            {
                "display_name" "Shared Proton"
            }
        }
        "#,
    );

    create_tool(&tool_one, shared_alias_definition);
    create_tool(&tool_two, shared_alias_definition);

    write_steam_config(
        steam_root.path(),
        r#"
        "root"
        {
            "CompatToolMapping"
            {
                "111"
                {
                    "name" "Shared Proton"
                }
                "222"
                {
                    "name" "Missing Proton"
                }
            }
        }
        "#,
    );

    let mut diagnostics = Vec::new();
    let ambiguous =
        resolve_proton_path("111", &[steam_root.path().to_path_buf()], &mut diagnostics);
    assert_eq!(ambiguous.state, SteamAutoPopulateFieldState::Ambiguous);
    assert!(ambiguous.proton_path.as_os_str().is_empty());

    let missing = resolve_proton_path("222", &[steam_root.path().to_path_buf()], &mut diagnostics);
    assert_eq!(missing.state, SteamAutoPopulateFieldState::NotFound);
    assert!(missing.proton_path.as_os_str().is_empty());
}
