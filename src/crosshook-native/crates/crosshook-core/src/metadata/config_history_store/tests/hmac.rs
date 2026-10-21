//! Config-history HMAC tamper evidence (ADR-0005).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use rusqlite::params;
use tempfile::{tempdir, TempDir};

use crate::metadata::config_history_hmac::{self, KEY_FILE_NAME};
use crate::metadata::{
    sha256_hex, ConfigRevisionRow, ConfigRevisionSource, HashOnlyReason, IntegrityFailure,
    MetadataStore, MetadataStoreError, RevisionIntegrity, MAX_CONFIG_REVISIONS_PER_PROFILE,
};

const PROFILE_ID: &str = "profile-1";
const SNAPSHOT: &str = "[game]\nname = \"Elden Ring\"\n";

fn db_path(dir: &TempDir) -> PathBuf {
    dir.path().join("metadata.db")
}

fn key_file(dir: &TempDir) -> PathBuf {
    dir.path().join(KEY_FILE_NAME)
}

fn open_store(path: &Path) -> MetadataStore {
    let store = MetadataStore::with_path(path).expect("open file-backed store");
    store
        .with_sqlite_conn("seed profile", |conn| {
            conn.execute(
                "INSERT OR IGNORE INTO profiles
                     (profile_id, current_filename, current_path, created_at, updated_at)
                 VALUES (?1, ?1, '/profiles/p.toml', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![PROFILE_ID],
            )?;
            Ok(())
        })
        .unwrap();
    store
}

fn insert(store: &MetadataStore, snapshot: &str) -> ConfigRevisionRow {
    let id = store
        .insert_config_revision(
            PROFILE_ID,
            "Profile",
            ConfigRevisionSource::ManualSave,
            &sha256_hex(snapshot.as_bytes()),
            snapshot,
            None,
            MAX_CONFIG_REVISIONS_PER_PROFILE,
        )
        .unwrap()
        .expect("revision inserted");
    store.get_config_revision(PROFILE_ID, id).unwrap().unwrap()
}

/// Simulates a direct edit of `metadata.db` behind CrossHook's back.
fn tamper(store: &MetadataStore, id: i64, sql: &str, value: Option<&str>) {
    store
        .with_sqlite_conn("tamper", |conn| {
            conn.execute(sql, params![value, id])?;
            Ok(())
        })
        .unwrap();
}

fn reload(store: &MetadataStore, id: i64) -> ConfigRevisionRow {
    store.get_config_revision(PROFILE_ID, id).unwrap().unwrap()
}

#[test]
fn signed_revision_round_trips() {
    let dir = tempdir().unwrap();
    let store = open_store(&db_path(&dir));
    let row = insert(&store, SNAPSHOT);

    let key_id = store.config_history_key.as_ref().unwrap().id().to_owned();
    let signature = row.hmac_signature.as_deref().expect("new rows are signed");
    let parts: Vec<&str> = signature.split(':').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], "v1");
    assert_eq!(parts[1], key_id);
    assert_eq!(parts[2].len(), 64);

    assert_eq!(
        store.verify_config_revision(&row).unwrap(),
        RevisionIntegrity::Signed
    );
}

#[test]
fn tampered_snapshot_with_recomputed_hash_is_rejected() {
    let dir = tempdir().unwrap();
    let store = open_store(&db_path(&dir));
    let row = insert(&store, SNAPSHOT);

    let forged = "[game]\nname = \"Evil\"\n";
    store
        .with_sqlite_conn("tamper", |conn| {
            conn.execute(
                "UPDATE config_revisions SET snapshot_toml = ?1, content_hash = ?2 WHERE id = ?3",
                params![forged, sha256_hex(forged.as_bytes()), row.id],
            )?;
            Ok(())
        })
        .unwrap();

    let result = store.verify_config_revision(&reload(&store, row.id));
    assert!(
        matches!(
            result,
            Err(MetadataStoreError::IntegrityCheckFailed {
                failure: IntegrityFailure::SignatureMismatch,
                ..
            })
        ),
        "got {result:?}"
    );
}

#[test]
fn tampered_snapshot_without_rehash_fails_content_hash() {
    let dir = tempdir().unwrap();
    let store = open_store(&db_path(&dir));
    let row = insert(&store, SNAPSHOT);
    tamper(
        &store,
        row.id,
        "UPDATE config_revisions SET snapshot_toml = ?1 WHERE id = ?2",
        Some("[game]\nname = \"Evil\"\n"),
    );

    let result = store.verify_config_revision(&reload(&store, row.id));
    assert!(matches!(
        result,
        Err(MetadataStoreError::IntegrityCheckFailed {
            failure: IntegrityFailure::ContentHashMismatch,
            ..
        })
    ));
}

#[test]
fn legacy_unsigned_row_verifies_hash_only() {
    let dir = tempdir().unwrap();
    let store = open_store(&db_path(&dir));
    let row = insert(&store, SNAPSHOT);
    tamper(
        &store,
        row.id,
        "UPDATE config_revisions SET hmac_signature = ?1 WHERE id = ?2",
        None,
    );

    assert_eq!(
        store
            .verify_config_revision(&reload(&store, row.id))
            .unwrap(),
        RevisionIntegrity::HashOnly {
            reason: HashOnlyReason::Unsigned
        }
    );
}

#[test]
fn foreign_key_id_and_unknown_version_verify_hash_only() {
    let dir = tempdir().unwrap();
    let store = open_store(&db_path(&dir));
    let row = insert(&store, SNAPSHOT);

    let other_dir = tempdir().unwrap();
    let other = config_history_hmac::load_or_create(&key_file(&other_dir)).unwrap();
    let foreign = other.sign(PROFILE_ID, &row.content_hash, &row.snapshot_toml);
    tamper(
        &store,
        row.id,
        "UPDATE config_revisions SET hmac_signature = ?1 WHERE id = ?2",
        Some(&foreign),
    );
    assert_eq!(
        store
            .verify_config_revision(&reload(&store, row.id))
            .unwrap(),
        RevisionIntegrity::HashOnly {
            reason: HashOnlyReason::ForeignKey
        }
    );

    tamper(
        &store,
        row.id,
        "UPDATE config_revisions SET hmac_signature = ?1 WHERE id = ?2",
        Some("v9:abcdef:0011"),
    );
    assert_eq!(
        store
            .verify_config_revision(&reload(&store, row.id))
            .unwrap(),
        RevisionIntegrity::HashOnly {
            reason: HashOnlyReason::UnknownVersion
        }
    );
}

#[test]
fn recreated_key_treats_old_signatures_as_foreign() {
    let dir = tempdir().unwrap();
    let path = db_path(&dir);
    let id = insert(&open_store(&path), SNAPSHOT).id;

    fs::remove_file(key_file(&dir)).unwrap();
    let store = open_store(&path);
    assert_eq!(
        store.verify_config_revision(&reload(&store, id)).unwrap(),
        RevisionIntegrity::HashOnly {
            reason: HashOnlyReason::ForeignKey
        }
    );
}

#[test]
fn unreadable_key_inserts_unsigned_and_verifies_hash_only() {
    let dir = tempdir().unwrap();
    let path = db_path(&dir);
    let signed_id = insert(&open_store(&path), SNAPSHOT).id;

    // Replace the key with something that cannot be loaded (and must not be
    // overwritten by key creation).
    fs::remove_file(key_file(&dir)).unwrap();
    fs::create_dir(key_file(&dir)).unwrap();
    let store = open_store(&path);
    assert!(store.config_history_key.is_none());

    let unsigned = insert(&store, "[game]\nname = \"Second\"\n");
    assert_eq!(unsigned.hmac_signature, None, "history is kept, unsigned");
    assert_eq!(
        store.verify_config_revision(&unsigned).unwrap(),
        RevisionIntegrity::HashOnly {
            reason: HashOnlyReason::Unsigned
        }
    );
    assert_eq!(
        store
            .verify_config_revision(&reload(&store, signed_id))
            .unwrap(),
        RevisionIntegrity::HashOnly {
            reason: HashOnlyReason::KeyUnavailable
        }
    );
    assert!(key_file(&dir).is_dir());
}

#[test]
fn in_memory_store_inserts_unsigned() {
    let store = MetadataStore::open_in_memory().unwrap();
    store
        .with_sqlite_conn("seed profile", |conn| {
            conn.execute(
                "INSERT INTO profiles
                     (profile_id, current_filename, current_path, created_at, updated_at)
                 VALUES (?1, ?1, '/p.toml', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![PROFILE_ID],
            )?;
            Ok(())
        })
        .unwrap();
    let row = insert(&store, SNAPSHOT);
    assert_eq!(row.hmac_signature, None);
}

#[test]
fn key_file_is_private_and_never_overwritten() {
    let dir = tempdir().unwrap();
    let path = db_path(&dir);
    let first_id = open_store(&path)
        .config_history_key
        .as_ref()
        .unwrap()
        .id()
        .to_owned();

    let key = key_file(&dir);
    let meta = fs::symlink_metadata(&key).unwrap();
    assert!(meta.is_file());
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    let bytes = fs::read(&key).unwrap();
    assert_eq!(bytes.len(), 32);

    // Reopen and race-loser creation both adopt the existing key.
    let reopened = open_store(&path);
    assert_eq!(reopened.config_history_key.as_ref().unwrap().id(), first_id);
    let adopted = config_history_hmac::create_or_adopt(&key).unwrap();
    assert_eq!(adopted.id(), first_id);
    assert_eq!(fs::read(&key).unwrap(), bytes);

    // No stray temp files are left behind.
    let leftovers: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".key."))
        .collect();
    assert!(leftovers.is_empty());
}
