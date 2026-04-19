use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::platform;

use super::super::models::ProtonInstall;
use super::super::vdf::parse_vdf;
use super::matching::{normalize_alias, push_alias};
use super::types::{CompatToolMappings, SYSTEM_COMPAT_TOOL_ROOTS};
use super::util::safe_enumerate_directories;

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
            false,
        );

        let custom_tools_root = steam_root_candidate.join("compatibilitytools.d");
        discover_tools_in_root(
            &custom_tools_root,
            false,
            &mut tools,
            &mut seen_proton_paths,
            diagnostics,
            false,
        );
    }

    for system_compat_tool_root in system_compat_tool_roots {
        let use_host_fs = platform::is_flatpak();
        if use_host_fs {
            if !platform::host_path_is_dir(&system_compat_tool_root) {
                continue;
            }
        } else if !system_compat_tool_root.is_dir() {
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
            use_host_fs,
        );
    }

    tools
}

fn discover_tools_in_root(
    tool_root: &Path,
    is_official: bool,
    tools: &mut Vec<ProtonInstall>,
    seen_proton_paths: &mut HashSet<PathBuf>,
    diagnostics: &mut Vec<String>,
    use_host_fs: bool,
) {
    let directories: Vec<PathBuf> = if use_host_fs {
        if !platform::host_path_is_dir(tool_root) {
            return;
        }
        match platform::host_read_dir_names(tool_root) {
            Ok(names) => names
                .into_iter()
                .map(|n| tool_root.join(n))
                .filter(|p| platform::host_path_is_dir(p))
                .collect(),
            Err(error) => {
                diagnostics.push(format!(
                    "Failed to list host compat-tool directory '{}': {error}",
                    tool_root.display()
                ));
                return;
            }
        }
    } else {
        safe_enumerate_directories(tool_root, diagnostics)
    };

    for tool_directory_path in directories {
        try_add_compat_tool_install(
            tools,
            seen_proton_paths,
            &tool_directory_path,
            is_official,
            diagnostics,
            use_host_fs,
        );
    }
}

fn try_add_compat_tool_install(
    tools: &mut Vec<ProtonInstall>,
    seen_proton_paths: &mut HashSet<PathBuf>,
    tool_directory_path: &Path,
    is_official: bool,
    diagnostics: &mut Vec<String>,
    use_host_fs: bool,
) {
    let proton_path = tool_directory_path.join("proton");
    let proton_ok = if use_host_fs {
        platform::host_path_is_file(&proton_path)
    } else {
        proton_path.is_file()
    };
    if !proton_ok || !seen_proton_paths.insert(proton_path.clone()) {
        return;
    }

    let Some(name) = tool_directory_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(std::string::ToString::to_string)
    else {
        return;
    };

    let mut aliases = Vec::new();
    let mut seen_aliases = HashSet::new();
    push_alias(&mut aliases, &mut seen_aliases, &name);

    let compatibility_tool_definition_path = tool_directory_path.join("compatibilitytool.vdf");
    let vdf_is_file = if use_host_fs {
        platform::host_path_is_file(&compatibility_tool_definition_path)
    } else {
        compatibility_tool_definition_path.is_file()
    };
    if vdf_is_file {
        let read_result = if use_host_fs {
            platform::host_read_file_bytes_if_system_path(&compatibility_tool_definition_path)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        } else {
            fs::read_to_string(&compatibility_tool_definition_path)
        };
        match read_result {
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
