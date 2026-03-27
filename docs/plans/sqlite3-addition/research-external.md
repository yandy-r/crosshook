# SQLite3 Addition - External Research

_Second-pass review completed 2026-03-27. All version numbers, URLs, and behavioral claims re-verified against upstream documentation and crates.io._

## Executive Summary

SQLite is a strong fit for CrossHook's secondary local data layer because it provides durable relational storage, transactional history, efficient indexing, and optional JSON and FTS capabilities without introducing a separate service. The most relevant operational guidance is to explicitly enable foreign keys per connection, use WAL mode for responsive desktop reads during writes, use UPSERT for idempotent sync from TOML and filesystem scans, and treat `application_id`/`user_version` as part of a stable on-disk contract. For Rust integration, `rusqlite` is the most natural match for the current synchronous, local-first architecture and lets CrossHook adopt SQLite without forcing async SQL abstractions across `crosshook-core`.

**Second-pass key corrections and enhancements:**
- `rusqlite` current stable is **0.39.0** (released 2025-03-15); bundles **SQLite 3.51.3** (fixed the WAL-reset data-race bug present in 3.7.0–3.51.2).
- JSONB binary format was added in SQLite **3.45.0** (2024-01-15), not just implied by 3.38.0 JSON improvements.
- UPSERT requires SQLite ≥ 3.24.0 (2018-06-04); the multi-clause form requires ≥ 3.35.0.
- WAL mode is **persistent** across connections once set — no need to re-enable it on every open.
- `rusqlite_migration` 2.5.0 is the recommended companion for schema versioning (uses `user_version` internally).
- FTS5 requires the `SQLITE_ENABLE_FTS5` compile flag; the `bundled` feature in `libsqlite3-sys` includes FTS5 by default.
- `BEGIN IMMEDIATE` should be used for write transactions to avoid `SQLITE_BUSY` upgrade races.

---

## Primary APIs

### SQLite Core

- **Documentation URL**: <https://www.sqlite.org/docs.html>
- **Auth model**: none; embedded local database
- **Pricing**: public domain / no service cost
- **Rate limits**: none beyond local disk and process limits

#### WAL Mode

- **URL**: <https://www.sqlite.org/wal.html>
- Enable with: `PRAGMA journal_mode=WAL;` — returns `"wal"` on success.
- **Persistent**: once set, WAL mode survives connection close/reopen. No need to re-issue per connection.
- Sidecar files: `database.db-wal` (~4 MB typical) and `database.db-shm` (~32 KB); auto-deleted when the last connection closes cleanly.
- Backup requirement: both sidecar files must accompany the main database file during any file-level backup; alternatively run `PRAGMA wal_checkpoint(TRUNCATE)` or switch back to DELETE mode before copying.
- **WAL-reset bug**: versions 3.7.0–3.51.2 had a rare data-race that could corrupt a WAL database under simultaneous concurrent writes. Fixed in 3.51.3. `rusqlite` 0.39.0 bundles 3.51.3, so the `bundled` feature is safe.
- Checkpoint modes: `PASSIVE` (default, non-blocking), `FULL`, `RESTART`, `TRUNCATE`. Automatic checkpoint triggers at 1,000 pages (≈4 MB); configurable via `PRAGMA wal_autocheckpoint`.
- **Limitation**: WAL does not work on network filesystems; requires all processes to share memory on the same host. Not an issue for a desktop AppImage.

#### PRAGMA Reference

| PRAGMA | URL | Key behavior |
|---|---|---|
| `foreign_keys` | <https://www.sqlite.org/pragma.html#pragma_foreign_keys> | OFF by default; must be set per connection; no-op inside a transaction |
| `application_id` | <https://www.sqlite.org/pragma.html#pragma_application_id> | 32-bit signed int at file offset 68; marks the file as CrossHook-owned for tools like `file(1)` |
| `user_version` | <https://www.sqlite.org/pragma.html#pragma_user_version> | 32-bit int at file offset 60; used by `rusqlite_migration` to track schema version |
| `optimize` | <https://www.sqlite.org/pragma.html#pragma_optimize> | Run before closing short-lived connections or periodically on long-lived ones; runs selective `ANALYZE` |
| `integrity_check` | <https://www.sqlite.org/pragma.html#pragma_integrity_check> | Does NOT check FK violations; use `foreign_key_check` separately |
| `foreign_key_check` | <https://www.sqlite.org/pragma.html#pragma_foreign_key_check> | Returns one row per FK violation; can target a single table |
| `synchronous` | <https://www.sqlite.org/pragma.html#pragma_synchronous> | `NORMAL` is safe and sufficient with WAL; FULL required for rollback mode durability |
| `busy_timeout` | <https://www.sqlite.org/pragma.html#pragma_busy_timeout> | Set on every new connection; controls wait-and-retry before returning `SQLITE_BUSY` |
| `cache_size` | <https://www.sqlite.org/pragma.html#pragma_cache_size> | Default -2000 (≈2 MB); negative values = kibibytes |
| `journal_mode` | <https://www.sqlite.org/pragma.html#pragma_journal_mode> | Cannot change while a transaction is active |

**PRAGMA silent-failure gotcha**: some PRAGMAs silently ignore unknown names and return without error; setup code must verify the effective value by re-reading the PRAGMA rather than assuming success.

#### UPSERT

- **URL**: <https://sqlite.org/lang_upsert.html>
- Available since SQLite **3.24.0** (2018-06-04).
- Multi-conflict-clause form (multiple `ON CONFLICT` without a target) available since **3.35.0** (2021-03-12).
- Syntax: `INSERT INTO t (...) VALUES (...) ON CONFLICT(col) DO UPDATE SET col = excluded.col`
- `excluded.col` references the value that would have been inserted.
- Only applies to uniqueness constraint violations (UNIQUE, PRIMARY KEY, unique indexes). NOT NULL, CHECK, and FK violations are not handled by UPSERT.
- DO UPDATE always uses ABORT resolution: any constraint violation inside the UPDATE rolls back the entire INSERT.

#### JSON Functions

- **URL**: <https://www.sqlite.org/json1.html>
- JSON1 (`json_extract`, `json_set`, `json_valid`, `json_each`, `json_tree`) built-in by default since **3.38.0** (2022-02-22). Can be omitted via `-DSQLITE_OMIT_JSON` compile flag.
- **JSONB** (binary JSON storage): added in **3.45.0** (2024-01-15). Functions prefixed `jsonb_` return binary blobs; `json_*` functions accept both text and JSONB blobs as input. JSONB avoids re-parsing on read and uses slightly less disk space.
- **URL (JSONB)**: <https://sqlite.org/jsonb.html>

#### FTS5

- **URL**: <https://www.sqlite.org/fts5.html>
- Introduced in SQLite **3.9.0** (2015-10-14).
- Requires `SQLITE_ENABLE_FTS5` compile flag. The `libsqlite3-sys` `bundled` feature enables FTS5 by default.
- Supports: unicode61 (default), ascii, porter, and trigram tokenizers.
- **External-content mode**: stores only the FTS index; the actual rows live in a normal table with triggers keeping them in sync. Preferred pattern for CrossHook since game/trainer rows have other indexed columns.
- **Trigram tokenizer**: enables substring and LIKE/GLOB queries without prefix knowledge — useful for fuzzy game-name search.
- Prefix indexes speed up prefix queries; enable with `prefix='2 3'` option.
- Auxiliary functions: `bm25()` for relevance ranking, `highlight()`, `snippet()`.
- Maintenance: `INSERT INTO fts(fts) VALUES('optimize')` to compact; `rebuild` to regenerate from content table.
- **Caution**: FTS5 availability depends on the SQLite actually linked into the binary. With `bundled` it is guaranteed; with system SQLite it may not be present.

---

## Libraries and SDKs

| Library | Version | Purpose | Installation |
|---|---|---|---|
| `rusqlite` | **0.39.0** (2025-03-15) | Primary embedded SQLite access in Rust | `cargo add rusqlite --features bundled` |
| `libsqlite3-sys` | transitive (via rusqlite) | Low-level C bindings; bundled SQLite **3.51.3** | via `rusqlite` features |
| `rusqlite_migration` | **2.5.0** | Schema migration using `user_version` | `cargo add rusqlite_migration` |
| `r2d2_sqlite` | latest | r2d2 connection pool adapter for rusqlite | optional if threading needed |
| `sqlite3` CLI | system tool | Local inspection, ad-hoc migration, debugging | distro package |

### rusqlite 0.39.0 Feature Flags (46 total)

Key flags for CrossHook:

| Flag | Purpose |
|---|---|
| `bundled` | Compile and link SQLite 3.51.3 statically; enables `modern_sqlite`; required for AppImage |
| `bundled-full` | Like `bundled` but also enables `modern-full` feature set |
| `serde_json` | `FromSql`/`ToSql` for `serde_json::Value` — store JSON payloads directly |
| `uuid` | `FromSql`/`ToSql` for `uuid::Uuid` stored as BLOBs |
| `chrono` | `FromSql`/`ToSql` for `chrono` date/time types |
| `modern_sqlite` | Uses bundled bindings for contemporary SQLite API surface |
| `backup` | Exposes the `sqlite3_backup_*` online backup API |
| `session` | Change-set/patch-set session API (requires `hooks`) |
| `hooks` | Data-change notification callbacks |
| `trace` | Query trace and profile hooks |
| `vtab` | Virtual table API |
| `window` | Window functions |
| `modern-full` | All of the above combined |

**Full list**: `cache`, `hashlink`, `array`, `backup`, `blob`, `buildtime_bindgen`, `bundled`, `bundled-full`, `bundled-sqlcipher`, `bundled-sqlcipher-vendored-openssl`, `bundled-windows`, `chrono`, `collation`, `column_decltype`, `column_metadata`, `csv`, `csvtab`, `extra_check`, `fallible_uint`, `functions`, `hooks`, `i128_blob`, `in_gecko`, `jiff`, `limits`, `load_extension`, `loadable_extension`, `modern-full`, `modern_sqlite`, `pointer`, `preupdate_hook`, `rusqlite-macros`, `serde_json`, `serialize`, `series`, `session`, `sqlcipher`, `time`, `trace`, `unlock_notify`, `url`, `uuid`, `vtab`, `wasm32-wasi-vfs`, `window`, `with-asan`.

**Confidence**: High — verified from [docs.rs/crate/rusqlite/latest/features](https://docs.rs/crate/rusqlite/latest/features) and [GitHub releases](https://github.com/rusqlite/rusqlite/releases).

---

## Integration Patterns

### Connection Setup

Every connection must be configured explicitly; do not rely on defaults:

```rust
let conn = Connection::open(path)?;
// WAL is persistent but explicit on first open is idiomatic
conn.execute_batch("
    PRAGMA journal_mode=WAL;
    PRAGMA foreign_keys=ON;
    PRAGMA synchronous=NORMAL;
    PRAGMA busy_timeout=5000;
")?;
```

After executing, re-read `journal_mode` and `foreign_keys` to verify they took effect — silent PRAGMA failure is a real gotcha.

### Write Transactions

Use `BEGIN IMMEDIATE` for any transaction that will write. Starting with `BEGIN DEFERRED` (the default) and then issuing a write forces SQLite to upgrade the lock, which can return `SQLITE_BUSY` with no retry. `BEGIN IMMEDIATE` acquires the reserved lock upfront.

```rust
let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
// ... writes ...
tx.commit()?;
```

Use savepoints for per-profile sync units inside a longer import transaction:

```rust
let sp = tx.savepoint()?;
// ... per-profile work ...
sp.commit()?;
```

### UPSERT-Based Reconciliation

Idempotent sync from filesystem/TOML scans:

```sql
INSERT INTO games (steam_app_id, name, last_seen_at)
VALUES (?1, ?2, ?3)
ON CONFLICT(steam_app_id) DO UPDATE SET
    name = excluded.name,
    last_seen_at = excluded.last_seen_at;
```

Repeated sync passes are safe and crash-recoverable.

### Schema Migrations with rusqlite_migration

`rusqlite_migration` 2.5.0 uses `PRAGMA user_version` as its version counter (no migration table needed). Define migrations as SQL strings in Rust, call `Migrations::to_latest()` on startup. **Do not modify `user_version` outside this crate** — it will desync the migration state.

```rust
let migrations = Migrations::new(vec![
    M::up("CREATE TABLE games (...)"),
    M::up("ALTER TABLE games ADD COLUMN last_seen_at INTEGER"),
]);
migrations.to_latest(&mut conn)?;
```

### Connection Management for CrossHook

CrossHook's synchronous, single-process architecture fits a simple pattern:
- **One dedicated write connection** (long-lived, WAL, IMMEDIATE transactions).
- **Short-lived read connections** opened on demand — WAL allows concurrent reads with no blocking.
- For future async/multi-thread, `r2d2_sqlite` (sync pool) or `deadpool-sqlite` (async) are available.

### PRAGMA optimize

Run before closing short-lived connections, or hourly/daily for long-lived ones. After bulk sync or index refresh, run with mask `0x10002` to analyze all tables:

```sql
PRAGMA optimize=0x10002;
```

---

## Constraints and Gotchas

1. **Foreign keys disabled by default** — must enable per connection; is a no-op inside an open transaction. Set in connection initialization before any other work.
2. **PRAGMA silent failures** — unknown or invalid PRAGMA names are ignored without error. Always verify effective values by re-reading the PRAGMA.
3. **WAL sidecar files** — `.db-wal` and `.db-shm` must travel with the main database file. File-level backup without them may lose committed transactions. Run `PRAGMA wal_checkpoint(TRUNCATE)` before archiving, or include sidecars.
4. **WAL-reset bug (historical)** — data corruption possible in SQLite 3.7.0–3.51.2 under extreme concurrent write/checkpoint timing. Fixed in 3.51.3. Using `rusqlite`'s `bundled` feature at 0.39.0 guarantees 3.51.3.
5. **SQLITE_BUSY with lock upgrades** — deferred transactions that attempt writes can return `SQLITE_BUSY` immediately even with a busy timeout set. Always use `BEGIN IMMEDIATE` for write transactions.
6. **FTS5 availability** — FTS5 requires `SQLITE_ENABLE_FTS5` at compile time. Bundled SQLite in `rusqlite` includes it; system SQLite on a user's machine may not.
7. **JSONB availability** — JSONB (`jsonb_*` functions) requires SQLite ≥ 3.45.0. The `bundled` feature (3.51.3) includes it; system SQLite on older distros may not.
8. **user_version ownership** — `rusqlite_migration` owns `user_version`; no other code should read/write it or migration state will desync.
9. **UPSERT constraint scope** — UPSERT only handles uniqueness constraint violations. NOT NULL, CHECK, and foreign key violations are not intercepted.
10. **SQLite is not a file-system authority** — filesystem artifacts (launcher scripts, logs, Proton prefixes, tap worktrees, TOML profiles) remain canonical; SQLite is a cache/projection layer, not the source of truth.

---

## Open Decisions

1. **Bundle vs. system SQLite**: The `bundled` feature (via `libsqlite3-sys`) statically links SQLite 3.51.3 into the AppImage binary. This guarantees FTS5, JSONB, the WAL-reset fix, and predictable behavior across all user distros. System SQLite on older Debian/Ubuntu-based SteamOS may be significantly older. **Recommendation: use `bundled` for the AppImage target.**
2. **FTS5 in v1**: Regular B-tree indexes with `LIKE` queries cover basic game/trainer name lookup. FTS5 with the trigram tokenizer adds substring and fuzzy match without prefix knowledge. Deferring FTS5 to v2 is reasonable; the schema can be extended later without data loss.
3. **JSONB vs. JSON text columns**: `jsonb_*` functions (≥ 3.45.0) reduce parse overhead for diagnostic payloads stored in blob columns. If all query-relevant fields are promoted to first-class columns (recommended), JSON/JSONB columns become opaque storage only and the distinction is minor.
4. **rusqlite_migration vs. hand-rolled migrations**: `rusqlite_migration` 2.5.0 is simple, well-maintained, and avoids a custom migration table by reusing `user_version`. It is the pragmatic choice unless CrossHook needs migration reversibility (which the crate does not support).
5. **Connection pool**: The current synchronous architecture needs at most one write connection and on-demand read connections. r2d2_sqlite or deadpool-sqlite are available if background async work is introduced later.

---

## Sources

- SQLite documentation: <https://www.sqlite.org/docs.html>
- SQLite WAL: <https://www.sqlite.org/wal.html>
- SQLite PRAGMAs: <https://www.sqlite.org/pragma.html>
- SQLite UPSERT: <https://sqlite.org/lang_upsert.html>
- SQLite ON CONFLICT: <https://sqlite.org/lang_conflict.html>
- SQLite JSON functions: <https://www.sqlite.org/json1.html>
- SQLite JSONB format: <https://sqlite.org/jsonb.html>
- SQLite FTS5: <https://www.sqlite.org/fts5.html>
- SQLite foreign keys: <https://sqlite.org/foreignkeys.html>
- rusqlite crates.io: <https://crates.io/crates/rusqlite>
- rusqlite docs: <https://docs.rs/rusqlite/latest/rusqlite/>
- rusqlite feature flags: <https://docs.rs/crate/rusqlite/latest/features>
- rusqlite releases: <https://github.com/rusqlite/rusqlite/releases>
- rusqlite_migration docs: <https://docs.rs/rusqlite_migration/latest/rusqlite_migration/>
- libsqlite3-sys crates.io: <https://crates.io/crates/libsqlite3-sys>
- r2d2_sqlite: <https://docs.rs/r2d2_sqlite/>
- JSONB in SQLite 3.45 (Fedora Magazine): <https://fedoramagazine.org/json-and-jsonb-support-in-sqlite-3-45-0/>
- WAL-reset bug discussion: <https://www.sqlite.org/wal.html>
