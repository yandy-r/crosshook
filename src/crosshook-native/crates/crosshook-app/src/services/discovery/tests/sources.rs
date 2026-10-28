use crosshook_core::discovery::fling_default_source;
use crosshook_core::metadata::MetadataStore;

use super::helpers::Harness;
use crate::AppErrorKind;

#[tokio::test]
async fn source_crud_retains_local_extras_and_preserves_error_texts() {
    let harness = Harness::new(MetadataStore::open_in_memory().unwrap());
    harness
        .settings
        .update(|settings| {
            settings
                .extra
                .insert("future_setting".into(), toml::Value::Boolean(true));
            settings.external_trainer_sources[0]
                .extra
                .insert("future_source".into(), toml::Value::Boolean(true));
            Ok::<_, ()>(())
        })
        .unwrap()
        .unwrap();
    let mut source = fling_default_source();
    source.source_id = "other".into();
    source
        .extra
        .insert("untrusted_extra".into(), toml::Value::Boolean(true));
    let added = harness
        .service
        .add_external_source(source.clone())
        .await
        .unwrap();
    assert_eq!(added.len(), 2);
    assert!(added.iter().all(|source| source.extra.is_empty()));
    let persisted = harness.settings.load().unwrap();
    assert!(persisted.extra.contains_key("future_setting"));
    assert!(persisted.external_trainer_sources[0]
        .extra
        .contains_key("future_source"));
    assert!(persisted.external_trainer_sources[1].extra.is_empty());
    assert!(harness
        .service
        .list_external_sources()
        .await
        .unwrap()
        .iter()
        .all(|source| source.extra.is_empty()));

    let duplicate = harness
        .service
        .add_external_source(source)
        .await
        .unwrap_err();
    assert_eq!(duplicate.kind(), AppErrorKind::Conflict);
    assert_eq!(
        duplicate.message(),
        r#"source with id "other" already exists"#
    );
    let removed = harness
        .service
        .remove_external_source("other".into())
        .await
        .unwrap();
    assert_eq!(removed.len(), 1);
    assert!(removed[0].extra.is_empty());
    let missing = harness
        .service
        .remove_external_source("other".into())
        .await
        .unwrap_err();
    assert_eq!(missing.kind(), AppErrorKind::NotFound);
    assert_eq!(missing.message(), r#"no source with id "other" found"#);
    let persisted = harness.settings.load().unwrap();
    assert!(persisted.extra.contains_key("future_setting"));
    assert!(persisted.external_trainer_sources[0]
        .extra
        .contains_key("future_source"));
}

#[tokio::test]
async fn invalid_source_is_rejected_without_settings_write() {
    let harness = Harness::new(MetadataStore::disabled());
    let mut source = fling_default_source();
    source.source_id.clear();
    let error = harness
        .service
        .add_external_source(source)
        .await
        .unwrap_err();
    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "external_source"
        }
    );
    assert_eq!(error.message(), "source_id is required");
    assert!(!harness.settings.settings_path().exists());
}
