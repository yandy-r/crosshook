//! Read-only status queries: the repair status of a prefix and the cached
//! dependency states of a profile.

use crosshook_core::metadata::PrefixDependencyStateRow;
use crosshook_core::prefix_deps::{DependencyState, PrefixDependencyStatus};

use super::{PrefixDepsService, PrefixVersionRepairStatus};
use crate::services::process::prefix_job_key;
use crate::services::run_blocking;
use crate::AppError;

impl PrefixDepsService {
    /// The Windows-version repair status of `prefix_path`.
    ///
    /// The raw path is normalized to its canonical `pfx`, so aliases share one
    /// status (I5). `in_progress` reports whether a job (install, repair or
    /// recovery) holds the prefix right now; `last_recovery` is the last
    /// automatic recovery of the prefix in this session.
    ///
    /// # Errors
    /// `Validation { field: "prefix_path" }` for a missing prefix or a corrupt
    /// row, and the store's load error.
    pub async fn repair_status(
        &self,
        prefix_path: String,
    ) -> Result<PrefixVersionRepairStatus, AppError> {
        let service = self.clone();
        run_blocking(move || {
            let (canonical, key) = prefix_job_key(&prefix_path)?;
            let mut status = service.journal.status(&canonical)?;
            status.in_progress = service.jobs.is_key_held(&key);
            status.last_recovery = service.recoveries.get(&canonical);
            Ok(status)
        })
        .await
    }

    /// The cached dependency states of `profile_name` for `prefix_path` (the
    /// raw path the states were recorded with). An unindexed profile is
    /// looked up under its name, as before.
    ///
    /// # Errors
    /// The metadata store's load error.
    pub async fn dependency_status(
        &self,
        profile_name: String,
        prefix_path: String,
    ) -> Result<Vec<PrefixDependencyStatus>, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || {
            let profile_id = metadata
                .lookup_profile_id(&profile_name)
                .ok()
                .flatten()
                .unwrap_or(profile_name);
            Ok(metadata
                .load_prefix_dep_states(&profile_id)?
                .into_iter()
                .filter(|row| row.prefix_path == prefix_path)
                .map(dependency_status_from_row)
                .collect())
        })
        .await
    }
}

fn dependency_status_from_row(row: PrefixDependencyStateRow) -> PrefixDependencyStatus {
    PrefixDependencyStatus {
        package_name: row.package_name,
        state: match row.state.as_str() {
            "installed" => DependencyState::Installed,
            "missing" => DependencyState::Missing,
            "install_failed" => DependencyState::InstallFailed,
            "check_failed" => DependencyState::CheckFailed,
            "user_skipped" => DependencyState::UserSkipped,
            _ => DependencyState::Unknown,
        },
        checked_at: row.checked_at,
        installed_at: row.installed_at,
        last_error: row.last_error,
    }
}
