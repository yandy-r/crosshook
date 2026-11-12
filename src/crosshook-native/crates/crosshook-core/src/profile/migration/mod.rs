mod apply;
mod proton;
mod scan;
mod types;

pub use apply::prepare_profile_migration;
pub use scan::scan_proton_migrations;
pub use types::{
    ApplyMigrationRequest, BatchMigrationRequest, BatchMigrationResult, MigrationApplyResult,
    MigrationOutcome, MigrationScanResult, MigrationSuggestion, ProtonInstallInfo, ProtonPathField,
    UnmatchedProfile,
};

#[cfg(test)]
mod tests;
