use std::path::Path;

use crate::profile::models::GameProfile;

use super::types::{
    ApplyMigrationRequest, MigrationApplyResult, MigrationOutcome, ProtonPathField,
};

/// Prepares a single migration on an already loaded effective `profile`.
///
/// Checks that the replacement is a regular file, short-circuits with
/// `AlreadyValid` when the current path still exists, and otherwise rewrites
/// the requested field in place. `Applied` means `profile` was mutated and
/// is ready to persist; `AlreadyValid` and `Failed` leave it untouched.
/// Performs no profile I/O.
pub fn prepare_profile_migration(
    profile: &mut GameProfile,
    request: &ApplyMigrationRequest,
) -> MigrationApplyResult {
    let old_path = match request.field {
        ProtonPathField::SteamProtonPath => profile.steam.proton_path.clone(),
        ProtonPathField::RuntimeProtonPath => profile.runtime.proton_path.clone(),
    };

    // Validate replacement path exists and is a regular file.
    if !Path::new(&request.new_path).is_file() {
        let error = format!(
            "Replacement path does not exist or is not a file: {}",
            request.new_path
        );
        return result(request, old_path, MigrationOutcome::Failed, Some(error));
    }

    // If the current path is still valid, no migration is needed.
    if matches!(Path::new(&old_path).try_exists(), Ok(true)) {
        return result(request, old_path, MigrationOutcome::AlreadyValid, None);
    }

    // Update the correct field on the effective profile.
    match request.field {
        ProtonPathField::SteamProtonPath => {
            profile.steam.proton_path = request.new_path.clone();
        }
        ProtonPathField::RuntimeProtonPath => {
            profile.runtime.proton_path = request.new_path.clone();
        }
    }

    result(request, old_path, MigrationOutcome::Applied, None)
}

fn result(
    request: &ApplyMigrationRequest,
    old_path: String,
    outcome: MigrationOutcome,
    error: Option<String>,
) -> MigrationApplyResult {
    MigrationApplyResult {
        profile_name: request.profile_name.clone(),
        field: request.field,
        old_path,
        new_path: request.new_path.clone(),
        outcome,
        error,
    }
}
