use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::super::request::{
    LaunchRequest, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use super::catalog::{global_catalog, CommandArgumentCatalog};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ResolvedCommandArguments {
    pub tokens: Vec<String>,
}

impl ResolvedCommandArguments {
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandArgumentResolveError {
    Unknown(String),
    Duplicate(String),
    NotSupportedForMethod { argument_id: String, method: String },
    Incompatible { first: String, second: String },
    UnsupportedLaunchMethod(String),
}

pub fn is_known_command_argument_id(argument_id: &str) -> bool {
    global_catalog().is_known_id(argument_id)
}

/// Resolves command arguments for a launch method using the global catalog.
pub fn resolve_command_arguments_for_method(
    enabled_argument_ids: &[String],
    custom_args: &[String],
    resolved_method: &str,
) -> Result<ResolvedCommandArguments, CommandArgumentResolveError> {
    resolve_command_arguments_with_catalog(
        enabled_argument_ids,
        custom_args,
        resolved_method,
        global_catalog(),
    )
}

/// Resolves command arguments against a specific catalog (for tests).
pub(crate) fn resolve_command_arguments_with_catalog(
    enabled_argument_ids: &[String],
    custom_args: &[String],
    resolved_method: &str,
    catalog: &CommandArgumentCatalog,
) -> Result<ResolvedCommandArguments, CommandArgumentResolveError> {
    if enabled_argument_ids.is_empty() && custom_args.is_empty() {
        return Ok(ResolvedCommandArguments::default());
    }

    let mut seen_ids = HashSet::new();
    for argument_id in enabled_argument_ids {
        if !seen_ids.insert(argument_id.as_str()) {
            return Err(CommandArgumentResolveError::Duplicate(argument_id.clone()));
        }

        if !catalog.is_known_id(argument_id) {
            return Err(CommandArgumentResolveError::Unknown(argument_id.clone()));
        }
    }

    let selected_ids = seen_ids;
    let mut tokens = Vec::new();

    for entry in &catalog.entries {
        if !selected_ids.contains(entry.id.as_str()) {
            continue;
        }

        if !entry
            .applicable_methods
            .iter()
            .any(|method| method == resolved_method)
        {
            return Err(CommandArgumentResolveError::NotSupportedForMethod {
                argument_id: entry.id.clone(),
                method: resolved_method.to_string(),
            });
        }

        for conflicting_id in &entry.conflicts_with {
            if selected_ids.contains(conflicting_id.as_str()) {
                return Err(CommandArgumentResolveError::Incompatible {
                    first: entry.id.clone(),
                    second: conflicting_id.clone(),
                });
            }
        }

        for token in &entry.tokens {
            tokens.push(token.clone());
        }
    }

    for custom_arg in custom_args {
        tokens.push(custom_arg.clone());
    }

    Ok(ResolvedCommandArguments { tokens })
}

/// Resolves command arguments from a launch request.
pub fn resolve_command_arguments(
    request: &LaunchRequest,
) -> Result<ResolvedCommandArguments, CommandArgumentResolveError> {
    let enabled_argument_ids = &request.command_arguments.enabled_argument_ids;
    let custom_args = &request.command_arguments.custom_args;

    if enabled_argument_ids.is_empty() && custom_args.is_empty() {
        return Ok(ResolvedCommandArguments::default());
    }

    let resolved_method = request.resolved_method();
    if resolved_method == METHOD_NATIVE {
        return Err(CommandArgumentResolveError::UnsupportedLaunchMethod(
            resolved_method.to_string(),
        ));
    }

    if resolved_method != METHOD_PROTON_RUN && resolved_method != METHOD_STEAM_APPLAUNCH {
        return Err(CommandArgumentResolveError::UnsupportedLaunchMethod(
            resolved_method.to_string(),
        ));
    }

    resolve_command_arguments_for_method(enabled_argument_ids, custom_args, resolved_method)
}
