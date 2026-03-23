//! Profile data models and profile persistence helpers.

mod legacy;
mod models;
mod toml_store;

pub use legacy::{delete, list, load, save, validate_name};
pub use models::{
    GameProfile, GameSection, InjectionSection, LaunchSection, LauncherSection, LegacyProfileData,
    SteamSection, TrainerSection,
};
pub use toml_store::{ProfileStore, ProfileStoreError};
