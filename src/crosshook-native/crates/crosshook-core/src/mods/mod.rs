//! Per-profile mod coexistence registry: models, read-only detection scan,
//! and pure trainer-coexistence advisory analysis (Forgejo #28).
//!
//! Persistence lives in `crate::metadata` (`mods_store` / `mods_ops`);
//! launch wiring lives in `crate::launch::mod_coexistence`.

mod coexistence;
mod detection;
mod model;

#[cfg(test)]
mod tests;

pub use coexistence::{analyze_mod_coexistence, CoexistenceAdvisory, CoexistenceContext};
pub use detection::{
    mark_already_registered, scan_game_directory, DetectedModCandidate, DetectionScanReport,
    ModDetectionError,
};
pub use model::{ModCategory, ModProvenance, ProfileModInput, ProfileModRecord};
