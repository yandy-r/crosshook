use tempfile::tempdir;

use super::super::flatpak::prefer_user_local_compat_tool_path_with_roots;
use super::create_tool;

#[test]
fn flatpak_preference_preserves_matching_configured_system_tool() {
    let steam_root = tempdir().expect("steam root");
    let system_root = tempdir().expect("system root");
    let local_tool = steam_root
        .path()
        .join("compatibilitytools.d/Proton-CachyOS-SLR-Home");
    let system_tool = system_root.path().join("proton-cachyos-slr");

    create_tool(
        &local_tool,
        Some(
            r#"
            "compat_tools"
            {
                "proton-cachyos-slr"
                {
                    "display_name" "Proton CachyOS SLR"
                }
            }
            "#,
        ),
    );
    create_tool(
        &system_tool,
        Some(
            r#"
            "compat_tools"
            {
                "proton-cachyos-slr"
                {
                    "display_name" "Proton CachyOS SLR"
                }
            }
            "#,
        ),
    );

    let mut diagnostics = Vec::new();
    let preferred = prefer_user_local_compat_tool_path_with_roots(
        &system_tool.join("proton"),
        vec![steam_root.path().to_path_buf()],
        vec![system_root.path().to_path_buf()],
        &mut diagnostics,
    );

    assert_eq!(preferred, system_tool.join("proton"));
    assert!(!diagnostics
        .iter()
        .any(|entry| entry.contains("preferred user-local compat tool")));
}

#[test]
fn flatpak_preference_redirects_missing_configured_system_tool_to_single_local_install() {
    let steam_root = tempdir().expect("steam root");
    let system_root = tempdir().expect("system root");
    let local_tool = steam_root
        .path()
        .join("compatibilitytools.d/Proton-CachyOS-SLR-Home");

    create_tool(
        &local_tool,
        Some(
            r#"
            "compat_tools"
            {
                "proton-cachyos-slr"
                {
                    "display_name" "Proton CachyOS SLR"
                }
            }
            "#,
        ),
    );

    let mut diagnostics = Vec::new();
    let preferred = prefer_user_local_compat_tool_path_with_roots(
        &system_root.path().join("proton-cachyos-slr/proton"),
        vec![steam_root.path().to_path_buf()],
        vec![system_root.path().to_path_buf()],
        &mut diagnostics,
    );

    assert_eq!(preferred, local_tool.join("proton"));
    assert!(diagnostics
        .iter()
        .any(|entry| entry.contains("preferred user-local compat tool")));
}

#[test]
fn flatpak_preference_keeps_configured_path_when_local_matches_are_ambiguous() {
    let steam_root = tempdir().expect("steam root");
    let system_root = tempdir().expect("system root");
    let local_tool_one = steam_root
        .path()
        .join("compatibilitytools.d/Proton-CachyOS-SLR-One");
    let local_tool_two = steam_root
        .path()
        .join("compatibilitytools.d/Proton-CachyOS-SLR-Two");

    let alias_definition = Some(
        r#"
        "compat_tools"
        {
            "proton-cachyos-slr"
            {
                "display_name" "Proton CachyOS SLR"
            }
        }
        "#,
    );
    create_tool(&local_tool_one, alias_definition);
    create_tool(&local_tool_two, alias_definition);

    let configured_path = system_root.path().join("proton-cachyos-slr/proton");
    let mut diagnostics = Vec::new();
    let preferred = prefer_user_local_compat_tool_path_with_roots(
        &configured_path,
        vec![steam_root.path().to_path_buf()],
        vec![system_root.path().to_path_buf()],
        &mut diagnostics,
    );

    assert_eq!(preferred, configured_path);
    assert!(diagnostics.iter().any(|entry| {
        entry.contains("multiple user-local matches were found")
            && entry.contains("Keeping the configured path")
    }));
}
