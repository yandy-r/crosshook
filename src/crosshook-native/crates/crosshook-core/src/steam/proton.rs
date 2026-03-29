use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::models::ProtonInstall;
use super::models::SteamAutoPopulateFieldState;
use super::vdf::parse_vdf;

const SYSTEM_COMPAT_TOOL_ROOTS: &[&str] = &[
    "/usr/share/steam/compatibilitytools.d",
    "/usr/local/share/steam/compatibilitytools.d",
    "/usr/share/steam/compatibilitytools",
    "/usr/local/share/steam/compatibilitytools",
];

pub type CompatToolMappings = HashMap<String, BTreeSet<String>>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProtonResolution {
    pub state: SteamAutoPopulateFieldState,
    pub proton_path: PathBuf,
}

pub fn discover_compat_tools(
    steam_root_candidates: &[PathBuf],
    diagnostics: &mut Vec<String>,
) -> Vec<ProtonInstall> {
    let system_compat_tool_roots = SYSTEM_COMPAT_TOOL_ROOTS.iter().map(PathBuf::from);
    discover_compat_tools_with_roots(
        steam_root_candidates.iter().cloned(),
        system_compat_tool_roots,
        diagnostics,
    )
}

pub fn collect_compat_tool_mappings(
    steam_root_candidates: &[PathBuf],
    diagnostics: &mut Vec<String>,
) -> CompatToolMappings {
    collect_compat_tool_mappings_from_roots(steam_root_candidates.iter().cloned(), diagnostics)
}

pub(crate) fn collect_compat_tool_mappings_from_roots<I>(
    steam_root_candidates: I,
    diagnostics: &mut Vec<String>,
) -> CompatToolMappings
where
    I: IntoIterator<Item = PathBuf>,
{
    let mut mappings = CompatToolMappings::new();

    for steam_root_candidate in steam_root_candidates {
        if !steam_root_candidate.is_dir() {
            continue;
        }

        let mut config_paths = vec![steam_root_candidate.join("config").join("config.vdf")];
        config_paths.extend(
            safe_enumerate_directories(&steam_root_candidate.join("userdata"), diagnostics)
                .into_iter()
                .map(|user_data_directory| {
                    user_data_directory.join("config").join("localconfig.vdf")
                }),
        );

        for config_path in config_paths {
            if !config_path.is_file() {
                continue;
            }

            match fs::read_to_string(&config_path) {
                Ok(content) => match parse_vdf(&content) {
                    Ok(config_root) => {
                        if let Some(compat_tool_mapping_node) =
                            config_root.find_descendant("CompatToolMapping")
                        {
                            for (appid, mapping_node) in &compat_tool_mapping_node.children {
                                let tool_name = mapping_node
                                    .get_child("name")
                                    .and_then(|node| node.value.as_deref())
                                    .map(str::trim)
                                    .filter(|value| !value.is_empty());

                                let Some(tool_name) = tool_name else {
                                    continue;
                                };

                                mappings
                                    .entry(appid.to_string())
                                    .or_default()
                                    .insert(tool_name.to_string());
                            }
                        }
                    }
                    Err(error) => diagnostics.push(format!(
                        "Failed to parse Steam config '{}': {}",
                        config_path.display(),
                        error
                    )),
                },
                Err(error) => diagnostics.push(format!(
                    "Failed to read Steam config '{}': {}",
                    config_path.display(),
                    error
                )),
            }
        }
    }

    mappings
}

pub fn resolve_proton_path(
    steam_app_id: &str,
    steam_root_candidates: &[PathBuf],
    diagnostics: &mut Vec<String>,
) -> ProtonResolution {
    let compat_tool_mappings = collect_compat_tool_mappings(steam_root_candidates, diagnostics);
    let installed_tools = discover_compat_tools(steam_root_candidates, diagnostics);

    let exact_tool_names = mapping_names(&compat_tool_mappings, steam_app_id);
    if exact_tool_names.len() > 1 {
        diagnostics.push(format!(
            "Multiple app-specific Proton mappings were found for App ID {}: {}",
            steam_app_id,
            exact_tool_names.join(", ")
        ));
        return ProtonResolution {
            state: SteamAutoPopulateFieldState::Ambiguous,
            proton_path: PathBuf::new(),
        };
    }

    let default_tool_names = mapping_names(&compat_tool_mappings, "0");
    if exact_tool_names.is_empty() && default_tool_names.len() > 1 {
        diagnostics.push(format!(
            "Multiple default Proton mappings were found: {}",
            default_tool_names.join(", ")
        ));
        return ProtonResolution {
            state: SteamAutoPopulateFieldState::Ambiguous,
            proton_path: PathBuf::new(),
        };
    }

    let requested_tool_name = exact_tool_names
        .first()
        .cloned()
        .or_else(|| default_tool_names.first().cloned());

    let Some(requested_tool_name) = requested_tool_name else {
        diagnostics.push(format!(
            "No Proton mapping was found for App ID {}.",
            steam_app_id
        ));
        return ProtonResolution {
            state: SteamAutoPopulateFieldState::NotFound,
            proton_path: PathBuf::new(),
        };
    };

    let matching_tools = resolve_compat_tool_by_name(&requested_tool_name, &installed_tools);
    match matching_tools.len() {
        1 => {
            diagnostics.push(format!(
                "Resolved Proton tool '{}' to: {}",
                requested_tool_name,
                matching_tools[0].path.display()
            ));
            ProtonResolution {
                state: SteamAutoPopulateFieldState::Found,
                proton_path: matching_tools[0].path.clone(),
            }
        }
        count if count > 1 => {
            diagnostics.push(format!(
                "Proton tool '{}' resolved to multiple installs. Auto-populate will not guess the Proton path.",
                requested_tool_name
            ));
            let mut conflicting_paths = matching_tools
                .iter()
                .map(|tool| tool.path.display().to_string())
                .collect::<Vec<_>>();
            conflicting_paths.sort();

            for path in conflicting_paths {
                diagnostics.push(format!("Conflicting Proton install: {path}"));
            }

            ProtonResolution {
                state: SteamAutoPopulateFieldState::Ambiguous,
                proton_path: PathBuf::new(),
            }
        }
        _ => {
            diagnostics.push(format!(
                "CrossHook could not resolve Proton mapping '{}' to an installed Proton executable.",
                requested_tool_name
            ));
            ProtonResolution {
                state: SteamAutoPopulateFieldState::NotFound,
                proton_path: PathBuf::new(),
            }
        }
    }
}

pub(crate) fn discover_compat_tools_with_roots<I, J>(
    steam_root_candidates: I,
    system_compat_tool_roots: J,
    diagnostics: &mut Vec<String>,
) -> Vec<ProtonInstall>
where
    I: IntoIterator<Item = PathBuf>,
    J: IntoIterator<Item = PathBuf>,
{
    let mut tools = Vec::new();
    let mut seen_proton_paths = HashSet::new();

    for steam_root_candidate in steam_root_candidates {
        if !steam_root_candidate.is_dir() {
            continue;
        }

        let official_tools_root = steam_root_candidate.join("steamapps").join("common");
        discover_tools_in_root(
            &official_tools_root,
            true,
            &mut tools,
            &mut seen_proton_paths,
            diagnostics,
        );

        let custom_tools_root = steam_root_candidate.join("compatibilitytools.d");
        discover_tools_in_root(
            &custom_tools_root,
            false,
            &mut tools,
            &mut seen_proton_paths,
            diagnostics,
        );
    }

    for system_compat_tool_root in system_compat_tool_roots {
        if !system_compat_tool_root.is_dir() {
            continue;
        }

        diagnostics.push(format!(
            "System Steam compat-tool root: {}",
            system_compat_tool_root.display()
        ));

        discover_tools_in_root(
            &system_compat_tool_root,
            false,
            &mut tools,
            &mut seen_proton_paths,
            diagnostics,
        );
    }

    tools
}

fn mapping_names(compat_tool_mappings: &CompatToolMappings, key: &str) -> Vec<String> {
    compat_tool_mappings
        .get(&key.trim().to_ascii_lowercase())
        .map(|tool_names| tool_names.iter().cloned().collect())
        .unwrap_or_default()
}

pub(crate) fn resolve_compat_tool_by_name<'a>(
    requested_tool_name: &str,
    installed_tools: &'a [ProtonInstall],
) -> Vec<&'a ProtonInstall> {
    if requested_tool_name.trim().is_empty() {
        return Vec::new();
    }

    let exact_matches = installed_tools
        .iter()
        .filter(|tool| {
            tool.aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(requested_tool_name))
        })
        .collect::<Vec<_>>();
    if !exact_matches.is_empty() {
        return exact_matches;
    }

    if let Some(normalized_requested_tool_name) = normalize_alias(requested_tool_name) {
        let normalized_matches = installed_tools
            .iter()
            .filter(|tool| {
                tool.normalized_aliases
                    .contains(&normalized_requested_tool_name)
            })
            .collect::<Vec<_>>();
        if !normalized_matches.is_empty() {
            return normalized_matches;
        }
    }

    installed_tools
        .iter()
        .filter(|tool| tool_matches_requested_name_heuristically(requested_tool_name, tool))
        .collect()
}

fn discover_tools_in_root(
    tool_root: &Path,
    is_official: bool,
    tools: &mut Vec<ProtonInstall>,
    seen_proton_paths: &mut HashSet<PathBuf>,
    diagnostics: &mut Vec<String>,
) {
    for tool_directory_path in safe_enumerate_directories(tool_root, diagnostics) {
        try_add_compat_tool_install(
            tools,
            seen_proton_paths,
            &tool_directory_path,
            is_official,
            diagnostics,
        );
    }
}

fn try_add_compat_tool_install(
    tools: &mut Vec<ProtonInstall>,
    seen_proton_paths: &mut HashSet<PathBuf>,
    tool_directory_path: &Path,
    is_official: bool,
    diagnostics: &mut Vec<String>,
) {
    let proton_path = tool_directory_path.join("proton");
    if !proton_path.is_file() || !seen_proton_paths.insert(proton_path.clone()) {
        return;
    }

    let Some(name) = tool_directory_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
    else {
        return;
    };

    let mut aliases = Vec::new();
    let mut seen_aliases = HashSet::new();
    push_alias(&mut aliases, &mut seen_aliases, &name);

    let compatibility_tool_definition_path = tool_directory_path.join("compatibilitytool.vdf");
    if compatibility_tool_definition_path.is_file() {
        match fs::read_to_string(&compatibility_tool_definition_path) {
            Ok(content) => match parse_vdf(&content) {
                Ok(definition_root) => {
                    if let Some(compat_tools) = definition_root.find_descendant("compat_tools") {
                        for (alias_name, alias_node) in &compat_tools.children {
                            push_alias(&mut aliases, &mut seen_aliases, alias_name);
                            if let Some(display_name) = alias_node
                                .get_child("display_name")
                                .and_then(|node| node.value.as_ref())
                            {
                                push_alias(&mut aliases, &mut seen_aliases, display_name);
                            }
                        }
                    }
                }
                Err(error) => diagnostics.push(format!(
                    "Failed to parse compatibility tool metadata '{}': {}",
                    compatibility_tool_definition_path.display(),
                    error
                )),
            },
            Err(error) => diagnostics.push(format!(
                "Failed to read compatibility tool metadata '{}': {}",
                compatibility_tool_definition_path.display(),
                error
            )),
        }
    }

    let normalized_aliases = aliases
        .iter()
        .filter_map(|alias| normalize_alias(alias))
        .collect::<BTreeSet<_>>();

    tools.push(ProtonInstall {
        name,
        path: proton_path,
        is_official,
        aliases,
        normalized_aliases,
    });
}

fn push_alias(aliases: &mut Vec<String>, seen_aliases: &mut HashSet<String>, alias: &str) {
    let trimmed = alias.trim();
    if trimmed.is_empty() {
        return;
    }

    let normalized = trimmed.to_lowercase();
    if seen_aliases.insert(normalized) {
        aliases.push(trimmed.to_string());
    }
}

pub(crate) fn normalize_alias(alias: &str) -> Option<String> {
    let normalized = alias
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn tool_matches_requested_name_heuristically(
    requested_tool_name: &str,
    installed_tool: &ProtonInstall,
) -> bool {
    let normalized_requested_tool_name = normalize_alias(requested_tool_name);
    let Some(normalized_requested_tool_name) = normalized_requested_tool_name else {
        return false;
    };

    if installed_tool.normalized_aliases.iter().any(|alias| {
        alias.contains(&normalized_requested_tool_name)
            || normalized_requested_tool_name.contains(alias)
    }) {
        return true;
    }

    if normalized_requested_tool_name.starts_with("proton") {
        let requested_version = normalized_requested_tool_name
            .chars()
            .filter(|character| character.is_ascii_digit())
            .collect::<String>();

        if !requested_version.is_empty() {
            return installed_tool
                .normalized_aliases
                .iter()
                .any(|alias| alias.starts_with("proton") && alias.contains(&requested_version));
        }
    }

    false
}

fn safe_enumerate_directories(
    directory_path: &Path,
    diagnostics: &mut Vec<String>,
) -> Vec<PathBuf> {
    if !directory_path.is_dir() {
        return Vec::new();
    }

    let entries = match fs::read_dir(directory_path) {
        Ok(entries) => entries,
        Err(error) => {
            diagnostics.push(format!(
                "Failed to read directory '{}': {error}",
                directory_path.display()
            ));
            return Vec::new();
        }
    };

    let mut directories = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_dir() {
                    directories.push(path);
                }
            }
            Err(error) => {
                diagnostics.push(format!(
                    "Failed to read entry in '{}': {error}",
                    directory_path.display()
                ));
            }
        }
    }

    directories.sort();
    directories
}

#[cfg(test)]
mod tests {
    use super::{
        collect_compat_tool_mappings, discover_compat_tools_with_roots, normalize_alias,
        resolve_proton_path,
    };
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn create_tool(directory_path: &Path, compatibilitytool_vdf: Option<&str>) {
        fs::create_dir_all(directory_path).expect("tool dir");
        fs::write(directory_path.join("proton"), b"#!/bin/sh\n").expect("proton file");

        if let Some(content) = compatibilitytool_vdf {
            fs::write(directory_path.join("compatibilitytool.vdf"), content).expect("vdf");
        }
    }

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

    fn write_steam_config(root: &Path, content: &str) {
        let config_dir = root.join("config");
        fs::create_dir_all(&config_dir).expect("config dir");
        fs::write(config_dir.join("config.vdf"), content).expect("config.vdf");
    }

    fn write_userdata_config(root: &Path, user_id: &str, content: &str) {
        let config_dir = root.join("userdata").join(user_id).join("config");
        fs::create_dir_all(&config_dir).expect("userdata config dir");
        fs::write(config_dir.join("localconfig.vdf"), content).expect("localconfig.vdf");
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
        let exact =
            resolve_proton_path("111", &[steam_root.path().to_path_buf()], &mut diagnostics);
        assert_eq!(exact.state, super::SteamAutoPopulateFieldState::Found);
        assert_eq!(exact.proton_path, exact_tool.join("proton"));

        let normalized =
            resolve_proton_path("222", &[steam_root.path().to_path_buf()], &mut diagnostics);
        assert_eq!(normalized.state, super::SteamAutoPopulateFieldState::Found);
        assert_eq!(normalized.proton_path, normalized_tool.join("proton"));

        let heuristic =
            resolve_proton_path("333", &[steam_root.path().to_path_buf()], &mut diagnostics);
        assert_eq!(heuristic.state, super::SteamAutoPopulateFieldState::Found);
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
        assert_eq!(
            ambiguous.state,
            super::SteamAutoPopulateFieldState::Ambiguous
        );
        assert!(ambiguous.proton_path.as_os_str().is_empty());

        let missing =
            resolve_proton_path("222", &[steam_root.path().to_path_buf()], &mut diagnostics);
        assert_eq!(missing.state, super::SteamAutoPopulateFieldState::NotFound);
        assert!(missing.proton_path.as_os_str().is_empty());
    }
}
