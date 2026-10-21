# Config history HMAC tamper evidence — plan

Tracking: Linear YAN-762 / GitHub yandy-r/crosshook#505. ADR: [`docs/architecture/adr-0005-config-history-hmac.md`](../../architecture/adr-0005-config-history-hmac.md).

## Goal

Every new `config_revisions` row carries an HMAC-SHA256 signature produced with a machine-local secret key. Rollback verifies the
signature (constant time) on top of the existing SHA-256 content-hash check, so a revision edited directly in `metadata.db`
(with a recomputed `content_hash`) is rejected instead of silently written back to the profile TOML.

## Decisions

| Topic         | Decision                                                                                                                                                                                                                                                                                                                                                          |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Schema        | Migration v27→v28 (`migrations/v27_v28.rs`) adds nullable `config_revisions.hmac_signature TEXT`. Existing rows stay `NULL`. `SUPPORTED_MAX_VERSION = 28`.                                                                                                                                                                                                        |
| Crates        | `hmac = "0.13"` (digest 0.11, matches `sha2 0.11`); `getrandom = "0.4"` (already in `Cargo.lock` via `tempfile`/`uuid`).                                                                                                                                                                                                                                          |
| Key           | 32 bytes from the OS CSPRNG. File `config_history_hmac.key` next to `metadata.db` (same directory as the store path), mode `0600`.                                                                                                                                                                                                                                |
| Key creation  | Written to a private temp file in the same directory, fsynced, then published with `persist_noclobber` (no-replace link/rename). Readers never see a partial key; an existing key is never overwritten; losing the race reads the winner's key. Only writable (`MetadataStatus::Ok`) stores create a key; read-only stores load an existing key only.             |
| Key exposure  | Never in SQLite, never logged (`Debug` shows only the key id), never crosses IPC. In-memory and disabled stores have no key.                                                                                                                                                                                                                                      |
| Message       | HMAC over length-prefixed fields: `tag`, `profile_id`, `content_hash`, `snapshot_toml`; each field = u64 big-endian length + bytes. Tag `crosshook.config_revision.v1`.                                                                                                                                                                                           |
| Stored format | `v1:<key_id>:<hex_mac>`; `key_id` = first 8 bytes (16 hex chars) of SHA-256(key).                                                                                                                                                                                                                                                                                 |
| Insert        | `MetadataStore::insert_config_revision` signs after the dedup check (dedup unchanged). No key ⇒ insert unsigned + `tracing::warn!`. History is never dropped for lack of a key.                                                                                                                                                                                   |
| Verify        | `MetadataStore::verify_config_revision(&ConfigRevisionRow) -> Result<RevisionIntegrity, MetadataStoreError>`. Content-hash mismatch or HMAC mismatch under the current key ⇒ `MetadataStoreError::IntegrityCheckFailed`. Legacy `NULL`, foreign `key_id`, unknown version prefix, or no key ⇒ `RevisionIntegrity::HashOnly { reason }` + warn; rollback proceeds. |
| IPC           | Unchanged. `ConfigRevisionRow` is not serialized; `ConfigRevisionSummary` does not gain any field. `profile_config_rollback` replaces its inline hash check with the core call.                                                                                                                                                                                   |
| Flatpak       | `crosshook/config_history_hmac.key` joins the metadata DB file set in `DATA_INCLUDE_FILES` (copied after the DB, same all-or-nothing idempotency and rollback; `fs::copy` keeps `0600`).                                                                                                                                                                          |

## Storage boundary

- HMAC key — filesystem secret (`config_history_hmac.key`, `0600`), not TOML, not SQLite.
- `hmac_signature` — SQLite metadata (`config_revisions`).
- Verification outcome — runtime-only (logs).
- No TOML settings change.

## Persistence & usability

- Migration: additive nullable column; old rows remain valid (hash-only). v28 DBs opened by older binaries with the forward guard open read-only; v0.6.0 ignores the column.
- Offline: fully local, no network.
- Degraded: missing/unreadable key ⇒ unsigned inserts and hash-only rollback with warnings. Deleting the key never blocks rollback; a new key is created on the next writable start and old signatures report as foreign key.
- Visibility: no UI; users can inspect/delete the key file. Integrity failures surface as the existing rollback error string.

## Tasks

1. Migration v27→v28 + ladder + version bump; fix tests that pin version literals or replay migrations.
2. `metadata/config_history_hmac.rs`: key load/create, key id, sign, verify; wire key into `MetadataStore::with_path`.
3. Store/ops: `hmac_signature` in insert/list/get; `RevisionIntegrity` + `verify_config_revision`; new error variant.
4. Tauri `profile_config_rollback` → core verification.
5. Flatpak migration key copy + test.
6. Tests: sign/verify round trip, tampered snapshot rejected, legacy → HashOnly, foreign key id → HashOnly, missing key → unsigned + HashOnly, migration preserves rows, key file `0600` and not overwritten.
7. Docs: ADR-0005, CLAUDE.md / AGENTS.md / `.cursor/rules/project.mdc` schema notes, ADR-0004 include-file list.

## Verification

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- `cargo check --manifest-path src/crosshook-native/Cargo.toml --workspace`
- `cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings`
- `./scripts/lint.sh`, `./scripts/check-schema-version.sh`, `./scripts/check-host-gateway.sh`
