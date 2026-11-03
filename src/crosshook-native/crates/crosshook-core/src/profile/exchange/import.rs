use super::super::community_schema::CommunityProfileManifest;
use super::super::ProfileStore;
use super::error::CommunityExchangeError;
use super::types::{CommunityImportPreview, CommunityImportResult};
use super::utils::{derive_import_name, hydrate_imported_profile};
use super::validation::{validate_manifest_value, validate_schema_version};
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Highest `-N` suffix tried before an import gives up on finding a free name.
const MAX_UNIQUE_NAME_SUFFIX: u32 = 1000;

/// Imports the community manifest at `json_path` as a new profile in `profiles_dir`.
///
/// The profile is saved under the name derived from the manifest, or under the first free
/// `<name>-2` … `<name>-1000` when that name is taken; an existing profile is never
/// overwritten. [`CommunityImportResult::profile_name`] reports the name actually used.
///
/// # Errors
/// Unreadable or invalid manifest, unsupported schema version, every candidate name already
/// taken (`NoFreeName`), or a profile write failure.
pub fn import_community_profile(
    json_path: &Path,
    profiles_dir: &Path,
) -> Result<CommunityImportResult, CommunityExchangeError> {
    let preview = preview_community_profile_import(json_path)?;
    let manifest = preview.manifest;
    let mut profile = preview.profile;
    if let Some(h) = &manifest.metadata.trainer_sha256 {
        let t = h.trim();
        if !t.is_empty() {
            profile.trainer.community_trainer_sha256 = t.to_string();
        }
    }

    let store = ProfileStore::with_base_path(profiles_dir.to_path_buf());
    let profile_name = unique_profile_name(&store, &preview.profile_name)?;
    store.save(&profile_name, &profile)?;

    Ok(CommunityImportResult {
        profile_path: profiles_dir.join(format!("{profile_name}.toml")),
        profile_name,
        source_path: json_path.to_path_buf(),
        profile,
        manifest,
    })
}

/// Returns `base` when nothing occupies its file (a dangling symlink counts as taken), else the
/// first free `<base>-N`; `NoFreeName` when every candidate is taken.
fn unique_profile_name(store: &ProfileStore, base: &str) -> Result<String, CommunityExchangeError> {
    store
        .first_free_name(base, MAX_UNIQUE_NAME_SUFFIX)?
        .ok_or_else(|| CommunityExchangeError::NoFreeName {
            base: base.to_string(),
            max_suffix: MAX_UNIQUE_NAME_SUFFIX,
        })
}

pub fn preview_community_profile_import(
    json_path: &Path,
) -> Result<CommunityImportPreview, CommunityExchangeError> {
    let content = fs::read_to_string(json_path).map_err(|error| CommunityExchangeError::Io {
        action: "read the community profile JSON".to_string(),
        path: json_path.to_path_buf(),
        message: error.to_string(),
    })?;

    let value: Value =
        serde_json::from_str(&content).map_err(|error| CommunityExchangeError::Json {
            path: json_path.to_path_buf(),
            message: error.to_string(),
        })?;

    validate_manifest_value(&value)?;
    let manifest: CommunityProfileManifest =
        serde_json::from_value(value).map_err(|error| CommunityExchangeError::Json {
            path: json_path.to_path_buf(),
            message: error.to_string(),
        })?;

    validate_schema_version(manifest.schema_version)?;

    let profile_name = derive_import_name(&manifest, json_path);
    let required_prefix_deps = manifest.profile.trainer.required_protontricks.clone();
    let hydrated_profile = hydrate_imported_profile(&manifest.profile);

    Ok(CommunityImportPreview {
        profile_name,
        source_path: json_path.to_path_buf(),
        profile: hydrated_profile,
        manifest,
        required_prefix_deps,
    })
}
