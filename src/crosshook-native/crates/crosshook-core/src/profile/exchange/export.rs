use super::super::community_schema::{CommunityProfileManifest, COMMUNITY_PROFILE_SCHEMA_VERSION};
use super::super::ProfileStore;
use super::error::CommunityExchangeError;
use super::types::CommunityExportResult;
use super::utils::{build_metadata, sanitize_profile_for_community_export, write_manifest};
use super::validation::validate_schema_version;
use std::path::Path;

pub fn export_community_profile(
    profiles_dir: &Path,
    profile_name: &str,
    output_path: &Path,
) -> Result<CommunityExportResult, CommunityExchangeError> {
    validate_schema_version(COMMUNITY_PROFILE_SCHEMA_VERSION)?;

    let store = ProfileStore::with_base_path(profiles_dir.to_path_buf());
    let profile = store.load(profile_name)?;
    // Metadata (e.g. trainer display name) is derived from the on-disk profile before stripping paths.
    let metadata = build_metadata(&profile);
    let shareable_profile = sanitize_profile_for_community_export(&profile);
    let manifest = CommunityProfileManifest::new(metadata, shareable_profile);

    write_manifest(output_path, &manifest)?;

    Ok(CommunityExportResult {
        profile_name: profile_name.to_string(),
        output_path: output_path.to_path_buf(),
        manifest,
    })
}
