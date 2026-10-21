//! Config-history HMAC tamper evidence (ADR-0005).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rusqlite::params;
use tempfile::{tempdir, TempDir};

use crate::metadata::config_history_hmac::{self, RevisionFields, Trust, KEY_FILE_NAME};
use crate::metadata::{
    sha256_hex, ConfigRevisionRow, ConfigRevisionSource, HashOnlyReason, IntegrityFailure,
    MetadataStore, MetadataStoreError, RevisionIntegrity, MAX_CONFIG_REVISIONS_PER_PROFILE,
};

const PROFILE_ID: &str = "profile-1";
const SNAPSHOT: &str = "[game]\nname = \"Elden Ring\"\n";
const FORGED: &str = "[game]\nname = \"Evil\"\n";

fn db_path(dir: &TempDir) -> PathBuf {
    dir.path().join("metadata.db")
}

fn key_file(dir: &TempDir) -> PathBuf {
    dir.path().join(KEY_FILE_NAME)
}

fn seed_profile(store: &MetadataStore) {
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
}

fn open_store(path: &Path) -> MetadataStore {
    let store = MetadataStore::with_path(path).expect("open file-backed store");
    seed_profile(&store);
    store
}

fn insert_from(store: &MetadataStore, snapshot: &str, source: Option<i64>) -> ConfigRevisionRow {
    let kind = if source.is_some() {
        ConfigRevisionSource::RollbackApply
    } else {
        ConfigRevisionSource::ManualSave
    };
    let id = store
        .insert_config_revision(
            PROFILE_ID,
            "Profile",
            kind,
            &sha256_hex(snapshot.as_bytes()),
            snapshot,
            source,
            MAX_CONFIG_REVISIONS_PER_PROFILE,
        )
        .unwrap()
        .expect("revision inserted");
    reload(store, id)
}

fn insert(store: &MetadataStore, snapshot: &str) -> ConfigRevisionRow {
    insert_from(store, snapshot, None)
}

fn reload(store: &MetadataStore, id: i64) -> ConfigRevisionRow {
    store.get_config_revision(PROFILE_ID, id).unwrap().unwrap()
}

/// Simulates a DB-only edit behind CrossHook's back.
fn exec(store: &MetadataStore, sql: &str, params: impl rusqlite::Params) {
    store
        .with_sqlite_conn("tamper", |conn| {
            conn.execute(sql, params)?;
            Ok(())
        })
        .unwrap();
}

fn forge_content(store: &MetadataStore, id: i64) {
    exec(
        store,
        "UPDATE config_revisions SET snapshot_toml = ?1, content_hash = ?2 WHERE id = ?3",
        params![FORGED, sha256_hex(FORGED.as_bytes()), id],
    );
}

fn set_signature(store: &MetadataStore, id: i64, signature: Option<&str>) {
    exec(
        store,
        "UPDATE config_revisions SET hmac_signature = ?1 WHERE id = ?2",
        params![signature, id],
    );
}

fn assert_failure(result: Result<RevisionIntegrity, MetadataStoreError>, want: IntegrityFailure) {
    assert!(
        matches!(result, Err(MetadataStoreError::IntegrityCheckFailed { failure, .. }) if failure == want),
        "expected {want:?}, got {result:?}"
    );
}

/// Writes a row that predates the key (legacy, unsigned) by inserting it
/// before the store (and therefore the key) exists.
fn legacy_row(path: &Path) -> i64 {
    let store = open_store(path);
    let id = insert(&store, SNAPSHOT).id;
    drop(store);
    fs::remove_file(path.with_file_name(KEY_FILE_NAME)).unwrap();
    let store = open_store(path);
    set_signature(&store, id, None);
    id
}

#[test]
fn signed_revision_round_trips() {
    let dir = tempdir().unwrap();
    let store = open_store(&db_path(&dir));
    let row = insert(&store, SNAPSHOT);

    let key_id = store.config_history_key.as_ref().unwrap().id().to_owned();
    let signature = row.hmac_signature.as_deref().expect("new rows are signed");
    let parts: Vec<&str> = signature.split(':').collect();
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0], "v1");
    assert_eq!(parts[1], key_id);
    assert_eq!(parts[2], "verified");
    assert_eq!(parts[3].len(), 64);

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
    forge_content(&store, row.id);
    assert_failure(
        store.verify_config_revision(&reload(&store, row.id)),
        IntegrityFailure::SignatureMismatch,
    );
}

#[test]
fn tampered_snapshot_without_rehash_fails_content_hash() {
    let dir = tempdir().unwrap();
    let store = open_store(&db_path(&dir));
    let row = insert(&store, SNAPSHOT);
    exec(
        &store,
        "UPDATE config_revisions SET snapshot_toml = ?1 WHERE id = ?2",
        params![FORGED, row.id],
    );
    assert_failure(
        store.verify_config_revision(&reload(&store, row.id)),
        IntegrityFailure::ContentHashMismatch,
    );
}

#[test]
fn post_epoch_signature_downgrades_are_rejected() {
    let dir = tempdir().unwrap();
    let store = open_store(&db_path(&dir));
    let row = insert(&store, SNAPSHOT);
    forge_content(&store, row.id);

    let other_dir = tempdir().unwrap();
    let other = config_history_hmac::load_or_create(&key_file(&other_dir), || Ok(0)).unwrap();
    let forged_row = reload(&store, row.id);
    let foreign = other.sign(&RevisionFields::from_row(&forged_row), Trust::Verified);

    for signature in [
        None,
        Some(foreign.as_str()),
        Some("v1:0000000000000000:verified:00"),
        Some("v9:abcdef:0011"),
        Some("v1:x"),
    ] {
        set_signature(&store, row.id, signature);
        assert_failure(
            store.verify_config_revision(&reload(&store, row.id)),
            IntegrityFailure::SignatureRequired,
        );
    }
}

#[test]
fn trust_field_tampering_is_rejected() {
    let dir = tempdir().unwrap();
    let store = open_store(&db_path(&dir));
    let row = insert(&store, SNAPSHOT);
    let signature = row.hmac_signature.unwrap();

    for (from, to) in [(":verified:", ":hash_only:"), (":verified:", ":trusted:")] {
        set_signature(&store, row.id, Some(&signature.replace(from, to)));
        assert_failure(
            store.verify_config_revision(&reload(&store, row.id)),
            IntegrityFailure::SignatureMismatch,
        );
    }
}

#[test]
fn signed_content_swapped_into_another_row_is_rejected() {
    let dir = tempdir().unwrap();
    let store = open_store(&db_path(&dir));
    let old = insert(&store, SNAPSHOT);
    let new = insert(&store, "[game]\nname = \"Current\"\n");

    // Copy the older, validly signed snapshot + signature into the newer row.
    exec(
        &store,
        "UPDATE config_revisions
         SET snapshot_toml = ?1, content_hash = ?2, hmac_signature = ?3
         WHERE id = ?4",
        params![
            old.snapshot_toml,
            old.content_hash,
            old.hmac_signature,
            new.id
        ],
    );
    assert_failure(
        store.verify_config_revision(&reload(&store, new.id)),
        IntegrityFailure::SignatureMismatch,
    );
}

#[test]
fn pre_epoch_legacy_row_verifies_hash_only() {
    let dir = tempdir().unwrap();
    let path = db_path(&dir);
    let id = legacy_row(&path);
    let store = open_store(&path);
    assert!(store.config_history_key.as_ref().unwrap().signed_from() >= id);
    assert_eq!(
        store.verify_config_revision(&reload(&store, id)).unwrap(),
        RevisionIntegrity::HashOnly {
            reason: HashOnlyReason::Unsigned
        }
    );
}

#[test]
fn recreated_key_treats_old_signatures_as_foreign_pre_epoch() {
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
fn epoch_survives_pruning_of_highest_rows() {
    let dir = tempdir().unwrap();
    let path = db_path(&dir);
    let id = insert(&open_store(&path), SNAPSHOT).id;
    {
        let store = open_store(&path);
        exec(&store, "DELETE FROM config_revisions", []);
    }
    fs::remove_file(key_file(&dir)).unwrap();
    // AUTOINCREMENT high-water mark keeps the deleted id inside the epoch.
    let store = open_store(&path);
    assert_eq!(store.config_history_key.as_ref().unwrap().signed_from(), id);
}

#[test]
fn rollback_from_hash_only_source_carries_hash_only_trust() {
    let dir = tempdir().unwrap();
    let path = db_path(&dir);
    let legacy_id = legacy_row(&path);
    let store = open_store(&path);
    // Forge the legacy content: still passes hash-only verification.
    forge_content(&store, legacy_id);
    let legacy = reload(&store, legacy_id);
    assert!(matches!(
        store.verify_config_revision(&legacy).unwrap(),
        RevisionIntegrity::HashOnly { .. }
    ));

    // A newer save, then rollback apply re-captures the restored (forged) content.
    insert(&store, "[game]\nname = \"Newer\"\n");
    let derived = insert_from(&store, FORGED, Some(legacy_id));
    let signature = derived.hmac_signature.as_deref().unwrap();
    assert!(signature.contains(":hash_only:"), "got {signature}");
    let derived_integrity = RevisionIntegrity::HashOnly {
        reason: HashOnlyReason::DerivedFromHashOnly,
    };
    assert_eq!(
        store.verify_config_revision(&derived).unwrap(),
        derived_integrity
    );

    // Rolling back to the derived row is allowed (after another save) and
    // stays hash-only.
    assert_eq!(
        store
            .verify_config_revision(&reload(&store, derived.id))
            .unwrap(),
        derived_integrity
    );
    insert(&store, "[game]\nname = \"Newest\"\n");
    let again = insert_from(&store, FORGED, Some(derived.id));
    assert!(again
        .hmac_signature
        .as_deref()
        .unwrap()
        .contains(":hash_only:"));
    assert_eq!(
        store.verify_config_revision(&again).unwrap(),
        derived_integrity
    );
}

#[test]
fn derived_trust_ignores_forged_stored_trust() {
    // Parent trust is derived by verifying the parent, not by reading its
    // stored trust: a tampered parent can never yield a verified child.
    let dir = tempdir().unwrap();
    let store = open_store(&db_path(&dir));
    let parent = insert(&store, SNAPSHOT);
    forge_content(&store, parent.id);
    let parent = reload(&store, parent.id);
    let key = store.config_history_key.as_deref().unwrap();
    assert_eq!(
        config_history_hmac::derived_trust(key, Some(&parent)),
        Trust::HashOnly
    );
    let signed = insert(&store, "[game]\nname = \"Fine\"\n");
    assert_eq!(
        config_history_hmac::derived_trust(key, Some(&signed)),
        Trust::Verified
    );
}

#[test]
fn rollback_from_signed_source_stays_signed() {
    let dir = tempdir().unwrap();
    let store = open_store(&db_path(&dir));
    let first = insert(&store, SNAPSHOT);
    insert(&store, "[game]\nname = \"Second\"\n");
    let derived = insert_from(&store, SNAPSHOT, Some(first.id));
    assert_eq!(
        store.verify_config_revision(&derived).unwrap(),
        RevisionIntegrity::Signed
    );
}

#[test]
fn unreadable_key_inserts_unsigned_and_verifies_hash_only() {
    let dir = tempdir().unwrap();
    let path = db_path(&dir);
    let signed_id = insert(&open_store(&path), SNAPSHOT).id;

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
fn wrong_length_key_file_is_unavailable() {
    let dir = tempdir().unwrap();
    let path = db_path(&dir);
    drop(open_store(&path));
    fs::write(key_file(&dir), [1_u8; 32]).unwrap();
    let store = open_store(&path);
    assert!(store.config_history_key.is_none());
    assert_eq!(fs::read(key_file(&dir)).unwrap(), vec![1_u8; 32]);
}

#[test]
fn lax_key_permissions_are_tightened_on_load() {
    let dir = tempdir().unwrap();
    let path = db_path(&dir);
    drop(open_store(&path));
    fs::set_permissions(key_file(&dir), fs::Permissions::from_mode(0o644)).unwrap();
    let store = open_store(&path);
    assert!(store.config_history_key.is_some());
    assert_eq!(
        fs::metadata(key_file(&dir)).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn in_memory_store_inserts_unsigned() {
    let store = MetadataStore::open_in_memory().unwrap();
    seed_profile(&store);
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
    assert_eq!(bytes.len(), 40);

    // Reopen and race-loser creation both adopt the existing key.
    let reopened = open_store(&path);
    assert_eq!(reopened.config_history_key.as_ref().unwrap().id(), first_id);
    let adopted = config_history_hmac::create_or_adopt(&key, 99).unwrap();
    assert_eq!(adopted.id(), first_id);
    assert_eq!(adopted.signed_from(), 0);
    assert_eq!(fs::read(&key).unwrap(), bytes);

    let leftovers: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".key."))
        .collect();
    assert!(leftovers.is_empty());
}

#[test]
fn stale_key_temp_files_are_removed_on_writable_open() {
    let dir = tempdir().unwrap();
    let path = db_path(&dir);
    drop(open_store(&path));
    let stale = dir.path().join(".config_history_hmac.key.stale1");
    let fresh = dir.path().join(".config_history_hmac.key.fresh1");
    fs::write(&stale, [0_u8; 40]).unwrap();
    fs::write(&fresh, [0_u8; 40]).unwrap();
    let old = SystemTime::now() - Duration::from_secs(2 * 60 * 60);
    fs::File::options()
        .write(true)
        .open(&stale)
        .unwrap()
        .set_modified(old)
        .unwrap();

    drop(open_store(&path));
    assert!(!stale.exists(), "stale temp key must be removed");
    assert!(
        fresh.exists(),
        "recent temp file may belong to a live writer"
    );
}

#[test]
fn recreated_database_lowers_key_epoch_so_new_rows_require_signatures() {
    let dir = tempdir().unwrap();
    let path = db_path(&dir);
    {
        let store = open_store(&path);
        insert(&store, SNAPSHOT);
        insert(&store, "[game]\nname = \"Second\"\n");
        insert(&store, "[game]\nname = \"Third\"\n");
    }
    let key_before = fs::read(key_file(&dir)).unwrap();

    // Delete the database (and sidecars) but keep the key.
    for name in ["metadata.db", "metadata.db-wal", "metadata.db-shm"] {
        let _ = fs::remove_file(dir.path().join(name));
    }
    let store = open_store(&path);
    let key = store.config_history_key.as_ref().unwrap();
    assert_eq!(
        key.signed_from(),
        0,
        "epoch lowered to the new high-water mark"
    );
    let key_after = fs::read(key_file(&dir)).unwrap();
    assert_eq!(key_after.len(), 40);
    assert_eq!(
        key_after[..32],
        key_before[..32],
        "same key bytes, no rotation"
    );
    assert_eq!(
        fs::metadata(key_file(&dir)).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let row = insert(&store, SNAPSHOT);
    forge_content(&store, row.id);
    set_signature(&store, row.id, None);
    assert_failure(
        store.verify_config_revision(&reload(&store, row.id)),
        IntegrityFailure::SignatureRequired,
    );

    // Reopen: the persisted lower epoch sticks and is never raised.
    drop(store);
    let reopened = open_store(&path);
    assert_eq!(
        reopened.config_history_key.as_ref().unwrap().signed_from(),
        0
    );
}

#[test]
fn key_unavailable_session_rows_are_listed_but_not_restorable() {
    // Fail closed (ADR-0005): rows recorded while the key file existed but
    // could not be loaded are kept, yet cannot be restored once it loads again.
    let dir = tempdir().unwrap();
    let path = db_path(&dir);
    insert(&open_store(&path), SNAPSHOT);
    let key_bytes = fs::read(key_file(&dir)).unwrap();

    fs::set_permissions(key_file(&dir), fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(key_file(&dir), [9_u8; 7]).unwrap(); // unreadable: wrong length
    let unsigned_id = {
        let store = open_store(&path);
        assert!(store.config_history_key.is_none());
        insert(&store, "[game]\nname = \"While keyless\"\n").id
    };

    fs::write(key_file(&dir), &key_bytes).unwrap();
    let store = open_store(&path);
    assert!(store.config_history_key.is_some());
    let row = reload(&store, unsigned_id);
    assert_eq!(row.hmac_signature, None);
    assert_eq!(
        store.list_config_revisions(PROFILE_ID, None).unwrap().len(),
        2
    );
    let result = store.verify_config_revision(&row);
    assert_failure(result, IntegrityFailure::SignatureRequired);
    let message = store.verify_config_revision(&row).unwrap_err().to_string();
    assert_eq!(
        message,
        format!(
            "revision {unsigned_id} was recorded without a valid signature and cannot be restored; integrity cannot be verified"
        )
    );
}
