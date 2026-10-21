# Config history HMAC tamper evidence — plan

Tracking: Linear YAN-762 / GitHub yandy-r/crosshook#505. ADR: [`docs/architecture/adr-0005-config-history-hmac.md`](../../architecture/adr-0005-config-history-hmac.md).

## Goal

Every new `config_revisions` row carries an HMAC-SHA256 signature produced with a machine-local secret key. Rollback verifies the
signature (constant time) on top of the existing SHA-256 content-hash check, so a revision edited directly in `metadata.db`
(with a recomputed `content_hash`) is rejected instead of silently written back to the profile TOML.

## Decisions

| Topic         | Decision                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Schema        | Migration v27→v28 (`migrations/v27_v28.rs`) adds nullable `config_revisions.hmac_signature TEXT`. Existing rows stay `NULL`. `SUPPORTED_MAX_VERSION = 28`.                                                                                                                                                                                                                                                                                                   |
| Crates        | `hmac = "0.13"` with `zeroize` (digest 0.11, matches `sha2 0.11`); `getrandom = "0.4"` and `zeroize = "1"` (both already in `Cargo.lock`).                                                                                                                                                                                                                                                                                                                   |
| Key           | 40-byte file `config_history_hmac.key` next to `metadata.db`, mode `0600`: 32-byte CSPRNG key + u64 BE signing epoch (`config_revisions` AUTOINCREMENT high-water mark at key creation). Load: `O_NOFOLLOW`, fstat owner = euid, group/other bits reset to `0600`, exact length. On writable open, an epoch above the DB high-water mark (DB recreated/restored) is lowered atomically (same key bytes; temp + fsync + rename); never raised, never rotated. |
| Key creation  | Written to a private temp file in the same directory, fsynced, then published with `persist_noclobber` (no-replace link/rename). Readers never see a partial key; an existing key is never overwritten; losing the race reads the winner's key. Only writable (`MetadataStatus::Ok`) stores create a key; read-only stores load an existing key only.                                                                                                        |
| Key exposure  | Never in SQLite, never logged (`Debug` shows only the key id and epoch), never crosses IPC; key bytes held in `Zeroizing` buffers. In-memory and disabled stores have no key.                                                                                                                                                                                                                                                                                |
| Message       | HMAC over length-prefixed fields (u64 BE length + bytes): `tag`, `id`, `profile_id`, `source`, `created_at`, `content_hash`, `snapshot_toml`, `trust`. Tag `crosshook.config_revision.v1`. Signed after insert in the same savepoint (binds the assigned id).                                                                                                                                                                                                |
| Stored format | `v1:<key_id>:<trust>:<hex_mac>`; `key_id` = first 8 bytes (16 hex chars) of SHA-256(key); `trust` = `verified`, or `hash_only` for a rollback whose source does not re-verify as `Signed`.                                                                                                                                                                                                                                                                   |
| Insert        | `MetadataStore::insert_config_revision` signs after the dedup check (dedup unchanged). No key ⇒ insert unsigned + `tracing::warn!`. History is never dropped, but unsigned rows above the epoch are not restorable once a key loads (fail closed).                                                                                                                                                                                                           |
| Verify        | `MetadataStore::verify_config_revision(&ConfigRevisionRow) -> Result<RevisionIntegrity, MetadataStoreError>`. Hash mismatch ⇒ error. Key loaded and `id > epoch` ⇒ only a valid current-key signature passes (stripped/foreign/unknown ⇒ `SignatureRequired`, MAC/trust mismatch ⇒ `SignatureMismatch`). Pre-epoch unsigned/foreign rows, `hash_only` trust, or no key ⇒ `HashOnly { reason }` + warn.                                                       |
| IPC           | Unchanged. `ConfigRevisionRow` is not serialized; `ConfigRevisionSummary` does not gain any field. `profile_config_rollback` replaces its inline hash check with the core call.                                                                                                                                                                                                                                                                              |
| Flatpak       | `DATA_INCLUDE_FILES` stays the DB trio (only it decides "already imported"). With a DB import only: a leftover sandbox key is renamed to `config_history_hmac.key.orphaned-<unix_ts>` (never deleted), then the host key is copied with `0600`; rollback restores the original sandbox key.                                                                                                                                                                  |

## Storage boundary

- HMAC key — filesystem secret (`config_history_hmac.key`, `0600`), not TOML, not SQLite.
- `hmac_signature` — SQLite metadata (`config_revisions`).
- Verification outcome — runtime-only (logs).
- No TOML settings change.

## Persistence & usability

- Migration: additive nullable column; old rows remain valid (hash-only). v28 DBs opened by older binaries with the forward guard open read-only; v0.6.0 writes `NULL` signatures, and those rows are not restorable after upgrading again if above the epoch (fail closed).
- Offline: fully local, no network.
- Degraded: a missing key is recreated on the next writable start, and older signatures report as foreign key (hash-only). An unreadable key file means that session's revisions are recorded unsigned: listable/diffable, not restorable once the key loads again.
- Visibility: no UI; users can inspect/delete the key file. Integrity failures surface as rollback errors; unsigned post-epoch rows report "recorded without a valid signature and cannot be restored".

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
