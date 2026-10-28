//! Injectable host probe seam. Production delegates to `crosshook-core`, which
//! routes every host command through the platform gateway.

use crosshook_core::onboarding::readiness::{check_generalized_readiness, check_system_readiness};
use crosshook_core::onboarding::{
    global_readiness_catalog, probe_host_tool_details, HostToolDetails, ReadinessCheckResult,
};

/// Host probes used by readiness checks and host-tool details. Implementations
/// block (they may spawn host processes) and run on the blocking pool.
pub trait HostProbe: Send + Sync + 'static {
    /// Steam/Proton/umu system readiness (`check_readiness`).
    fn system_readiness(&self) -> ReadinessCheckResult;
    /// Catalog-driven generalized readiness including host tool checks.
    fn generalized_readiness(&self) -> ReadinessCheckResult;
    /// Version and resolved path of one catalog tool; unknown ids yield
    /// unavailable details.
    fn tool_details(&self, tool_id: &str) -> HostToolDetails;
}

/// Live host probe over the `crosshook-core` onboarding functions.
#[derive(Debug, Clone, Copy, Default)]
pub struct CoreHostProbe;

impl HostProbe for CoreHostProbe {
    fn system_readiness(&self) -> ReadinessCheckResult {
        check_system_readiness()
    }

    fn generalized_readiness(&self) -> ReadinessCheckResult {
        check_generalized_readiness(global_readiness_catalog())
    }

    fn tool_details(&self, tool_id: &str) -> HostToolDetails {
        probe_host_tool_details(tool_id)
    }
}
