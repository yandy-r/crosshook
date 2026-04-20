//! Flatpak first-run migration: import host AppImage data into the sandbox
//! on first launch. See `docs/prps/plans/flatpak-isolation.plan.md` and
//! `docs/architecture/adr-0003-flatpak-per-app-isolation.md` (forthcoming).

mod copier;
mod detector;
mod prefix_root;
mod types;

pub use prefix_root::host_prefix_root;
pub use types::{
    FlatpakMigrationError, MigrationOutcome, CONFIG_ROOT_SEGMENT, DATA_INCLUDE_FILES,
    DATA_INCLUDE_SUBTREES, DATA_SKIP_SUBTREES,
};

#[allow(unused_imports)] // consumed by tasks 4.1 and 4.2
pub(crate) use prefix_root::{host_prefix_root_with, is_isolation_mode_active};

/// Placeholder — real implementation lands in Task 3.1.
pub fn run() -> Result<MigrationOutcome, FlatpakMigrationError> {
    unimplemented!("flatpak_migration::run wired up in Task 3.1")
}
