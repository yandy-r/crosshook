use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::launch::runtime_helpers::{host_environment_map, resolve_wine_prefix_path};
use crate::platform::{
    host_command_with_env_and_directory_lifecycle_inner, is_flatpak, normalize_flatpak_host_path,
};
use crate::prefix_deps::PrefixDepsError;

/// Build a prefix dependency command bound to the Flatpak proxy lifetime so
/// cancellation and timeouts also stop the host process.
pub(super) fn build_prefix_dep_mutation_command(
    binary_path: &str,
    prefix_path: &str,
) -> Result<(Command, PathBuf), PrefixDepsError> {
    build_prefix_dep_command_with(binary_path, prefix_path, is_flatpak())
}

/// [`build_prefix_dep_mutation_command`] with an explicit Flatpak decision.
pub(super) fn build_prefix_dep_command_with(
    binary_path: &str,
    prefix_path: &str,
    flatpak: bool,
) -> Result<(Command, PathBuf), PrefixDepsError> {
    let resolved_prefix = resolved_prefix_path(prefix_path)?;
    let normalized_binary = normalize_flatpak_host_path(binary_path);
    let mut environment = host_environment_map();
    environment.insert(
        "WINEPREFIX".to_string(),
        resolved_prefix.to_string_lossy().into_owned(),
    );
    let command = host_command_with_env_and_directory_lifecycle_inner(
        &normalized_binary,
        &environment,
        None,
        flatpak,
        &BTreeMap::new(),
    );
    Ok((command, resolved_prefix))
}

fn resolved_prefix_path(prefix_path: &str) -> Result<PathBuf, PrefixDepsError> {
    let normalized_prefix = normalize_flatpak_host_path(prefix_path);
    let resolved_prefix = resolve_wine_prefix_path(Path::new(&normalized_prefix));
    if !resolved_prefix.is_dir() {
        return Err(PrefixDepsError::PrefixNotInitialized {
            path: resolved_prefix.to_string_lossy().into_owned(),
        });
    }
    fs::canonicalize(&resolved_prefix).map_err(|error| {
        PrefixDepsError::ValidationError(format!(
            "could not canonicalize prefix path {}: {error}",
            resolved_prefix.display()
        ))
    })
}

/// Normalize a host-visible prefix path and resolve its effective `pfx` directory.
pub fn normalize_prefix_path(prefix_path: &str) -> Result<String, PrefixDepsError> {
    Ok(resolved_prefix_path(prefix_path)?
        .to_string_lossy()
        .into_owned())
}
