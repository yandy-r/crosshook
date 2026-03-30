use super::models::{
    ConfigRevisionRow, ConfigRevisionSource, MAX_CONFIG_REVISIONS_PER_PROFILE,
    MAX_SNAPSHOT_TOML_BYTES,
};
use super::MetadataStoreError;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

/// Insert a new config revision for the given profile, skipping the insert when
/// the content hash matches the latest recorded revision (dedup). Prunes older
/// rows beyond the retention limit in the same transaction. Returns the new row's
/// id when a row is inserted, or `None` when skipped due to dedup.
pub fn insert_config_revision(
    conn: &Connection,
    profile_id: &str,
    profile_name_at_write: &str,
    source: ConfigRevisionSource,
    content_hash: &str,
    snapshot_toml: &str,
    source_revision_id: Option<i64>,
) -> Result<Option<i64>, MetadataStoreError> {
    if snapshot_toml.len() > MAX_SNAPSHOT_TOML_BYTES {
        return Err(MetadataStoreError::Validation(format!(
            "snapshot_toml for profile '{profile_id}' exceeds the {MAX_SNAPSHOT_TOML_BYTES}-byte limit ({} bytes)",
            snapshot_toml.len()
        )));
    }

    let tx =
        Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(|source| {
            MetadataStoreError::Database {
                action: "start a config revision insert transaction",
                source,
            }
        })?;

    // Dedup: skip if the latest revision for this profile already has the same hash.
    let latest_hash: Option<String> = tx
        .query_row(
            "SELECT content_hash FROM config_revisions
             WHERE profile_id = ?1
             ORDER BY id DESC
             LIMIT 1",
            params![profile_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| MetadataStoreError::Database {
            action: "check latest config revision hash for dedup",
            source,
        })?;

    if latest_hash.as_deref() == Some(content_hash) {
        tx.commit().map_err(|source| MetadataStoreError::Database {
            action: "commit the config revision dedup check transaction",
            source,
        })?;
        return Ok(None);
    }

    // Validate that source_revision_id (if provided) belongs to the same profile.
    if let Some(src_rev_id) = source_revision_id {
        let owner: Option<String> = tx
            .query_row(
                "SELECT profile_id FROM config_revisions WHERE id = ?1",
                params![src_rev_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| MetadataStoreError::Database {
                action: "validate source_revision_id ownership",
                source,
            })?;
        match owner.as_deref() {
            Some(owner_pid) if owner_pid == profile_id => {}
            _ => {
                return Err(MetadataStoreError::Validation(format!(
                    "source_revision_id {src_rev_id} does not belong to profile {profile_id}"
                )));
            }
        }
    }

    let now = Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO config_revisions
             (profile_id, profile_name_at_write, source, content_hash, snapshot_toml,
              source_revision_id, is_last_known_working, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
        params![
            profile_id,
            profile_name_at_write,
            source.as_str(),
            content_hash,
            snapshot_toml,
            source_revision_id,
            now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "insert a config revision row",
        source,
    })?;

    let new_id = tx.last_insert_rowid();

    tx.execute(
        "DELETE FROM config_revisions
         WHERE profile_id = ?1
           AND id NOT IN (
               SELECT id FROM config_revisions
               WHERE profile_id = ?1
               ORDER BY id DESC
               LIMIT ?2
           )
           AND id NOT IN (
               SELECT source_revision_id FROM config_revisions
               WHERE profile_id = ?1
                 AND source_revision_id IS NOT NULL
           )",
        params![profile_id, MAX_CONFIG_REVISIONS_PER_PROFILE as i64],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "prune old config revision rows",
        source,
    })?;

    tx.commit().map_err(|source| MetadataStoreError::Database {
        action: "commit the config revision insert transaction",
        source,
    })?;

    Ok(Some(new_id))
}

/// List config revisions for a profile ordered by id DESC (newest first).
/// `limit` defaults to `MAX_CONFIG_REVISIONS_PER_PROFILE` when `None`.
pub fn list_config_revisions(
    conn: &Connection,
    profile_id: &str,
    limit: Option<usize>,
) -> Result<Vec<ConfigRevisionRow>, MetadataStoreError> {
    let row_limit = limit.unwrap_or(MAX_CONFIG_REVISIONS_PER_PROFILE) as i64;

    let mut stmt = conn
        .prepare(
            "SELECT id, profile_id, profile_name_at_write, source, content_hash, snapshot_toml,
                    source_revision_id, is_last_known_working, created_at
             FROM config_revisions
             WHERE profile_id = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare list config revisions query",
            source,
        })?;

    let rows = stmt
        .query_map(params![profile_id, row_limit], |row| {
            Ok(ConfigRevisionRow {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                profile_name_at_write: row.get(2)?,
                source: row.get(3)?,
                content_hash: row.get(4)?,
                snapshot_toml: row.get(5)?,
                source_revision_id: row.get(6)?,
                is_last_known_working: row.get::<_, i64>(7)? != 0,
                created_at: row.get(8)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "query config revisions for profile",
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "collect config revision rows",
            source,
        })?;

    Ok(rows)
}

/// Get a single config revision by id, scoped to `profile_id` to enforce ownership.
/// Returns `None` if the revision does not exist or belongs to a different profile.
pub fn get_config_revision(
    conn: &Connection,
    profile_id: &str,
    revision_id: i64,
) -> Result<Option<ConfigRevisionRow>, MetadataStoreError> {
    conn.query_row(
        "SELECT id, profile_id, profile_name_at_write, source, content_hash, snapshot_toml,
                source_revision_id, is_last_known_working, created_at
         FROM config_revisions
         WHERE id = ?1 AND profile_id = ?2",
        params![revision_id, profile_id],
        |row| {
            Ok(ConfigRevisionRow {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                profile_name_at_write: row.get(2)?,
                source: row.get(3)?,
                content_hash: row.get(4)?,
                snapshot_toml: row.get(5)?,
                source_revision_id: row.get(6)?,
                is_last_known_working: row.get::<_, i64>(7)? != 0,
                created_at: row.get(8)?,
            })
        },
    )
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "get a config revision by id",
        source,
    })
}

/// Mark the given revision as known-good for its profile. Clears the known-good
/// marker on all other revisions for the same profile in the same transaction to
/// enforce single-active-marker semantics. Returns an error if the revision is not
/// found for the given profile.
pub fn set_known_good_revision(
    conn: &Connection,
    profile_id: &str,
    revision_id: i64,
) -> Result<(), MetadataStoreError> {
    let tx =
        Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(|source| {
            MetadataStoreError::Database {
                action: "start a set-known-good transaction",
                source,
            }
        })?;

    tx.execute(
        "UPDATE config_revisions
         SET is_last_known_working = 0
         WHERE profile_id = ?1",
        params![profile_id],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "clear known-good markers for profile",
        source,
    })?;

    let updated = tx
        .execute(
            "UPDATE config_revisions
             SET is_last_known_working = 1
             WHERE id = ?1 AND profile_id = ?2",
            params![revision_id, profile_id],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "set known-good marker on config revision",
            source,
        })?;

    if updated == 0 {
        return Err(MetadataStoreError::Corrupt(format!(
            "config revision {revision_id} not found for profile '{profile_id}' when setting known-good"
        )));
    }

    tx.commit().map_err(|source| MetadataStoreError::Database {
        action: "commit the set-known-good transaction",
        source,
    })?;

    Ok(())
}

/// Clear the known-good marker from all revisions for the given profile.
pub fn clear_known_good_revision(
    conn: &Connection,
    profile_id: &str,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "UPDATE config_revisions
         SET is_last_known_working = 0
         WHERE profile_id = ?1",
        params![profile_id],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "clear known-good markers for profile",
        source,
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        clear_known_good_revision, get_config_revision, insert_config_revision,
        list_config_revisions, set_known_good_revision,
    };
    use crate::metadata::{
        db, migrations, ConfigRevisionSource, MetadataStore, MetadataStoreError,
        MAX_CONFIG_REVISIONS_PER_PROFILE, MAX_SNAPSHOT_TOML_BYTES,
    };
    use rusqlite::{params, Connection};

    fn open_test_db() -> Connection {
        let conn = db::open_in_memory().expect("open in-memory db");
        migrations::run_migrations(&conn).expect("run migrations");
        conn
    }

    /// Insert a minimal `profiles` row so that `config_revisions` FK constraints
    /// are satisfied. The `profile_id` doubles as the filename to keep tests
    /// self-contained.
    fn ensure_profile(conn: &Connection, profile_id: &str) {
        let now = "2024-01-01T00:00:00Z";
        conn.execute(
            "INSERT OR IGNORE INTO profiles
                 (profile_id, current_filename, current_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                profile_id,
                profile_id,
                format!("/profiles/{profile_id}.toml"),
                now,
                now,
            ],
        )
        .expect("ensure_profile insert must not fail");
    }

    fn insert_revision(conn: &Connection, profile_id: &str, hash: &str) -> i64 {
        ensure_profile(conn, profile_id);
        insert_config_revision(
            conn,
            profile_id,
            "Test Profile",
            ConfigRevisionSource::ManualSave,
            hash,
            "some toml content",
            None,
        )
        .expect("insert must not fail")
        .expect("insert must not be deduped against a different hash")
    }

    // ── insert / list ─────────────────────────────────────────────────────────

    #[test]
    fn insert_returns_id_and_list_is_newest_first() {
        let conn = open_test_db();
        let id1 = insert_revision(&conn, "profile-1", "hash1");
        let id2 = insert_revision(&conn, "profile-1", "hash2");
        assert!(id2 > id1, "ids must increase monotonically");

        let revisions = list_config_revisions(&conn, "profile-1", None).unwrap();
        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[0].id, id2, "newest revision must be first");
        assert_eq!(revisions[1].id, id1, "oldest revision must be last");
        assert_eq!(revisions[0].profile_id, "profile-1");
        assert_eq!(revisions[0].profile_name_at_write, "Test Profile");
    }

    #[test]
    fn list_returns_empty_for_profile_with_no_revisions() {
        let conn = open_test_db();
        let revisions = list_config_revisions(&conn, "no-such-profile", None).unwrap();
        assert!(revisions.is_empty());
    }

    #[test]
    fn list_respects_custom_limit() {
        let conn = open_test_db();
        insert_revision(&conn, "profile-1", "hash1");
        insert_revision(&conn, "profile-1", "hash2");
        insert_revision(&conn, "profile-1", "hash3");

        let limited = list_config_revisions(&conn, "profile-1", Some(2)).unwrap();
        assert_eq!(limited.len(), 2, "limit parameter must be honoured");
        assert_eq!(limited[0].content_hash, "hash3", "newest within limit first");
        assert_eq!(limited[1].content_hash, "hash2");
    }

    // ── dedup ─────────────────────────────────────────────────────────────────

    #[test]
    fn insert_dedup_skips_when_latest_hash_matches() {
        let conn = open_test_db();
        ensure_profile(&conn, "profile-1");
        let id1 = insert_revision(&conn, "profile-1", "hash1");

        let deduped = insert_config_revision(
            &conn,
            "profile-1",
            "Test Profile",
            ConfigRevisionSource::ManualSave,
            "hash1",
            "some toml content",
            None,
        )
        .unwrap();
        assert!(deduped.is_none(), "identical hash must be skipped");

        let revisions = list_config_revisions(&conn, "profile-1", None).unwrap();
        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions[0].id, id1);
    }

    #[test]
    fn insert_dedup_does_not_apply_to_non_latest_hash() {
        // Dedup is only against the single latest row; re-inserting an older
        // hash must create a new row.
        let conn = open_test_db();
        ensure_profile(&conn, "profile-1");
        insert_revision(&conn, "profile-1", "hash1");
        insert_revision(&conn, "profile-1", "hash2");

        let result = insert_config_revision(
            &conn,
            "profile-1",
            "Test Profile",
            ConfigRevisionSource::ManualSave,
            "hash1",
            "some toml content",
            None,
        )
        .unwrap();
        assert!(
            result.is_some(),
            "re-inserting a non-latest hash must not be deduped"
        );

        let revisions = list_config_revisions(&conn, "profile-1", None).unwrap();
        assert_eq!(revisions.len(), 3);
    }

    #[test]
    fn insert_dedup_is_scoped_to_profile() {
        let conn = open_test_db();
        ensure_profile(&conn, "profile-1");
        ensure_profile(&conn, "profile-2");
        insert_revision(&conn, "profile-1", "hash1");

        let result = insert_config_revision(
            &conn,
            "profile-2",
            "Test Profile",
            ConfigRevisionSource::ManualSave,
            "hash1",
            "some toml content",
            None,
        )
        .unwrap();
        assert!(
            result.is_some(),
            "same hash for a different profile must not be deduped"
        );
    }

    // ── pruning ───────────────────────────────────────────────────────────────

    #[test]
    fn pruning_respects_max_revisions_per_profile() {
        let conn = open_test_db();
        ensure_profile(&conn, "profile-1");
        let over_limit = MAX_CONFIG_REVISIONS_PER_PROFILE + 1;
        for i in 0..over_limit {
            insert_config_revision(
                &conn,
                "profile-1",
                "Test Profile",
                ConfigRevisionSource::ManualSave,
                &format!("hash-{i}"),
                "some toml content",
                None,
            )
            .expect("insert must not fail")
            .expect("each unique-hash insert should succeed");
        }

        let revisions = list_config_revisions(&conn, "profile-1", None).unwrap();
        assert_eq!(
            revisions.len(),
            MAX_CONFIG_REVISIONS_PER_PROFILE,
            "revision count must not exceed retention limit"
        );
        assert!(
            revisions.iter().all(|r| r.content_hash != "hash-0"),
            "oldest revision must be pruned"
        );
        assert!(
            revisions
                .iter()
                .any(|r| r.content_hash == format!("hash-{}", over_limit - 1)),
            "newest revision must be retained"
        );
    }

    #[test]
    fn pruning_does_not_affect_other_profiles() {
        let conn = open_test_db();
        ensure_profile(&conn, "profile-1");
        ensure_profile(&conn, "profile-2");
        let over_limit = MAX_CONFIG_REVISIONS_PER_PROFILE + 1;
        for i in 0..over_limit {
            insert_config_revision(
                &conn,
                "profile-1",
                "Profile One",
                ConfigRevisionSource::ManualSave,
                &format!("p1-hash-{i}"),
                "toml",
                None,
            )
            .unwrap();
        }
        insert_revision(&conn, "profile-2", "p2-hash-1");

        let p2_revisions = list_config_revisions(&conn, "profile-2", None).unwrap();
        assert_eq!(
            p2_revisions.len(),
            1,
            "profile-2 must be unaffected by profile-1 pruning"
        );
    }

    #[test]
    fn pruning_retains_revisions_referenced_by_source_revision_id() {
        let conn = open_test_db();
        ensure_profile(&conn, "profile-1");
        let mut oldest_id: i64 = 0;
        for i in 0..MAX_CONFIG_REVISIONS_PER_PROFILE {
            let id = insert_config_revision(
                &conn,
                "profile-1",
                "Test Profile",
                ConfigRevisionSource::ManualSave,
                &format!("chain-hash-{i}"),
                "toml",
                None,
            )
            .expect("insert must not fail")
            .expect("each insert must create a row");
            if i == 0 {
                oldest_id = id;
            }
        }
        assert!(oldest_id > 0, "oldest revision id must be set");

        // New rollback row points at the oldest revision; pruning must not delete that parent
        // (would violate FK on config_revisions.source_revision_id).
        insert_config_revision(
            &conn,
            "profile-1",
            "Test Profile",
            ConfigRevisionSource::RollbackApply,
            "rollback-child-hash",
            "rollback toml",
            Some(oldest_id),
        )
        .expect("insert with parent reference must succeed after FK-safe pruning");

        let parent = get_config_revision(&conn, "profile-1", oldest_id)
            .unwrap()
            .expect("referenced parent revision must still exist");
        assert_eq!(parent.id, oldest_id);
    }

    // ── lineage ───────────────────────────────────────────────────────────────

    #[test]
    fn lineage_source_revision_id_stored_and_retrieved() {
        let conn = open_test_db();
        ensure_profile(&conn, "profile-1");
        let parent_id = insert_revision(&conn, "profile-1", "hash1");
        let child_id = insert_config_revision(
            &conn,
            "profile-1",
            "Test Profile",
            ConfigRevisionSource::RollbackApply,
            "hash2",
            "rollback toml content",
            Some(parent_id),
        )
        .unwrap()
        .expect("child insert should succeed");

        let child = get_config_revision(&conn, "profile-1", child_id)
            .unwrap()
            .expect("child must exist");
        assert_eq!(child.source_revision_id, Some(parent_id));
        assert_eq!(child.source, ConfigRevisionSource::RollbackApply.as_str());
    }

    // ── known-good ────────────────────────────────────────────────────────────

    #[test]
    fn known_good_supersede_clears_previous_marker() {
        let conn = open_test_db();
        ensure_profile(&conn, "profile-1");
        let id1 = insert_revision(&conn, "profile-1", "hash1");
        let id2 = insert_revision(&conn, "profile-1", "hash2");

        set_known_good_revision(&conn, "profile-1", id1).unwrap();
        let r1 = get_config_revision(&conn, "profile-1", id1)
            .unwrap()
            .unwrap();
        assert!(r1.is_last_known_working, "initial known-good marker must be set");

        // Supersede: mark id2 as known-good
        set_known_good_revision(&conn, "profile-1", id2).unwrap();

        let r1 = get_config_revision(&conn, "profile-1", id1)
            .unwrap()
            .unwrap();
        let r2 = get_config_revision(&conn, "profile-1", id2)
            .unwrap()
            .unwrap();
        assert!(
            !r1.is_last_known_working,
            "previous known-good marker must be cleared on supersede"
        );
        assert!(
            r2.is_last_known_working,
            "new known-good marker must be set"
        );
    }

    #[test]
    fn known_good_is_isolated_per_profile() {
        let conn = open_test_db();
        ensure_profile(&conn, "profile-1");
        ensure_profile(&conn, "profile-2");
        let id_p1 = insert_revision(&conn, "profile-1", "hash1");
        let id_p2 = insert_revision(&conn, "profile-2", "hash2");

        set_known_good_revision(&conn, "profile-1", id_p1).unwrap();

        let r_p2 = get_config_revision(&conn, "profile-2", id_p2)
            .unwrap()
            .unwrap();
        assert!(
            !r_p2.is_last_known_working,
            "profile-2 must be unaffected by profile-1 known-good change"
        );
    }

    #[test]
    fn clear_known_good_removes_all_markers_for_profile() {
        let conn = open_test_db();
        ensure_profile(&conn, "profile-1");
        let id = insert_revision(&conn, "profile-1", "hash1");
        set_known_good_revision(&conn, "profile-1", id).unwrap();

        clear_known_good_revision(&conn, "profile-1").unwrap();

        let r = get_config_revision(&conn, "profile-1", id)
            .unwrap()
            .unwrap();
        assert!(!r.is_last_known_working, "known-good marker must be cleared");
    }

    // ── get / ownership ───────────────────────────────────────────────────────

    #[test]
    fn get_revision_enforces_profile_ownership() {
        let conn = open_test_db();
        ensure_profile(&conn, "profile-1");
        let id = insert_revision(&conn, "profile-1", "hash1");

        let result = get_config_revision(&conn, "profile-2", id).unwrap();
        assert!(
            result.is_none(),
            "cross-profile revision access must return None"
        );
    }

    // ── validation ────────────────────────────────────────────────────────────

    #[test]
    fn oversized_snapshot_toml_is_rejected() {
        let conn = open_test_db();
        ensure_profile(&conn, "profile-1");
        let oversized = "x".repeat(MAX_SNAPSHOT_TOML_BYTES + 1);
        let result = insert_config_revision(
            &conn,
            "profile-1",
            "Test Profile",
            ConfigRevisionSource::ManualSave,
            "hash1",
            &oversized,
            None,
        );
        assert!(
            matches!(result, Err(MetadataStoreError::Validation(_))),
            "oversized payload must be rejected with Validation error"
        );
    }

    #[test]
    fn set_known_good_on_nonexistent_revision_errors() {
        let conn = open_test_db();
        let result = set_known_good_revision(&conn, "profile-1", 9999);
        assert!(
            matches!(result, Err(MetadataStoreError::Corrupt(_))),
            "nonexistent revision must return Corrupt error"
        );
    }

    // ── disabled / unavailable store ──────────────────────────────────────────

    #[test]
    fn disabled_store_returns_ok_with_defaults() {
        let store = MetadataStore::disabled();

        let insert_result = store
            .insert_config_revision(
                "profile-1",
                "Test Profile",
                ConfigRevisionSource::ManualSave,
                "hash1",
                "some toml",
                None,
            )
            .unwrap();
        assert!(
            insert_result.is_none(),
            "disabled store insert must return None"
        );

        let list_result = store.list_config_revisions("profile-1", None).unwrap();
        assert!(
            list_result.is_empty(),
            "disabled store list must return empty vec"
        );

        let get_result = store.get_config_revision("profile-1", 1).unwrap();
        assert!(
            get_result.is_none(),
            "disabled store get must return None"
        );

        assert!(
            store.set_known_good_revision("profile-1", 1).is_ok(),
            "disabled store set_known_good must return Ok"
        );
        assert!(
            store.clear_known_good_revision("profile-1").is_ok(),
            "disabled store clear_known_good must return Ok"
        );
    }

    // ── source_revision_id ownership ─────────────────────────────────────────

    #[test]
    fn cross_profile_lineage_is_rejected() {
        let conn = open_test_db();
        let rev_a = insert_revision(&conn, "profile-a", "hash-a");
        ensure_profile(&conn, "profile-b");

        let result = insert_config_revision(
            &conn,
            "profile-b",
            "Profile B",
            ConfigRevisionSource::RollbackApply,
            "hash-b",
            "some toml",
            Some(rev_a), // points to profile-a's revision
        );

        assert!(
            matches!(result, Err(MetadataStoreError::Validation(_))),
            "cross-profile source_revision_id must be rejected, got {result:?}"
        );
    }
}
