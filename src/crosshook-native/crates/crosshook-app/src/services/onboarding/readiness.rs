use crosshook_core::onboarding::readiness::{
    apply_install_nag_dismissal, apply_readiness_nag_dismissals, apply_steam_deck_caveats_dismissal,
};
use crosshook_core::onboarding::{derive_capabilities, Capability, ReadinessCheckResult};

use super::snapshot::{
    load_cached_host_readiness_snapshot, readiness_result_from_snapshot, sanitize_tool_checks,
};
use super::OnboardingService;
use crate::jobs::JobSpec;
use crate::presenter::display_path_tilde;
use crate::services::run_job;
use crate::AppError;

impl OnboardingService {
    /// Probes legacy system readiness under a supervised job and applies saved dismissals.
    pub async fn check_readiness(&self) -> Result<ReadinessCheckResult, AppError> {
        let service = self.clone();
        run_job(&self.jobs, JobSpec::new("host-readiness"), move || {
            let settings = service.settings.load()?;
            let mut result = service.probe.system_readiness();
            service.apply_dismissals(&mut result, &settings)?;
            sanitize_readiness_paths(&mut result);
            Ok(result)
        })
        .await
    }

    /// Probes host readiness, persists the raw snapshot, then applies display overlays.
    pub async fn check_generalized_readiness(&self) -> Result<ReadinessCheckResult, AppError> {
        let service = self.clone();
        run_job(&self.jobs, JobSpec::new("host-readiness"), move || {
            let settings = service.settings.load()?;
            let mut result = service.probe.generalized_readiness();
            service.persist_snapshot(&result)?;
            service.apply_dismissals(&mut result, &settings)?;
            sanitize_readiness_paths(&mut result);
            Ok(result)
        })
        .await
    }

    /// Derives capabilities from the cached snapshot, or probes and caches when absent.
    pub async fn capabilities(&self) -> Result<Vec<Capability>, AppError> {
        let service = self.clone();
        run_job(&self.jobs, JobSpec::new("host-readiness"), move || {
            if let Some(snapshot) = load_cached_host_readiness_snapshot(&service.metadata)? {
                return Ok(sanitize_capabilities(derive_capabilities(
                    &readiness_result_from_snapshot(snapshot),
                )));
            }
            // The live capability fallback deliberately has no dismissal overlays,
            // matching the existing command's behaviour.
            let mut live = service.probe.generalized_readiness();
            service.persist_snapshot(&live)?;
            live.tool_checks = sanitize_tool_checks(live.tool_checks);
            Ok(sanitize_capabilities(derive_capabilities(&live)))
        })
        .await
    }

    fn persist_snapshot(&self, result: &ReadinessCheckResult) -> Result<(), AppError> {
        if self.metadata.is_available() {
            self.metadata.upsert_host_readiness_snapshot(
                &result.tool_checks,
                &result.detected_distro_family,
                result.all_passed,
                result.critical_failures,
                result.warnings,
            )?;
        }
        Ok(())
    }

    fn apply_dismissals(
        &self,
        result: &mut ReadinessCheckResult,
        settings: &crosshook_core::settings::AppSettingsData,
    ) -> Result<(), AppError> {
        apply_install_nag_dismissal(result, &settings.install_nag_dismissed_at);
        apply_steam_deck_caveats_dismissal(result, &settings.steam_deck_caveats_dismissed_at);
        if self.metadata.is_available() {
            let dismissed = self.metadata.get_dismissed_readiness_nags()?;
            apply_readiness_nag_dismissals(result, &dismissed);
        }
        Ok(())
    }
}

fn sanitize_readiness_paths(result: &mut ReadinessCheckResult) {
    for issue in &mut result.checks {
        issue.path = display_path_tilde(&issue.path);
    }
}

fn sanitize_capabilities(capabilities: Vec<Capability>) -> Vec<Capability> {
    capabilities
        .into_iter()
        .map(|mut capability| {
            capability.missing_required = sanitize_tool_checks(capability.missing_required);
            capability.missing_optional = sanitize_tool_checks(capability.missing_optional);
            capability
        })
        .collect()
}
