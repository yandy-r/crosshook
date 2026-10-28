use crosshook_core::discovery::CatalogQuery;
use crosshook_core::metadata::MetadataStore;

use super::helpers::{query, Harness};
use crate::AppErrorKind;

#[tokio::test]
async fn offline_search_serves_fresh_stale_and_empty_cache_even_when_forced() {
    for stale in [false, true] {
        for empty in [false, true] {
            let harness = Harness::new(MetadataStore::open_in_memory().unwrap());
            harness.offline();
            harness.seed_cache(stale, empty);
            let result = harness.service.search_external(query()).await.unwrap();
            assert!(result.cached);
            assert_eq!(result.is_stale, stale);
            assert_eq!(result.results.is_empty(), empty);
        }
    }
}

#[test]
fn offline_search_completes_with_single_blocking_worker() {
    let harness = Harness::new(MetadataStore::open_in_memory().unwrap());
    harness.offline();
    harness.seed_cache(false, false);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();

    // One worker must handle both settings and the core's cached-source work.
    // Holding that worker in block_on would starve the source task forever.
    let result = runtime.block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            harness.service.search_external(query()),
        )
        .await
    });
    // A regression leaves a non-abortable blocking task: never wait forever
    // for it during runtime teardown, including when the assertion fails.
    runtime.shutdown_background();

    let response = result
        .expect("search starved the core source task on the single blocking worker")
        .unwrap();
    assert!(response.cached);
    assert!(!response.is_stale);
    assert!(!response.offline);
    assert_eq!(response.results.len(), 1);
}

#[tokio::test]
async fn offline_search_without_cache_is_unavailable_but_empty_search_is_usable() {
    for metadata in [
        MetadataStore::open_in_memory().unwrap(),
        MetadataStore::disabled(),
    ] {
        let harness = Harness::new(metadata);
        harness.offline();
        let error = harness.service.search_external(query()).await.unwrap_err();
        assert_eq!(error.kind(), AppErrorKind::Unavailable);
        assert_eq!(error.message(), "offline mode is enabled");
        let mut empty = query();
        empty.game_name = "  ".into();
        assert!(
            !harness
                .service
                .search_external(empty)
                .await
                .unwrap()
                .offline
        );
        harness
            .settings
            .update(|settings| {
                settings.external_trainer_sources[0].enabled = false;
                Ok::<_, ()>(())
            })
            .unwrap()
            .unwrap();
        assert!(
            !harness
                .service
                .search_external(query())
                .await
                .unwrap()
                .offline
        );
    }
}

#[tokio::test]
async fn catalog_falls_back_for_disabled_and_failed_database() {
    let failed = MetadataStore::open_in_memory().unwrap();
    failed
        .with_sqlite_conn("break catalog fixture", |conn| {
            conn.execute_batch("DROP TABLE community_profiles").unwrap();
            Ok(())
        })
        .unwrap();
    for metadata in [MetadataStore::disabled(), failed] {
        let harness = Harness::new(metadata);
        let page = harness
            .service
            .catalog(CatalogQuery::default())
            .await
            .unwrap();
        assert!(page.degraded);
        assert!(page.entries.is_empty());
    }
}

#[tokio::test]
async fn compatibility_preserves_unknown_profile_text() {
    let harness = Harness::new(MetadataStore::open_in_memory().unwrap());
    let error = harness
        .service
        .check_version_compatibility("Missing".into(), None)
        .await
        .unwrap_err();
    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "profile_name"
        }
    );
    assert_eq!(error.message(), "unknown profile: Missing");
}
