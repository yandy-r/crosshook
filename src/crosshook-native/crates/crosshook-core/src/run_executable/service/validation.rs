use std::path::Path;

use crate::platform::{
    normalize_flatpak_host_path, normalized_path_is_dir, normalized_path_is_executable_file,
    normalized_path_is_file,
};
use crate::run_executable::RunExecutableValidationError;

pub fn validate_run_executable_request(
    request: &crate::run_executable::RunExecutableRequest,
) -> Result<(), RunExecutableValidationError> {
    validate_executable_path(request.executable_path.trim())?;
    validate_proton_path(request.proton_path.trim())?;
    validate_optional_prefix_path(request.prefix_path.trim())?;

    Ok(())
}

fn validate_executable_path(path: &str) -> Result<(), RunExecutableValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Err(RunExecutableValidationError::ExecutablePathRequired);
    }

    let path = Path::new(normalized_path.trim());
    if !normalized_path_is_file(normalized_path.trim()) {
        if !path.exists() {
            return Err(RunExecutableValidationError::ExecutablePathMissing);
        }
        return Err(RunExecutableValidationError::ExecutablePathNotFile);
    }
    if !is_windows_runnable_executable(path) {
        return Err(RunExecutableValidationError::ExecutablePathNotWindowsExecutable);
    }

    Ok(())
}

fn validate_proton_path(path: &str) -> Result<(), RunExecutableValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Err(RunExecutableValidationError::ProtonPathRequired);
    }

    let path = Path::new(normalized_path.trim());
    if !normalized_path_is_executable_file(normalized_path.trim()) {
        if !path.exists() {
            return Err(RunExecutableValidationError::ProtonPathMissing);
        }
        return Err(RunExecutableValidationError::ProtonPathNotExecutable);
    }

    Ok(())
}

fn validate_optional_prefix_path(path: &str) -> Result<(), RunExecutableValidationError> {
    let normalized_path = normalize_flatpak_host_path(path);
    if normalized_path.is_empty() {
        return Ok(());
    }

    let path = Path::new(normalized_path.trim());
    if !normalized_path_is_dir(normalized_path.trim()) {
        if !path.exists() {
            return Err(RunExecutableValidationError::PrefixPathMissing);
        }
        return Err(RunExecutableValidationError::PrefixPathNotDirectory);
    }

    Ok(())
}

fn is_windows_runnable_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("msi")
        })
}
