//! Profile data models and profile persistence helpers.

pub mod community_schema;
mod exchange;
mod legacy;
mod models;
mod toml_store;

pub use community_schema::{
    CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating,
    COMMUNITY_PROFILE_SCHEMA_VERSION,
};
pub use exchange::{
    export_community_profile, import_community_profile, CommunityExchangeError,
    CommunityExportResult, CommunityImportResult,
};
pub use legacy::{delete, list, load, save, validate_name};
pub use models::{
    GameProfile, GameSection, InjectionSection, LaunchOptimizationsSection, LaunchSection,
    LauncherSection, LegacyProfileData, RuntimeSection, SteamSection, TrainerLoadingMode,
    TrainerSection,
};
pub use toml_store::{ProfileStore, ProfileStoreError};
