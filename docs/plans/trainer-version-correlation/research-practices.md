# Engineering Practices Research — Trainer Version Correlation

## Executive Summary

The codebase provides all necessary building blocks for trainer-version-correlation without any new dependencies. The feature maps directly onto three existing patterns: the `health_store.rs` upsert/load/lookup module pattern, the `steam/manifest.rs` ACF parse-and-extend pattern, and the `commands/health.rs` enrichment layer pattern. The only new Rust code needed is a `metadata/version_store.rs`, one new migration, one `buildid` field read in `steam/manifest.rs`, and a Tauri command following the health enrichment shape. No semver library, no version comparison engine — Steam build IDs are integers; `!=` is the entire comparison.

---

## Existing Reusable Code

| Module / Utility                                | Location                              | Purpose                                                                  | How to Reuse                                                                                                             |
| ----------------------------------------------- | ------------------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| `db::new_id()`                                  | `metadata/db.rs:64`                   | UUID v4 generation                                                       | Use for `version_correlation` row IDs                                                                                    |
| `db::open_in_memory()`                          | `metadata/db.rs:53`                   | In-memory SQLite for tests                                               | Use in version_store unit tests                                                                                          |
| `migrations::run_migrations()`                  | `metadata/migrations.rs:4`            | Sequential `user_version` PRAGMA migrations                              | Append `migrate_8_to_9()` for new table                                                                                  |
| `MetadataStoreError` enum                       | `metadata/models.rs:8`                | Structured error type for all DB operations                              | Re-use `Database` / `Validation` variants as-is                                                                          |
| `MAX_DIAGNOSTIC_JSON_BYTES` constant pattern    | `metadata/models.rs:148`              | Defensive payload size cap                                               | Model `MAX_VERSION_STRING_BYTES` similarly                                                                               |
| `health_store::upsert_health_snapshot()`        | `metadata/health_store.rs:13`         | `INSERT OR REPLACE` pattern with `i64` range guard                       | Copy this function signature shape for `upsert_version_record()`                                                         |
| `health_store::load_health_snapshots()`         | `metadata/health_store.rs:37`         | `prepare` → `query_map` → `collect` pattern                              | Identical query structure for `load_version_records()`                                                                   |
| `health_store::lookup_health_snapshot()`        | `metadata/health_store.rs:75`         | Single-row lookup with `.optional()` extension                           | Copy for `lookup_version_record(profile_id)`                                                                             |
| `profile_sync::compute_content_hash()` (SHA256) | `metadata/profile_sync.rs`            | SHA256 of `GameProfile` content                                          | Hash trainer file bytes with the same `sha2` crate                                                                       |
| `chrono::Utc::now().to_rfc3339()`               | `metadata/launch_history.rs:15`       | Timestamp generation                                                     | Use identically for `checked_at` column                                                                                  |
| `MetadataStore::with_conn()`                    | `metadata/mod.rs:79`                  | Safe `Arc<Mutex<Connection>>` access pattern                             | Call from `MetadataStore` public methods, same as all other stores                                                       |
| `steam/manifest.rs::parse_manifest()`           | `steam/manifest.rs:110`               | Reads `appid` + `installdir` from `.acf` VDF                             | Extend to also return `buildid` from the same `AppState` node                                                            |
| `steam/vdf.rs::VdfNode::get_child()`            | `steam/vdf.rs:18`                     | Typed VDF key access                                                     | Already used inside `parse_manifest`; `buildid` access is one more `get_child("buildid")` call                           |
| `commands/health.rs` enrichment pattern         | `src-tauri/src/commands/health.rs:60` | `BatchMetadataPrefetch` + `enrich_profile()` + fail-soft                 | Add version correlation as an optional field in the same enrichment pass                                                 |
| `useProfileHealth.ts` hook pattern              | `src/hooks/useProfileHealth.ts:116`   | `useReducer` + `useCallback` + Tauri event listeners + `AbortController` | Mirror this structure for `useVersionCorrelation`                                                                        |
| `CommunityProfileMetadata`                      | `profile/community_schema.rs:22`      | Already has `game_version` + `trainer_version` string fields             | These string fields are the "recorded version at import time" — they are the seed data for community-profile correlation |

---

## Modularity Design

### Recommended boundaries

**`crosshook-core/src/metadata/version_store.rs`** — new file, follows `health_store.rs` exactly:

- `upsert_version_record(conn, profile_id, game_build_id, trainer_file_hash, status, checked_at)` — `INSERT OR REPLACE`
- `load_version_records(conn) -> Vec<VersionRecordRow>` — all non-deleted profiles JOIN
- `lookup_version_record(conn, profile_id) -> Option<VersionRecordRow>`

**`crosshook-core/src/metadata/migrations.rs`** — add `migrate_8_to_9()`:

```sql
CREATE TABLE IF NOT EXISTS version_correlation (
    profile_id        TEXT PRIMARY KEY REFERENCES profiles(profile_id) ON DELETE CASCADE,
    game_build_id     TEXT,          -- Steam buildid integer stored as TEXT (NULL if non-Steam)
    trainer_file_hash TEXT,          -- SHA256 of trainer binary at record time (NULL if absent)
    status            TEXT NOT NULL, -- 'aligned' | 'game_updated' | 'trainer_updated' | 'unknown'
    checked_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_version_correlation_checked_at ON version_correlation(checked_at);
```

**`crosshook-core/src/metadata/models.rs`** — add `VersionRecordRow` struct and `VersionCorrelationStatus` enum (mirror `DriftState`).

**`crosshook-core/src/steam/manifest.rs`** — extend `parse_manifest()` return type to `(app_id, install_dir, build_id)` where `build_id: Option<String>`. `buildid` is a direct child of `AppState` in ACF files, accessed with one more `get_child("buildid")` call. **Do not add a new public function — extend the existing private one only.**

**`src-tauri/src/commands/profile.rs` or `commands/health.rs`** — add `check_version_correlation(profile_name)` Tauri command. Reads the current Steam build ID from the manifest, hashes the trainer file, compares against the stored record, upserts result.

**No new top-level module in `crosshook-core`** is needed. This is metadata behavior — it belongs in `metadata/`, just like `health_store.rs`, `launch_history.rs`, and `launcher_sync.rs`.

---

## KISS Assessment

| Approach                                                                     | Verdict                                                                                                                                                              |
| ---------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Full semver comparison engine (parse "1.2.3" strings, compare semver fields) | **Over-engineered.** Steam does not use semver. Trainers don't report semver. Reject.                                                                                |
| External `semver` crate                                                      | **Unnecessary dependency.** All known version signals are integers (Steam build ID) or opaque strings. `!=` equality is sufficient.                                  |
| Steam build ID as `u64` column                                               | **Needlessly strict.** Store as `TEXT`. The `.acf` `buildid` field is an integer string but treating it as opaque text avoids parse failures on malformed manifests. |
| Trainer version from binary filename or embedded manifest                    | **Unreliable.** Trainer filenames are inconsistent across distributors (FLiNG, WeMod, etc.). Do not attempt.                                                         |
| Trainer version via file hash (SHA256)                                       | **Simple and correct.** Detects any change to the trainer binary including silent updates. `sha2` is already a dependency.                                           |
| Trainer version via `mtime`                                                  | **Fragile.** mtime can be reset by copy operations. SHA256 is better and already available.                                                                          |
| Polling for changes at launch time                                           | **Right timing.** Check at `check_version_correlation` call, triggered on demand or before launch. Do not add a background watcher.                                  |
| Storing full version history                                                 | **Over-engineered for v1.** One row per profile (latest record) is sufficient. `launch_history` already provides the temporal view.                                  |

**Simplest correct design**: One new 5-column table. One new `version_store.rs` with three functions. One extended field in `parse_manifest()`. One new Tauri command. Zero new crates.

---

## Abstraction vs. Repetition

| Decision                                                                                        | Recommendation                                                                                                                                                                                                                                                                                                                   |
| ----------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The `upsert` / `load` / `lookup` triad in `version_store.rs` looks similar to `health_store.rs` | **Repeat, do not abstract.** Each store has slightly different columns and JOIN logic. A shared generic query builder would be premature. Three functions is cheap.                                                                                                                                                              |
| SHA256 file hashing already used in `profile_sync.rs`                                           | **Repeat the call pattern**, but the hash of a file (streaming read) differs from hashing serialized TOML. Extract into a shared `hash_file(path) -> Option<String>` helper only if a third callsite appears.                                                                                                                    |
| `VersionCorrelationStatus` enum mirrors `DriftState` and `LaunchOutcome`                        | **Repeat the pattern** (enum + `as_str()` + `impl FromStr`). All three represent different state machines. Do not unify.                                                                                                                                                                                                         |
| Enrichment struct `EnrichedProfileHealthReport` already exists                                  | **Extend, do not duplicate.** Add `version_correlation: Option<VersionCorrelationInfo>` as an optional field to `EnrichedProfileHealthReport` or create a companion `EnrichedVersionReport`. If the feature is always displayed alongside health, prefer extending the existing struct — it avoids a second batch-prefetch call. |
| React hook for version correlation                                                              | **Reuse `useProfileHealth` if correlation data is piggybacked onto health**; create `useVersionCorrelation` only if the feature has a standalone UI page.                                                                                                                                                                        |

---

## Interface Design

### Tauri command pattern (follow `commands/health.rs`)

```rust
// In src-tauri/src/commands/profile.rs or a new version.rs
#[tauri::command]
pub fn check_version_correlation(
    name: String,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<VersionCorrelationResult, String> {
    // 1. load profile
    // 2. get game build_id from ACF manifest (steam.app_id path → find manifest)
    // 3. hash trainer binary
    // 4. compare against stored record
    // 5. upsert record with new status (fail-soft on DB error)
    // 6. return VersionCorrelationResult
}

#[tauri::command]
pub fn get_cached_version_records(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<VersionCorrelationRecord>, String> {
    // Mirror get_cached_health_snapshots
}
```

Key patterns to preserve from existing commands:

- Accept `State<'_>` for `ProfileStore` and `MetadataStore`
- Return `Result<T, String>` (IPC error string, not `anyhow::Error`)
- Sanitize paths before returning (see `sanitize_display_path` in `commands/shared.rs`)
- Fail-soft on `MetadataStore` unavailability (`metadata_store.is_available()` guard)
- Register in the `tauri::generate_handler![]` macro in `lib.rs`

### React hook pattern — confirmed: reuse `useProfileHealth`, do not create a new hook

UX research confirms version mismatch surfaces in the Health Dashboard and as an inline banner on the Launch page — not as a standalone page or separate data stream. This means:

- **No `useVersionCorrelation` hook needed for v1.** Add `versionRecord?: VersionCorrelationRecord` to `EnrichedProfileHealthReport` on the Rust side; the existing `useProfileHealth` hook delivers it automatically.
- If a `VersionMismatchBanner` is needed on the Launch page, it reads from the already-loaded `healthByName` map in the `ProfileHealthContext` — no second `invoke()` call.
- A standalone `useVersionCorrelation` hook is only warranted if version correlation gets its own dedicated UI page, which UX explicitly discourages.

### Frontend component reuse (from UX research)

- **`HealthBadge`**: extend with `stale` styling for version mismatch state — no new CSS class needed
- **`IssueCategory` enum** in `HealthDashboardPage`: add `version_changed` as a new value, same pattern as `missing_trainer`
- **`formatRelativeTime`**: already exists — use for "Last checked X ago" in version status tooltips
- **`CollapsibleSection`**: use for any version detail expansion, consistent with existing panels
- **`VersionMismatchBanner`** on Launch page: the one genuinely new component. Should use a generalized `crosshook-banner--warning` CSS class so other pages can reuse the pattern.

Anti-patterns confirmed by UX research to avoid in implementation:

- No separate "version status" page
- No dedicated modal for version warnings — inline/banner only
- `game_build_id` stays in SQLite metadata only, not in profile TOML (portable profile portability contract)

**Display rule (confirmed by UX):** Always render the human-readable `game_version` string (from TOML / community schema) as the primary version label shown to users. The raw `steam_build_id` integer is a power-user detail — show it only in tooltip or collapsed detail view (e.g., "Steam build 11651527"). The build ID has no meaning outside the user's own Steam install and must not be the primary version display.

### IPC types (follow `CommunityProfileMetadata` and `EnrichedProfileHealthReport`)

```rust
// Derive Serialize + Deserialize for all IPC-crossing types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCorrelationRecord {
    pub profile_id: String,
    pub profile_name: String,
    pub game_build_id: Option<String>,
    pub trainer_file_hash: Option<String>,
    pub status: String,       // "aligned" | "game_updated" | "trainer_updated" | "unknown"
    pub checked_at: String,
}
```

---

## Testability Patterns

| Component                                      | Test approach                                                                                                                                                          | Precedent                                                                                         |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `steam/manifest.rs` buildid extraction         | Unit tests with `tempfile::tempdir()`, write `.acf` with `buildid` field, assert parsed value                                                                          | `manifest.rs:219-346` — full test suite already present                                           |
| `metadata/version_store.rs` upsert/load/lookup | `MetadataStore::open_in_memory()` + `run_migrations()`                                                                                                                 | `health_store.rs` functions can be tested the same way                                            |
| Mismatch detection logic                       | Pure function `compute_correlation_status(stored_build_id, current_build_id, stored_hash, current_hash) -> VersionCorrelationStatus` — no I/O, trivially unit testable | Follow `resolve_launch_method()` in `profile/models.rs` — pure function with `#[cfg(test)]` block |
| File hashing                                   | Write a temp file with known content, assert SHA256 matches expected hex                                                                                               | `sha2` is deterministic; `tempfile` already in `[dev-dependencies]`                               |
| Tauri command integration                      | No frontend test framework exists; rely on Rust unit tests for logic, manual smoke testing for IPC                                                                     | Consistent with existing practice per CLAUDE.md                                                   |

The single most important testability move: **extract the correlation status computation into a pure function** before the upsert call. This keeps I/O in the command handler and logic in a testable function.

---

## Build vs. Depend

| Need                                        | Existing capability                                                                                                  | Add dependency?                                                                                                   |
| ------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| SQLite persistence                          | `rusqlite = "0.38"` bundled — already in `Cargo.toml`                                                                | No                                                                                                                |
| Timestamps                                  | `chrono = "0.4"` — already in `Cargo.toml`                                                                           | No                                                                                                                |
| File hashing                                | `sha2 = "0.10"` — already in `Cargo.toml`                                                                            | No                                                                                                                |
| UUID for new row IDs                        | `uuid = "1"` — already in `Cargo.toml`                                                                               | No                                                                                                                |
| Build ID comparison                         | Parse `.acf` `buildid` string as `u64`, compare with `!=`. Zero library needed.                                      | No                                                                                                                |
| `LastUpdated` timestamp comparison          | `.acf` `LastUpdated` is a `u64` Unix timestamp. `chrono` already present if human-readable formatting is needed.     | No                                                                                                                |
| Semver version range matching               | Not in scope for v1 — opaque string equality covers MVP                                                              | No (defer `semver` crate to v2 if range queries are required)                                                     |
| VDF/ACF parsing                             | `steam/vdf.rs` — custom parser, well-tested, BTreeMap-backed, case-insensitive                                       | No                                                                                                                |
| IPC serialization                           | `serde + serde_json` — already in `Cargo.toml`                                                                       | No                                                                                                                |
| Filesystem event watching (live ACF reload) | Not in scope for v1 — poll on demand is architecturally consistent                                                   | No (v2: `notify-debouncer-full` 0.7, MIT/Apache-2, handles Steam's write-rename pattern better than raw `notify`) |
| PE VERSIONINFO extraction from trainer .exe | Not in scope for v1 — SHA256 hash is sufficient mismatch signal; user-provided version string for community profiles | No (v2: `pelite` — pure Rust, no unsafe, zero new transitive deps if trainer version auto-detection is requested) |

**Zero new dependencies required for v1.**

---

## Cross-Team Findings (Incorporated from Teammates)

### Module naming alignment

Tech-designer proposed `metadata/version_tracking.rs` and a separate `steam/version.rs`. The practices assessment recommends against the extra Steam module:

- `buildid` extraction is **one additional `get_child()` call** inside the existing private `parse_manifest()`. There is no justification for a new `steam/version.rs` file — that would split a single conceptual operation across two files with no clear boundary.
- `metadata/version_tracking.rs` or `version_store.rs` — either name works. Recommend `version_store.rs` to match `health_store.rs` naming exactly and signal it is a peer module.
- A `steam/version.rs` module becomes warranted only if `LastUpdated` timestamp extraction or update-detection logic is added as a separate concern in a later iteration.

### `check_a6_bounds()` gap — confirmed

The security-researcher correctly identified that `check_a6_bounds()` in `metadata/community_index.rs:258` validates `game_name`, `description`, `platform_tags`, `trainer_name`, and `author` — **but not `game_version`, `trainer_version`, or `proton_version`**. These three fields are stored in `community_profiles` without length bounds enforcement. This is a pre-existing gap, not introduced by the new feature. However:

- When version correlation reads `game_version` / `trainer_version` from `CommunityProfileRow` to seed correlation records, those values may be arbitrarily long.
- The fix is simple: add three bounds checks to `check_a6_bounds()` using the existing `MAX_TRAINER_NAME_BYTES = 512` constant for `trainer_version` and a new `MAX_VERSION_STRING_BYTES: usize = 256` for `game_version` and `proton_version`.
- **This should be a prerequisite fix before version correlation reads from those columns.**

### Health system as the integration point — confirmed

Business-analyzer and recommendations-agent both confirm: version mismatch should surface as a `HealthIssue` with `severity: Warning` in the existing `ProfileHealthReport`, not as a parallel alerting system. This means:

- `check_version_correlation()` is called inside `check_profile_health()` or as a post-pass enrichment (like `commands/health.rs` does today)
- The UI notification path (`HealthDashboardPage`, `HealthBadge`) already exists and handles warnings
- No new UI component or separate `VersionMismatchBanner` is needed for the first pass — version warnings appear in the existing health issue list

### Launch integration — flag, not block

Tech-designer proposed hooking version check into `commands/launch.rs` pre-launch flow. **Do not do this in v1.** The existing launch path is synchronous and latency-sensitive. File hashing and manifest scanning before every launch would degrade the user experience, particularly on slow storage. The correct pattern (consistent with how health checks work) is: check on demand, surface warnings in the health dashboard, let the user launch anyway. If the user wants a hard block, that is a settings option in a future iteration.

### Over-engineering risks — confirmed from recommendations-agent

All five flagged risks are valid and should constrain the implementation:

1. No semver parser — confirmed, equality-only
2. No filesystem watchers — confirmed, poll-on-launch or on-demand
3. No version history table — one row per profile, latest only
4. No new top-level crate — stays in `crosshook-core`
5. No real-time notification events — no new Tauri event stream needed

---

## Open Questions

1. **Non-Steam games**: For `launch_method = "native"` or `proton_run` without a Steam app ID, there is no ACF manifest and no `buildid`. The feature should gracefully degrade — `game_build_id = NULL`, status = `"unknown"`. Confirm with business requirements whether non-Steam game correlation is in scope for v1.

2. **Trainer version for non-community profiles**: For local trainer binaries (FLiNG `.exe`, WeMod overlay), there is no structured version metadata. SHA256 of the binary is the best proxy. Confirm this is acceptable — the UX implication is "trainer file changed" rather than "trainer version changed."

3. **When to trigger correlation check**: Options are (a) on profile load, (b) before every launch, (c) on demand from health dashboard. Option (b) adds latency to launch. Option (a) is implicit and may surprise users. Option (c) follows the health dashboard's explicit `batch_validate_profiles` call — recommended.

4. **Build ID staleness window**: Steam updates happen continuously. A `buildid` mismatch after a game update is expected and actionable. Define whether the UI should show "needs re-validation" vs. "hard mismatch" — affects status enum design.

5. **Manifest path discovery**: `parse_manifest()` currently requires the manifest path. Finding a manifest path from a Steam app ID requires scanning `steamapps/` directories. The existing `find_game_match()` in `manifest.rs` already does this scan — confirm whether `steam.app_id` is reliably populated in profiles where version correlation is desired, or whether a fallback scan is needed.
