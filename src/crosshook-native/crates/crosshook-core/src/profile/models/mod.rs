//! Profile data models — structs, enums, serde shapes, and the core `GameProfile` type.

mod game_meta;
mod gamescope;
mod launch;
mod legacy;
mod local_override;
mod mangohud;
mod profile;
mod resolve;
mod runtime;
mod trainer;

#[cfg(test)]
mod tests;

pub use game_meta::{GameSection, InjectionSection, LauncherSection, SteamSection};
pub use gamescope::{GamescopeConfig, GamescopeFilter};
pub use launch::{CollectionDefaultsSection, LaunchOptimizationsSection, LaunchSection};
pub use legacy::LegacyProfileData;
pub use local_override::{
    LocalOverrideGameSection, LocalOverrideRuntimeSection, LocalOverrideSection,
    LocalOverrideSteamSection, LocalOverrideTrainerSection,
};
pub use mangohud::{MangoHudConfig, MangoHudPosition};
pub use profile::GameProfile;
pub use resolve::{resolve_art_app_id, resolve_launch_method, validate_steam_app_id};
pub use runtime::RuntimeSection;
pub use trainer::{TrainerLoadingMode, TrainerSection};
