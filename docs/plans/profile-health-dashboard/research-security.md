# Security Research: Profile Health Dashboard

**Date**: 2026-03-28 (Second Pass — SQLite Metadata Integration)
**Scope**: CrossHook native Linux desktop app (Tauri v2 / Rust backend)
**Feature**: Batch-validate filesystem paths stored in all saved profiles; surface per-profile health status with remediation suggestions
**Revision note**: This is a second pass incorporating the SQLite3 metadata layer (PRs 89-91). See change markers throughout.

---

## Executive Summary

The profile health dashboard remains a low-risk feature under this threat model. The SQLite3 metadata layer (Phases 1-3 now implemented) changes the security picture in three meaningful ways:

1. **Health data can now be persisted** — launch history and health-relevant metadata live in `launch_operations` and `profiles` tables. Persisting health check results alongside this history would widen the sensitive-data footprint in `metadata.db`. **The recommendation is still no-persist for health status itself**: health checks remain on-demand reads only.
2. **SQLite-sourced paths require re-validation before filesystem ops** — W6 from the SQLite3 spec directly applies. Paths read back from `launch_operations.game_path`, `launch_operations.trainer_path`, or `launchers.script_path` must not be used in `metadata()` calls without re-validation.
3. **`diagnostic_json` contains serialized `DiagnosticReport` structs** — if health enrichment surfaces any fields from this column (path references, pattern summaries, suggestions), `sanitize_display_path()` must be applied before those strings reach the IPC boundary.

Four findings from the original spec remain valid and unchanged. Two findings have been revised. Four new SQLite-specific findings are added. No critical blockers in either pass.

### What Changed from the First Pass

| Finding                        | Status in Second Pass                                                                                                 |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| W-1 (CSP)                      | **Unchanged** — still must address                                                                                    |
| W-2 (path sanitization on IPC) | **Revised** — now also applies to SQLite-sourced paths including `launchers.script_path`                              |
| W-3 (diagnostic bundle export) | **Unchanged** — still must address                                                                                    |
| A-1 (3-state path status)      | **Unchanged** — still advisory                                                                                        |
| A-2 (symlink following)        | **Unchanged** — still advisory                                                                                        |
| A-3 (TOCTOU)                   | **Revised** — now references SQLite timestamp-tracking                                                                |
| A-4 (mutex poison)             | **Removed** — already handled in `MetadataStore.with_conn()`; health features reuse the same `Arc<Mutex<Connection>>` |
| A-5 (batch concurrency)        | **Unchanged** — still advisory                                                                                        |
| A-6 (IPC result type)          | **Unchanged** — still advisory                                                                                        |
| **NEW: N-1**                   | Health queries reading SQLite paths must re-validate (W6 application)                                                 |
| **NEW: N-2**                   | `diagnostic_json` field sensitivity when surfaced in health enrichment                                                |
| **NEW: N-3**                   | `sanitize_display_path()` must be applied before persistence, not only before IPC                                     |
| **NEW: N-4**                   | Health queries joining `profiles` must filter `deleted_at IS NULL`; existing queries do not                           |

---

## Findings by Severity

### CRITICAL — Hard Stops

**None.** The local desktop threat model does not surface any showstopper security issues for this feature. Path existence checks on user-configured paths are functionally equivalent to what the launch validation already does. SQLite integration does not introduce new critical findings; the W1-W8 mitigations from the SQLite3 spec are already implemented in `metadata/db.rs`.

---

### WARNING — Must Address, Alternatives Welcome

#### W-1: CSP is disabled (`"csp": null`)

**File**: `src-tauri/tauri.conf.json:23`
**Status**: Unchanged from first pass.

The current Tauri configuration sets `"csp": null`, disabling Content Security Policy for the WebView. This means any XSS that reaches the WebView can invoke any registered Tauri command without restriction.

The health dashboard adds new IPC commands that perform filesystem metadata lookups. With CSP disabled, a successful XSS could batch-probe arbitrary paths by invoking these commands. The SQLite layer makes this marginally worse: an XSS could also invoke any metadata-reading command to extract launch history, paths, and game names from the database.

**Mitigations** (team may choose one):

- **Preferred**: Enable CSP before or alongside this feature: `"csp": "default-src 'self'; script-src 'self'"` in `tauri.conf.json`. Tauri v2 supports this directly.
- **Acceptable alternative**: Add it to the tech debt backlog with a documented decision noting the local-app threat model.
- **Not acceptable**: Leaving it undocumented.

**Confidence**: High — `tauri.conf.json:23` confirms `"csp": null`. Tauri v2 CSP support confirmed by official docs.

---

#### W-2: Path sanitization applies to both profile-content paths AND SQLite-sourced paths

**Status**: Revised from first pass — SQLite adds a second path-injection vector.

**Original finding**: Remediation messages built from profile TOML content (`game_path`, `trainer_path`, etc.) must pass through `sanitize_display_path()` before reaching the IPC boundary.

**Added in second pass**: Health enrichment queries may read paths back from SQLite (`launch_operations.game_path`, `launch_operations.trainer_path`, `launchers.script_path`, `launchers.desktop_entry_path`). These stored paths must also be sanitized before IPC serialization. This is the direct application of SQLite3 spec finding W2 to the health dashboard context.

`sanitize_display_path()` is already in `src-tauri/src/commands/shared.rs:20`. The health dashboard Tauri command layer uses it the same way `sanitize_diagnostic_report()` does in the launch flow.

**Two distinct cases require sanitization:**

1. **Paths from profile TOML content** — user-entered fields read directly from `GameProfile` struct fields.
2. **Paths from SQLite columns** — `game_path`, `trainer_path`, `script_path`, `desktop_entry_path` read via health history queries; treat as untrusted input even though CrossHook wrote them.

**Mitigation**: Apply `sanitize_display_path()` to every path string in the health IPC response payload, regardless of source. The health command should not bypass this step for "obviously safe" paths — the sanitizer is cheap and the habit matters.

**Confidence**: High.

---

#### W-3: Diagnostic bundle export must sanitize health report paths

**Status**: Unchanged from first pass.

If health report data flows into a future shareable diagnostic bundle (e.g., issue #49), raw absolute paths in the report expose the user's home directory, username, and installed software layout. This is a higher-risk surface than the UI because the data leaves the local machine.

**Mitigation**: Apply `sanitize_display_path()` to all path fields before they enter any export or share pipeline. If diagnostic bundle integration is out of scope for this feature, document it as a hard dependency: "health report paths must be sanitized before inclusion in any diagnostic bundle."

**Confidence**: High.

---

#### N-1: SQLite-sourced paths must be re-validated before any filesystem operation (W6 application)

**Status**: New finding — SQLite integration.

SQLite3 spec finding W6 states: "Re-apply `validate_name()` / path-safety checks on SQLite-sourced paths before fs ops." The health dashboard is the first feature that would read paths _from SQLite_ and then call `std::fs::metadata()` on them.

The risk scenario: if `launch_operations.game_path` or `launch_operations.trainer_path` were written with an unexpected value (corrupted DB, manually tampered row, future bug in a write path), using that path directly in a `metadata()` call could confirm the existence of an attacker-controlled path on the local filesystem.

**This applies to**: any health enrichment that reads stored paths from `launch_operations`, `launchers`, or `profiles.current_path` and then calls `metadata()`.

**It does NOT apply to**: health checks that load profile content fresh from TOML (the normal health check path). TOML profiles are the canonical authority; paths read from them are subject only to the existing display sanitization rules.

**Mitigation**: For any code path that reads a path from SQLite and then calls `std::fs::metadata()` on it, validate the path using the same checks applied at TOML-load time: it must be a non-empty `PathBuf` that is not obviously a traversal (no `..` components pointing outside expected roots). For simple health checks that just check existence, a non-empty absolute path check is sufficient. Do not fabricate elaborate allowlists — the realistic risk is low; the rule is a defense-in-depth habit.

**Confidence**: High — W6 from the SQLite3 spec is directly applicable; the implementation pattern is new.

---

### ADVISORY — Best Practices, Safe to Defer

#### A-1: Permission errors must be distinguished from "not found"

**Status**: Unchanged from first pass.

When calling `std::fs::metadata(path)`, the error kind can be:

- `io::ErrorKind::NotFound` — file/directory does not exist
- `io::ErrorKind::PermissionDenied` — exists but cannot be read by this process

The health status model should represent three states, not two:

- `Healthy` — path exists and has the expected type (file/dir/executable)
- `Missing` — `ENOENT`
- `Inaccessible` — `EACCES` or other I/O error

Collapsing `Inaccessible` into `Missing` gives wrong remediation advice.

**Confidence**: High — standard Rust I/O error semantics.

---

#### A-2: Symlink-following behavior is correct but should be documented

**Status**: Unchanged from first pass.

Rust's `Path::exists()` and `std::fs::metadata()` follow symlinks. For a health check, this is the correct behavior. A symlink pointing at a broken target returns `NotFound` from `metadata()`, which is the correct health status (`Missing`).

The existing symlink protection in `metadata/db.rs` (`symlink_metadata()` check before `Connection::open()`) is specifically for the database file itself — it does not conflict with using `metadata()` to follow symlinks for health-checked game paths.

**Recommendation**: Use `std::fs::metadata()` (follows symlinks) rather than `fs::symlink_metadata()`. Add a code comment explaining the distinction.

**Confidence**: High.

---

#### A-3: TOCTOU is inherent but non-exploitable; SQLite enables better staleness UX

**Status**: Revised from first pass — SQLite enables the "last checked" timestamp the original spec recommended.

Any "check then display" pattern has a TOCTOU gap. For a health dashboard this is acceptable because:

- The result is advisory only; it drives no automated action.
- A local adversary who can race the check has direct filesystem access already.

**Revised recommendation**: The original spec suggested displaying a "last checked at" timestamp and not persisting health status to disk. With SQLite now available, there is a tempting path to persist health check results in a new column or table. **Resist this.** Persisting health status creates a stale-data UX problem (status says "Healthy" but game was uninstalled) that is worse than always checking live. The `launch_operations` table already captures "last launched at" and outcome, which provides richer historical signal than a persisted "last health check passed" flag. Display the `launch_operations.started_at` value for the most recent operation as a "last known good" timestamp instead.

**Confidence**: High — well-understood limitation of all health-check UIs; recommendation updated for SQLite context.

---

#### A-4: (Removed) Mutex poison handling

**Status**: Removed — already handled by the implemented `MetadataStore`.

The original finding warned about mutex poisoning if a health cache used `Arc<Mutex<...>>`. The `MetadataStore` already uses `Arc<Mutex<Connection>>` and handles `PoisonError` explicitly in `with_conn()` (returning `MetadataStoreError::Corrupt`). The health feature reuses `MetadataStore` — it does not create a new mutex. No action needed.

---

#### A-5: Batch validation should be sequential or rate-limited, not fully concurrent

**Status**: Unchanged from first pass.

Checking all profiles concurrently via `tokio::join!` on a system with hundreds of profiles and slow storage (SD card, network-backed home) could cause momentary I/O pressure.

**Recommendation**: Process profiles sequentially or with bounded concurrency (e.g., 4 concurrent `metadata()` calls via `tokio::Semaphore`). Sequential is simplest and fine for typical profile counts.

**Confidence**: Medium.

---

#### A-6: IPC result type must not include raw profile file paths

**Status**: Unchanged from first pass; now also covers SQLite-sourced paths.

The health check IPC response will include health status per profile. If the response includes raw filesystem paths (from TOML content or SQLite columns), those paths must pass through `sanitize_display_path()` first.

**Recommendation**: Prefer an enum-tagged field indicating what kind of path is problematic (`GameExecutable`, `TrainerExecutable`, `ProtonBinary`, `CompatdataDir`, `WinePrefix`) rather than raw path strings. If raw paths are needed for display, apply `sanitize_display_path()` server-side before serialization.

**Confidence**: High.

---

#### N-2: `diagnostic_json` field sensitivity in health enrichment

**Status**: New finding — SQLite integration.

`launch_operations.diagnostic_json` stores up to 4 KB of serialized `DiagnosticReport`. The `DiagnosticReport` struct includes free-text `summary`, `description`, `suggestion.title`, `suggestion.description`, and `log_tail_path` fields that may contain absolute paths. This is already handled in the launch flow — `sanitize_diagnostic_report()` in `commands/launch.rs` applies `sanitize_display_path()` to each of those fields before IPC.

If health enrichment surfaces any fields from `diagnostic_json` (e.g., last-failure summary, last failure mode, last suggestion), the same sanitization must be applied. The `severity` and `failure_mode` promoted columns are enum strings — they are safe to surface without sanitization. Any free-text content from the JSON blob requires sanitization.

**Mitigation**: For health history queries that read from `diagnostic_json`, either:

1. Read only the promoted enum columns (`severity`, `failure_mode`) — these are safe.
2. If reading free-text fields from the JSON blob, deserialize to `DiagnosticReport` and apply `sanitize_diagnostic_report()` before including in the IPC response.

**Confidence**: High — the sanitization requirement for `DiagnosticReport` fields is already established and tested in the launch command pipeline.

---

#### N-3: `sanitize_display_path()` must be applied before persistence, not only before IPC

**Status**: New finding — identified during tech design review.

If `health_snapshots` or any future health persistence table stores path-related fields (e.g., a `last_broken_path` column for displaying which path was problematic at last check), `sanitize_display_path()` must be applied before the value is written to SQLite — not only before it is serialized for the IPC response.

The existing W-2 advisory (and its SQLite3 spec predecessor W2) covers sanitization at the IPC boundary. This finding closes a gap where an unsanitized absolute path could be written to disk and later read back already sanitized, creating inconsistent behavior: paths written before sanitization-at-persistence was enforced would contain raw `$HOME` values, while paths written after would contain `~`. This asymmetry is a source of bugs and a mild information-disclosure risk if `metadata.db` is ever inspected directly (e.g., with the `sqlite3` CLI during debugging).

**Applies to**: any `MetadataStore` write method that accepts a path string derived from a health check result.

**Does not apply to**: `launchers.script_path` and `launchers.desktop_entry_path` — these are written by the launcher export pipeline, not by health checks. Sanitization for launcher paths at export time is a separate concern for the launcher export command.

**Mitigation**: Sanitize path strings with `sanitize_display_path()` at the point they are assembled into a health result struct — before either persistence or IPC serialization. One sanitization call covers both surfaces.

**Confidence**: High.

---

#### N-4: Health queries joining `profiles` must filter `deleted_at IS NULL`

**Status**: New finding — identified by code inspection of existing queries.

The existing `query_last_success_per_profile()` and `query_failure_trends()` in `metadata/mod.rs` query `launch_operations` grouped by `profile_name`. These queries do not join `profiles` and therefore do not filter on `deleted_at`. For the current use of these methods (usage analytics), this is acceptable — historical launch data for deleted profiles is useful context.

However, if health commands surface results for **all profiles including soft-deleted ones**, users will see health status for profiles they have already deleted. This is confusing at best and a mild information disclosure at worst (a deleted profile's path structure is surfaced in a "current health" view).

**Code evidence**: `collections.rs:143` already uses `WHERE ... p.deleted_at IS NULL` in its `add_profile_to_collection` join. `profile_sync.rs:77` uses `WHERE current_filename = ?1 AND deleted_at IS NULL` in `lookup_profile_id`. The pattern is established — health queries must follow it.

**Applies to**:
- Any new `MetadataStore` method added for health enrichment that joins or references the `profiles` table.
- The collection-scoped health query path: `JOIN collection_profiles cp ON ... JOIN profiles p ON ... WHERE ... p.deleted_at IS NULL` — must include the tombstone filter.
- `query_last_success_per_profile()` and `query_failure_trends()` as currently implemented do not need this filter for their existing purpose, but if health commands use them to drive which profiles appear in results, the health command layer must cross-reference against the live `ProfileStore` list (which only returns non-deleted profiles via `ProfileStore::list()`).

**Mitigation**: Health commands build their profile list from `ProfileStore::list()` (TOML-authoritative, never returns deleted profiles) and use SQLite only for enrichment keyed to those names. This is the correct architecture — the health command should never independently discover profiles from SQLite. If a dedicated health-scoped query method is added to `MetadataStore`, it must include `WHERE p.deleted_at IS NULL` in any `profiles` join.

**Confidence**: High — `deleted_at` tombstone behavior is confirmed in `migrations.rs:73` and enforced in `profile_sync.rs`, `collections.rs`. The two existing enrichment queries do not expose this issue in their current form, but new collection-scoped health joins must guard against it.

---

## SQLite3 Spec Cross-Reference

The SQLite3 security findings (W1-W8, A1-A6) from `docs/plans/sqlite3-addition/feature-spec.md` and their relevance to the health dashboard:

| SQLite3 Finding                                        | Status for Health Dashboard                                                                                                                                                                                                         |
| ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| W1 — World-readable DB permissions                     | **Already mitigated** in `metadata/db.rs` (`0o600` / `0o700`). Health feature inherits protection. No action needed.                                                                                                                |
| W2 — Inconsistent path sanitization in IPC             | **Directly applies** — health IPC returns path-containing data from both TOML and SQLite sources. Also applies before persistence (N-3). See W-2, N-2, N-3 above.                                                                  |
| W3 — Unbounded cached payload sizes                    | **Does not apply** — health checks do not write cache payloads. If `health_snapshots` is added, it is bounded to one row per profile with no payload column.                                                                       |
| W4 — SQL injection via dynamic query construction      | **Applies at design time** — any new health queries (e.g., `SELECT last N launches for profile_id`) must use `params![]`, never `format!()`. Already enforced by `MetadataStore` pattern; health queries must follow the same rule. |
| W5 — Symlink attack on DB creation                     | **Already mitigated** in `metadata/db.rs`. Health feature reads the existing DB, does not create a new one.                                                                                                                         |
| W6 — Stored paths used in fs ops without re-validation | **Directly applies** — health enrichment that reads paths from SQLite and calls `metadata()` must re-validate. See N-1 above.                                                                                                       |
| W7 — `execute_batch()` with non-literal strings        | **Does not apply** — health feature adds read queries, not new schema migrations. If health-related schema changes are added, this rule applies to the migration SQL.                                                               |
| W8 — Community tap fields rendered in React WebView    | **Does not apply directly** — health status values are not community-sourced. Profile names in health responses are already `validate_name()`-checked.                                                                              |
| A1 — Track bundled SQLite version                      | **Already mitigated** — `rusqlite` 0.39.0 with `bundled` feature. Health feature adds no new version dependency.                                                                                                                    |
| A2 — `PRAGMA secure_delete=ON`                         | **Already mitigated** in `configure_connection()`.                                                                                                                                                                                  |
| A3 — Map errors to opaque `MetadataError`              | **Applies** — any new health query methods added to `MetadataStore` must return `MetadataStoreError`, not raw `rusqlite::Error`.                                                                                                    |
| A4 — Single connection factory                         | **Already enforced** — health queries go through `MetadataStore.with_conn()`.                                                                                                                                                       |
| A5 — `PRAGMA quick_check` at startup                   | **Already mitigated** in `configure_connection()`.                                                                                                                                                                                  |
| A6 — Validate string lengths before insert             | **Does not apply** — health feature adds reads, not new inserts.                                                                                                                                                                    |

---

## Path Validation Security

### Profile Name Traversal

`validate_name()` in `profile/toml_store.rs` is robust. Health check IPC accepting a profile name flows through `ProfileStore::load()` which calls `validate_name()`. **No additional validation needed.**

### Profile Content Path Traversal

Paths stored inside profiles (`game.executable_path`, `trainer.path`, etc.) are user-entered. The health check treats these identically to launch validation: call `std::fs::metadata(path)` and check the result. No expanded attack surface relative to existing code when working from TOML.

### SQLite-Sourced Path Handling (New)

Paths read back from `launch_operations` or `launchers` table columns should not be assumed to be identical to what TOML contains at the time of the health check. The TOML profile is the canonical authority. For health enrichment, prefer reading paths from TOML (via `ProfileStore::load`) over reading them from SQLite history rows. Use SQLite paths only for display of historical context (last-used path), and apply both `sanitize_display_path()` (for display) and a basic non-traversal check (for any `metadata()` call).

---

## IPC Security

### New Tauri Command Surface

The health dashboard will add new Tauri commands:

| Command                              | Input                    | Output                     |
| ------------------------------------ | ------------------------ | -------------------------- |
| `profile_health_check_all`           | None                     | `Vec<ProfileHealthReport>` |
| `profile_health_check(name: String)` | Profile name (validated) | `ProfileHealthReport`      |

**Input validation**: Profile name passes through `ProfileStore` which calls `validate_name()`. ✓
**Output safety**: Must apply `sanitize_display_path()` to all path strings, both TOML-sourced and SQLite-sourced. See W-2.
**Query safety**: Any SQLite queries for health enrichment must use `params![]`. No `format!()` in SQL strings.

### Capabilities

No new Tauri capabilities needed. `std::fs::metadata()` does not require a Tauri plugin capability. SQLite operations go through the existing `MetadataStore` which is already managed in Tauri state.

### Event Emission

If health check streams progress updates (e.g., "checked 3 of 12 profiles"), it should emit Tauri events rather than polling IPC. Event payloads must similarly avoid raw paths. Use profile names (validated) and structured status enums.

---

## Health Data Persistence Security

### Recommendation: Do Not Persist Health Status

The SQLite metadata layer makes it _possible_ to persist health check results. The recommendation remains: **do not**. Reasons:

1. **Staleness risk**: A persisted "Healthy" status will mislead users after a game is uninstalled. Always-live checks eliminate this class of bug.
2. **Data sensitivity**: Health results reveal filesystem layout (which games exist, which trainers, which Proton builds). `launch_operations` already captures this at launch time; adding a separate health persistence table would not add value proportional to the cost.
3. **Fail-soft compatibility**: Health checks fall back to filesystem-only when SQLite is unavailable. This behavior is already correct and requires no change.

If a future requirement demands health persistence (e.g., "show health status on startup without waiting for a scan"), use the `profiles.updated_at` field and the `launch_operations` most-recent-success query as a proxy rather than introducing a new health-specific table.

---

## Information Disclosure

### `diagnostic_json` Sensitivity

The `diagnostic_json` column stores serialized `DiagnosticReport` including free-text summaries and suggestions that may contain paths. This data is Medium-High sensitivity (see SQLite3 spec data sensitivity table). Health enrichment that surfaces any of this data must apply `sanitize_diagnostic_report()` or equivalent field-level sanitization. Do not surface raw `diagnostic_json` blobs via IPC.

### `launch_operations.log_path` Sensitivity

The `log_path` column stores an absolute path to the launch log file. If the health command surfaces "last launch log available at..." messaging, this path must pass through `sanitize_display_path()`.

### What Health Status Reveals

Health status reveals whether user-configured paths exist on the filesystem:

- **Intentional**: the entire purpose of the feature.
- **Non-sensitive in the single-user local threat model**: the user has access to all their own paths.
- **Potentially sensitive if IPC is abused**: CSP (W-1) mitigates this vector.

---

## Dependency Security

### New Crates Required

**None.** The SQLite layer (`rusqlite`, `uuid`) is already a dependency. All required filesystem metadata APIs are in `std`. Zero new crates.

### Existing Dependency Health

No changes to the dependency health assessment from the first pass. `rusqlite` 0.39.0 bundles SQLite 3.51.3, addressing all known CVEs.

---

## Fail-Soft Security

When `MetadataStore` is unavailable (`available: false`), `with_conn()` returns `T::default()` immediately. For health enrichment queries:

- Queries that return `Vec<_>` will return an empty vec — the health check falls back to filesystem-only checks from TOML. This is the correct fallback.
- No new attack surface exists in degraded mode. SQLite-sourced enrichment simply does not run.
- The health command must not fail or degrade UI usability when SQLite enrichment is unavailable. Health checks from TOML are sufficient and always-available.

---

## Secure Coding Guidelines

For the health check implementation:

1. **Use `std::fs::metadata(path)` — not `path.exists()`**: `metadata()` returns `Result<Metadata, io::Error>` and lets you distinguish `NotFound` from `PermissionDenied`.

   ```rust
   match std::fs::metadata(path) {
       Ok(meta)  => { /* check is_file(), is_dir(), mode bits */ }
       Err(e) => match e.kind() {
           io::ErrorKind::NotFound         => PathStatus::Missing,
           io::ErrorKind::PermissionDenied => PathStatus::Inaccessible,
           _                               => PathStatus::Inaccessible,
       }
   }
   ```

2. **Never read file content from profile paths**: `metadata()` only. Do not open any file descriptor for reading on game_path, trainer_path, etc.

3. **Return structured status, not raw error strings**: IPC result type should be an enum (`Healthy`, `Missing`, `Inaccessible`, `NotConfigured`).

4. **Apply `sanitize_display_path()` to all path strings in IPC responses**: covers both TOML-sourced and SQLite-sourced paths.

5. **Profile batch check must not fail on single path error**: Use `match metadata(path) { Ok(m) => ..., Err(e) => log and continue }`. Never use `?` to propagate errors out of the per-path check loop.

6. **Check executable bit on Linux using `PermissionsExt`**: For Proton binary and game executable paths, use `metadata.permissions().mode() & 0o111 != 0`.

7. **Log at `debug` level, not `info`**: Health check results for individual paths should be logged at `debug` level only. Do not log raw paths at `info` or `warn` level.

8. **Health queries use `params![]` exclusively**: Any SQL in health-related `MetadataStore` methods follows the existing parameterized-query rule. No `format!()` in SQL strings. Note: `query_failure_trends()` in `mod.rs:442` uses `format!("-{days} days")` to build a string that is then passed to `params![]` — this is not an injection risk (the value is still a bound parameter), but the `format!()` call is unnecessary. New health query methods should use `format!()` only for the interval string construction pattern shown there, and in no other SQL context.

9. **Re-validate SQLite-sourced paths before `metadata()` calls** (new): For any code path that reads a path from SQLite and then calls `metadata()`, validate the path is non-empty and absolute. This is defense-in-depth against DB corruption; the realistic risk is low.

10. **Surface only promoted columns from `diagnostic_json` where possible**: For health enrichment, prefer `severity` and `failure_mode` enum columns over deserializing the full JSON blob. If free-text fields are needed, apply `sanitize_diagnostic_report()` before IPC.

11. **Apply `sanitize_display_path()` before persistence, not only before IPC** (new — N-3): If any health-derived path string is written to SQLite (e.g., a `health_snapshots.last_broken_path` column), sanitize it before the write. One call at struct-assembly time covers both the write path and the IPC path.

12. **Filter `deleted_at IS NULL` in all health queries joining `profiles`** (new — N-4): Health commands build their profile list from `ProfileStore::list()` (TOML-authoritative). Any `MetadataStore` method used for health enrichment that joins `profiles` must include `WHERE p.deleted_at IS NULL`. Do not surface health data for tombstoned profiles. Reference: existing pattern in `collections.rs:143` and `profile_sync.rs:77`.

---

## Trade-off Recommendations

| Trade-off                                                  | Recommended Decision                                                                                                                         |
| ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| CSP: enforce now vs. defer                                 | Enforce alongside this feature (W-1). One-line `tauri.conf.json` change.                                                                     |
| Raw paths in IPC: include vs. omit                         | Omit raw paths from IPC; use enum-tagged field types (A-6). Frontend displays human-readable labels without knowing the raw path.            |
| Batch concurrency: sequential vs. async                    | Sequential for simplicity; add bounded concurrency only if profiling reveals it matters (A-5).                                               |
| Health status persistence: persist vs. live-only           | Live-only — do not add a health status persistence table. Use `launch_operations` history as a proxy for "last known good".                  |
| SQLite enrichment: always vs. best-effort                  | Best-effort only. Health checks from TOML must work standalone. SQLite enrichment is additive and must not block or fail the health command. |
| `diagnostic_json` exposure: promoted columns vs. full blob | Prefer promoted `severity`/`failure_mode` columns. Only deserialize the JSON blob if free-text display is required, and sanitize before IPC. |
| Path scope restriction: unrestricted vs. known dirs        | Unrestricted — users install games anywhere. Accept the "existence confirmation" information disclosure as inherent.                         |
| Error distinction: 2-state vs. 3-state                     | 3-state (`Healthy` / `Missing` / `Inaccessible`) — negligible implementation cost, significant UX improvement (A-1).                         |

---

## Open Questions

1. **Will health enrichment read paths from SQLite at all?** If the health check reads paths exclusively from the loaded `GameProfile` TOML struct, N-1 (W6 application) becomes a preventive note rather than an active concern. The tech designer should clarify which fields, if any, are sourced from SQLite rather than TOML.

2. **Will health surface last-launch summary from `diagnostic_json`?** If the health report shows "last launch failed with: [reason]", the free-text fields require `sanitize_diagnostic_report()`. Confirm scope with tech designer and UX researcher.

3. **Should `launch_operations` health queries be bounded by recency?** If `profile_health_check` queries launch history for a given profile, bound the query (e.g., `LIMIT 10` most recent) to avoid unbounded reads on profiles with long histories.

4. **CSP enforcement scope**: If CSP is enabled, the existing `devUrl: "http://localhost:5173"` dev setup may require `script-src 'self' 'unsafe-eval'` for Vite dev mode. Production AppImage should use strict CSP.

5. **Should `Optional` paths (empty strings) be health-checked?** The health check should skip empty-string paths with a `NotConfigured` status rather than reporting them as `Missing`.

6. **How should `Inaccessible` vs `Missing` render in the UI?** IPC layer must distinguish them; UX decision delegated to ux-researcher.
