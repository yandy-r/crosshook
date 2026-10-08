# Metadata DB schema policy

## Purpose

`metadata.db` (`~/.local/share/crosshook/metadata.db`) is migrated forward only. Without a guard, an older binary opens
a newer DB, ignores unknown schema, and can corrupt or silently drop data. From v0.6.1 the binary refuses to modify a DB
newer than it understands and backs up older DBs before migrating them (YAN-851).

## `SUPPORTED_MAX_VERSION` contract

- Defined in `src/crosshook-native/crates/crosshook-core/src/metadata/migrations/mod.rs` as
  `pub const SUPPORTED_MAX_VERSION: u32 = N;` (one line, exact form — `scripts/check-schema-version.sh` parses it).
- `N` is the highest `PRAGMA user_version` this binary can migrate to and safely write.
- It must equal the latest migration number, `**Current schema version**` in `CLAUDE.md`, and
  `**Current schema version**` in `AGENTS.md`.

## Behaviour matrix

`user_version` of the DB on disk vs. the running binary:

| Binary          | DB older (`0 < v < max`)                                       | DB equal (`v == max`) | DB newer (`v > max`)                                                             | DB fresh (`v == 0`)         |
| --------------- | -------------------------------------------------------------- | --------------------- | -------------------------------------------------------------------------------- | --------------------------- |
| ≤ v0.6.0        | Migrates, no backup                                            | Normal                | **Unsafe**: no guard, opens read/write and may corrupt or lose newer data        | Normal migration            |
| ≥ v0.6.1 (this) | Backup, then migrate. Backup failure → no migration, read-only | Normal                | No migration, no writes. Opened read-only; read-only open fails → store disabled | Normal migration, no backup |

Degraded modes (read-only, disabled):

- Reads work in read-only mode. Writes return a typed error; callers degrade as they do for a disabled store.
- The UI shows a dismissible banner. The diagnostics export includes the store status and the backup file list.
- Migration holds an SQLite write lock (`BEGIN IMMEDIATE`) and re-reads `user_version` before migrating, so a newer
  binary upgrading the DB concurrently is detected and the store falls back to read-only instead of migrating.
- WAL sidecars (`-wal`, `-shm`) may be created by SQLite in read-only mode; the main DB content is not modified.

## Backups

- Created only when `0 < user_version < SUPPORTED_MAX_VERSION`, before any migration step runs.
- Method: WAL-safe `VACUUM INTO` (not a file copy, so the WAL is included).
- Name: `metadata.db.bak-v<from>-<UTC %Y%m%dT%H%M%SZ>`, beside the DB, for example
  `metadata.db.bak-v26-20261008T120000Z`. Append `-<collision>` if timestamp name already exists.
- Permissions: `0600`.
- Retention: keep newest 2 exact matching regular files, ordered by timestamp and then filename; the newly created backup counts toward the two. Older backups are deleted only after a successful backup.
- Migration runs offline; no network access is required.

Restore manually:

1. Quit all CrossHook binaries.
2. Move current `metadata.db` and any `metadata.db-wal` / `metadata.db-shm` sidecars together into a safekeeping directory. Preserve these originals before restoring.
3. Copy the chosen standalone backup to `metadata.db` and set permissions to `0600`.
4. Start a binary whose `SUPPORTED_MAX_VERSION` is ≥ `<from>`.

Restored history omits changes made after backup time; preserved originals retain those changes. The Rust core migration regression test in `src/crosshook-native/crates/crosshook-core/src/metadata/migrations/tests/` must verify latest migration version equals `SUPPORTED_MAX_VERSION`; shell check intentionally checks only exact constant/document agreement.

## Rule for every schema bump (v28 onward)

A PR that adds a migration must, in the same PR:

1. Bump `SUPPORTED_MAX_VERSION` to the new version.
2. Update `**Current schema version**` in `CLAUDE.md` and `AGENTS.md`.
3. Add the migration note to `CLAUDE.md` and the table row to `AGENTS.md`.
4. Update the matrix above if behaviour changes.
5. Keep the core Rust regression test in `src/crosshook-native/crates/crosshook-core/src/metadata/migrations/tests/` green: it asserts the latest migration leaves `user_version` equal to `SUPPORTED_MAX_VERSION`. Do not re-implement that invariant in shell.

## Enforcement

`./scripts/check-schema-version.sh` compares the constant with both docs and fails on mismatch or if any value cannot
be found. It runs as part of `./scripts/lint.sh` (default and `--schema-version`). It does not apply to a DB already
on disk — only the docs-versus-code drift.
