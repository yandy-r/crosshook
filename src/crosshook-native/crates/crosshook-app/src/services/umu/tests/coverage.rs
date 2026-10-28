use crosshook_core::metadata::{MetadataStore, UmuGameIdCacheRow};
use crosshook_core::umu_database::CsvCoverage;

use super::super::UmuService;

#[tokio::test]
async fn missing_and_blank_app_ids_have_unknown_coverage() {
    let service = UmuService::new(MetadataStore::open_in_memory().expect("metadata"));

    for app_id in [None, Some(String::new()), Some(" \t\n ".to_owned())] {
        assert!(matches!(
            service.coverage(app_id).await.expect("coverage"),
            CsvCoverage::Unknown
        ));
    }
}

#[tokio::test]
async fn clear_gameid_cache_returns_deleted_row_count() {
    let metadata = MetadataStore::open_in_memory().expect("metadata");
    for (app_id, umu_id) in [("10", "umu-10"), ("20", "umu-20")] {
        let row =
            UmuGameIdCacheRow::found("steam", app_id, umu_id, None, "2026-10-28T00:00:00Z", None)
                .expect("cache row");
        metadata
            .put_umu_gameid_cache_entry(&row)
            .expect("seed cache");
    }
    let service = UmuService::new(metadata.clone());

    assert_eq!(service.clear_gameid_cache().await.expect("clear cache"), 2);
    for app_id in ["10", "20"] {
        assert!(metadata
            .get_stale_umu_gameid_cache_entry("steam", app_id)
            .expect("read cache")
            .is_none());
    }
    assert_eq!(service.clear_gameid_cache().await.expect("empty cache"), 0);
}
