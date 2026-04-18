#![cfg(test)]

use std::path::PathBuf;

use rusqlite::params;

use super::test_support::connection;
use super::MetadataStore;
use crate::community::index::{CommunityProfileIndex, CommunityProfileIndexEntry};
use crate::community::taps::{
    CommunityTapSubscription, CommunityTapSyncResult, CommunityTapSyncStatus, CommunityTapWorkspace,
};
use crate::community::{CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating};
use crate::profile::GameProfile;

fn sample_tap_workspace(url: &str) -> CommunityTapWorkspace {
    CommunityTapWorkspace {
        subscription: CommunityTapSubscription {
            url: url.to_string(),
            branch: None,
            pinned_commit: None,
        },
        local_path: PathBuf::from("/tmp/test-tap"),
    }
}

fn sample_index_entry(
    tap_url: &str,
    relative_path: &str,
    game_name: &str,
) -> CommunityProfileIndexEntry {
    CommunityProfileIndexEntry {
        tap_url: tap_url.to_string(),
        tap_branch: None,
        tap_path: PathBuf::from("/tmp/test-tap"),
        manifest_path: PathBuf::from(format!("/tmp/test-tap/{relative_path}")),
        relative_path: PathBuf::from(relative_path),
        manifest: CommunityProfileManifest::new(
            CommunityProfileMetadata {
                game_name: game_name.to_string(),
                game_version: "1.0".to_string(),
                trainer_name: "TestTrainer".to_string(),
                trainer_version: "1".to_string(),
                proton_version: "9".to_string(),
                platform_tags: vec!["linux".to_string()],
                compatibility_rating: CompatibilityRating::Working,
                author: "TestAuthor".to_string(),
                description: "Test profile".to_string(),
                trainer_sha256: None,
            },
            GameProfile::default(),
        ),
    }
}

fn sample_sync_result(
    tap_url: &str,
    head_commit: &str,
    entries: Vec<CommunityProfileIndexEntry>,
) -> CommunityTapSyncResult {
    CommunityTapSyncResult {
        workspace: sample_tap_workspace(tap_url),
        status: CommunityTapSyncStatus::Updated,
        head_commit: head_commit.to_string(),
        index: CommunityProfileIndex {
            entries,
            diagnostics: vec![],
            trainer_sources: vec![],
        },
        from_cache: false,
        last_sync_at: None,
    }
}

#[test]
fn test_index_tap_result_inserts_tap_and_profile_rows() {
    let store = MetadataStore::open_in_memory().unwrap();
    let tap_url = "https://example.invalid/tap.git";
    let result = sample_sync_result(
        tap_url,
        "abc123",
        vec![
            sample_index_entry(tap_url, "profiles/game-a/community-profile.json", "Game A"),
            sample_index_entry(tap_url, "profiles/game-b/community-profile.json", "Game B"),
        ],
    );

    store.index_community_tap_result(&result).unwrap();

    let conn = connection(&store);

    let (tap_count, last_head_commit, profile_count): (i64, String, i64) = conn
        .query_row(
            "SELECT COUNT(*), last_head_commit, profile_count FROM community_taps WHERE tap_url = ?1",
            params![tap_url],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(tap_count, 1);
    assert_eq!(last_head_commit, "abc123");
    assert_eq!(profile_count, 2);

    let community_profile_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM community_profiles cp \
             JOIN community_taps ct ON cp.tap_id = ct.tap_id \
             WHERE ct.tap_url = ?1",
            params![tap_url],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(community_profile_count, 2);
}

#[test]
fn test_index_tap_result_skips_on_unchanged_head() {
    let store = MetadataStore::open_in_memory().unwrap();
    let tap_url = "https://example.invalid/tap.git";
    let result = sample_sync_result(
        tap_url,
        "abc123",
        vec![sample_index_entry(
            tap_url,
            "profiles/game-a/community-profile.json",
            "Game A",
        )],
    );

    store.index_community_tap_result(&result).unwrap();

    let updated_at_first: String = {
        let conn = connection(&store);
        conn.query_row(
            "SELECT updated_at FROM community_taps WHERE tap_url = ?1",
            params![tap_url],
            |row| row.get(0),
        )
        .unwrap()
    };

    // Index again with same head_commit — should be a no-op watermark skip.
    store.index_community_tap_result(&result).unwrap();

    let (updated_at_second, profile_count): (String, i64) = {
        let conn = connection(&store);
        conn.query_row(
            "SELECT updated_at, profile_count FROM community_taps WHERE tap_url = ?1",
            params![tap_url],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
    };

    assert_eq!(
        updated_at_first, updated_at_second,
        "updated_at must not change on watermark skip"
    );
    assert_eq!(profile_count, 1);
}

#[test]
fn test_index_tap_result_replaces_stale_profiles() {
    let store = MetadataStore::open_in_memory().unwrap();
    let tap_url = "https://example.invalid/tap.git";

    // First index: 3 profiles.
    let result_v1 = sample_sync_result(
        tap_url,
        "commit-v1",
        vec![
            sample_index_entry(tap_url, "profiles/game-a/community-profile.json", "Game A"),
            sample_index_entry(tap_url, "profiles/game-b/community-profile.json", "Game B"),
            sample_index_entry(tap_url, "profiles/game-c/community-profile.json", "Game C"),
        ],
    );
    store.index_community_tap_result(&result_v1).unwrap();

    // Second index: only 1 profile, different HEAD commit.
    let result_v2 = sample_sync_result(
        tap_url,
        "commit-v2",
        vec![sample_index_entry(
            tap_url,
            "profiles/game-a/community-profile.json",
            "Game A",
        )],
    );
    store.index_community_tap_result(&result_v2).unwrap();

    let conn = connection(&store);
    let profile_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM community_profiles cp \
             JOIN community_taps ct ON cp.tap_id = ct.tap_id \
             WHERE ct.tap_url = ?1",
            params![tap_url],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        profile_count, 1,
        "stale profiles should have been removed on re-index"
    );
}

#[test]
fn test_community_profiles_fk_cascades_on_tap_delete() {
    let store = MetadataStore::open_in_memory().unwrap();
    let tap_url = "https://example.invalid/tap.git";
    let result = sample_sync_result(
        tap_url,
        "commit-v1",
        vec![sample_index_entry(
            tap_url,
            "profiles/game-a/community-profile.json",
            "Game A",
        )],
    );
    store.index_community_tap_result(&result).unwrap();

    let conn = connection(&store);
    let tap_id: String = conn
        .query_row(
            "SELECT tap_id FROM community_taps WHERE tap_url = ?1",
            params![tap_url],
            |row| row.get(0),
        )
        .unwrap();

    conn.execute(
        "DELETE FROM community_taps WHERE tap_id = ?1",
        params![&tap_id],
    )
    .unwrap();

    let orphan_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM community_profiles WHERE tap_id = ?1",
            params![&tap_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        orphan_count, 0,
        "deleting a tap should cascade delete community profiles"
    );
}

#[test]
fn test_index_tap_result_disabled_store_noop() {
    let store = MetadataStore::disabled();
    let tap_url = "https://example.invalid/tap.git";
    let result = sample_sync_result(tap_url, "abc123", vec![]);

    let outcome = store.index_community_tap_result(&result);
    assert!(outcome.is_ok());
}
