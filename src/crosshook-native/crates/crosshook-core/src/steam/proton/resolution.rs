use std::path::PathBuf;

use super::super::models::SteamAutoPopulateFieldState;
use super::discovery::{collect_compat_tool_mappings, discover_compat_tools};
use super::matching::resolve_compat_tool_by_name;
use super::types::{CompatToolMappings, ProtonResolution};

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
            "No Proton mapping was found for App ID {steam_app_id}."
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
                "Proton tool '{requested_tool_name}' resolved to multiple installs. Auto-populate will not guess the Proton path."
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
                "CrossHook could not resolve Proton mapping '{requested_tool_name}' to an installed Proton executable."
            ));
            ProtonResolution {
                state: SteamAutoPopulateFieldState::NotFound,
                proton_path: PathBuf::new(),
            }
        }
    }
}

fn mapping_names(compat_tool_mappings: &CompatToolMappings, key: &str) -> Vec<String> {
    compat_tool_mappings
        .get(&key.trim().to_ascii_lowercase())
        .map(|tool_names| tool_names.iter().cloned().collect())
        .unwrap_or_default()
}
