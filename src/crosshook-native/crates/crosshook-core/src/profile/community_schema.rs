use serde::{Deserialize, Serialize};

use crate::profile::GameProfile;

pub const COMMUNITY_PROFILE_SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    COMMUNITY_PROFILE_SCHEMA_VERSION
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityRating {
    #[default]
    Unknown,
    Broken,
    Partial,
    Working,
    Platinum,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CommunityProfileMetadata {
    #[serde(default)]
    pub game_name: String,
    #[serde(default)]
    pub game_version: String,
    #[serde(default)]
    pub trainer_name: String,
    #[serde(default)]
    pub trainer_version: String,
    #[serde(default)]
    pub proton_version: String,
    #[serde(default)]
    pub platform_tags: Vec<String>,
    #[serde(default)]
    pub compatibility_rating: CompatibilityRating,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityProfileManifest {
    #[serde(
        default = "default_schema_version",
        rename = "schema_version",
        skip_serializing_if = "is_default_schema_version"
    )]
    pub schema_version: u32,
    #[serde(default)]
    pub metadata: CommunityProfileMetadata,
    #[serde(default)]
    pub profile: GameProfile,
}

impl Default for CommunityProfileManifest {
    fn default() -> Self {
        Self {
            schema_version: COMMUNITY_PROFILE_SCHEMA_VERSION,
            metadata: CommunityProfileMetadata::default(),
            profile: GameProfile::default(),
        }
    }
}

fn is_default_schema_version(value: &u32) -> bool {
    *value == COMMUNITY_PROFILE_SCHEMA_VERSION
}

impl CommunityProfileManifest {
    pub fn new(metadata: CommunityProfileMetadata, profile: GameProfile) -> Self {
        Self {
            schema_version: COMMUNITY_PROFILE_SCHEMA_VERSION,
            metadata,
            profile,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_current_schema_version() {
        let manifest = CommunityProfileManifest::default();

        assert_eq!(manifest.schema_version, COMMUNITY_PROFILE_SCHEMA_VERSION);
        assert_eq!(manifest.metadata, CommunityProfileMetadata::default());
        assert_eq!(manifest.profile, GameProfile::default());
    }

    #[test]
    fn round_trips_metadata_and_profile() {
        let manifest = CommunityProfileManifest::new(
            CommunityProfileMetadata {
                game_name: "Elden Ring".to_string(),
                game_version: "1.12.3".to_string(),
                trainer_name: "FLiNG Trainer".to_string(),
                trainer_version: "v1".to_string(),
                proton_version: "9.0-4".to_string(),
                platform_tags: vec!["steam-deck".to_string(), "linux".to_string()],
                compatibility_rating: CompatibilityRating::Platinum,
                author: "crosshook".to_string(),
                description: "Known-good launch profile".to_string(),
            },
            GameProfile::default(),
        );

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.metadata.game_name, "Elden Ring");
        assert_eq!(
            manifest.metadata.compatibility_rating,
            CompatibilityRating::Platinum
        );
        assert!(manifest
            .metadata
            .platform_tags
            .contains(&"steam-deck".to_string()));
    }
}
