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
2. **Key.** A 32-byte key from the OS CSPRNG (`getrandom`) is stored as `config_history_hmac.key` in the same directory
   as `metadata.db` (normally `~/.local/share/crosshook/`), mode `0600`. Creation writes and fsyncs a private temp file
   in that directory, then publishes it with a no-replace rename/link (`tempfile::persist_noclobber`). An existing key
   is never overwritten; a process that loses the creation race reads the winner's key. Reads refuse symlinks and
   non-regular files and require exactly 32 bytes. Only writable stores create a key; read-only stores (newer schema,
   disabled) load an existing one. In-memory and disabled stores have no key.
3. **Signing.** `MetadataStore::insert_config_revision` signs every new row after the existing dedup check:
   HMAC-SHA256 over the domain-separated, length-prefixed message
   `tag ‖ profile_id ‖ content_hash ‖ snapshot_toml`, where each field is a u64 big-endian length followed by its bytes
   and `tag = "crosshook.config_revision.v1"`. The stored value is self-describing: `v1:<key_id>:<hex_mac>`, with
   `key_id` = the first 8 bytes (16 hex chars) of SHA-256(key). The key id does not reveal the key.
4. **Verification.** `MetadataStore::verify_config_revision(&ConfigRevisionRow)` in `crosshook-core` always re-checks
   the SHA-256 content hash, then verifies the HMAC in constant time (`Mac::verify_slice`) when the signature is `v1`
   and its key id matches the current key. It returns `RevisionIntegrity::Signed` or
   `RevisionIntegrity::HashOnly { reason }`, or `MetadataStoreError::IntegrityCheckFailed` (content-hash mismatch or
   signature mismatch). `profile_config_rollback` calls it in place of its old inline hash check and rejects the
   rollback on error.
5. **Degraded paths.** These cases fall back to the hash check, log a `tracing::warn!` (revision id, profile id, and
   reason only; never key material or MACs), and let the rollback proceed:
   - `NULL` signature (legacy row or key-less insert) → `HashOnly { Unsigned }`
   - a different key id (key file deleted and recreated, or a DB copied from another machine) → `HashOnly { ForeignKey }`
   - an unknown version prefix → `HashOnly { UnknownVersion }`
   - a missing or unreadable key → `HashOnly { KeyUnavailable }`

   If no key is available at insert time, the revision is still inserted, unsigned, with a warning. History is never
   dropped and dedup is unchanged.

6. **Flatpak import.** `crosshook/config_history_hmac.key` joins `DATA_INCLUDE_FILES`, the all-or-nothing metadata DB
   file set in `flatpak_migration`. It is copied after the `.db` with the same idempotency and rollback rules, and
   `fs::copy` keeps its `0600` mode, so imported signatures keep verifying inside the sandbox.
7. **IPC.** Unchanged. `ConfigRevisionRow` is not serialized, and `ConfigRevisionSummary` gains no field. Neither the
   key nor the signature crosses IPC.

## Threat model

- **In scope (tamper evidence):** edits made only through the database (direct SQLite edits, a third-party tool
  rewriting `metadata.db`, a DB file swapped in from elsewhere) and silent corruption. If the key file is intact, a
  forged `snapshot_toml` with a recomputed `content_hash` fails the HMAC check and rollback is refused.
- **Out of scope:** an attacker with filesystem access as the user. They can read the key and forge signatures, or
  edit the profile TOML directly. This is evidence, not protection. Replacing or deleting the key makes earlier
  signatures foreign, so they degrade to hash-only. That is deliberate, so a lost key never blocks recovery.
- **Not covered:** removing rows, or resetting `hmac_signature` to `NULL` (that row degrades to hash-only with a
  warning). Rollback still writes only snapshots whose content hash matches.

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
  Deleting the key recreates it on the next writable start, and older signatures report as foreign-key (hash-only).
- **User visibility / editability:** no UI. Users can inspect, back up, or delete the key file. An integrity failure
  appears as the existing rollback error string
  (`integrity check failed for revision N: HMAC signature mismatch`).

## Downgrade

v28 is additive (one nullable column). Binaries with the forward guard (`SUPPORTED_MAX_VERSION` < 28) open a v28
database read-only and do not migrate or write it. v0.6.0, which predates the guard, ignores the unknown column. Its
inserts leave `hmac_signature` `NULL`, and those rows verify hash-only after upgrading again.

## Consequences

- Rollback refuses forged revisions while the key is intact, with no UI changes.
- One more `0600` secret file sits next to `metadata.db`. Backup and restore tooling should copy it together with the
  DB to keep signatures verifiable. Restores without it still work, hash-only.
- New dependencies: `hmac 0.13` (digest 0.11, matching `sha2 0.11`) and `getrandom 0.4`, which was already in the
  lockfile.
