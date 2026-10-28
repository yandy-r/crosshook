use crosshook_core::community::CommunityTapSyncResult;
use crosshook_core::metadata::MetadataStore;

use crate::jobs::JobContext;
use crate::AppError;

pub(super) fn finalize_sync_results(
    metadata: &MetadataStore,
    mut results: Vec<CommunityTapSyncResult>,
    ctx: &JobContext,
) -> Result<Vec<CommunityTapSyncResult>, AppError> {
    for result in &results {
        check_cancelled(ctx)?;
        if metadata.index_community_tap_result(result).is_err() {
            tracing::warn!("community tap index sync failed");
        }
        check_cancelled(ctx)?;
        if metadata
            .upsert_community_tap_offline_from_sync_result(result)
            .is_err()
        {
            tracing::warn!("community tap offline state upsert failed");
        }
    }
    for result in &mut results {
        check_cancelled(ctx)?;
        let subscription = &result.workspace.subscription;
        match metadata.lookup_community_tap_id(
            &subscription.url,
            subscription.branch.as_deref().unwrap_or(""),
        ) {
            Ok(Some(id)) => {
                check_cancelled(ctx)?;
                match metadata.lookup_community_tap_offline_state_row(&id) {
                    Ok(Some(row)) => result.last_sync_at = row.last_sync_at,
                    Ok(None) => {}
                    Err(_) => tracing::warn!("community tap sync timestamp lookup failed"),
                }
            }
            Ok(None) => {}
            Err(_) => tracing::warn!("community tap id lookup failed"),
        }
        result.workspace.subscription.extra.clear();
    }
    check_cancelled(ctx)?;
    Ok(results)
}

pub(super) fn check_cancelled(ctx: &JobContext) -> Result<(), AppError> {
    if ctx.is_cancelled() {
        Err(AppError::cancelled())
    } else {
        Ok(())
    }
}
