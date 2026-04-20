use std::fs;
use std::path::PathBuf;

use crate::profile::toml_store::ProfileStore;

use super::types::{
    ApplyMigrationRequest, MigrationApplyResult, MigrationOutcome, ProtonPathField,
};

/// Applies a single migration using an atomic temp-file + rename write (W-1).
///
/// Does NOT delegate to `ProfileStore::save()` which uses non-atomic `fs::write()`.
pub fn apply_single_migration(
    store: &ProfileStore,
    request: &ApplyMigrationRequest,
) -> MigrationApplyResult {
    let mut profile = match store.load(&request.profile_name) {
        Ok(p) => p,
        Err(err) => {
            return MigrationApplyResult {
                profile_name: request.profile_name.clone(),
                field: request.field,
                old_path: String::new(),
                new_path: request.new_path.clone(),
                outcome: MigrationOutcome::Failed,
                error: Some(err.to_string()),
            };
        }
    };

    let old_path = match request.field {
        ProtonPathField::SteamProtonPath => profile.steam.proton_path.clone(),
        ProtonPathField::RuntimeProtonPath => profile.runtime.proton_path.clone(),
    };

    // Validate replacement path exists and is a regular file.
    if !PathBuf::from(&request.new_path).is_file() {
        return MigrationApplyResult {
            profile_name: request.profile_name.clone(),
            field: request.field,
            old_path,
            new_path: request.new_path.clone(),
            outcome: MigrationOutcome::Failed,
            error: Some(format!(
                "Replacement path does not exist or is not a file: {}",
                request.new_path
            )),
        };
    }

    // If the current path is still valid, no migration is needed.
    if matches!(PathBuf::from(&old_path).try_exists(), Ok(true)) {
        return MigrationApplyResult {
            profile_name: request.profile_name.clone(),
            field: request.field,
            old_path,
            new_path: request.new_path.clone(),
            outcome: MigrationOutcome::AlreadyValid,
            error: None,
        };
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

    // Atomic write: serialize storage form → .toml.tmp → rename to .toml.
    let profile_path = store
        .base_path
        .join(format!("{}.toml", request.profile_name));
    let tmp_path = profile_path.with_extension("toml.tmp");

    let toml_str = match toml::to_string_pretty(&profile.storage_profile()) {
        Ok(s) => s,
        Err(err) => {
            return MigrationApplyResult {
                profile_name: request.profile_name.clone(),
                field: request.field,
                old_path,
                new_path: request.new_path.clone(),
                outcome: MigrationOutcome::Failed,
                error: Some(err.to_string()),
            };
        }
    };

    if let Err(err) = fs::write(&tmp_path, &toml_str) {
        return MigrationApplyResult {
            profile_name: request.profile_name.clone(),
            field: request.field,
            old_path,
            new_path: request.new_path.clone(),
            outcome: MigrationOutcome::Failed,
            error: Some(err.to_string()),
        };
    }

    if let Err(err) = fs::rename(&tmp_path, &profile_path) {
        return MigrationApplyResult {
            profile_name: request.profile_name.clone(),
            field: request.field,
            old_path,
            new_path: request.new_path.clone(),
            outcome: MigrationOutcome::Failed,
            error: Some(err.to_string()),
        };
    }

    MigrationApplyResult {
        profile_name: request.profile_name.clone(),
        field: request.field,
        old_path,
        new_path: request.new_path.clone(),
        outcome: MigrationOutcome::Applied,
        error: None,
    }
}
