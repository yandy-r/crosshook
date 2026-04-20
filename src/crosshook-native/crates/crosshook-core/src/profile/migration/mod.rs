mod apply;
mod proton;
mod scan;
mod types;

pub use apply::apply_single_migration;
pub use proton::{
    extract_name_from_proton_path, extract_proton_family, extract_version_segments,
    find_best_replacement,
};
pub use scan::scan_proton_migrations;
pub use types::{
    ApplyMigrationRequest, BatchMigrationRequest, BatchMigrationResult, MigrationApplyResult,
    MigrationOutcome, MigrationScanResult, MigrationSuggestion, ProtonInstallInfo, ProtonPathField,
    UnmatchedProfile,
};

#[cfg(test)]
mod tests;
