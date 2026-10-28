//! Tap subscription management: normalized dedupe, validation, extras and removal.

use crosshook_core::community::{
    CommunityProfileIndex, CommunityTapSubscription, CommunityTapSyncResult, CommunityTapSyncStatus,
};

use super::helpers::{tap, write_manifest, Harness};
use crate::AppErrorKind;

const URL: &str = "https://example.invalid/tap.git";
const PIN: &str = "abcdef1234567";

fn tap_with(url: &str, branch: Option<&str>, pin: Option<&str>) -> CommunityTapSubscription {
    CommunityTapSubscription {
        url: url.to_string(),
        branch: branch.map(str::to_string),
        pinned_commit: pin.map(str::to_string),
        ..CommunityTapSubscription::default()
    }
}

fn extra_note(value: &str) -> toml::Table {
    let mut table = toml::Table::new();
    table.insert("note".to_string(), toml::Value::String(value.to_string()));
    table
}

#[tokio::test]
async fn add_tap_dedupes_on_the_normalized_triple() {
    let harness = Harness::new();

    let first = harness.service.add_tap(tap(URL)).await.unwrap();
    assert_eq!(first, vec![tap(URL)]);

    // Whitespace around every field and blank optional fields normalize to the
    // same identity as the plain subscription.
    let padded = tap_with(&format!("  {URL}  "), Some("   "), Some(""));
    let second = harness.service.add_tap(padded).await.unwrap();
    assert_eq!(second, vec![tap(URL)]);

    let padded_main = tap_with(URL, Some(" main "), None);
    let third = harness.service.add_tap(padded_main).await.unwrap();
    assert_eq!(
        third,
        vec![tap(URL), tap_with(URL, Some("main"), None)],
        "an explicit branch is a distinct identity from the unset branch"
    );

    let again = harness
        .service
        .add_tap(tap_with(URL, Some("main"), Some("  ")))
        .await
        .unwrap();
    assert_eq!(again.len(), 2);
    assert_eq!(harness.saved_taps().len(), 2);
}

#[tokio::test]
async fn add_tap_keeps_distinct_branches_and_pins() {
    let harness = Harness::new();

    harness.service.add_tap(tap(URL)).await.unwrap();
    harness
        .service
        .add_tap(tap_with(URL, Some("dev"), None))
        .await
        .unwrap();
    harness
        .service
        .add_tap(tap_with(URL, None, Some(PIN)))
        .await
        .unwrap();
    let all = harness
        .service
        .add_tap(tap_with(URL, Some("dev"), Some(&format!(" {PIN} "))))
        .await
        .unwrap();

    assert_eq!(
        all,
        vec![
            tap(URL),
            tap_with(URL, Some("dev"), None),
            tap_with(URL, None, Some(PIN)),
            tap_with(URL, Some("dev"), Some(PIN)),
        ]
    );
    assert_eq!(harness.saved_taps(), all);
}

#[tokio::test]
async fn add_tap_dedupes_existing_unnormalized_persisted_entries() {
    let harness = Harness::new();
    harness.set_taps(vec![
        tap_with(&format!(" {URL} "), Some(""), None),
        tap(URL),
    ]);

    let taps = harness
        .service
        .add_tap(tap_with(URL, Some("dev"), None))
        .await
        .unwrap();

    assert_eq!(taps, vec![tap(URL), tap_with(URL, Some("dev"), None)]);
    assert_eq!(harness.saved_taps(), taps);
}

#[tokio::test]
async fn invalid_taps_are_rejected_and_leave_settings_unchanged() {
    let harness = Harness::new();
    harness.set_taps(vec![tap(URL)]);
    let before = harness.settings_bytes();
    assert!(!before.is_empty());

    let invalid = [
        tap(""),
        tap("   "),
        tap("file:///etc/passwd"),
        tap("git://example.invalid/tap.git"),
        tap("relative/path"),
        tap("https://example.invalid/with space.git"),
        tap_with(URL, Some("-flag"), None),
        tap_with(URL, Some("bad branch"), None),
        tap_with(URL, None, Some("--force")),
        tap_with(URL, None, Some("xyz1234")),
        tap_with(URL, None, Some("abc")),
    ];
    for subscription in invalid {
        let error = harness
            .service
            .add_tap(subscription.clone())
            .await
            .expect_err(&format!("{subscription:?} must be rejected"));
        assert_eq!(
            error.kind(),
            AppErrorKind::Validation { field: "tap" },
            "{subscription:?}"
        );
        assert_eq!(harness.settings_bytes(), before, "{subscription:?}");
    }
    assert_eq!(harness.saved_taps(), vec![tap(URL)]);
}

#[tokio::test]
async fn invalid_tap_never_creates_the_settings_file() {
    let harness = Harness::new();

    harness.service.add_tap(tap("file:///x")).await.unwrap_err();

    assert!(harness.settings_bytes().is_empty());
}

#[tokio::test]
async fn persisted_extras_are_retained_but_responses_are_cleared() {
    let harness = Harness::new();
    let mut existing = tap(URL);
    existing.extra = extra_note("keep me");
    harness.set_taps(vec![existing]);

    let added = harness
        .service
        .add_tap(tap_with(URL, Some("dev"), None))
        .await
        .unwrap();

    assert_eq!(added, vec![tap(URL), tap_with(URL, Some("dev"), None)]);
    assert!(added.iter().all(|entry| entry.extra.is_empty()));
    let saved = harness.saved_taps();
    assert_eq!(saved[0].extra, extra_note("keep me"));
    assert!(saved[1].extra.is_empty());
}

#[tokio::test]
async fn re_adding_an_existing_identity_keeps_its_persisted_extras() {
    let harness = Harness::new();
    let mut existing = tap(URL);
    existing.extra = extra_note("keep me");
    harness.set_taps(vec![existing]);

    let mut incoming = tap(&format!(" {URL}"));
    incoming.extra = extra_note("incoming");
    let taps = harness.service.add_tap(incoming).await.unwrap();

    assert_eq!(taps, vec![tap(URL)]);
    let saved = harness.saved_taps();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].extra, extra_note("keep me"));
}

#[tokio::test]
async fn incoming_tap_extras_are_not_persisted() {
    let harness = Harness::new();
    let mut incoming = tap(URL);
    incoming.extra = extra_note("incoming");

    let taps = harness.service.add_tap(incoming).await.unwrap();

    assert!(taps[0].extra.is_empty());
    assert!(harness.saved_taps()[0].extra.is_empty());
}

#[tokio::test]
async fn remove_tap_matches_normalized_identity_and_keeps_other_extras() {
    let harness = Harness::new();
    let mut keep = tap_with(URL, Some("dev"), None);
    keep.extra = extra_note("keep me");
    harness.set_taps(vec![tap(URL), keep]);

    let remaining = harness
        .service
        .remove_tap(tap_with(&format!(" {URL} "), Some(" "), Some("")))
        .await
        .unwrap();

    assert_eq!(remaining, vec![tap_with(URL, Some("dev"), None)]);
    let saved = harness.saved_taps();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].extra, extra_note("keep me"));

    // Idempotent: removing a missing identity is not an error.
    let again = harness.service.remove_tap(tap(URL)).await.unwrap();
    assert_eq!(again, remaining);
}

#[tokio::test]
async fn remove_tap_rejects_invalid_identity_without_writing() {
    let harness = Harness::new();
    harness.set_taps(vec![tap(URL)]);
    let before = harness.settings_bytes();

    let error = harness
        .service
        .remove_tap(tap("file:///x"))
        .await
        .unwrap_err();

    assert_eq!(error.kind(), AppErrorKind::Validation { field: "tap" });
    assert_eq!(harness.settings_bytes(), before);
}

#[tokio::test]
async fn remove_tap_leaves_the_clone_and_metadata_rows() {
    let harness = Harness::new();
    let workspace = harness.subscribe_workspace(URL);
    let manifest_path = workspace.join("profiles/game/community-profile.json");
    write_manifest(&manifest_path, "Kept Game");

    // Index the clone the way a completed sync would.
    let resolved = harness.taps.resolve_workspace(&tap(URL)).unwrap();
    let index: CommunityProfileIndex = harness
        .taps
        .index_workspaces(std::slice::from_ref(&resolved))
        .unwrap();
    assert_eq!(index.entries.len(), 1);
    let result = CommunityTapSyncResult {
        workspace: resolved,
        status: CommunityTapSyncStatus::Updated,
        head_commit: "abc1234".to_string(),
        index,
        from_cache: false,
        last_sync_at: None,
    };
    harness
        .metadata
        .index_community_tap_result(&result)
        .unwrap();
    assert_eq!(
        harness.service.list_indexed_profiles().await.unwrap().len(),
        1
    );

    let remaining = harness.service.remove_tap(tap(URL)).await.unwrap();

    assert!(remaining.is_empty());
    assert!(harness.saved_taps().is_empty());
    assert!(manifest_path.is_file(), "the local clone must stay on disk");
    let rows = harness.service.list_indexed_profiles().await.unwrap();
    assert_eq!(rows.len(), 1, "indexed rows must survive removal");
    assert_eq!(rows[0].game_name.as_deref(), Some("Kept Game"));
    assert_eq!(rows[0].tap_url, URL);
}

#[tokio::test]
async fn invalid_stored_taps_do_not_block_add_or_remove() {
    let harness = Harness::new();
    let bad_pin = tap_with(URL, None, Some("not-a-sha"));
    let bad_url = tap("file:///etc/passwd");
    let mut bad_branch = tap_with(URL, Some("-flag"), None);
    bad_branch.extra = extra_note("keep me");
    harness.set_taps(vec![bad_pin.clone(), bad_url.clone(), bad_branch.clone()]);

    let added = harness
        .service
        .add_tap(tap_with(URL, Some("dev"), None))
        .await
        .unwrap();
    let mut cleared_bad_branch = bad_branch.clone();
    cleared_bad_branch.extra.clear();
    assert_eq!(
        added,
        vec![
            bad_pin.clone(),
            bad_url.clone(),
            cleared_bad_branch,
            tap_with(URL, Some("dev"), None)
        ]
    );
    // Invalid stored entries are persisted unchanged, extras included.
    assert_eq!(
        harness.saved_taps()[..3],
        [bad_pin, bad_url.clone(), bad_branch.clone()]
    );

    let remaining = harness.service.remove_tap(bad_url).await.unwrap();
    assert_eq!(remaining.len(), 3);
    assert!(remaining.iter().all(|tap| tap.url == URL));
    let remaining = harness.service.remove_tap(bad_branch).await.unwrap();
    assert_eq!(remaining.len(), 2);
}
