//! Host-tool detection and platform queries (gamescope session, Flatpak and
//! `unshare --net` capabilities).

mod probe;

#[cfg(test)]
pub(crate) mod fake;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use crosshook_core::launch::LaunchPlatformCapabilities;
use crosshook_core::onboarding::HostToolDetails;

use crate::jobs::{JobRegistry, JobSpec};
use crate::presenter::display_path_tilde;
use crate::services::{run_blocking, run_job};
use crate::AppError;

pub use probe::{CoreHostProbe, HostProbe};

/// Async host-tools service. Clones are cheap.
#[derive(Clone)]
pub struct HostToolsService {
    jobs: JobRegistry,
    probe: Arc<dyn HostProbe>,
}

impl HostToolsService {
    /// Builds the service over the live host probe.
    pub fn new(jobs: JobRegistry) -> Self {
        Self::with_probe(jobs, Arc::new(CoreHostProbe))
    }

    /// Builds the service over an injected host probe (tests, fixtures).
    pub fn with_probe(jobs: JobRegistry, probe: Arc<dyn HostProbe>) -> Self {
        Self { jobs, probe }
    }

    /// Probes a catalog tool off the caller thread and redacts its display path.
    pub async fn tool_details(&self, tool_id: String) -> Result<HostToolDetails, AppError> {
        let probe = self.probe.clone();
        run_job(&self.jobs, JobSpec::new("host-tool-details"), move || {
            let mut details = probe.tool_details(&tool_id);
            details.resolved_path = details.resolved_path.map(|path| display_path_tilde(&path));
            Ok(details)
        })
        .await
    }

    /// Reports whether the current session is already inside gamescope.
    pub fn gamescope_session(&self) -> bool {
        crosshook_core::launch::is_inside_gamescope_session()
    }

    /// Reads launch platform capabilities on the blocking pool.
    ///
    /// The first call may probe `unshare --net` through the core host gateway.
    pub async fn platform_status(&self) -> Result<LaunchPlatformCapabilities, AppError> {
        run_blocking(|| Ok(crosshook_core::launch::launch_platform_capabilities())).await
    }
}
