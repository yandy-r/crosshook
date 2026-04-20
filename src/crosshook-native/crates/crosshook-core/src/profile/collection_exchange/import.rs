//! Collection preset TOML import-preview (parse + classify against local profiles).

use std::fs;
use std::path::Path;

use crate::profile::collection_schema::{
    CollectionPresetManifest, COLLECTION_PRESET_SCHEMA_VERSION,
};
use crate::profile::ProfileStore;

use super::error::CollectionExchangeError;
use super::matching::{
    build_local_match_index, candidates_for_names, classify_descriptor, MatchClass,
};
use super::types::{
    CollectionImportPreview, CollectionPresetAmbiguousEntry, CollectionPresetMatchedEntry,
};

/// Parses and validates a preset file, then classifies each descriptor against local profiles.
pub fn preview_collection_preset_import(
    profile_store: &ProfileStore,
    path: &Path,
) -> Result<CollectionImportPreview, CollectionExchangeError> {
    let content = fs::read_to_string(path).map_err(|error| CollectionExchangeError::Io {
        action: "read the collection preset file".to_string(),
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;

    let manifest = parse_collection_preset_toml(&content, path)?;
    let index = build_local_match_index(profile_store)?;

    let mut matched = Vec::new();
    let mut ambiguous = Vec::new();
    let mut unmatched = Vec::new();

    for d in &manifest.profiles {
        match classify_descriptor(d, &index) {
            MatchClass::Matched { profile } => {
                matched.push(CollectionPresetMatchedEntry {
                    descriptor: d.clone(),
                    local_profile_name: profile,
                });
            }
            MatchClass::Ambiguous { names } => {
                ambiguous.push(CollectionPresetAmbiguousEntry {
                    descriptor: d.clone(),
                    candidates: candidates_for_names(&names, &index.profile_display),
                });
            }
            MatchClass::Unmatched => unmatched.push(d.clone()),
        }
    }

    Ok(CollectionImportPreview {
        source_path: path.to_path_buf(),
        manifest,
        matched,
        ambiguous,
        unmatched,
    })
}

fn parse_collection_preset_toml(
    content: &str,
    path: &Path,
) -> Result<CollectionPresetManifest, CollectionExchangeError> {
    let manifest: CollectionPresetManifest =
        toml::from_str(content).map_err(|e| CollectionExchangeError::Toml {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

    manifest.validate().map_err(|msg| {
        if !manifest.schema_version.is_empty()
            && manifest.schema_version != COLLECTION_PRESET_SCHEMA_VERSION
        {
            CollectionExchangeError::UnsupportedSchemaVersion {
                version: manifest.schema_version.clone(),
                supported: COLLECTION_PRESET_SCHEMA_VERSION.to_string(),
            }
        } else {
            CollectionExchangeError::InvalidManifest { message: msg }
        }
    })?;

    Ok(manifest)
}
