use std::fs;
use std::path::Path;

use super::helpers::Harness;

#[tokio::test]
async fn cleanup_deletes_orphan_and_stale_trainer_and_deduplicates_requests() {
    let harness = Harness::new(true);
    let orphan = harness.orphan();
    let staged = harness.staged(true);

    let result = harness
        .service
        .cleanup(vec![
            orphan.clone(),
            staged.clone(),
            orphan.clone(),
            staged.clone(),
        ])
        .await
        .unwrap();

    assert_eq!(result.deleted, vec![orphan.clone(), staged.clone()]);
    assert!(result.skipped.is_empty());
    assert!(result.reclaimed_bytes > 0);
    assert!(!Path::new(&orphan.target_path).exists());
    assert!(!Path::new(&staged.target_path).exists());
    assert!(harness.referenced.is_dir());
    let rows = harness.service.history().await.unwrap().audit;
    assert_eq!(rows.len(), 3);
    for target in [&orphan, &staged] {
        let row = rows
            .iter()
            .find(|row| row.target_path == target.target_path)
            .unwrap();
        assert_eq!(row.result, "deleted");
        assert!(row.reason.is_none());
        assert_eq!(row.reclaimed_bytes, 0);
        assert_eq!(row.resolved_prefix_path, target.resolved_prefix_path);
    }
    let summary = rows
        .iter()
        .find(|row| row.target_kind == "summary")
        .unwrap();
    assert_eq!(summary.reclaimed_bytes, result.reclaimed_bytes as i64);
    assert!(summary.target_path.is_empty());
    assert!(summary.resolved_prefix_path.is_empty());
    assert!(rows.iter().all(|row| row.created_at == summary.created_at));
    assert_eq!(
        rows.iter()
            .filter(|row| row.target_kind == "orphan_prefix")
            .count(),
        1
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.target_kind == "stale_staged_trainer")
            .count(),
        1
    );
}

#[tokio::test]
async fn missing_fresh_scan_target_is_skipped_once_and_audited_without_summary() {
    let harness = Harness::new(true);
    // This newly-created trainer is not stale and must not be authorized.
    let target = harness.staged(false);
    let result = harness
        .service
        .cleanup(vec![target.clone(), target.clone()])
        .await
        .unwrap();
    assert!(result.deleted.is_empty());
    assert_eq!(result.reclaimed_bytes, 0);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].target, target);
    assert_eq!(
        result.skipped[0].reason,
        "not present in a fresh storage scan"
    );
    assert!(Path::new(&target.target_path).is_dir());
    let rows = harness.service.history().await.unwrap().audit;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].target_kind, "stale_staged_trainer");
    assert_eq!(rows[0].result, "skipped");
    assert_eq!(
        rows[0].reason.as_deref(),
        Some("not present in a fresh storage scan")
    );
    assert_eq!(rows[0].target_path, target.target_path);
}

#[tokio::test]
async fn cleanup_does_not_trust_an_old_job_result_when_inventory_changes() {
    let harness = Harness::new(true);
    let orphan = harness.orphan();
    let scan = harness.service.scan().await.unwrap();
    assert_eq!(scan.orphan_targets, vec![orphan.clone()]);
    let mut profile = harness.profiles.load("Active").unwrap();
    profile.runtime.prefix_path = orphan.resolved_prefix_path.clone();
    harness.profiles.save("Now referenced", &profile).unwrap();

    let result = harness.service.cleanup(scan.orphan_targets).await.unwrap();

    assert!(result.deleted.is_empty());
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(
        result.skipped[0].reason,
        "not present in a fresh storage scan"
    );
    assert!(Path::new(&orphan.target_path).is_dir());
}

#[tokio::test]
async fn cleanup_discovers_targets_without_a_prior_scan_job() {
    let harness = Harness::new(true);
    let orphan = harness.orphan();
    let result = harness.service.cleanup(vec![orphan.clone()]).await.unwrap();
    assert_eq!(result.deleted, vec![orphan]);
    // Cleanup scans authorize only; snapshot history is written by scan jobs.
    assert!(harness
        .service
        .history()
        .await
        .unwrap()
        .snapshots
        .is_empty());
}

#[tokio::test]
async fn cleanup_with_disabled_metadata_still_deletes_and_reports_skips() {
    let harness = Harness::new(false);
    let orphan = harness.orphan();
    let fresh = harness.staged(false);
    let result = harness
        .service
        .cleanup(vec![orphan.clone(), fresh.clone()])
        .await
        .unwrap();
    assert_eq!(result.deleted, vec![orphan.clone()]);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].target, fresh);
    assert_eq!(
        result.skipped[0].reason,
        "not present in a fresh storage scan"
    );
    assert!(!Path::new(&orphan.target_path).exists());
    let history = harness.service.history().await.unwrap();
    assert!(!history.available);
    assert!(history.audit.is_empty());
}

#[tokio::test]
async fn audit_write_failure_does_not_undo_or_fail_cleanup() {
    let harness = Harness::new(true);
    let orphan = harness.orphan();
    super::helpers::execute_sql(&harness.metadata, "DROP TABLE prefix_storage_cleanup_audit");
    let result = harness.service.cleanup(vec![orphan.clone()]).await.unwrap();
    assert_eq!(result.deleted, vec![orphan.clone()]);
    assert!(!Path::new(&orphan.target_path).exists());
}

#[tokio::test]
async fn cleanup_with_no_targets_writes_no_audit_rows() {
    let harness = Harness::new(true);
    let result = harness.service.cleanup(vec![]).await.unwrap();
    assert!(result.deleted.is_empty());
    assert!(result.skipped.is_empty());
    assert_eq!(result.reclaimed_bytes, 0);
    assert!(harness.service.history().await.unwrap().audit.is_empty());
}

#[tokio::test]
async fn unreadable_inventory_fails_before_deletion() {
    let harness = Harness::new(true);
    let orphan = harness.orphan();
    fs::remove_dir_all(&harness.profiles.base_path).unwrap();
    fs::write(&harness.profiles.base_path, b"not a directory").unwrap();
    assert!(harness.service.cleanup(vec![orphan.clone()]).await.is_err());
    assert!(Path::new(&orphan.target_path).is_dir());
}
