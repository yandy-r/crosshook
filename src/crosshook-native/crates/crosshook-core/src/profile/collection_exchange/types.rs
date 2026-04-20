//! Public DTOs for collection preset export and import-preview results.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::profile::collection_schema::{
    CollectionPresetManifest, CollectionPresetProfileDescriptor,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionExportResult {
    pub collection_id: String,
    pub output_path: PathBuf,
    pub manifest: CollectionPresetManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionPresetMatchCandidate {
    pub profile_name: String,
    pub game_name: String,
    pub steam_app_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionPresetMatchedEntry {
    pub descriptor: CollectionPresetProfileDescriptor,
    pub local_profile_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionPresetAmbiguousEntry {
    pub descriptor: CollectionPresetProfileDescriptor,
    pub candidates: Vec<CollectionPresetMatchCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionImportPreview {
    pub source_path: PathBuf,
    pub manifest: CollectionPresetManifest,
    pub matched: Vec<CollectionPresetMatchedEntry>,
    pub ambiguous: Vec<CollectionPresetAmbiguousEntry>,
    pub unmatched: Vec<CollectionPresetProfileDescriptor>,
}
