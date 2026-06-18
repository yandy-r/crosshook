//! Profile data models and profile persistence helpers.

mod collection_exchange;
pub mod collection_schema;
pub mod community_schema;
pub mod config_diff;
pub mod config_semantic_diff;
mod creation_defaults;
mod exchange;
mod legacy;
mod models;
mod toml_store;

pub use collection_exchange::{
    export_collection_preset_to_toml, preview_collection_preset_import, CollectionExchangeError,
    CollectionExportResult, CollectionImportPreview, CollectionPresetAmbiguousEntry,
    CollectionPresetMatchCandidate, CollectionPresetMatchedEntry,
};
pub use collection_schema::{
    CollectionPresetManifest, CollectionPresetProfileDescriptor, COLLECTION_PRESET_SCHEMA_VERSION,
};
pub use community_schema::{
    CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating,
    COMMUNITY_PROFILE_SCHEMA_VERSION,
};
pub use config_diff::{
    cap_diff_output_bytes, compute_unified_diff, UnifiedDiffResult, DIFF_CONTEXT_LINES,
    DIFF_MAX_LINES, MAX_DIFF_OUTPUT_BYTES,
};
pub use config_semantic_diff::{
    compute_semantic_diff, SemanticChange, SemanticChangeKind, SemanticDiffResult,
    MAX_SEMANTIC_CHANGES,
};
pub use creation_defaults::apply_profile_creation_defaults_from_settings;
pub use exchange::{
    export_community_profile, import_community_profile, preview_community_profile_import,
    CommunityExchangeError, CommunityExportResult, CommunityImportPreview, CommunityImportResult,
};
pub use legacy::{delete, list, load, save, validate_name};
pub use models::{
    resolve_art_app_id, resolve_launch_method, validate_steam_app_id, CollectionDefaultsSection,
    GameProfile, GameSection, GamescopeConfig, GamescopeFilter, HookStage, InjectionFallback,
    InjectionMethod, InjectionSection, InjectionStage, LaunchHook, LaunchOptimizationsSection,
    LaunchSection, LauncherSection, LegacyProfileData, LoadedDllHook, LocalOverrideGameSection,
    LocalOverrideRuntimeSection, LocalOverrideSection, LocalOverrideSteamSection,
    LocalOverrideTrainerSection, MangoHudConfig, MangoHudPosition, RuntimeSection, SteamSection,
    TrainerLoadingMode, TrainerSection,
};
pub use toml_store::{
    bundled_optimization_preset_toml_key, profile_to_shareable_toml,
    profile_to_shareable_toml_with_options, DuplicateProfileResult, ProfileStore,
    ProfileStoreError, ShareableTomlOptions,
};
pub mod health;
pub mod mangohud;
pub use health::{
    HealthCheckSummary, HealthIssue, HealthIssueSeverity, HealthStatus, ProfileHealthReport,
};
pub mod migration;
