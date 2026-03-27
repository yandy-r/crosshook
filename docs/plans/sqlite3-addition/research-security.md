# Security Research: sqlite3-addition

## Executive Summary

The SQLite metadata layer is a local-only, single-user store with no network-accessible attack surface — the overall security posture is good. The most actionable risks are operational: world-readable default file permissions on the database (and WAL/SHM sidecars) exposing aggregated launch history and trainer paths, inconsistent application of path sanitization across new IPC commands, and unbounded payload sizes for cached external content. No SQL injection vectors exist if rusqlite's parameterized query API is used consistently, but that discipline must be enforced at review time because the API does not prevent dynamic query string construction at compile time. Use `rusqlite` 0.39.0+ which bundles SQLite 3.51.3 — this version resolves all known CVEs including a rare WAL write+checkpoint data-corruption race present in SQLite 3.7.0–3.51.2.

_Last updated with input from: api-researcher, business-analyzer, tech-designer, practices-researcher, ux-researcher._

---

## Findings by Severity

### CRITICAL — Hard Stops

No critical findings identified.

The database is local-only, single-user, and stores no credentials or secrets. CrossHook does not accept SQL from external sources, and rusqlite's parameterized query API prevents injection when used correctly. The threat model does not include remote code execution via SQLite.

---

### WARNING — Must Address

| # | Finding | Risk | Suggested Mitigation | Alternatives |
|---|---------|------|----------------------|--------------|
| W1 | **World-readable DB permissions** | `metadata.db`, `-wal`, and `-shm` sidecar files created with default umask permissions (typically `0644`) are readable by any process on the same machine. The DB is unique compared to individual TOML profile files because it **aggregates** launch history, trainer paths, usage patterns, and community tap URLs into a single queryable file — making it both higher-value and easier to exfiltrate than the scattered TOML files. Note: the existing codebase does not use explicit `set_permissions` for TOML files; the argument for doing so here is the aggregate sensitivity of the SQLite file. | Explicitly `chmod 0600` the DB file immediately after creation using `std::fs::set_permissions` before opening for reads/writes. SQLite inherits WAL/SHM permissions from the DB file. Parent directory `~/.local/share/crosshook/` should be `0700`. | If treated as consistent with current TOML file handling (no explicit chmod), downgrade to ADVISORY — the threat requires a second process running as the same user, which implies a compromised session. Security team recommendation is to enforce 0600 given the aggregated sensitivity. |
| W2 | **Inconsistent path sanitization in new IPC commands** | Existing launch commands apply `sanitize_display_path()` before emitting events to the frontend (replacing `$HOME` with `~`). New SQLite-backed commands — `get_profile_catalog()`, launch history responses, launcher drift responses — will return stored paths. If path sanitization is not applied to these new code paths, full home directory paths leak to the frontend and into any user-facing error messages or logs. | Create a shared `sanitize_path_for_display(path: &str) -> String` utility in `crosshook-core` and require its use in every function that returns a path string across the Tauri IPC boundary. | Apply sanitization at the Tauri command boundary as a serialization step, so all path fields in response structs pass through it before serialization. |
| W3 | **Unbounded cached payload sizes** | `external_cache_entries.payload_json` stores cached ProtonDB and cover-art responses without size limits. `launch_operations.diagnostic_summary` stores arbitrary diagnostic text. A malformed or adversarial response from an external API (or a mitm) could insert megabytes of JSON into the local database, causing memory pressure during deserialization and unbounded disk growth. | Enforce a maximum payload size before writing to `external_cache_entries` (e.g., 512 KB per entry) and a maximum length for `diagnostic_summary` (e.g., 4 KB). Reject or truncate payloads that exceed limits. | Store only validated, schema-conforming fields from ProtonDB responses as first-class columns rather than raw JSON blobs, which bounds size by schema definition. |
| W4 | **SQL injection via dynamic query construction** | rusqlite's API accepts `&str` query strings, meaning `conn.execute(&format!("... WHERE name='{name}'"), [])` is syntactically valid and produces a SQL injection vector. If any sync, search, or filter operation in the metadata module builds SQL strings through string interpolation instead of parameterized placeholders, untrusted data from TOML profiles or community manifests could execute arbitrary SQL against the local database. | Enforce a code review rule: **all SQL strings in the metadata module must be string literals; no `format!()` calls inside SQL strings.** Use `rusqlite::params![]` or tuple parameters for every bound value. | Use a thin query builder that accepts parameters separately from query structure, making dynamic conditions (e.g., optional filters) explicit and injection-safe. |
| W5 | **Symlink attack on DB creation** | If a malicious process creates a symlink at `~/.local/share/crosshook/metadata.db` before CrossHook's first run, SQLite will follow the symlink and open whatever file it points to. This could corrupt an unrelated file the user owns, or cause CrossHook to read attacker-controlled data as the database. | After resolving the DB path and before opening it, verify that if the path exists it is a regular file (not a symlink) using `std::fs::symlink_metadata`. If a symlink is found, fail loudly with an actionable error rather than following it. | This attack requires local write access to `~/.local/share/crosshook/` before first run, which implies a compromised user session — acceptable to classify as defense-in-depth rather than hard stop. Defer to ADVISORY if the threat model does not include a compromised user session. |
| W6 | **SQLite names from DB used in filesystem operations without re-validation** | `launch_operations.helper_log_path` and `launchers.expected_script_path` are stored path strings that are later used to open files or determine launcher ownership. If these values are used directly in filesystem calls without passing through `validate_name()` or a path-safety check, a corrupted or tampered DB could cause path traversal operations. Identified by business-analyzer from code review of `validate_name()` usage. | When reading path strings from SQLite for use in any filesystem operation (file open, delete, rename), apply the same validation checks used when they were first derived. Never assume stored data is safe by virtue of having been stored. | Lower priority if only the app itself writes the DB — the threat requires DB tampering by a separate process. Still correct practice for defense in depth. |
| W7 | **`execute_batch()` for PRAGMA setup must receive only hard-coded strings** | rusqlite's `execute_batch()` accepts a `&str` and executes multiple SQL statements. It is used (and proposed) for PRAGMA setup at connection open. If any non-literal string is passed (e.g., a config-derived PRAGMA value), it creates a SQL injection vector because `execute_batch()` does not support parameterized values. Identified by api-researcher. | `execute_batch()` calls for PRAGMA setup must **only** receive string literals defined in source code. Never pass config values, user input, or runtime-derived strings to `execute_batch()`. PRAGMAs with variable values (e.g., `PRAGMA user_version = X`) must use `conn.pragma_update()` instead. | Not deferrable — applies to Phase 1 connection bootstrap code. |
| W8 | **Community tap `description` and `game_name` fields rendered in React WebView without escaping** | Community manifest fields (`game_name`, `trainer_name`, `author`, `description`) come from untrusted git repositories and will be displayed in the CommunityBrowser component. React's JSX renders text content safely via `textContent` by default, but if any component uses `dangerouslySetInnerHTML` or concatenates these values into raw HTML/CSS, XSS is possible in the Tauri WebView. Identified by ux-researcher. | Audit all React components that render community tap manifest fields. Never use `dangerouslySetInnerHTML` for user-supplied or externally-sourced strings. Prefer `{value}` interpolation (which React escapes) over raw HTML. Apply the same audit to rename disambiguation prompts which display TOML-derived filenames. | React escapes by default, so this only applies if `dangerouslySetInnerHTML` is present. Verify at implementation time — the risk is contained if standard JSX interpolation is used throughout. |

---

### ADVISORY — Best Practices

| # | Finding | Benefit | Recommendation | Defer Justification |
|---|---------|---------|----------------|---------------------|
| A1 | **Bundled SQLite version tracking (ongoing)** | rusqlite 0.39.0 bundles SQLite 3.51.3 which resolves all current CVEs and the WAL-reset data-corruption bug. Future rusqlite releases will bundle newer SQLite versions. Bundled SQLite provides AppImage predictability (no distro version variance) but places update responsibility on CrossHook. | Use `rusqlite = { version = "0.39", features = ["bundled"] }` as the initial pin. Add CI or Dependabot tracking for rusqlite version updates. When a new rusqlite release bundles a newer SQLite version, evaluate and update. | Not deferrable for the initial dependency choice (use 0.39.0). The ongoing tracking process is deferrable for later cycles. |
| A2 | **`PRAGMA secure_delete=ON`** | SQLite's default behavior leaves deleted row data in free pages on disk; it is not overwritten until those pages are reused. For launch history rows (containing trainer paths and exit codes) that users delete, residual data persists until vacuumed. | Enable `PRAGMA secure_delete=ON` at connection setup alongside `foreign_keys=ON` and `journal_mode=WAL`. Minimal performance impact for CrossHook's expected write volume. | Acceptable to defer — this is defense-in-depth for a local single-user database. The primary threat model is user-controlled local storage, not forensic recovery by a third party. |
| A3 | **Error message information leakage through IPC** | rusqlite errors include SQL statement text, table names, and constraint names. If a Tauri command propagates a raw `rusqlite::Error::SqliteFailure` as a string to the frontend, it exposes internal schema structure, column names, and potentially query parameters to any process that can inspect the frontend's IPC messages. | Map rusqlite errors to typed application errors at the metadata module boundary. Log full error detail at `tracing::error!` for local debugging, but return generic categorized errors across the IPC boundary (e.g., `MetadataError::DbUnavailable`, `MetadataError::SyncFailed`). | Acceptable to defer — this is a local app and IPC messages are not network-accessible. Still a good practice to establish early before the metadata module grows large. |
| A4 | **PRAGMA enforcement per connection** | SQLite disables `foreign_keys` and defaults to `DELETE` journal mode on every new connection. If any connection path (e.g., a read-only diagnostic query opened separately) skips the PRAGMA setup, referential integrity breaks silently and WAL benefits disappear. | Define a single `open_metadata_connection()` function in `db.rs` that unconditionally applies all required PRAGMAs (`foreign_keys=ON`, `journal_mode=WAL`, `application_id`, `user_version`) before returning the connection handle. Never open a raw connection elsewhere. | Not deferrable if multiple code paths open connections — implement in Phase 1 DB bootstrap. |
| A5 | **DB integrity check at startup** | A corrupt database (from crash during a write, disk error, or external modification) can return silently incorrect query results before WAL checkpointing detects the issue. | Run `PRAGMA quick_check` (faster than `integrity_check`) at startup when opening the database. On failure, trigger the documented rebuild path. Skip in release builds if startup latency is a concern; run in background. | Acceptable to defer to Phase 2 — Phase 1 should define the rebuild path first, so there is a safe action to take on integrity failure. |
| A6 | **Community tap manifest content injection into SQLite** | Community manifest fields (`game_name`, `trainer_name`, `author`, `description`, `platform_tags_json`) come from untrusted git repositories. While parameterized queries prevent SQL injection, extremely long strings could cause index bloat or UI issues. | Validate string lengths before inserting community manifest rows: e.g., `game_name` ≤ 512 bytes, `description` ≤ 4 KB, `platform_tags_json` ≤ 2 KB. Return a diagnostic entry for manifests that exceed limits rather than silently truncating. | Low risk in practice given schema version gating already rejects unknown-version manifests. Defer if tap manifests come only from trusted community sources. |

---

## Data Protection

**Sensitivity classification of data stored in SQLite:**

| Table | Sensitivity | Contents |
|-------|------------|----------|
| `profiles` | Medium | Profile names, TOML file paths (may reveal home directory layout) |
| `profile_name_history` | Low | Historical profile names and paths |
| `launchers` | Medium | Expected script paths, desktop entry paths, display names |
| `launch_operations` | Medium–High | Timestamps of game launches, exit codes, signal numbers, log file paths |
| `launch_operations.helper_log_path` | Medium | Absolute path to log file (leaks home dir structure; Proton compatdata paths embed Steam App IDs) |
| `launch_operations.diagnostic_summary` | Medium | May contain command fragments, Proton output lines, path fragments |
| `community_profiles` | Low | Public community manifest metadata |
| `community_catalog_entry.manifest_json_cache` | **High** | Caches full `GameProfile` payloads from community taps — includes trainer paths, game paths, dll_paths. Same sensitivity as profile TOML. |
| `external_cache_entries.payload_json` | Low–Medium | Cached ProtonDB/cover-art responses; may contain Steam App IDs and game names |
| `profile_preferences` | Low | Favorites, pins, local annotations |
| `community_taps.url` | Low | Git repository URLs the user subscribes to |
| Tombstone/deleted profile rows | Medium | History rows retained after profile deletion. If `diagnostic_summary` or log content captured launch arguments at any point, credentials or tokens passed as CLI args could persist in tombstone rows indefinitely. |

**Privacy implications:**
- Launch history creates a durable local record of which games and trainers the user runs, with timestamps. This is expected and useful functionality, but users should be able to clear it.
- Trainer paths embedded in `launch_operations` may point to files in directories that suggest pirated or DRM-stripped game content.
- **Tombstone records** (history rows retained for deleted profiles) must not persist sensitive launch arguments. If CrossHook ever captures CLI arguments in `diagnostic_summary` or a related field, tokens or credentials passed on the command line could survive profile deletion indefinitely. Recommendation: never store raw argument lists; store only structured, pre-parsed diagnostic fields. Identified by ux-researcher.
- **Launcher drift messages** that include full filesystem paths (e.g., "Re-linked to `/home/user/scripts/game.sh`") should use `~`-normalized display paths or filename-only display by default. Full paths in UI messages reveal home directory structure and install locations, which is visible in screenshots and shared support logs. Identified by ux-researcher.
- The database is explicitly local-only per spec — no external sync is planned. This is the correct privacy default and should be enforced in code by the absence of any outbound DB sync path.

**Data at rest:**
- The database is unencrypted. This is appropriate for a single-user desktop tool where the OS filesystem ACL (file permissions) provides access control.
- **The critical requirement is that the database file and its WAL/SHM sidecars be created with `0600` permissions** (see W1). Without this, any other process running as the same user — including a malicious or compromised application — can read the full launch history, trainer paths, and usage data.
- Backup tools, cloud sync agents (Dropbox, Syncthing, etc.), and dotfile managers targeting `~/.local/share/` will include the database by default. Users should be informed that the database contains personal usage data so they can make informed decisions about backup scope.

**Confidence: High** — based on direct review of the data model spec and codebase.

---

## Dependency Security

### rusqlite

- **Recommended version**: **0.39.0** (released 2025-03-15), which bundles **SQLite 3.51.3** via libsqlite3-sys
- **libsqlite3-sys current**: 0.37.0 (security score: 100/100, zero known CVEs per Meterian as of 2026-03)
- **Versions < 0.26.0 of libsqlite3-sys**: Contained 1 critical vulnerability (patched)
- **Versions ≥ 0.26.0 through latest**: Zero reported vulnerabilities
- _Corrected from initial draft which cited SQLite 3.48.0 / rusqlite 0.33.0 — api-researcher confirmed 0.39.0 is current stable_

### SQLite CVEs and bugs resolved by using rusqlite 0.39.0 / SQLite 3.51.3

| Issue | Severity | Fixed In SQLite | Description | CrossHook exposure |
|-------|----------|----------------|-------------|-------------------|
| **WAL write+checkpoint race** | **High (data integrity)** | **3.51.3** | Rare data-race in SQLite 3.7.0–3.51.2 could silently corrupt a WAL database under concurrent write+checkpoint timing. Not a CVE — an upstream bug fix. Relevant because CrossHook will use WAL mode and may have background tasks triggering concurrent access. | Addressed by using rusqlite 0.39.0 with bundled 3.51.3. **Do not use system SQLite on older distros (SteamOS base may ship affected versions).** |
| CVE-2025-3277 | High | 3.49.1 (Feb 2025) | Integer overflow in `concat_ws()`, write past buffer end | Low — CrossHook won't call `concat_ws()`. Fixed in 3.51.3. |
| CVE-2025-29087 | High | 3.49.1 | Duplicate of CVE-2025-3277 | Same as above |
| CVE-2025-6965 | Medium | 3.50.2 (Jun 2025) | Integer overflow in SQL injection vector | Not exploitable without prior SQL injection. Fixed in 3.51.3. |
| CVE-2025-7458 | Medium | 3.42.0 (May 2023) | Integer overflow, read off array end | Fixed in 3.51.3. |
| CVE-2025-7709 | Medium | 3.50.3 (Jul 2025) | FTS5 integer overflow | Not applicable unless FTS5 enabled. Fixed in 3.51.3. |
| CVE-2024-0232 | Medium | 3.43.2 (Oct 2023) | Use-after-free in JSON parser via SQL injection | Fixed in 3.51.3. |

**Recommendation:** Use `rusqlite = { version = "0.39", features = ["bundled"] }` in `Cargo.toml`. The `bundled` feature compiles SQLite 3.51.3 from source at build time, resolving all of the above. This is especially important given that SteamOS and older Linux distros ship SQLite versions affected by the WAL-reset bug. Add a comment: `# bundled required: system SQLite on SteamOS may be pre-3.51.3 (WAL race + CVEs)`.

### Supply chain posture

- `rusqlite` is MIT-licensed, actively maintained, and the de-facto standard Rust SQLite crate.
- `libsqlite3-sys` bundles the upstream SQLite amalgamation source — the source is auditable.
- The `bundled` feature compiles SQLite from source at build time, which is reproducible and does not depend on the host system's SQLite version.
- No evidence of supply chain compromise in either crate's public history.

**Confidence: High** — based on Meterian analysis, NVD entries, and SQLite's official CVE listing.

---

## Input Validation

### SQL Injection

**rusqlite's parameterized query API** is the primary control. The `execute()`, `query()`, and `query_row()` methods accept a second argument for bound parameters, which SQLite handles as typed values, never interpreted as SQL. The `params![]` macro and tuple parameters both use this path.

**The risk is developer discipline, not API capability.** rusqlite does not enforce that query strings are literals. The following is syntactically valid and injectable:

```rust
// UNSAFE — do not do this
conn.execute(&format!("UPDATE profiles SET current_name='{}' WHERE profile_id=?", user_input), [id])?;
```

**Required pattern:**

```rust
// CORRECT — parameterized
conn.execute(
    "UPDATE profiles SET current_name=? WHERE profile_id=?",
    params![new_name, profile_id],
)?;
```

**Enforcement recommendation:** Add a Clippy lint or CI check that rejects `format!()` macro calls within any string argument passed to rusqlite query methods. At minimum, include this as a documented code review requirement.

### Path traversal from TOML-sourced paths

The existing `validate_name()` function in `toml_store.rs` prevents path traversal in profile names (rejects `/`, `\`, `:`, absolute paths, `.`, `..`). This function is called before any filesystem operation on profile names.

When profile paths are stored in SQLite, they are stored as absolute paths derived from `ProfileStore::profile_path()`, which has already been validated. The risk is that paths stored in `launch_operations.helper_log_path` or `launchers.expected_script_path` are **not** necessarily validated through the same path — they may be constructed from raw user input in profile fields (trainer path, game executable path).

**Recommendation:** Before storing any path in SQLite that originated from user-editable TOML fields, verify that the resolved path is within an expected directory prefix (home directory, Steam library path, or a known safe root). Log and skip paths that escape expected boundaries rather than storing them.

### Deserialization of community tap manifest JSON

Community manifests are currently deserialized with `serde_json::from_str::<CommunityProfileManifest>()` in `community/index.rs`. The schema version is checked after deserialization; unknown versions are skipped.

For the SQLite indexing layer, manifests will be re-parsed and their field values stored in SQLite via parameterized queries. The existing schema version check provides a basic gate. Additional validation needed before indexing:

1. **String length bounds** on all indexed fields (see A6)
2. **`platform_tags_json`** must be validated as a JSON array of short strings, not arbitrary JSON
3. **`compatibility_rating`** must be validated against the known enum values

### Cached external JSON payloads

`external_cache_entries.payload_json` stores raw responses from ProtonDB and similar services. These are opaque blobs from the perspective of the cache layer. The risk is that:

1. A large payload exhausts available memory during deserialization upstream
2. Malformed JSON stored in the cache causes a panic when later deserialized

**Required controls:**
- Validate `json_valid(payload_json)` before storage (SQLite built-in)
- Enforce maximum payload size before writing (see W3)
- Deserialize cached payloads lazily and with error handling, never with `unwrap()`

**Confidence: High** — based on rusqlite API documentation and direct code review.

---

## Filesystem Security

### Database file permissions

SQLite creates the database file using `open()` with the process's current umask. On Linux with the common `umask 022`, a newly created file gets `0644` — world-readable.

For `~/.local/share/crosshook/metadata.db`, this means:
- Any process running as the same user can read the full database
- On multi-user systems with a misconfigured umask or shared directories, other users could read it

**Required mitigation (W1):** After the first call to `rusqlite::Connection::open()` creates the file, immediately call `std::fs::set_permissions(&db_path, Permissions::from_mode(0o600))`. Verify the permissions were applied. For WAL mode, SQLite creates `-wal` and `-shm` files with the same permissions as the main database file.

```rust
// In metadata/db.rs bootstrap
let conn = Connection::open(&db_path)?;
std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o600))
    .map_err(|e| format!("failed to secure database file permissions: {e}"))?;
```

Alternatively, pre-create the file with correct permissions before opening:

```rust
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
OpenOptions::new()
    .create(true)
    .write(true)
    .mode(0o600)
    .open(&db_path)?;
// Then open with rusqlite
let conn = Connection::open(&db_path)?;
```

### WAL/SHM sidecar files

SQLite creates `-wal` and `-shm` files in the same directory as the main database. These files:
- Are created with permissions derived from the database file (post-umask)
- Must be in the same directory as the database (SQLite requirement for shared memory)
- Are removed automatically when the last connection closes in WAL mode

If the database is created with `0600`, WAL/SHM files inherit `0600`. The parent directory `~/.local/share/crosshook/` should also be created with `0700` permissions.

### Concurrent access and race conditions

CrossHook's `crosshook-core` is described as synchronous. SQLite WAL mode supports one writer with multiple concurrent readers. In the current architecture (single Tauri desktop process), there is one writer (the app) and potentially concurrent reads from background tasks.

No significant race conditions are anticipated in the single-process model. The CLI binary (`crosshook-cli`) represents a potential second process accessing the same database — if the CLI and desktop app run simultaneously:
- WAL mode handles concurrent reads safely
- Concurrent writes will serialize through SQLite's locking mechanism
- `PRAGMA busy_timeout` should be set (e.g., 5000ms) to handle lock contention gracefully rather than failing immediately

### Symlink attack on DB path

If an attacker can write to `~/.local/share/crosshook/` before CrossHook's first run, they could place a symlink at `metadata.db` pointing to any file the user owns. When CrossHook opens the symlink, SQLite would operate on the target file.

**Mitigation (W5):** Before opening the database:

```rust
if db_path.exists() {
    let meta = std::fs::symlink_metadata(&db_path)?;
    if meta.file_type().is_symlink() {
        return Err(/* "metadata.db is a symlink — refusing to open" */);
    }
}
```

**Confidence: High** — based on SQLite WAL documentation and Linux filesystem semantics.

---

## Cache Poisoning

### External metadata (ProtonDB, cover art, Steam catalog)

External metadata fetched over HTTPS and cached in `external_cache_entries`:

1. **Network-level**: HTTPS with certificate validation prevents MITM injection. CrossHook should verify that HTTP requests use system certificate stores (default in Rust's `reqwest`/`hyper`). Do not allow HTTP fallback for cache fetches.

2. **Payload validation before storage:**
   - Validate JSON structure against expected schema (not just `json_valid()`)
   - Enforce size limit (W3): recommended ≤ 512 KB per entry
   - Validate `expires_at` is a valid RFC-3339 timestamp before storing
   - Validate `cache_key` format (should be `{bucket}:{identifier}`, e.g., `protondb:1234567`)

3. **Payload usage after retrieval:**
   - Deserialize cached JSON with full error handling; never assume stored data is valid
   - Apply the same schema validation on read as on write (cached data from an older app version may not match current expectations)

4. **Cache key injection:** `cache_key` is the primary key of `external_cache_entries`. If cache keys are constructed from user-supplied input (game names, app IDs from TOML), validate the components before constructing the key. Use parameterized queries even for the key lookup.

### Community tap manifest indexing

Manifests from untrusted git repositories are parsed and indexed into `community_profiles`. The attack surface:

- **`game_name`, `trainer_name`, `author`, `description`** — stored as text via parameterized queries; no injection risk if parameterized queries are used. Risk: excessively long strings bloating the index.
- **`platform_tags_json`** — must be validated as `JSON array of strings` before storage. A crafted payload like `[{"$ne": null}, ...]` is harmless in SQLite (no BSON/MongoDB operators) but would cause errors when the frontend processes it.
- **`compatibility_rating`** — should be validated against the known enum before storage; reject manifests with unknown rating values.
- **Schema version gate** — already implemented in `community/index.rs`; manifests with unknown schema versions are skipped and logged as diagnostics. This gate should remain in the SQLite indexing path.

**Confidence: Medium** — external API schemas not fully analyzed; recommendations based on common defensive patterns.

---

## IPC Surface Security

### Current baseline

The existing Tauri IPC surface (`commands/launch.rs`, `commands/profile.rs`) already applies path sanitization before emitting events:

```rust
// Existing good pattern in launch.rs
fn sanitize_display_path(path: &str) -> String {
    match env::var("HOME") {
        Ok(home) if path.starts_with(&home) => format!("~{}", &path[home.len()..]),
        _ => path.to_string(),
    }
}
```

The `sanitize_diagnostic_report()` function applies this to all string fields in `DiagnosticReport` before emitting via `app.emit("launch-diagnostic", ...)`.

### Risks introduced by new SQLite-backed commands

New Tauri commands will return data read from SQLite tables that contain stored path strings. These include:

- `get_profile_catalog(profile_id)` — returns `current_toml_path`, `helper_log_path`, `expected_script_path`, `expected_desktop_path`
- Launch history responses — return `helper_log_path`, `diagnostic_summary`
- Launcher drift responses — return `current_slug`, `expected_script_path`, `expected_desktop_path`

**Required (W2):** Two acceptable approaches:

**Option A — Sanitize at write time (recommended by business-analyzer):** Apply `sanitize_path_for_display()` before inserting paths into SQLite. Stored values are already sanitized; IPC reads require no additional transformation. The trade-off is that raw absolute paths are never stored, which prevents any future code path from accidentally surfacing them.

**Option B — Sanitize at read time:** Store raw paths in SQLite (enabling potential future direct-DB tooling use), apply sanitization at every IPC boundary. Requires enforcement across all new command functions.

```rust
/// Sanitize a stored absolute path for display in the frontend.
/// Replaces $HOME prefix with "~" to avoid leaking home directory structure.
pub fn sanitize_path_for_display(path: &str) -> String {
    // use $HOME from env, cached at startup
}
```

**Recommendation:** Use Option A (sanitize at write time) for paths that are display-only (log paths, display names, summary text). Use raw paths for values that the app needs to use in filesystem operations (`current_toml_path`, `expected_script_path`) — sanitize only when those values cross the IPC boundary. This is consistent with business-analyzer's recommendation and prevents `diagnostic_summary` from storing raw home paths that would need sanitization on every read.

### Error message leakage

rusqlite error messages include:

- SQL statement text (from `SQLITE_ERROR` variants)
- Constraint names (from `SQLITE_CONSTRAINT` failures)
- Table and column names (from `SQLITE_NOTFOUND` errors)

If Tauri commands propagate raw `rusqlite::Error::to_string()` to the frontend (as `map_err(|e| e.to_string())`), this information is visible in the frontend and in any error logging the frontend does.

**Recommendation (A3):** Define a `MetadataError` enum in the metadata module with opaque user-facing variants. Log full rusqlite error detail at `tracing::error!` level; return only the `MetadataError` variant across IPC:

```rust
pub enum MetadataError {
    DatabaseUnavailable,
    SyncFailed { operation: &'static str },
    NotFound,
    // ...
}
```

### Tauri capability scope

The current `capabilities/default.json` grants `core:default` and `dialog:default` — a minimal capability set. The metadata module adds no new network or filesystem capabilities beyond what `crosshook-core` already uses (file I/O under user home directories). No capability changes are required for the SQLite layer.

**Confidence: High** — based on direct code review of existing IPC commands and Tauri capability configuration.

---

## Secure Coding Guidelines

These guidelines apply specifically to the new `metadata` module in `crosshook-core`:

### 1. Parameterized queries — mandatory

```rust
// ✓ CORRECT
conn.execute(
    "INSERT INTO profiles (profile_id, current_name, current_toml_path, ...) VALUES (?, ?, ?, ...)",
    params![id, name, path, ...],
)?;

// ✗ WRONG — SQL injection risk
conn.execute(&format!("INSERT INTO profiles ... VALUES ('{name}', ...)"), [])?;
```

**Rule:** Never use `format!()`, string concatenation, or string interpolation inside SQL query strings. All variable values must be bound parameters.

### 2. Connection factory — single entry point

```rust
// In metadata/db.rs
pub fn open_connection(db_path: &Path) -> Result<Connection, MetadataError> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch("
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA busy_timeout = 5000;
        PRAGMA secure_delete = ON;
    ")?;
    Ok(conn)
}
```

All connection opens must go through this factory. No raw `Connection::open()` calls elsewhere.

### 3. File permission enforcement — on first create

```rust
// In metadata/db.rs bootstrap
let db_path = resolve_db_path()?;
if db_path.exists() {
    ensure_not_symlink(&db_path)?;  // W5
} else {
    ensure_parent_dir(&db_path)?;
}
let conn = open_connection(&db_path)?;
if !permissions_are_0600(&db_path) {
    std::fs::set_permissions(&db_path, Permissions::from_mode(0o600))?;  // W1
}
```

### 4. Error mapping at module boundary

```rust
// In each metadata sub-module, map rusqlite errors before returning
fn record_launch_started(...) -> Result<String, MetadataError> {
    conn.execute(...).map_err(|e| {
        tracing::error!(%e, "failed to record launch_started");
        MetadataError::SyncFailed { operation: "record_launch_started" }
    })?;
    // ...
}
```

### 5. Path sanitization in IPC responses

```rust
// In src-tauri/commands/ layer, not in crosshook-core
impl Serialize for ProfileCatalogResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // apply sanitize_path_for_display to all path fields
    }
}
```

Or use a `DisplayPath(String)` newtype that applies sanitization in its `Serialize` impl.

### 6. Payload size enforcement before storage

```rust
const MAX_CACHE_PAYLOAD_BYTES: usize = 512 * 1024;
const MAX_DIAGNOSTIC_SUMMARY_BYTES: usize = 4 * 1024;

fn store_cache_entry(payload_json: &str, ...) -> Result<(), MetadataError> {
    if payload_json.len() > MAX_CACHE_PAYLOAD_BYTES {
        tracing::warn!("cache payload exceeds limit, truncating");
        return Err(MetadataError::PayloadTooLarge);
    }
    // ...
}
```

### 7. `execute_batch()` must only receive hard-coded SQL

```rust
// ✓ CORRECT — literal SQL only
conn.execute_batch("
    PRAGMA foreign_keys = ON;
    PRAGMA journal_mode = WAL;
    PRAGMA busy_timeout = 5000;
")?;

// ✓ CORRECT — use pragma_update() for variable values
conn.pragma_update(None, "user_version", schema_version)?;

// ✗ WRONG — execute_batch does not support parameters
conn.execute_batch(&format!("PRAGMA user_version = {schema_version}"))?;
```

**Rule:** `execute_batch()` accepts a raw SQL string with no parameter binding support. Never pass config-derived values, user input, or runtime strings to `execute_batch()`. Use `conn.pragma_update()` for any PRAGMA that requires a runtime value.

### 8. Re-validate names and paths from SQLite before filesystem operations

```rust
// When using a stored path for a filesystem operation, verify it first
fn open_profile_from_stored_path(stored_path: &str) -> Result<GameProfile, MetadataError> {
    // stored_path came from SQLite — validate before using
    let path = Path::new(stored_path);
    if !path.is_absolute() || path.components().any(|c| c == Component::ParentDir) {
        return Err(MetadataError::InvalidStoredPath);
    }
    // proceed with file open
}
```

**Rule:** Do not assume that a value read from SQLite is safe for filesystem use just because it was stored by the app. Re-apply `validate_name()` to profile names and path-safety checks to path values before using them in filesystem operations.

### 9. Fail-soft without hiding errors

```rust
// In launch command: SQLite failure must not block launch
match metadata_store.record_launch_started(&profile_id, &request) {
    Ok(operation_id) => Some(operation_id),
    Err(e) => {
        tracing::warn!(%e, "metadata record_launch_started failed; continuing without history");
        None
    }
}
```

Do not silently swallow `unwrap()` or use `expect()` in metadata paths that run during launch.

---

## Trade-off Recommendations

### Bundled vs system SQLite

**Recommendation: Use bundled SQLite** (`rusqlite` with `features = ["bundled"]`).

- **Security pro**: CrossHook controls the exact SQLite version, enabling timely CVE response independent of distro packaging.
- **Security con**: Delays in updating the crate mean CVE exposure until the next rusqlite release or patch.
- **Trade-off**: For an AppImage distribution, bundled SQLite is the correct choice. Distro-provided SQLite versions vary widely (e.g., Ubuntu LTS may ship SQLite 3.37.x), which could expose users on older distros to CVEs that bundled mode would patch.
- **Required**: Pin `libsqlite3-sys` version explicitly and add CI-tracked update cadence.

### World-readable permissions vs usability

**Recommendation: Enforce `0600` unconditionally.**

- The database contains information (trainer paths, launch history) that users may not want other local processes to access.
- A `0600` database is readable by the owning user and root only.
- No usability impact: CrossHook is single-user, single-process (plus optional CLI), and SQLite WAL mode handles this access pattern.
- The WAL/SHM sidecars need write access, which `0600` also provides.

### FTS5 and JSON functions

**Recommendation: Defer FTS5; enable JSON functions (they're free).**

- FTS5 adds implementation complexity and the `CVE-2025-7709` FTS5 CVE argues for delaying until an up-to-date libsqlite3-sys version is pinned.
- SQLite JSON functions are built-in since SQLite 3.38.0 and are needed for `platform_tags_json` handling and `json_valid()` validation — enable them.

---

## Open Questions

1. **Will the CLI and desktop app ever run concurrently against the same database?** If yes, the `busy_timeout` PRAGMA and WAL mode are sufficient for read concurrency, but the connection lifecycle needs explicit documentation.

2. **Should raw CLI argument lists ever be captured in `diagnostic_summary` or any `launch_operations` field?** If yes, there is a risk of capturing tokens or credentials passed as arguments (common in Proton/Steam launch patterns). Recommendation: only store structured, parsed diagnostic fields — never raw argument arrays. Confirmed as a concern by ux-researcher review of tombstone record semantics.

2. **What is the data retention policy for `launch_operations`?** Without a pruning strategy, the history table grows unboundedly. A retention policy (e.g., keep last 500 operations per profile, or 90 days) should be defined before Phase 2 ships.

3. **Will backup/export features expose the SQLite database directly?** If CrossHook adds a "backup profile data" feature, the database should be treated as an internal artifact and not exported raw. User-facing exports should use a defined schema, not the raw SQLite file.

4. **Should `diagnostic_summary` in `launch_operations` have access to the same path sanitization as the IPC layer?** Stored summaries contain log output that may include home directory paths. Sanitization should be applied before storage (at record time) rather than only at read time, so the stored data is already clean.

5. **What happens to the database on AppImage auto-update?** The database lives in `~/.local/share/crosshook/metadata.db` (user storage), not inside the AppImage (read-only). Schema migrations must be forward-compatible and run at startup, not during install.

---

## Sources

- [SQLite CVE List](https://www.sqlite.org/cves.html) — official SQLite CVE tracking
- [libsqlite3-sys Security Analysis](https://www.meterian.io/components/rust/libsqlite3-sys/) — Meterian component security score
- [rusqlite Params API](https://docs.rs/rusqlite/latest/rusqlite/trait.Params.html) — parameterized query documentation
- [SQLite WAL Documentation](https://www.sqlite.org/wal.html) — WAL mode, SHM/WAL file behavior
- [OWASP SQL Injection Prevention](https://cheatsheetseries.owasp.org/cheatsheets/SQL_Injection_Prevention_Cheat_Sheet.html) — parameterized query guidance
- [Rust SQL Injection Guide](https://www.stackhawk.com/blog/rust-sql-injection-guide-examples-and-prevention/) — rusqlite injection examples
