use std::collections::HashSet;

use super::super::models::ProtonInstall;

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

pub(super) fn push_alias(
    aliases: &mut Vec<String>,
    seen_aliases: &mut HashSet<String>,
    alias: &str,
) {
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
        .filter(char::is_ascii_alphanumeric)
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
            .filter(char::is_ascii_digit)
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
