//! Collection preset TOML export.

use std::fs;
use std::path::Path;

use crate::metadata::MetadataStore;
use crate::profile::collection_schema::{
    CollectionPresetManifest, CollectionPresetProfileDescriptor, COLLECTION_PRESET_SCHEMA_VERSION,
};
use crate::profile::{resolve_art_app_id, GameProfile, ProfileStore, ProfileStoreError};

use super::error::CollectionExchangeError;
use super::types::CollectionExportResult;

/// Writes a collection preset TOML for the given collection id.
pub fn export_collection_preset_to_toml(
    metadata_store: &MetadataStore,
    profile_store: &ProfileStore,
    collection_id: &str,
    output_path: &Path,
) -> Result<CollectionExportResult, CollectionExchangeError> {
    let rows = metadata_store.list_collections()?;
    let row = rows
        .into_iter()
        .find(|r| r.collection_id == collection_id)
        .ok_or_else(|| CollectionExchangeError::InvalidManifest {
            message: format!("collection not found: {collection_id}"),
        })?;

    let description = row.description.as_ref().and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    });

    let defaults = metadata_store
        .get_collection_defaults(collection_id)?
        .filter(|d| !d.is_empty());

    let member_names = metadata_store.list_profiles_in_collection(collection_id)?;
    let mut descriptors = Vec::with_capacity(member_names.len());

    for name in &member_names {
        let profile = profile_store.load(name).map_err(|e| match e {
            ProfileStoreError::NotFound(path) => CollectionExchangeError::InvalidManifest {
                message: format!(
                    "profile file for collection member {name:?} is missing: {}",
                    path.display()
                ),
            },
            other => other.into(),
        })?;

        descriptors.push(descriptor_from_profile(&profile));
    }

    let manifest = CollectionPresetManifest {
        schema_version: COLLECTION_PRESET_SCHEMA_VERSION.to_string(),
        name: row.name,
        description,
        defaults,
        profiles: descriptors,
    };

    write_preset_toml(output_path, &manifest)?;

    Ok(CollectionExportResult {
        collection_id: collection_id.to_string(),
        output_path: output_path.to_path_buf(),
        manifest,
    })
}

fn descriptor_from_profile(profile: &GameProfile) -> CollectionPresetProfileDescriptor {
    CollectionPresetProfileDescriptor {
        steam_app_id: resolve_art_app_id(profile).to_string(),
        game_name: profile.game.name.clone(),
        trainer_community_trainer_sha256: profile.trainer.community_trainer_sha256.clone(),
    }
}

pub(super) fn write_preset_toml(
    output_path: &Path,
    manifest: &CollectionPresetManifest,
) -> Result<(), CollectionExchangeError> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CollectionExchangeError::Io {
            action: "create the collection preset export directory".to_string(),
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }

    let body = toml::to_string_pretty(manifest).map_err(|e| CollectionExchangeError::Toml {
        path: output_path.to_path_buf(),
        message: e.to_string(),
    })?;

    let out = format!(
        "# CrossHook collection preset\n\
         # https://github.com/yandy-r/crosshook\n\
         #\n\
         \n\
         {body}"
    );

    fs::write(output_path, out).map_err(|error| CollectionExchangeError::Io {
        action: "write the collection preset file".to_string(),
        path: output_path.to_path_buf(),
        message: error.to_string(),
    })
}
