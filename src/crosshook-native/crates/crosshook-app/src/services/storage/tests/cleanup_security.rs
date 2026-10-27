//! Adversarial service-boundary cleanup regressions (AC1).

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use crosshook_core::storage::{PrefixCleanupResult, PrefixCleanupTarget, PrefixCleanupTargetKind};

use super::helpers::{create_prefix, path_string, Harness};

const NOT_IN_FRESH_SCAN: &str = "not present in a fresh storage scan";
const STAGED_ROOT: &str = "drive_c/CrossHook/StagedTrainers";

async fn assert_rejected(
    harness: &Harness,
    requests: Vec<PrefixCleanupTarget>,
    target: &PrefixCleanupTarget,
    allowed_reasons: &[&str],
) -> PrefixCleanupResult {
    let result = harness.service.cleanup(requests).await.unwrap();
    assert!(result.deleted.is_empty());
    assert_eq!(result.reclaimed_bytes, 0);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(&result.skipped[0].target, target);
    assert!(
        allowed_reasons.contains(&result.skipped[0].reason.as_str()),
        "unexpected rejection: {:?}",
        result.skipped[0]
    );

    // No summary is written when neither deletion nor reclamation occurred.
    let rows = harness.service.history().await.unwrap().audit;
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.result, "skipped");
    assert_eq!(
        row.reason.as_deref(),
        Some(result.skipped[0].reason.as_str())
    );
    assert_eq!(row.resolved_prefix_path, target.resolved_prefix_path);
    assert_eq!(row.target_path, target.target_path);
    assert_eq!(row.reclaimed_bytes, 0);
    assert_eq!(
        row.target_kind,
        match target.kind {
            PrefixCleanupTargetKind::OrphanPrefix => "orphan_prefix",
            PrefixCleanupTargetKind::StaleStagedTrainer => "stale_staged_trainer",
        }
    );
    result
}

#[tokio::test]
async fn wrong_kind_prefix_or_target_is_rejected_and_audited_for_both_kinds() {
    for orphan_request in [true, false] {
        for wrong_field in ["kind", "prefix", "target"] {
            let harness = Harness::new(true);
            let orphan = harness.orphan();
            let staged = harness.staged(true);
            let scan = harness.service.scan().await.unwrap();
            assert_eq!(scan.orphan_targets, vec![orphan.clone()]);
            assert_eq!(scan.stale_staged_targets, vec![staged.clone()]);
            let (mut forged, other) = if orphan_request {
                (orphan.clone(), &staged)
            } else {
                (staged.clone(), &orphan)
            };
            match wrong_field {
                "kind" => forged.kind = other.kind.clone(),
                "prefix" => forged.resolved_prefix_path = other.resolved_prefix_path.clone(),
                "target" => forged.target_path = other.target_path.clone(),
                _ => unreachable!(),
            }

            assert_rejected(
                &harness,
                vec![forged.clone()],
                &forged,
                &[NOT_IN_FRESH_SCAN],
            )
            .await;

            assert_eq!(
                fs::read(Path::new(&orphan.target_path).join("drive_c/payload")).unwrap(),
                b"orphan contents"
            );
            assert_eq!(
                fs::read(Path::new(&staged.target_path).join("trainer.exe")).unwrap(),
                b"trainer contents"
            );
        }
    }
}

#[tokio::test]
async fn duplicate_missing_target_is_skipped_and_audited_once_without_deletion() {
    let harness = Harness::new(true);
    let target = harness.staged(false);
    assert!(harness
        .service
        .scan()
        .await
        .unwrap()
        .stale_staged_targets
        .is_empty());

    assert_rejected(
        &harness,
        vec![target.clone(), target.clone()],
        &target,
        &[NOT_IN_FRESH_SCAN],
    )
    .await;

    assert_eq!(
        fs::read(Path::new(&target.target_path).join("trainer.exe")).unwrap(),
        b"trainer contents"
    );
}

async fn assert_symlink_swaps_rejected(kind: PrefixCleanupTargetKind) {
    for location in ["inside staged root", "outside", "dangling"] {
        let harness = Harness::new(true);
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        fs::write(outside_root.join("sentinel"), b"outside survives").unwrap();
        let expected = match kind {
            PrefixCleanupTargetKind::OrphanPrefix => harness.orphan(),
            PrefixCleanupTargetKind::StaleStagedTrainer => harness.staged(true),
        };
        let scan = harness.service.scan().await.unwrap();
        let target = match kind {
            PrefixCleanupTargetKind::OrphanPrefix => scan.orphan_targets.into_iter(),
            PrefixCleanupTargetKind::StaleStagedTrainer => scan.stale_staged_targets.into_iter(),
        }
        .find(|target| target == &expected)
        .expect("the target must be authorized before the swap");
        let referent = match location {
            "inside staged root" => harness.referenced.join(STAGED_ROOT).join("referent"),
            "outside" | "dangling" => outside_root.join("referent"),
            _ => unreachable!(),
        };
        fs::create_dir_all(referent.parent().unwrap()).unwrap();
        if location == "dangling" {
            fs::remove_dir_all(&target.target_path).unwrap();
        } else {
            fs::rename(&target.target_path, &referent).unwrap();
        }
        symlink(&referent, &target.target_path).unwrap();

        // Cleanup rescans: either the fresh allow-list or core's no-follow
        // check must reject the old scan target. This is not an atomic-race test.
        assert_rejected(
            &harness,
            vec![target.clone()],
            &target,
            &[NOT_IN_FRESH_SCAN, "target is a symlink; refusing cleanup"],
        )
        .await;

        assert!(fs::symlink_metadata(&target.target_path)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_link(&target.target_path).unwrap(), referent);
        assert_eq!(
            fs::read(outside_root.join("sentinel")).unwrap(),
            b"outside survives"
        );
        if location == "dangling" {
            assert!(fs::symlink_metadata(&referent).is_err());
        } else {
            let (payload, contents): (&str, &[u8]) = match kind {
                PrefixCleanupTargetKind::OrphanPrefix => ("drive_c/payload", b"orphan contents"),
                PrefixCleanupTargetKind::StaleStagedTrainer => ("trainer.exe", b"trainer contents"),
            };
            assert_eq!(fs::read(referent.join(payload)).unwrap(), contents);
        }
    }
}

#[tokio::test]
async fn orphan_swapped_to_inside_outside_or_dangling_symlink_after_scan_is_rejected() {
    assert_symlink_swaps_rejected(PrefixCleanupTargetKind::OrphanPrefix).await;
}

#[tokio::test]
async fn staged_target_swapped_to_inside_outside_or_dangling_symlink_after_scan_is_rejected() {
    assert_symlink_swaps_rejected(PrefixCleanupTargetKind::StaleStagedTrainer).await;
}

#[tokio::test]
async fn staged_intermediate_or_root_swapped_to_symlink_after_scan_preserves_referent() {
    for component in ["drive_c", "drive_c/CrossHook", STAGED_ROOT] {
        let harness = Harness::new(true);
        let outside = tempfile::tempdir().unwrap();
        let staged = harness.staged(true);
        let target = harness
            .service
            .scan()
            .await
            .unwrap()
            .stale_staged_targets
            .into_iter()
            .find(|target| target == &staged)
            .unwrap();
        let replaced = harness.referenced.join(component);
        let referent = fs::canonicalize(outside.path()).unwrap().join("component");
        fs::rename(&replaced, &referent).unwrap();
        symlink(&referent, &replaced).unwrap();

        assert_rejected(
            &harness,
            vec![target.clone()],
            &target,
            &[
                NOT_IN_FRESH_SCAN,
                "intermediate path component is a symlink; refusing cleanup",
                "staged trainers root is a symlink; refusing cleanup",
            ],
        )
        .await;

        let payload = referent
            .join(
                Path::new(&staged.target_path)
                    .strip_prefix(&replaced)
                    .unwrap(),
            )
            .join("trainer.exe");
        assert_eq!(fs::read(payload).unwrap(), b"trainer contents");
        assert!(fs::symlink_metadata(replaced)
            .unwrap()
            .file_type()
            .is_symlink());
    }
}

#[tokio::test]
async fn forged_escape_root_nested_child_and_sibling_prefix_paths_are_rejected() {
    for attack in [
        "orphan escape",
        "staged escape",
        "prefix root",
        "resolved prefix root",
        "staged root",
        "nested child",
        "sibling prefix",
    ] {
        let harness = Harness::new(true);
        let orphan = harness.orphan();
        let staged = harness.staged(true);
        let prefix_root = harness.referenced.parent().unwrap();
        let sibling = create_prefix(&prefix_root.join("active-sibling"));
        let sibling_payload = sibling.join(STAGED_ROOT).join("protected/trainer.exe");
        fs::create_dir_all(sibling_payload.parent().unwrap()).unwrap();
        fs::write(&sibling_payload, b"sibling survives").unwrap();
        let outside = create_prefix(&harness.root.path().join("outside"));
        fs::write(outside.join("drive_c/sentinel"), b"outside survives").unwrap();
        let nested = Path::new(&staged.target_path).join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("sentinel"), b"nested survives").unwrap();
        let mut forged = staged.clone();
        match attack {
            "orphan escape" => {
                forged = orphan.clone();
                forged.target_path = path_string(&prefix_root.join("../outside"));
                forged.resolved_prefix_path = forged.target_path.clone();
            }
            "staged escape" => {
                forged.target_path = path_string(
                    &harness
                        .referenced
                        .join("../active-sibling")
                        .join(STAGED_ROOT)
                        .join("protected"),
                );
            }
            "prefix root" => {
                forged = orphan.clone();
                forged.target_path = path_string(prefix_root);
                forged.resolved_prefix_path = forged.target_path.clone();
            }
            "resolved prefix root" => forged.target_path = path_string(&harness.referenced),
            "staged root" => {
                forged.target_path = path_string(&harness.referenced.join(STAGED_ROOT));
            }
            "nested child" => forged.target_path = path_string(&nested),
            "sibling prefix" => forged.target_path = path_string(sibling_payload.parent().unwrap()),
            _ => unreachable!(),
        }

        assert_rejected(
            &harness,
            vec![forged.clone()],
            &forged,
            &[NOT_IN_FRESH_SCAN],
        )
        .await;

        assert_eq!(fs::read(sibling_payload).unwrap(), b"sibling survives");
        assert_eq!(
            fs::read(outside.join("drive_c/sentinel")).unwrap(),
            b"outside survives"
        );
        assert_eq!(
            fs::read(nested.join("sentinel")).unwrap(),
            b"nested survives"
        );
        assert_eq!(
            fs::read(Path::new(&orphan.target_path).join("drive_c/payload")).unwrap(),
            b"orphan contents"
        );
        assert!(harness.referenced.is_dir());
        assert!(prefix_root.is_dir());
    }
}

#[tokio::test]
async fn compatdata_nested_pfx_link_discovered_outside_trusted_roots_is_rejected() {
    let harness = Harness::new(true);
    let outside = tempfile::tempdir().unwrap();
    let outside_prefix = create_prefix(&outside.path().join("protected"));
    fs::write(outside_prefix.join("drive_c/sentinel"), b"outside survives").unwrap();
    let compatdata = harness.root.path().join("compatdata");
    let active = create_prefix(&compatdata.join("100/pfx"));
    let mut profile = harness.profiles.load("Active").unwrap();
    profile.runtime.prefix_path = path_string(&active);
    harness.profiles.save("Active", &profile).unwrap();
    fs::create_dir_all(compatdata.join("200")).unwrap();
    let link = compatdata.join("200/pfx");
    symlink(&outside_prefix, &link).unwrap();
    // Discovery canonicalizes the nested pfx link. Even a server-produced
    // tuple must fail core containment against inventory-derived roots.
    let target = harness
        .service
        .scan()
        .await
        .unwrap()
        .orphan_targets
        .into_iter()
        .find(|target| target.target_path == path_string(&outside_prefix))
        .unwrap();

    assert_rejected(
        &harness,
        vec![target.clone()],
        &target,
        &["target is not strictly inside a trusted prefix root"],
    )
    .await;

    assert_eq!(
        fs::read(outside_prefix.join("drive_c/sentinel")).unwrap(),
        b"outside survives"
    );
    assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
    assert!(active.is_dir());
}

#[tokio::test]
async fn accepted_targets_are_deleted_once_and_audited_with_skips_and_reclaimed_summary() {
    let harness = Harness::new(true);
    let orphan = harness.orphan();
    let staged = harness.staged(true);
    let mut forged = staged.clone();
    forged.resolved_prefix_path = orphan.resolved_prefix_path.clone();

    let result = harness
        .service
        .cleanup(vec![
            orphan.clone(),
            staged.clone(),
            orphan.clone(),
            staged.clone(),
            forged.clone(),
            forged.clone(),
        ])
        .await
        .unwrap();

    assert_eq!(result.deleted, vec![orphan.clone(), staged.clone()]);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].target, forged);
    assert_eq!(result.skipped[0].reason, NOT_IN_FRESH_SCAN);
    assert_eq!(
        result.reclaimed_bytes,
        (b"orphan contents".len() + b"trainer contents".len()) as u64
    );
    assert!(!Path::new(&orphan.target_path).exists());
    assert!(!Path::new(&staged.target_path).exists());
    assert!(harness.referenced.is_dir());

    let rows = harness.service.history().await.unwrap().audit;
    assert_eq!(rows.len(), 4);
    for target in [&orphan, &staged] {
        let matching: Vec<_> = rows
            .iter()
            .filter(|row| row.result == "deleted" && row.target_path == target.target_path)
            .collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(
            matching[0].resolved_prefix_path,
            target.resolved_prefix_path
        );
        assert_eq!(
            matching[0].target_kind,
            match target.kind {
                PrefixCleanupTargetKind::OrphanPrefix => "orphan_prefix",
                PrefixCleanupTargetKind::StaleStagedTrainer => "stale_staged_trainer",
            }
        );
        assert!(matching[0].reason.is_none());
        assert_eq!(matching[0].reclaimed_bytes, 0);
    }
    let skipped = rows.iter().find(|row| row.result == "skipped").unwrap();
    assert_eq!(skipped.target_path, forged.target_path);
    assert_eq!(skipped.resolved_prefix_path, forged.resolved_prefix_path);
    assert_eq!(skipped.target_kind, "stale_staged_trainer");
    assert_eq!(skipped.reason.as_deref(), Some(NOT_IN_FRESH_SCAN));
    assert_eq!(skipped.reclaimed_bytes, 0);
    let summary = rows
        .iter()
        .find(|row| row.target_kind == "summary")
        .unwrap();
    assert_eq!(summary.result, "deleted");
    assert_eq!(summary.reclaimed_bytes, result.reclaimed_bytes as i64);
    assert!(summary.reason.is_none());
    assert!(summary.target_path.is_empty());
    assert!(summary.resolved_prefix_path.is_empty());
    assert!(rows.iter().all(|row| row.created_at == summary.created_at));
}
