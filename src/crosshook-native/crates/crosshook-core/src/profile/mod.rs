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
    export_community_profile, import_community_profile, preview_community_profile_import,
    CommunityExchangeError, CommunityExportResult, CommunityImportPreview, CommunityImportResult,
};
pub use legacy::{delete, list, load, save, validate_name};
pub use models::{
    resolve_art_app_id, resolve_launch_method, validate_steam_app_id, GameProfile, GameSection,
    GamescopeConfig, GamescopeFilter, InjectionSection, LaunchOptimizationsSection, LaunchSection,
    LauncherSection, LegacyProfileData, LocalOverrideGameSection, LocalOverrideRuntimeSection,
    LocalOverrideSection, LocalOverrideSteamSection, LocalOverrideTrainerSection, MangoHudConfig,
    MangoHudPosition, RuntimeSection, SteamSection, TrainerLoadingMode, TrainerSection,
};
pub use toml_store::{
    bundled_optimization_preset_toml_key, profile_to_shareable_toml, DuplicateProfileResult,
    ProfileStore, ProfileStoreError,
};
pub mod health;
pub mod mangohud;
pub use health::{
    HealthCheckSummary, HealthIssue, HealthIssueSeverity, HealthStatus, ProfileHealthReport,
};
pub mod migration;
