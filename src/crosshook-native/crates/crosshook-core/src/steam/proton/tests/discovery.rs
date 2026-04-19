use tempfile::tempdir;

use super::super::discovery::{collect_compat_tool_mappings, discover_compat_tools_with_roots};
use super::super::matching::normalize_alias;
use super::{create_tool, write_steam_config, write_userdata_config};

#[test]
fn discovers_official_custom_and_system_tools() {
    let steam_root = tempdir().expect("steam root");
    let system_root = tempdir().expect("system root");

    create_tool(
        &steam_root.path().join("steamapps/common/Official-Proton"),
        None,
    );
    create_tool(
        &steam_root.path().join("compatibilitytools.d/Custom-Proton"),
        Some(
            r#"
            "compat_tools"
            {
                "GE-Proton"
                {
                    "display_name" "GE Proton"
                }
            }
            "#,
        ),
    );
    create_tool(
        &system_root.path().join("System-Proton"),
        Some(
            r#"
            "compat_tools"
            {
                "SystemAlias"
                {
                    "display_name" "System Proton"
                }
            }
            "#,
        ),
    );

    let mut diagnostics = Vec::new();
    let tools = discover_compat_tools_with_roots(
        vec![steam_root.path().to_path_buf()],
        vec![system_root.path().to_path_buf()],
        &mut diagnostics,
    );

    assert_eq!(tools.len(), 3);

    let official = tools
        .iter()
        .find(|tool| tool.name == "Official-Proton")
        .expect("official tool");
    assert!(official.is_official);
    assert!(official
        .aliases
        .iter()
        .any(|alias| alias == "Official-Proton"));
    assert!(official.normalized_aliases.contains("officialproton"));

    let custom = tools
        .iter()
        .find(|tool| tool.name == "Custom-Proton")
        .expect("custom tool");
    assert!(!custom.is_official);
    assert!(custom.aliases.iter().any(|alias| alias == "ge-proton"));
    assert!(custom.aliases.iter().any(|alias| alias == "GE Proton"));
    assert!(custom.normalized_aliases.contains("geproton"));
    assert!(custom.normalized_aliases.contains("geproton"));

    let system = tools
        .iter()
        .find(|tool| tool.name == "System-Proton")
        .expect("system tool");
    assert!(!system.is_official);
    assert!(system.aliases.iter().any(|alias| alias == "systemalias"));
    assert!(system.aliases.iter().any(|alias| alias == "System Proton"));
    assert!(system.normalized_aliases.contains("systemalias"));

    assert!(diagnostics
        .iter()
        .any(|entry| entry.contains("System Steam compat-tool root")));
}

#[test]
fn normalizes_aliases_to_lowercase_alphanumeric_only() {
    assert_eq!(
        normalize_alias("GE Proton 9.7"),
        Some("geproton97".to_string())
    );
    assert_eq!(normalize_alias("   "), None);
}

#[test]
fn collects_app_specific_and_default_mappings_from_config_files() {
    let steam_root = tempdir().expect("steam root");
    write_steam_config(
        steam_root.path(),
        r#"
        "InstallConfigStore"
        {
            "CompatToolMapping"
            {
                "12345"
                {
                    "name" "GE-Proton 9-4"
                }
            }
        }
        "#,
    );
    write_userdata_config(
        steam_root.path(),
        "1000",
        r#"
        "root"
        {
            "CompatToolMapping"
            {
                "0"
                {
                    "name" "Proton Experimental"
                }
            }
        }
        "#,
    );

    let mut diagnostics = Vec::new();
    let mappings =
        collect_compat_tool_mappings(&[steam_root.path().to_path_buf()], &mut diagnostics);

    assert_eq!(
        mappings
            .get("12345")
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["GE-Proton 9-4".to_string()]
    );
    assert_eq!(
        mappings
            .get("0")
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["Proton Experimental".to_string()]
    );
    assert!(diagnostics.is_empty());
}
