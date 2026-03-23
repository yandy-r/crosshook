//! Profile data models and profile persistence helpers.

mod legacy;
mod models;
mod toml_store;

pub use models::{
    GameProfile, GameSection, InjectionSection, LaunchSection, LegacyProfileData, LauncherSection,
    SteamSection, TrainerSection,
};
pub use legacy::{delete, list, load, save, validate_name};
pub use toml_store::{ProfileStore, ProfileStoreError};
