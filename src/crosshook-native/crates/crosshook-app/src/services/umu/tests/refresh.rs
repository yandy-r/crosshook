use std::sync::{Arc, Mutex};

use crosshook_core::metadata::MetadataStore;
use crosshook_core::umu_database::{Error, UmuDatabaseRefreshStatus};

use super::super::refresh::UMU_REFRESH_FAILED;
use super::super::seams::UmuRefresher;
use super::super::UmuService;
use crate::services::BoxFuture;
use crate::AppErrorKind;

struct FakeRefresher(Mutex<Option<Result<UmuDatabaseRefreshStatus, Error>>>);

impl UmuRefresher for FakeRefresher {
    fn refresh(&self) -> BoxFuture<'_, Result<UmuDatabaseRefreshStatus, Error>> {
        let result = self
            .0
            .lock()
            .expect("refresher lock")
            .take()
            .expect("one call");
        Box::pin(async move { result })
    }
}

fn service(result: Result<UmuDatabaseRefreshStatus, Error>) -> UmuService {
    UmuService::with_refresher(
        MetadataStore::open_in_memory().expect("metadata"),
        Arc::new(FakeRefresher(Mutex::new(Some(result)))),
    )
}

fn status(refreshed: bool, reason: &str) -> UmuDatabaseRefreshStatus {
    UmuDatabaseRefreshStatus {
        refreshed,
        cached_at: Some("2026-10-28T00:00:00Z".to_owned()),
        source_url: "https://example.invalid/umu-database.csv".to_owned(),
        reason: reason.to_owned(),
    }
}

#[tokio::test]
async fn refreshed_status_is_preserved() {
    let expected = status(true, "downloaded");
    let actual = service(Ok(expected.clone()))
        .refresh()
        .await
        .expect("refresh");

    assert!(actual.refreshed);
    assert_eq!(actual.cached_at, expected.cached_at);
    assert_eq!(actual.source_url, expected.source_url);
    assert_eq!(actual.reason, expected.reason);
}

#[tokio::test]
async fn not_modified_status_is_preserved() {
    let expected = status(false, "not-modified");
    let actual = service(Ok(expected.clone()))
        .refresh()
        .await
        .expect("refresh");

    assert!(!actual.refreshed);
    assert_eq!(actual.cached_at, expected.cached_at);
    assert_eq!(actual.source_url, expected.source_url);
    assert_eq!(actual.reason, expected.reason);
}

#[tokio::test]
async fn refresh_errors_are_path_free_and_categorized() {
    for (source, kind) in [
        (
            Error::Io(std::io::Error::other("failed writing /tmp/private/umu.csv")),
            AppErrorKind::Io,
        ),
        (
            Error::Metadata("https://example.invalid/private /tmp/metadata".to_owned()),
            AppErrorKind::Unavailable,
        ),
        (
            Error::Base("cannot resolve /tmp/private/home".to_owned()),
            AppErrorKind::Io,
        ),
    ] {
        let error = service(Err(source))
            .refresh()
            .await
            .expect_err("refresh error");

        assert_eq!(error.kind(), kind);
        assert_eq!(error.message(), UMU_REFRESH_FAILED);
        assert!(!error.message().contains('/'));
        assert!(!error.message().contains("http"));
    }
}
