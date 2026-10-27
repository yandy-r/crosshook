use crosshook_core::metadata::{PrefixStorageCleanupAuditRow, PrefixStorageSnapshotRow};
use crosshook_core::storage::{PrefixCleanupResult, PrefixCleanupTarget, PrefixCleanupTargetKind};

use crate::services::storage::audit::persist_cleanup_audit;

use super::helpers::{create_prefix, execute_sql, path_string, Harness};

#[tokio::test]
async fn history_reports_empty_enabled_and_disabled_stores_honestly() {
    for available in [true, false] {
        let harness = Harness::new(available);
        let history = harness.service.history().await.unwrap();
        assert_eq!(history.available, available);
        assert!(history.snapshots.is_empty());
        assert!(history.audit.is_empty());
        let json = serde_json::to_value(&history).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "available": available, "snapshots": [], "audit": [] })
        );
    }
}

#[tokio::test]
async fn history_limits_snapshots_to_50_and_audit_to_100_newest_first() {
    let harness = Harness::new(true);
    for index in 0..120 {
        let timestamp = format!("2026-01-01T00:{:02}:{:02}Z", index / 60, index % 60);
        harness
            .metadata
            .insert_prefix_storage_snapshot(&PrefixStorageSnapshotRow {
                id: format!("snapshot-{index}"),
                resolved_prefix_path: format!("fixture-{index}"),
                total_bytes: index,
                staged_trainers_bytes: 0,
                is_orphan: false,
                referenced_profiles_json: "[]".into(),
                stale_staged_count: 0,
                scanned_at: timestamp.clone(),
            })
            .unwrap();
        harness
            .metadata
            .insert_prefix_storage_cleanup_audit(&PrefixStorageCleanupAuditRow {
                id: format!("audit-{index}"),
                target_kind: "orphan_prefix".into(),
                resolved_prefix_path: format!("fixture-{index}"),
                target_path: format!("fixture-{index}"),
                result: "deleted".into(),
                reason: None,
                reclaimed_bytes: index,
                created_at: timestamp,
            })
            .unwrap();
    }

    let history = harness.service.history().await.unwrap();

    assert!(history.available);
    assert_eq!(history.snapshots.len(), 50);
    assert_eq!(history.audit.len(), 100);
    assert_eq!(history.snapshots.first().unwrap().id, "snapshot-119");
    assert_eq!(history.snapshots.last().unwrap().id, "snapshot-70");
    assert_eq!(history.audit.first().unwrap().id, "audit-119");
    assert_eq!(history.audit.last().unwrap().id, "audit-20");
}

#[tokio::test]
async fn history_propagates_snapshot_and_audit_read_errors() {
    for table in ["prefix_storage_snapshots", "prefix_storage_cleanup_audit"] {
        let harness = Harness::new(true);
        let sql = match table {
            "prefix_storage_snapshots" => "DROP TABLE prefix_storage_snapshots",
            "prefix_storage_cleanup_audit" => "DROP TABLE prefix_storage_cleanup_audit",
            _ => unreachable!(),
        };
        execute_sql(&harness.metadata, sql);
        assert!(harness.service.history().await.is_err());
    }
}

#[tokio::test]
async fn deletion_of_empty_prefix_still_writes_zero_byte_summary() {
    let harness = Harness::new(true);
    let orphan = create_prefix(&harness.root.path().join("prefixes/empty"));
    let target = PrefixCleanupTarget {
        kind: PrefixCleanupTargetKind::OrphanPrefix,
        resolved_prefix_path: path_string(&orphan),
        target_path: path_string(&orphan),
    };
    let result = harness.service.cleanup(vec![target.clone()]).await.unwrap();
    assert_eq!(result.deleted, vec![target]);
    assert_eq!(result.reclaimed_bytes, 0);
    let rows = harness.service.history().await.unwrap().audit;
    assert_eq!(rows.len(), 2);
    let summary = rows
        .iter()
        .find(|row| row.target_kind == "summary")
        .unwrap();
    assert_eq!(summary.reclaimed_bytes, 0);
    assert_eq!(summary.result, "deleted");
}

#[tokio::test]
async fn audit_summary_gate_covers_reclaimed_bytes_independently_of_deleted_targets() {
    let harness = Harness::new(true);
    persist_cleanup_audit(&harness.metadata, &PrefixCleanupResult::default());
    assert!(harness.service.history().await.unwrap().audit.is_empty());
    persist_cleanup_audit(
        &harness.metadata,
        &PrefixCleanupResult {
            reclaimed_bytes: 123,
            ..Default::default()
        },
    );
    let rows = harness.service.history().await.unwrap().audit;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].target_kind, "summary");
    assert_eq!(rows[0].reclaimed_bytes, 123);
}
