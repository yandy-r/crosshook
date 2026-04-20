use super::super::community_schema::CommunityProfileManifest;
use super::super::GameProfile;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityImportResult {
    pub profile_name: String,
    pub source_path: PathBuf,
    pub profile_path: PathBuf,
    pub profile: GameProfile,
    pub manifest: CommunityProfileManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityImportPreview {
    pub profile_name: String,
    pub source_path: PathBuf,
    pub profile: GameProfile,
    pub manifest: CommunityProfileManifest,
    pub required_prefix_deps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityExportResult {
    pub profile_name: String,
    pub output_path: PathBuf,
    pub manifest: CommunityProfileManifest,
}
