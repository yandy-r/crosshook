use std::path::{Path, PathBuf};

use crate::platform;

use super::discovery::discover_compat_tools_with_roots;
use super::matching::resolve_compat_tool_by_name;
use super::types::SYSTEM_COMPAT_TOOL_ROOTS;

pub fn prefer_user_local_compat_tool_path(
    configured_proton_path: &Path,
    steam_root_candidates: &[PathBuf],
    diagnostics: &mut Vec<String>,
) -> PathBuf {
    let system_roots = SYSTEM_COMPAT_TOOL_ROOTS
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    prefer_user_local_compat_tool_path_with_roots(
        configured_proton_path,
        steam_root_candidates.iter().cloned(),
        system_roots,
        diagnostics,
    )
}

pub(crate) fn prefer_user_local_compat_tool_path_with_roots<I>(
    configured_proton_path: &Path,
    steam_root_candidates: I,
    system_roots: Vec<PathBuf>,
    diagnostics: &mut Vec<String>,
) -> PathBuf
where
    I: IntoIterator<Item = PathBuf>,
{
    let normalized_path = PathBuf::from(
        platform::normalize_flatpak_host_path(&configured_proton_path.to_string_lossy()).trim(),
    );

    if !path_is_under_any_root(&normalized_path, &system_roots) {
        return configured_proton_path.to_path_buf();
    }

    let Some(requested_tool_name) = normalized_path
        .parent()
        .and_then(|path| path.file_name())
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return configured_proton_path.to_path_buf();
    };

    let installed_tools =
        discover_compat_tools_with_roots(steam_root_candidates, system_roots.clone(), diagnostics);
    let matching_tools = resolve_compat_tool_by_name(requested_tool_name, &installed_tools);
    if matching_tools.iter().any(|tool| {
        *normalized_path
            == *platform::normalize_flatpak_host_path(&tool.path.to_string_lossy()).trim()
    }) {
        return configured_proton_path.to_path_buf();
    }

    let local_matches = matching_tools
        .iter()
        .filter(|tool| {
            let tool_path = PathBuf::from(
                platform::normalize_flatpak_host_path(&tool.path.to_string_lossy()).trim(),
            );
            !path_is_under_any_root(&tool_path, &system_roots)
        })
        .collect::<Vec<_>>();
    if let [preferred_tool] = local_matches.as_slice() {
        diagnostics.push(format!(
            "Flatpak launch preferred user-local compat tool '{}' over system path '{}'.",
            preferred_tool.path.display(),
            configured_proton_path.display()
        ));
        return preferred_tool.path.clone();
    }

    if local_matches.len() > 1 {
        let mut local_paths = local_matches
            .iter()
            .map(|tool| tool.path.display().to_string())
            .collect::<Vec<_>>();
        local_paths.sort();
        let mut installed_paths = installed_tools
            .iter()
            .map(|tool| tool.path.display().to_string())
            .collect::<Vec<_>>();
        installed_paths.sort();
        diagnostics.push(format!(
            "Configured Proton path '{}' did not match an installed compat tool for '{}', and multiple user-local matches were found ({}). Keeping the configured path to avoid a silent rewrite. Installed tools: {}.",
            configured_proton_path.display(),
            requested_tool_name,
            local_paths.join(", "),
            installed_paths.join(", "),
        ));
        return configured_proton_path.to_path_buf();
    }

    if let [preferred_tool] = matching_tools.as_slice() {
        diagnostics.push(format!(
            "Flatpak launch replaced missing configured compat tool '{}' with discovered install '{}'.",
            configured_proton_path.display(),
            preferred_tool.path.display()
        ));
        return preferred_tool.path.clone();
    }

    configured_proton_path.to_path_buf()
}

fn path_is_under_any_root(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}
