use std::path::PathBuf;

use crosshook_core::metadata::{MetadataStore, SyncSource};
use crosshook_core::profile::{GameProfile, ProfileStore};

use super::OfflineService;
use crate::AppErrorKind;

#[tokio::test]
async fn disabled_metadata_store_is_unavailable_for_every_async_command() {
    let root = tempfile::tempdir().unwrap();
    let service = OfflineService::new(
        ProfileStore::with_base_path(root.path().join("profiles")),
        MetadataStore::disabled(),
    );

    let check = service.check_readiness("any".into()).await.unwrap_err();
    let batch = service.batch_readiness().await.unwrap_err();
    let verify = service.verify_trainer_hash("any".into()).await.unwrap_err();

    for error in [check, batch, verify] {
        assert_eq!(error.kind(), AppErrorKind::Unavailable);
        assert_eq!(error.message(), "metadata store unavailable");
    }
}

#[tokio::test]
async fn verify_trainer_hash_rejects_empty_trainer_path() {
    let root = tempfile::tempdir().unwrap();
    let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
    let metadata = MetadataStore::open_in_memory().unwrap();
    let profile = GameProfile::default();
    profiles.save("sample", &profile).unwrap();
    metadata
        .observe_profile_write(
            "sample",
            &profile,
            &PathBuf::from("/profiles/sample.toml"),
            SyncSource::AppWrite,
            None,
        )
        .unwrap();
    let service = OfflineService::new(profiles, metadata);

    let error = service
        .verify_trainer_hash("sample".into())
        .await
        .unwrap_err();

    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "trainer_path"
        }
    );
    assert_eq!(error.message(), "trainer path is empty");
}

#[tokio::test]
async fn network_status_runs_off_the_executor_and_answers_a_bool() {
    let root = tempfile::tempdir().unwrap();
    let service = OfflineService::new(
        ProfileStore::with_base_path(root.path().join("profiles")),
        MetadataStore::disabled(),
    );

    // The answer depends on the host; the contract is that it never errors.
    service.network_status().await.expect("network probe");
}

#[test]
fn trainer_type_catalog_returns_the_global_entries() {
    let root = tempfile::tempdir().unwrap();
    let service = OfflineService::new(
        ProfileStore::with_base_path(root.path().join("profiles")),
        MetadataStore::disabled(),
    );

    let entries = service.trainer_type_catalog();

    assert!(!entries.is_empty());
    assert_eq!(
        entries.len(),
        crosshook_core::offline::global_trainer_type_catalog()
            .entries()
            .len()
    );
}
