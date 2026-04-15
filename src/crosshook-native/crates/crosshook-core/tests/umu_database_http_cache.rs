//! Integration tests: HTTP fetch, ETag revalidation, and on-disk persistence
//! for the umu-database CSV cache client.
//!
//! Env isolation: each test overrides `HOME` and `XDG_DATA_HOME` to a fresh
//! `TempDir` so all filesystem and metadata-DB writes land in scratch space.
//! A file-local `ENV_LOCK` serialises the tests because `env::set_var` is
//! process-global.

// The ENV_LOCK guard MUST be held across the refresh_umu_database().await so
// concurrent async tests can't stomp each other's HOME/XDG_DATA_HOME env.
// Using std::sync::Mutex (not tokio::sync::Mutex) is intentional — tokio's
// Mutex has no advantage here and would complicate the poison-recovery path.
#![allow(clippy::await_holding_lock)]
// `.unwrap_or_else(|e| e.into_inner())` on PoisonError is fine as-is and
// reads more clearly than the `unwrap_or_else(PoisonError::into_inner)` form.
#![allow(clippy::redundant_closure, clippy::redundant_closure_for_method_calls)]

use crosshook_core::metadata::MetadataStore;
use crosshook_core::umu_database::refresh_umu_database;
use std::sync::Mutex;
use std::{env, fs};
use tempfile::TempDir;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

static ENV_LOCK: Mutex<()> = Mutex::new(());

const CSV_BODY: &str = "appid,store,codename,title,notes\n1234,steam,test_game,Test Game,\n";

/// Point `HOME` and `XDG_DATA_HOME` at a subdirectory of `temp`, returns the
/// path that `BaseDirs::data_local_dir()` will resolve to.
fn isolate_env(temp: &TempDir) -> std::path::PathBuf {
    let data_dir = temp.path().join(".local").join("share");
    fs::create_dir_all(&data_dir).expect("create test data dir");
    env::set_var("HOME", temp.path());
    env::set_var("XDG_DATA_HOME", &data_dir);
    data_dir
}

fn csv_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("crosshook").join("umu-database.csv")
}

fn db_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("crosshook").join("metadata.db")
}

// ── test 1 ──────────────────────────────────────────────────────────────────

/// A 200 response with an ETag must:
/// - write the response body to the expected on-disk CSV path
/// - upsert an `external_cache_entries` row whose `payload_json` contains the
///   ETag so the next call can send a conditional `If-None-Match` header.
#[tokio::test]
async fn client_persists_body_to_disk_and_metadata_on_2xx() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new().unwrap();
    let data_dir = isolate_env(&temp);

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("ETag", "\"etag-v1\"")
                .set_body_string(CSV_BODY),
        )
        .mount(&server)
        .await;

    env::set_var("CROSSHOOK_TEST_UMU_DATABASE_URL", server.uri());

    let status = refresh_umu_database().await.unwrap();
    assert!(status.refreshed, "200 response must set refreshed=true");

    // CSV written to disk with the exact response body
    let csv = csv_path(&data_dir);
    assert!(csv.exists(), "CSV must be written to disk after 200");
    assert_eq!(fs::read_to_string(&csv).unwrap(), CSV_BODY);

    // Metadata row upserted and contains the ETag
    let store = MetadataStore::with_path(&db_path(&data_dir)).unwrap();
    let payload = store
        .get_cache_entry("umu-database:csv")
        .unwrap()
        .expect("external_cache_entries row must exist after successful 200");
    assert!(
        payload.contains("etag-v1"),
        "payload_json must contain the ETag; got: {payload}"
    );
}

// ── test 2 ──────────────────────────────────────────────────────────────────

/// After a 200 primes the cache, a subsequent conditional request that receives
/// a 304 Not Modified must leave the on-disk CSV unchanged and return
/// `refreshed: false`.
#[tokio::test]
async fn client_leaves_body_unchanged_on_304() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new().unwrap();
    let data_dir = isolate_env(&temp);

    let server = MockServer::start().await;

    // First request: unconditional 200 with ETag (matched at most once)
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("ETag", "\"etag-v1\"")
                .set_body_string(CSV_BODY),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Second request: must carry If-None-Match header → 304
    Mock::given(method("GET"))
        .and(path("/"))
        .and(header("If-None-Match", "\"etag-v1\""))
        .respond_with(ResponseTemplate::new(304))
        .mount(&server)
        .await;

    env::set_var("CROSSHOOK_TEST_UMU_DATABASE_URL", server.uri());

    // Prime the cache with the initial 200
    refresh_umu_database().await.unwrap();

    let csv = csv_path(&data_dir);
    let content_before = fs::read(&csv).unwrap();

    // Revalidate — the server must return 304
    let status = refresh_umu_database().await.unwrap();
    assert!(!status.refreshed, "304 must yield refreshed=false");

    let content_after = fs::read(&csv).unwrap();
    assert_eq!(
        content_before, content_after,
        "CSV must be unchanged after 304"
    );
}

// ── test 3 ──────────────────────────────────────────────────────────────────

/// A network failure (server dropped before the request) must return `Err`
/// and leave any pre-existing CSV file intact.
#[tokio::test]
async fn client_returns_err_cleanly_on_network_failure() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new().unwrap();
    let data_dir = isolate_env(&temp);

    // Write a sentinel CSV before the failing request
    let csv = csv_path(&data_dir);
    fs::create_dir_all(csv.parent().unwrap()).unwrap();
    fs::write(&csv, "sentinel-content").unwrap();

    // Start a real server just to get a valid-looking URL, then immediately
    // drop it so the port becomes unreachable.
    let dead_url = {
        let server = MockServer::start().await;
        server.uri()
    };

    env::set_var("CROSSHOOK_TEST_UMU_DATABASE_URL", &dead_url);

    let result = refresh_umu_database().await;
    assert!(result.is_err(), "unreachable URL must yield Err");

    assert_eq!(
        fs::read_to_string(&csv).unwrap(),
        "sentinel-content",
        "pre-existing CSV must be untouched after network failure"
    );
}

// ── test 4 ──────────────────────────────────────────────────────────────────

/// Explicit ETag round-trip: the client must store the ETag from the first
/// 200 response and send it back via `If-None-Match` on the next request.
/// The wiremock expectation fails the test if the header is absent.
#[tokio::test]
async fn client_roundtrips_etag_via_if_none_match() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new().unwrap();
    let data_dir = isolate_env(&temp);
    drop(data_dir); // used only for env isolation; no CSV assertion needed

    let server = MockServer::start().await;

    // First call: returns ETag "A"
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("ETag", "\"A\"")
                .set_body_string(CSV_BODY),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Second call: wiremock REQUIRES If-None-Match: "A" to be present.
    // `.expect(1)` causes `server.verify()` to fail if this mock is never reached.
    Mock::given(method("GET"))
        .and(path("/"))
        .and(header("If-None-Match", "\"A\""))
        .respond_with(ResponseTemplate::new(304))
        .expect(1)
        .mount(&server)
        .await;

    env::set_var("CROSSHOOK_TEST_UMU_DATABASE_URL", server.uri());

    // Prime cache with ETag "A"
    refresh_umu_database().await.unwrap();

    // Second call must include If-None-Match: "A" and get 304
    let status = refresh_umu_database().await.unwrap();
    assert!(
        !status.refreshed,
        "second call must yield refreshed=false (304)"
    );

    // Confirm the If-None-Match mock was matched exactly once
    server.verify().await;
}
