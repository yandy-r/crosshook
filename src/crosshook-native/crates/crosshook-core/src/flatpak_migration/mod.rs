//! Flatpak first-run migration: import host AppImage data into the sandbox
//! on first launch. See `docs/prps/plans/flatpak-isolation.plan.md` and
//! `docs/architecture/adr-0003-flatpak-per-app-isolation.md` (forthcoming).

mod copier;
mod detector;
mod prefix_root;
mod types;

pub use types::{
    FlatpakMigrationError, MigrationOutcome, CONFIG_ROOT_SEGMENT, DATA_INCLUDE_FILES,
    DATA_INCLUDE_SUBTREES, DATA_SKIP_SUBTREES,
};

/// Placeholder — real implementation lands in Task 3.1.
pub fn run() -> Result<MigrationOutcome, FlatpakMigrationError> {
    unimplemented!("flatpak_migration::run wired up in Task 3.1")
}
