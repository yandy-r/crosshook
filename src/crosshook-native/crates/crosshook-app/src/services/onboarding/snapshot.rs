use std::collections::HashSet;

use crosshook_core::metadata::{HostReadinessSnapshotRow, MetadataStore};
use crosshook_core::onboarding::{HostToolCheckResult, ReadinessCheckResult};
use serde::{Deserialize, Serialize};

use super::OnboardingService;
use crate::presenter::display_path_tilde;
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

/// Last persisted host probe results, with display-safe paths and dismissed guidance removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedHostReadinessSnapshot {
    /// RFC3339 timestamp of the probe.
    pub checked_at: String,
    /// Distribution family detected by the probe.
    pub detected_distro_family: String,
    /// Per-tool readiness results.
    pub tool_checks: Vec<HostToolCheckResult>,
    /// Whether all readiness checks passed.
    pub all_passed: bool,
    /// Number of critical failures.
    pub critical_failures: usize,
    /// Number of warnings.
    pub warnings: usize,
}

impl OnboardingService {
    /// Loads the last host snapshot without probing; disabled metadata means no snapshot.
    pub async fn cached_host_readiness_snapshot(
        &self,
    ) -> Result<Option<CachedHostReadinessSnapshot>, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || load_cached_host_readiness_snapshot(&metadata)).await
    }
}

pub(super) fn sanitize_tool_checks(
    tool_checks: Vec<HostToolCheckResult>,
) -> Vec<HostToolCheckResult> {
    tool_checks
        .into_iter()
        .map(|mut tool_check| {
            tool_check.resolved_path = tool_check.resolved_path.as_deref().map(display_path_tilde);
            tool_check
        })
        .collect()
}

fn apply_tool_guidance_dismissals(
    tool_checks: Vec<HostToolCheckResult>,
    dismissed_tool_ids: &HashSet<String>,
) -> Vec<HostToolCheckResult> {
    tool_checks
        .into_iter()
        .map(|mut tool_check| {
            if dismissed_tool_ids.contains(&tool_check.tool_id) {
                tool_check.install_guidance = None;
            }
            tool_check
        })
        .collect()
}

pub(super) fn snapshot_row_to_payload(
    row: HostReadinessSnapshotRow,
) -> Result<CachedHostReadinessSnapshot, AppError> {
    let tool_checks: Vec<HostToolCheckResult> = serde_json::from_str(&row.tool_results_json)
        .map_err(|error| {
            AppError::new(
                AppErrorKind::Internal,
                format!("failed to parse cached host readiness snapshot: {error}"),
            )
        })?;
    let critical_failures = usize::try_from(row.critical_failures).map_err(|_| {
        AppError::new(
            AppErrorKind::Internal,
            format!(
                "invalid cached host readiness snapshot critical_failures value: {}",
                row.critical_failures
            ),
        )
    })?;
    let warnings = usize::try_from(row.warnings).map_err(|_| {
        AppError::new(
            AppErrorKind::Internal,
            format!(
                "invalid cached host readiness snapshot warnings value: {}",
                row.warnings
            ),
        )
    })?;

    Ok(CachedHostReadinessSnapshot {
        checked_at: row.checked_at,
        detected_distro_family: row.detected_distro_family,
        tool_checks: sanitize_tool_checks(tool_checks),
        all_passed: row.all_passed,
        critical_failures,
        warnings,
    })
}

pub(super) fn load_cached_host_readiness_snapshot(
    metadata: &MetadataStore,
) -> Result<Option<CachedHostReadinessSnapshot>, AppError> {
    if !metadata.is_available() {
        return Ok(None);
    }
    let dismissed_tool_ids = metadata.get_dismissed_readiness_nags()?;
    metadata
        .get_host_readiness_snapshot()?
        .map(snapshot_row_to_payload)
        .map(|payload_result| {
            payload_result.map(|mut payload| {
                payload.tool_checks =
                    apply_tool_guidance_dismissals(payload.tool_checks, &dismissed_tool_ids);
                payload
            })
        })
        .transpose()
}

pub(super) fn readiness_result_from_snapshot(
    snapshot: CachedHostReadinessSnapshot,
) -> ReadinessCheckResult {
    ReadinessCheckResult {
        checks: Vec::new(),
        all_passed: snapshot.all_passed,
        critical_failures: snapshot.critical_failures,
        warnings: snapshot.warnings,
        umu_install_guidance: None,
        steam_deck_caveats: None,
        tool_checks: snapshot.tool_checks,
        detected_distro_family: snapshot.detected_distro_family,
    }
}
