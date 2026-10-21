# ADR-0005: HMAC-SHA256 tamper evidence for config history

**Status**: Accepted — 2026-10-21

Tracking: Linear YAN-762 / GitHub yandy-r/crosshook#505.

---

## Context

Config history (`config_revisions`, schema v11) stores full TOML snapshots of each profile with a SHA-256 `content_hash`.
Rollback (`profile_config_rollback`) re-hashes the snapshot and compares it to `content_hash` before writing the snapshot
back to the profile TOML. That catches accidental corruption, but anyone who can edit `metadata.db` can change
`snapshot_toml` and recompute `content_hash` in the same `UPDATE`. The next rollback then writes the forged profile
(game/trainer paths, launch arguments, environment) to disk with no warning.

We want rollback to notice when a revision was changed outside CrossHook, without a new UI, without network access,
and without making history unusable when the evidence cannot be checked.

## Decision

1. **Signature column.** Migration v27→v28 adds nullable `config_revisions.hmac_signature TEXT`. Existing rows are
   not touched and stay `NULL`.
2. **Key and signing epoch.** The key file `config_history_hmac.key` sits in the same directory as `metadata.db`
   (normally `~/.local/share/crosshook/`), mode `0600`, exactly 40 bytes:
   - a 32-byte key from the OS CSPRNG (`getrandom`);
   - a u64 big-endian **signing epoch** `signed_from`: the `config_revisions` AUTOINCREMENT high-water mark
     (`MAX(sqlite_sequence, MAX(id))`) when the key was created.

   Every revision with `id > signed_from` was written under this key. The epoch lives outside SQLite, so an attacker
   who can only edit the DB cannot move it.

   Creation writes and fsyncs a private temp file in that directory, then publishes it with a no-replace rename/link
   (`tempfile::persist_noclobber`). An existing key is never overwritten, and a process that loses the creation race
   reads the winner's key.

   Loading opens with `O_NOFOLLOW` and runs `fstat` on the open handle. It rejects non-regular files, files owned by
   another uid, and any length other than 40. Group or other permission bits are reset on the open handle to `0600`,
   with a warning. Key bytes live in `Zeroizing` buffers, and `hmac` is built with `zeroize`.

   Only writable stores create a key. They also remove crash-leftover `.config_history_hmac.key.*` temp files that are
   regular, owned by us, and older than one hour. Read-only stores (newer schema, disabled) only load. In-memory and
   disabled stores have no key.

3. **Signing.** `MetadataStore::insert_config_revision` inserts the row after the existing dedup check. In the same
   savepoint it then stores `v1:<key_id>:<trust>:<hex_mac>`:
   - The MAC is HMAC-SHA256 over the length-prefixed (u64 big-endian) fields
     `tag ‖ id ‖ profile_id ‖ source ‖ created_at ‖ content_hash ‖ snapshot_toml ‖ trust`, with
     `tag = "crosshook.config_revision.v1"`.
   - `key_id` is the first 8 bytes (16 hex chars) of SHA-256(key) and does not reveal the key.
   - `trust` is `verified` for live saves.
   - A revision derived from `source_revision_id` (rollback) is `verified` only when core re-verifies the source row
     as fully `Signed`. Otherwise it is `hash_only`. The source's stored trust value is never read without a MAC
     check.

   This decision lives in core, so src-tauri is unchanged.

4. **Verification.** `MetadataStore::verify_config_revision(&ConfigRevisionRow)` in `crosshook-core` always re-checks
   the SHA-256 content hash. MACs are compared in constant time (`Mac::verify_slice`).
   - With a key loaded and `id > signed_from`, only a valid current-key signature passes:
     - `verified` gives `Signed`;
     - `hash_only` gives `HashOnly { DerivedFromHashOnly }`;
     - a `NULL`, foreign-key, or unknown-format signature gives `IntegrityCheckFailed { SignatureRequired }`;
     - a MAC or trust mismatch gives `IntegrityCheckFailed { SignatureMismatch }`.
   - Unknown formats and same-key mismatches are rejected for any row while a key is loaded.
   - `profile_config_rollback` calls this function in place of its old inline hash check and rejects the rollback on
     error.
5. **Degraded paths.** These cases fall back to the hash check, log a `tracing::warn!` (revision id, profile id, and
   reason only; never key material or MACs), and let the rollback proceed:
   - a pre-epoch row with a `NULL` signature (legacy, written before the key existed) → `HashOnly { Unsigned }`
   - a pre-epoch row signed by an earlier key (the key file was deleted and recreated) → `HashOnly { ForeignKey }`
   - no key loaded in this session (missing or unreadable key file) → `HashOnly { Unsigned }` or
     `HashOnly { KeyUnavailable }`
   - a derived row carrying `hash_only` trust → `HashOnly { DerivedFromHashOnly }`

   If no key is available at insert time, the revision is still inserted, unsigned, with a warning. History is never
   dropped and dedup is unchanged.

   **Fail closed.** The check is deliberately not relaxed for unsigned rows above the epoch. That covers rows recorded
   while the key file existed but could not be loaded, and rows written by v0.6.0 after a downgrade. A
   database-only attacker can produce the same `NULL` signature, and any marker kept in SQLite is attacker-controlled.
   Once a key is loaded, these rows stay listable and diffable but **cannot be restored**. The rollback error reads
   `revision N was recorded without a valid signature and cannot be restored; integrity cannot be verified`.

   **Epoch lowering.** On every writable open with an existing key, core recomputes the database high-water mark.
   If it is below `signed_from` (the DB was deleted, recreated, or restored from an older backup while the key was
   kept), the key file is rewritten atomically: same key bytes, `signed_from = high_water`, temp file + fsync +
   rename, `0600`. The epoch is never raised and the key is never rotated on this signal, because both would be
   downgrades a database-only attacker could trigger. If the rewrite fails, the lower epoch still applies for the
   session.

6. **Flatpak import.** `DATA_INCLUDE_FILES` stays the DB trio (`.db`, `-wal`, `-shm`), and only the trio decides
   whether the DB counts as already imported. `crosshook/config_history_hmac.key` is a companion file, handled only
   when the DB is copied in the same run:
   - A leftover sandbox key belongs to a sandbox DB that no longer exists. It is renamed aside to
     `config_history_hmac.key.orphaned-<unix_ts>` and never deleted.
   - The host key that signed the imported DB is then copied, with the destination set to `0600`. If the host has
     no key, the next open creates a fresh one with the correct epoch.
   - If the import rolls back, copied files are removed and the original sandbox key is restored.
7. **IPC.** Unchanged. `ConfigRevisionRow` is not serialized, and `ConfigRevisionSummary` gains no field. Neither the
   key nor the signature crosses IPC.

## Threat model

- **In scope (tamper evidence):** edits made only through the database (direct SQLite edits, or a third-party tool
  rewriting `metadata.db`) and silent corruption. If the key file is intact, all of these are rejected for any
  revision written after the signing epoch:
  - a forged `snapshot_toml` with a recomputed `content_hash`;
  - stripping, replacing, or reformatting the signature;
  - flipping its trust value;
  - moving a signed snapshot into another row (the id is in the MAC);
  - changing a row's `source` or `created_at`.

  Forged content that passes only hash verification is never upgraded to `Signed`, because rollbacks from it carry
  `hash_only` trust.

- **Out of scope:** an attacker with filesystem access as the user. They can read the key and forge signatures, or
  edit the profile TOML directly. This is evidence, not protection. Replacing or deleting the key resets the epoch to
  the current high-water mark, so earlier signatures become foreign and pre-epoch rows degrade to hash-only. That is
  deliberate, so a lost key never blocks recovery.
- **Residual risk (by design):**
  - **Rows at or below the epoch** (legacy pre-v28 history, rows from before a key was recreated) are hash-only. A
    DB-only attacker can still change them with a recomputed hash. Rollback warns, and anything derived from them
    stays `hash_only`.
  - **Profile-name repointing:** editing `profiles` so that profile X's name points to profile Y's `profile_id`
    would restore Y's validly signed snapshot into `X.toml`. The content is still CrossHook-authored.
  - **Row deletion** and edits to columns outside the MAC (`profile_name_at_write`, `source_revision_id`,
    `is_last_known_working`) are not detected.
- **Availability trade-off (fail closed):** revisions above the epoch recorded without a valid signature (a
  key-unavailable session, or a v0.6.0 downgrade) cannot be restored, because they are indistinguishable from a
  signature-stripping attack. They remain listable and diffable.

## Storage boundary

| Datum                         | Layer                                                                        |
| ----------------------------- | ---------------------------------------------------------------------------- |
| HMAC key                      | Filesystem secret (`config_history_hmac.key`, `0600`) — not TOML, not SQLite |
| `hmac_signature`              | SQLite metadata (`config_revisions`)                                         |
| Verification outcome / reason | Runtime-only (logs)                                                          |

No TOML settings are added.

## Persistence & usability

- **Migration / backward compatibility:** v28 adds one nullable column. Old rows verify hash-only. A database that
  older binaries cannot read is never produced.
- **Offline:** fully local. No network.
- **Degraded behavior:** a missing or unreadable key gives unsigned inserts and hash-only rollback with warnings.
  If the key file is missing, the next writable start creates a new key with a new epoch, and older rows fall at or
  below that epoch and verify hash-only (foreign key). If the key file exists but cannot be loaded (wrong length, wrong
  owner, I/O error), no new key is created. That session records revisions unsigned. They stay listable and diffable,
  but once the key loads again, any of them with an id above the epoch **cannot be restored** (fail closed, see
  Decision 5).
- **User visibility / editability:** no UI. Users can inspect, back up, or delete the key file. A tampered row fails
  rollback with `integrity check failed for revision N: HMAC signature mismatch` (or `content hash mismatch`). A row
  above the epoch without a valid signature fails with
  `revision N was recorded without a valid signature and cannot be restored; integrity cannot be verified`.

## Downgrade

v28 is additive (one nullable column). Binaries with the forward guard (`SUPPORTED_MAX_VERSION` < 28) open a v28
database read-only and do not migrate or write it. v0.6.0, which predates the guard, opens a v28 database read-write
and ignores the unknown column. Its inserts leave `hmac_signature` `NULL`. After upgrading again, those rows are still
listed and diffable, but any of them above the key's epoch **cannot be restored**: a database-only attacker can produce
exactly the same `NULL` signature, so CrossHook cannot tell the two apart and fails closed.

## Consequences

- Rollback refuses forged or signature-stripped revisions written after the epoch while the key is intact, with no UI
  changes. Restoring a database that is behind the key lowers the epoch, so this protection is never silently
  disabled.
- Rows recorded unsigned above the epoch (key-unavailable sessions, downgrades) cannot be restored (fail closed).
- One more `0600` secret file sits next to `metadata.db`. Backup and restore tooling should copy it together with the
  DB to keep signatures verifiable. Restoring a DB without its key still works: the new key's epoch covers every
  restored row, so they restore hash-only.
- New dependencies: `hmac 0.13` (digest 0.11, matching `sha2 0.11`) and `getrandom 0.4`, which was already in the
  lockfile.
