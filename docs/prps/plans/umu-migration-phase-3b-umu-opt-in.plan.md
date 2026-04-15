# Plan: umu Migration Phase 3b — umu-database Coverage Warning + HTTP Cache

> **Continuation of Phase 3.** Phase 3a (umu opt-in, `UmuPreference::Umu` branch, PROTONPATH, Steam opt-out, concurrent-PID regression test) shipped in commit `ae18b92`; see [`umu-migration-phase-3-umu-opt-in.plan.md`](./umu-migration-phase-3-umu-opt-in.plan.md) for the already-merged 16-task plan. This 3b plan adds three newly-promoted `phase:3` issues on top of the shipped work.

## Summary

Extend the shipped `UmuDecisionPreview` with a `csv_coverage: CsvCoverage { Found, Missing, Unknown }` diagnostic field, populated by a new `crosshook_core::umu_database` module that reads the upstream umu-launcher protonfix CSV in this precedence order: (1) a CrossHook-managed HTTP cache at `~/.local/share/crosshook/umu-database.csv`, (2) the host-bundled copy at `/usr/share/umu-protonfixes/umu-database.csv` and two alternates, (3) `$XDG_DATA_DIRS/umu-protonfixes/umu-database.csv` — falling through to `Unknown` when nothing is reachable.

Surface the status in the existing inline-styled chip at `LaunchPanel.tsx:432-458` — amber-tinted with a remediation hint ("override this profile's Runtime → umu launcher to `Proton`") when `will_use_umu && csv_coverage === 'missing'`. Add a compact amber ⚠ badge next to the profile chip in `PinnedProfilesStrip` for the same condition.

Background-refresh the CSV via `reqwest` with `If-None-Match` / `If-Modified-Since` into the existing `external_cache_entries` SQLite table (24 h TTL), persisting body metadata (ETag, Last-Modified) in SQLite and the CSV body to disk where the coverage resolver picks it up. Expose a manual "Refresh umu protonfix database" button in Settings.

## User Story

As a CrossHook user who enabled `UmuPreference::Umu` globally, I want the Launch Preview to tell me **before I click Launch** that umu has no protonfix entry for my Steam app id, so that I can flip that profile's Runtime section to `Proton` instead of discovering a silent ~1-second-post-init crash. As a Flatpak user, I want the same warning to work without a host `umu-launcher` install, via a CrossHook-managed cache. As a maintainer, I want a single `umu_database` module whose CSV coverage check is reused by the Launch Preview today and remains compatible with a future `runtime.umu_game_id` auto-resolver if that ever lands upstream as per-id HTTP endpoints.

## Problem → Solution

- **Current (Phase 3a shipped)**: `UmuDecisionPreview` tells users _whether_ the launch will use `umu-run` and _why_ (`requested_preference`, `umu_run_path_on_backend_path`, `will_use_umu`, `reason`). It cannot tell them whether umu will actually _apply a protonfix_ for their game — that depends on the CSV consumed by `umu-protonfixes/fix.py`, which CrossHook does not inspect. Users hit the failure mode empirically, via a crash. Witcher 3 (Steam app 292030) on proton-cachyos + Nvidia is the canonical case ([#262](https://github.com/yandy-r/crosshook/issues/262)): the CSV has no entry for 292030 (verified upstream 2026-04-14), so `fix.py` falls through to global defaults, and `umu/umu_run.py:515` unconditionally overwrites `STEAM_COMPAT_APP_ID` with an MD5 hash of the prefix path — preventing proton-cachyos's local Witcher 3 protonfix from applying.
- **Desired**: Preview additionally surfaces `csv_coverage` computed from a cached or host-bundled CSV; the Launch Preview + pinned profiles surfaces render an amber warning chip + badge when `will_use_umu && csv_coverage == Missing`. A background + on-demand HTTP refresh keeps the cache fresh without depending on a host `umu-launcher` install (critical for Flatpak users, who cannot see `/usr/share/umu-protonfixes/` without manifest expansion — deferred to Phase 5).

## Metadata

- **Complexity**: Medium (one new `umu_database` module + 3 UI touchpoints + 1 new Tauri command). Reuses `rusqlite`, `reqwest`, and the `external_cache_entries` table — all already workspace deps.
- **New dependency**: `csv = "1"` (BurntSushi) — the 7-column schema with optional trailing cols benefits from serde-derive over inline `split(',')`.
- **Source PRD**: [`docs/prps/prds/umu-launcher-migration.prd.md`](../prds/umu-launcher-migration.prd.md)
- **PRD Phase**: Phase 3 continuation (new scope added after Phase 3a shipped)
- **Tracking issues**: [#256](https://github.com/yandy-r/crosshook/issues/256) (re-opened phase tracker) — child issues [#263](https://github.com/yandy-r/crosshook/issues/263) (UI warning), [#247](https://github.com/yandy-r/crosshook/issues/247) (HTTP resolver + SQLite cache), [#251](https://github.com/yandy-r/crosshook/issues/251) (close as duplicate of #247). Related: [#262](https://github.com/yandy-r/crosshook/issues/262) (upstream Witcher 3 CSV PR — not done here).
- **Estimated Files**: ~20 (Rust: 4 new module files + 2 edits + 2 new integration tests + 1 new Tauri command; TypeScript: 5 edits; docs: PRD Decisions Log + Storage Boundary rows).

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order. C4 only reads back what C1.3 produces, so both can start after C1 lands.

| Batch | Tasks            | Depends On     | Parallel Width | File-ownership summary                                                                                       |
| ----- | ---------------- | -------------- | -------------- | ------------------------------------------------------------------------------------------------------------ |
| C1    | C1.1, C1.2, C1.3 | —              | 3              | Foundation: `Cargo.toml` + `lib.rs` re-export, `umu_database/coverage.rs`, `umu_database/paths.rs`           |
| C2    | C2.1, C2.2, C2.3 | C1             | 3              | Preview + IPC wiring: `preview.rs` field, TS type + mock, Tauri command scaffold                             |
| C3    | C3.1, C3.2       | C2             | 2              | UI: `LaunchPanel.tsx` chip extension + `theme.css`, `PinnedProfilesStrip.tsx` amber badge                    |
| C4    | C4.1, C4.2, C4.3 | C1             | 3              | (#247) `umu_database/client.rs` HTTP + ETag, startup background refresh, wire real body into Tauri command   |
| C5    | C5.1, C5.2       | C4             | 2              | Settings UI refresh button + TS binding, mock `refresh_umu_database`                                         |
| C6    | C6.1, C6.2, C6.3 | C2, C3, C4, C5 | 3              | Tests: coverage unit tests (fixture CSV), wiremock HTTP integration test, preview-level `csv_coverage` tests |
| C7    | C7.1             | C6             | 1              | PRD Decisions Log + Storage Boundary update, close #251 as duplicate, plan closeout + report                 |

- **Total tasks**: 15
- **Total batches**: 7
- **Max parallel width**: 3 (C1, C2, C4, C6)

## UX Design

### Before (Phase 3a shipped)

```
Launch Preview → Command Chain:
  ┌──────────────────────────────────────────┐
  │ umu decision: using umu-run              │  (green or amber by will_use_umu only)
  │ requested preference: umu · umu-run on   │
  │   PATH: /usr/bin/umu-run                 │
  │ using umu-run at /usr/bin/umu-run        │
  └──────────────────────────────────────────┘
```

### After (Phase 3b)

```
Launch Preview → Command Chain:
  ┌───────────────────────────────────────────────────────────────────────┐
  │ umu decision: using umu-run                                           │
  │ requested preference: umu · umu-run on PATH: /usr/bin/umu-run         │
  │ using umu-run at /usr/bin/umu-run                                     │
  │ umu protonfix coverage: missing                                       │
  │ ⚠ umu has no protonfix entry for this app id. umu will apply global   │
  │   defaults, which may crash some games (e.g. Witcher 3 on Nvidia).    │
  │   If you hit crashes, override this profile's Runtime → umu launcher  │
  │   to Proton.                                                          │
  └───────────────────────────────────────────────────────────────────────┘  (amber when missing; green when found; neutral when unknown)

PinnedProfilesStrip:
  [Witcher 3 ⚠] [Ghost of Tsushima] [RE4 Remake]
      └── amber badge, title="umu has no protonfix entry…"
          only when effective umu_preference == umu AND csv_coverage == missing

Settings → umu launcher:
  ┌──────────────────────────────────────────┐
  │ umu preference: [Umu ▾]                  │
  │                                          │
  │ [Refresh umu protonfix database]         │
  │ Last refreshed: 8 hours ago              │
  └──────────────────────────────────────────┘
```

### Interaction Changes

| Touchpoint                                  | Before (Phase 3a)                               | After (Phase 3b)                                                                                                     |
| ------------------------------------------- | ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| LaunchPanel `umu_decision` chip             | 3 lines; color green if will_use_umu else amber | 4 lines; color amber when `will_use_umu && csv_coverage===missing`; remediation sub-line in that state               |
| PinnedProfilesStrip chips                   | No badge                                        | Amber ⚠ badge next to name when effective `umu_preference==umu && csv_coverage===missing` (read from cached preview) |
| Settings panel (below umu preference)       | No controls                                     | "Refresh umu protonfix database" button + last-refreshed timestamp                                                   |
| Background on app startup                   | No umu-database activity                        | Non-blocking `tokio::spawn` of `refresh_umu_database` when cache is stale                                            |
| Launch preview for Steam-applaunch / native | `umu_decision` is None (gate at preview.rs:404) | **Unchanged** — `csv_coverage` never emitted outside `proton_run`                                                    |

## Decisions

| Decision                            | Choice                                                                                                                                                | Rationale                                                                                                                                                                                                   |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CSV source precedence               | HTTP cache → `/usr/share/umu-protonfixes/` → `/usr/share/umu/` → `/opt/umu-launcher/umu-protonfixes/` → `$XDG_DATA_DIRS/umu-protonfixes/` → `Unknown` | HTTP cache is Flatpak-safe and user-scoped. System-bundled is the first-boot fallback on native distros. `Unknown` never blocks launch.                                                                     |
| CSV parser                          | New workspace dep `csv = "1"`                                                                                                                         | 7-column schema with quoted optional trailing cols (`COMMON ACRONYM (Optional)`, etc.); inline `split(',')` is brittle                                                                                      |
| In-memory index                     | `HashMap<(store_lowercase, codename), CsvRow>` cached in `OnceLock<Mutex<Option<CacheEntry>>>` keyed on `(path, mtime)`                               | Parse once per CSV mtime; repeat launches reuse index. ~900 KB file, ~2,100 rows = <10 ms parse, ~300 KB heap                                                                                               |
| HTTP client                         | Existing workspace `reqwest = "0.13.2"` + `OnceLock<Client>` singleton                                                                                | Pattern established at `protondb/client.rs:175-190`. Zero new deps.                                                                                                                                         |
| HTTP source URL                     | `https://raw.githubusercontent.com/Open-Wine-Components/umu-database/main/umu-database.csv`                                                           | Stable raw URL; GitHub sends `ETag` + `Last-Modified` for cheap conditional GET                                                                                                                             |
| Cache storage                       | Existing `external_cache_entries` SQLite table; key `"umu-database:csv"`; body on disk (not in SQLite)                                                | Table + `put_cache_entry` / `get_cache_entry` already exist; no new migration. Storing a ~900 KB body in `payload_json` would blow the size cap — keep body on disk where the coverage resolver expects it. |
| Cache TTL                           | 24 h default, conditional GET (`If-None-Match`) on every refresh                                                                                      | CSV updates in bursts (verified upstream commits/main — multiple per week). 24 h is polite; revalidation is free on 304.                                                                                    |
| Disk location of HTTP-cached CSV    | `BaseDirs::new().data_local_dir().join("crosshook/umu-database.csv")` — parallel to `metadata.db`                                                     | Already the CrossHook data root; survives across launches                                                                                                                                                   |
| Refresh triggers                    | (1) background on app startup when cache is stale, (2) manual "Refresh" button in Settings                                                            | Passive freshness + explicit user control; no on-every-preview latency                                                                                                                                      |
| Flatpak strategy                    | HTTP path is the Flatpak escape hatch; no manifest change in 3b                                                                                       | Phase 5 owns `--filesystem=xdg-data/umu:create`. `/usr/share/umu-protonfixes/` is not under `xdg-data` — would require `host-os`, rejected as too broad                                                     |
| `Unknown` vs `Missing` semantics    | `Unknown` = no readable CSV source; `Missing` = CSV present, app id not in it                                                                         | Only `Missing` + `will_use_umu` triggers the amber warning. `Unknown` renders as a neutral muted line ("umu-database not available").                                                                       |
| UI: missing-coverage chip color     | Amber `var(--crosshook-color-warning)` — mirrors `HealthBadge.tsx:102-116` mismatch state                                                             | Consistent with existing mismatch-warning pattern                                                                                                                                                           |
| UI: profile-surface badge placement | `PinnedProfilesStrip` only (no dedicated `ProfileCard` exists)                                                                                        | Avoids inventing a new component. Badge only renders when profile would actually use umu AND coverage is missing.                                                                                           |
| Store lookup heuristic              | `store = "steam"` when `resolve_steam_app_id_for_umu(request)` returns a non-empty value; else skip (`Unknown`)                                       | CrossHook does not model "this profile is GOG/Epic/Humble"; Steam-id presence is the only reliable signal today                                                                                             |
| `runtime.umu_game_id` override      | Used as `codename` with `store = "steam"` (conservative default); inherits the Phase 3a `resolve_steam_app_id_for_umu` precedence                     | Keeps the shipped Phase 3a precedence `runtime.umu_game_id > steam.app_id > runtime.steam_app_id`                                                                                                           |
| Auto-resolve via HTTP per-id        | **Not building here**                                                                                                                                 | PRD explicitly defers per-id auto-resolve. #251 closed as duplicate of #247. Scope = full-CSV cache only.                                                                                                   |
| Close #251                          | In PR body: `Closes #247, #263; closes #251 (duplicate of #247)`                                                                                      | #251 body already states "covered by #247"                                                                                                                                                                  |

## Mandatory Reading

| Priority       | File                                                                                                  | Lines                          | Why                                                                                                                |
| -------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------ | ------------------------------------------------------------------------------------------------------------------ |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`                                    | 159-175, 404-467               | Where `UmuDecisionPreview` is declared, gated (proton_run only), and populated                                     |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`                              | 997-1054                       | `should_use_umu`, `resolve_steam_app_id_for_umu`, `warn_on_umu_fallback` — precedence for coverage lookup          |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`                              | 1-80                           | `get_cache_entry` / `put_cache_entry` — the table to reuse for (#247) cache metadata                               |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`                                   | 20-395                         | Canonical pattern for `OnceLock<reqwest::Client>` singleton + SQLite cache read-through                            |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`                                      | 46-160                         | `MetadataStore` API (`with_sqlite_conn`, `with_conn`); DB file at `data_local_dir().join("crosshook/metadata.db")` |
| P0 (critical)  | `src/crosshook-native/src/components/LaunchPanel.tsx`                                                 | 425-468                        | The exact chip block to extend with a coverage line + amber state                                                  |
| P0 (critical)  | `src/crosshook-native/src/types/launch.ts`                                                            | 144-171                        | `UmuDecisionPreview` TS counterpart + `LaunchPreview` importers                                                    |
| P0 (critical)  | `src/crosshook-native/src/lib/mocks/handlers/launch.ts`                                               | 213-325                        | Current `preview_launch` mock — **does not populate `umu_decision`**; 3b fixes this gap                            |
| P1 (important) | `src/crosshook-native/src/components/PinnedProfilesStrip.tsx`                                         | 20-47                          | Chip structure + class names for the amber-badge insertion                                                         |
| P1 (important) | `src/crosshook-native/src/components/HealthBadge.tsx`                                                 | 62-116                         | Amber mismatch-chip precedent + `--crosshook-color-warning` usage                                                  |
| P1 (important) | `src/crosshook-native/src/components/SettingsPanel.tsx`                                               | 1006-1030 + umu_preference row | Dropdown + async handler pattern for the "Refresh umu protonfix database" button                                   |
| P1 (important) | `src/crosshook-native/src-tauri/src/commands/launch.rs`                                               | 100-120                        | `#[tauri::command]` signature convention — mirror for `refresh_umu_database`                                       |
| P1 (important) | `src/crosshook-native/src-tauri/src/commands/settings.rs`                                             | 102-167                        | Command → core-crate wiring + `invoke_handler!` registration pattern                                               |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/Cargo.toml`                                               | all                            | Workspace deps — confirm `csv` is new, everything else already present                                             |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/platform.rs`                                          | 178-190, 742-756               | Flatpak-aware XDG lookup precedent (if `$XDG_DATA_DIRS` iteration turns up Flatpak-specific gotchas)               |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/tests/config_history_integration.rs`                      | all                            | Rust integration-test layout template for `umu_database_coverage.rs`                                               |
| P2 (reference) | External: `https://raw.githubusercontent.com/Open-Wine-Components/umu-database/main/umu-database.csv` | —                              | Schema + row-count reality check (as of 2026-04-14: ~2,100 rows, 7 columns, Witcher 3/292030 absent)               |

## External Documentation

| Topic                               | Source                                                                                                                    | Key Takeaway                                                                                                                                           |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| umu-database schema                 | `https://github.com/Open-Wine-Components/umu-database`                                                                    | Columns: `TITLE,STORE,CODENAME,UMU_ID,COMMON ACRONYM (Optional),NOTE (Optional),EXE_STRINGS (Optional)`. Rolling main, no releases/tags.               |
| Witcher 3 / MD5 STEAM_COMPAT_APP_ID | `https://raw.githubusercontent.com/Open-Wine-Components/umu-launcher/main/umu/umu_run.py` line ~515 (verified 2026-04-14) | `STEAM_COMPAT_APP_ID` unconditionally overwritten with `hashlib.md5(str(pfx)...)` — this is the root of the Witcher 3 / proton-cachyos failure in #262 |
| umu-protonfixes CSV read site       | `https://github.com/Open-Wine-Components/umu-protonfixes/blob/master/fix.py`                                              | CSV is read from `os.path.dirname(os.path.abspath(__file__))` — **bundled next to `fix.py`**, not via XDG. Install path varies per distro.             |
| GitHub raw ETag / Last-Modified     | raw.githubusercontent.com responses                                                                                       | Both headers sent; `If-None-Match` → 304 revalidation works; TODO: confirm with `curl -I` at implementation start                                      |
| `csv` crate usage                   | `https://crates.io/crates/csv`, `https://burntsushi.net/csv/`                                                             | Default features include `serde`; `ReaderBuilder::flexible(true)` tolerates short rows                                                                 |
| `rusqlite` bundled features         | `https://lib.rs/crates/rusqlite/features`                                                                                 | Already at `features = ["bundled"]` — no AppImage/Flatpak bundling work needed                                                                         |

## Patterns to Mirror

### SINGLETON_HTTP_CLIENT_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/protondb/client.rs:175-190
static PROTONDB_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
fn protondb_http_client() -> &'static reqwest::Client {
    PROTONDB_HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(6))
            .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("build reqwest client")
    })
}
// Phase 3b mirrors for: umu_database_http_client() targeting raw.githubusercontent.com
```

### EXTERNAL_CACHE_PERSIST_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/protondb/client.rs:320-344
if let Err(error) = metadata_store.put_cache_entry(
    source_url,
    cache_key,       // 3b: "umu-database:csv"
    &payload,        // 3b: JSON {etag, last_modified, body_sha256} — NOT the CSV body
    cache.expires_at.as_deref(),  // 3b: 24h in the future
) {
    tracing::warn!(cache_key, %error, "failed to persist … cache payload");
}
```

### ONCELOCK_INDEX_CACHE_PATTERN

```rust
// SOURCE (shape): src/crosshook-native/crates/crosshook-core/src/protondb/client.rs singleton style
// Phase 3b usage: coverage.rs parses CSV once per mtime and caches the HashMap
struct CacheEntry { path: PathBuf, mtime: SystemTime, index: HashMap<(String, String), CsvRow> }
static CACHE: OnceLock<Mutex<Option<CacheEntry>>> = OnceLock::new();
// Invalidate when (path, mtime) tuple changes.
```

### XDG_DATA_DIR_RESOLUTION_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:60-63
let base = BaseDirs::new().ok_or(/* … */)?;
let db_path = base.data_local_dir().join("crosshook/metadata.db");
// Phase 3b mirrors for: base.data_local_dir().join("crosshook/umu-database.csv")
```

### CHIP_WARNING_STATE_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/HealthBadge.tsx:102-116
<span style={{ color: 'var(--crosshook-color-warning)' }} aria-label="version mismatch">
  ⚠ {/* mismatch message */}
</span>
// Phase 3b mirrors for: the coverage-missing remediation sub-line inside the existing LaunchPanel chip
```

### MOCK_HANDLER_REGISTRATION_PATTERN

```ts
// SOURCE: src/crosshook-native/src/lib/mocks/handlers/launch.ts map.set(…)
map.set('preview_launch', async (payload) => {
  /* synthetic LaunchPreview */
});
// Phase 3b adds: map.set('refresh_umu_database', async () => ({ refreshed: true, cached_at, source_url, reason }));
// AND fills in the previously-empty umu_decision in preview_launch (latent gap).
```

### TAURI_COMMAND_SIGNATURE_PATTERN

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/launch.rs:109-112
#[tauri::command]
pub fn preview_launch(request: LaunchRequest) -> Result<LaunchPreview, String> {
    build_launch_preview(&request).map_err(|e| e.to_string())
}
// Phase 3b adds async variant:
// #[tauri::command]
// pub async fn refresh_umu_database() -> Result<UmuDatabaseRefreshStatus, String>
```

### WIREMOCK_INTEGRATION_TEST_PATTERN

```rust
// SOURCE: existing crosshook-core wiremock tests (grep `wiremock` under src/)
let server = wiremock::MockServer::start().await;
wiremock::Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_body_string("…")).mount(&server).await;
// Phase 3b: `umu_database::client::set_source_url_for_test(server.uri())` → verifies fetch + ETag roundtrip
```

## Files to Change

| File                                                                          | Action | Justification                                                                                     |
| ----------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/Cargo.toml`                       | UPDATE | Add `csv = "1"` dep                                                                               |
| `src/crosshook-native/crates/crosshook-core/src/lib.rs`                       | UPDATE | `pub mod umu_database;`                                                                           |
| `src/crosshook-native/crates/crosshook-core/src/umu_database/mod.rs`          | CREATE | Public API + `CsvCoverage` enum + re-exports                                                      |
| `src/crosshook-native/crates/crosshook-core/src/umu_database/coverage.rs`     | CREATE | `check_coverage`, CSV parse, in-memory index keyed on `(path, mtime)`                             |
| `src/crosshook-native/crates/crosshook-core/src/umu_database/paths.rs`        | CREATE | `resolve_umu_database_path` with 5-tier precedence                                                |
| `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs`       | CREATE | (#247) `reqwest` singleton + `If-None-Match` + persist body to disk + header metadata to SQLite   |
| `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`            | UPDATE | Add `csv_coverage: CsvCoverage` to `UmuDecisionPreview`; populate in `build_umu_decision_preview` |
| `src/crosshook-native/src-tauri/src/commands/umu_database.rs`                 | CREATE | Tauri command `refresh_umu_database`                                                              |
| `src/crosshook-native/src-tauri/src/commands/mod.rs`                          | UPDATE | Register `umu_database` submodule                                                                 |
| `src/crosshook-native/src-tauri/src/lib.rs`                                   | UPDATE | Add `refresh_umu_database` to `invoke_handler!`; startup `tokio::spawn` background refresh        |
| `src/crosshook-native/src/types/launch.ts`                                    | UPDATE | Add `csv_coverage: UmuCsvCoverage` to `UmuDecisionPreview`                                        |
| `src/crosshook-native/src/components/LaunchPanel.tsx`                         | UPDATE | Extend chip at 432-458 with coverage line + amber state when `will_use_umu && missing`            |
| `src/crosshook-native/src/components/PinnedProfilesStrip.tsx`                 | UPDATE | Amber ⚠ badge next to profile chip name under same condition                                      |
| `src/crosshook-native/src/components/SettingsPanel.tsx`                       | UPDATE | "Refresh umu protonfix database" button + last-refreshed timestamp                                |
| `src/crosshook-native/src/lib/mocks/handlers/launch.ts`                       | UPDATE | Populate `umu_decision.csv_coverage` (fixes latent gap); mock `refresh_umu_database`              |
| `src/crosshook-native/src/styles/theme.css`                                   | UPDATE | `.crosshook-pinned-strip__badge--warn` + any coverage-chip tokens                                 |
| `src/crosshook-native/crates/crosshook-core/tests/umu_database_coverage.rs`   | CREATE | Fixture-CSV parse + Found/Missing/Unknown integration test                                        |
| `src/crosshook-native/crates/crosshook-core/tests/umu_database_http_cache.rs` | CREATE | (#247) wiremock-backed fetch + SQLite persist + ETag revalidation                                 |
| `docs/prps/prds/umu-launcher-migration.prd.md`                                | UPDATE | Decisions Log row (CSV source precedence) + Storage Boundary rows (CSV body + cache metadata)     |

## NOT Building

- **No per-app-id HTTP auto-resolve** — PRD's "v1 `GAMEID` resolver" stays `steam_app_id → "umu-0"`. #251 closed as duplicate of #247; this 3b ships the full-CSV cache, not per-id endpoints.
- **No upstream umu-database PR for Witcher 3** — tracked as [#262](https://github.com/yandy-r/crosshook/issues/262); not CrossHook-side work.
- **No Flatpak manifest change** — `--filesystem=xdg-data/umu:create` stays deferred to Phase 5. HTTP path is the Flatpak escape hatch; `--filesystem=host-os` rejected as too broad.
- **No `csv_coverage` on Steam-applaunch or native previews** — `UmuDecisionPreview` is gated to `proton_run` at `preview.rs:404-408`; 3b inherits the gate.
- **No auto-flip to `Proton` when coverage is missing** — respects user intent (they opted into umu). Warning only; no silent override.
- **No new `priority:critical` severity for missing coverage** — amber chip is advisory. Per-profile `umu_preference` override (shipped in 3a) is the remediation.
- **No SettingsPanel toggle to disable the coverage check** — always-on; extra toggle bloats Settings.
- **No refactor of the existing inline-styled chip to a reusable `crosshook-status-chip`** — scope creep. Extend in place.
- **No changes to `onboarding/readiness.rs`** — Phase 5 owns onboarding copy.
- **No retry/backoff for HTTP refresh** — single try with 6 s timeout mirrors ProtonDB client. On failure: `tracing::warn!` + leave existing cache intact.
- **No scheduled-job refresh framework** — "background on app startup when cache is stale" + manual Settings button is sufficient.
- **No `tempfile` promotion from dev-dep to runtime-dep** — atomic disk writes use `std::fs::rename` with a sibling `.tmp` path (zero new deps).
- **No new settings field** — no `install_nag_dismissed_at`, no `umu_database_refresh_interval` toggle, etc. Default behavior is the only behavior.

## Step-by-Step Tasks

### Task C1.1: Add `csv` dep + `umu_database` module skeleton — Depends on [none]

- **BATCH**: C1
- **ACTION**: Add `csv = "1"` to `crates/crosshook-core/Cargo.toml` `[dependencies]`. Create `crates/crosshook-core/src/umu_database/mod.rs` with submodule declarations + `CsvCoverage` enum + top-level re-exports. Add `pub mod umu_database;` to `crates/crosshook-core/src/lib.rs` alongside existing module declarations.
- **IMPLEMENT**:

  ```rust
  // crates/crosshook-core/src/umu_database/mod.rs
  pub mod client;
  pub mod coverage;
  pub mod paths;

  use serde::{Deserialize, Serialize};

  /// Result of looking up a Steam app id in umu-launcher's protonfix CSV.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub enum CsvCoverage {
      /// CSV is readable and the app id has a matching row.
      Found,
      /// CSV is readable but the app id has no matching row — umu will apply
      /// global defaults (and overwrite STEAM_COMPAT_APP_ID with a prefix MD5
      /// per umu/umu_run.py:515 verified 2026-04-14, which can break per-Proton-build
      /// local fixes — see issue #262 Witcher 3 / proton-cachyos).
      Missing,
      /// CSV source not reachable — coverage cannot be determined.
      Unknown,
  }

  pub use client::{refresh_umu_database, UmuDatabaseRefreshStatus};
  pub use coverage::check_coverage;
  pub use paths::resolve_umu_database_path;
  ```

- **MIRROR**: Module shape of `crates/crosshook-core/src/protondb/mod.rs` (submodules + top-level re-exports). Serde `rename_all = "snake_case"` matches `UmuPreference`.
- **IMPORTS**: `serde::{Deserialize, Serialize}` (already in workspace).
- **GOTCHA**: The `csv` crate's default features include `serde` — leave defaults on. Do not enable optional deps like `bytecount`. Adding `pub mod umu_database;` to `lib.rs` must preserve the existing module ordering convention (alphabetical or grouped — grep `pub mod` in `lib.rs` for the existing pattern).
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` compiles clean. `cargo tree -p crosshook-core -e normal | grep '^csv '` shows `csv v1`.

### Task C1.2: CSV parse + in-memory coverage lookup — Depends on [none]

- **BATCH**: C1
- **ACTION**: Create `crates/crosshook-core/src/umu_database/coverage.rs`. Implements `pub fn check_coverage(app_id: &str, store: Option<&str>) -> CsvCoverage` with mtime-keyed in-memory index caching.
- **IMPLEMENT**:

  ```rust
  use super::{paths, CsvCoverage};
  use serde::Deserialize;
  use std::{
      collections::HashMap,
      fs,
      path::{Path, PathBuf},
      sync::{Mutex, OnceLock},
      time::SystemTime,
  };

  // Upstream schema verified 2026-04-14 against
  // https://raw.githubusercontent.com/Open-Wine-Components/umu-database/main/umu-database.csv
  // TITLE,STORE,CODENAME,UMU_ID,COMMON ACRONYM (Optional),NOTE (Optional),EXE_STRINGS (Optional)
  #[derive(Debug, Clone, Deserialize)]
  #[allow(dead_code)]
  pub(crate) struct CsvRow {
      #[serde(rename = "TITLE")] pub title: String,
      #[serde(rename = "STORE")] pub store: String,
      #[serde(rename = "CODENAME")] pub codename: String,
      #[serde(rename = "UMU_ID")] pub umu_id: String,
      #[serde(rename = "COMMON ACRONYM (Optional)", default)] pub common_acronym: String,
      #[serde(rename = "NOTE (Optional)", default)] pub note: String,
      #[serde(rename = "EXE_STRINGS (Optional)", default)] pub exe_strings: String,
  }

  type Index = HashMap<(String, String), CsvRow>;

  struct CacheEntry { path: PathBuf, mtime: SystemTime, index: Index }
  static CACHE: OnceLock<Mutex<Option<CacheEntry>>> = OnceLock::new();

  pub fn check_coverage(app_id: &str, store: Option<&str>) -> CsvCoverage {
      let app_id = app_id.trim();
      if app_id.is_empty() { return CsvCoverage::Unknown; }
      let Some(path) = paths::resolve_umu_database_path() else { return CsvCoverage::Unknown; };
      let mtime = match fs::metadata(&path).and_then(|m| m.modified()) {
          Ok(t) => t,
          Err(_) => return CsvCoverage::Unknown,
      };
      let store_key = store.unwrap_or("steam").to_ascii_lowercase();
      let mutex = CACHE.get_or_init(|| Mutex::new(None));
      let mut guard = mutex.lock().expect("umu_database cache mutex poisoned");

      let needs_reload = match guard.as_ref() {
          Some(e) => e.path != path || e.mtime != mtime,
          None => true,
      };
      if needs_reload {
          match load_index(&path) {
              Ok(index) => *guard = Some(CacheEntry { path: path.clone(), mtime, index }),
              Err(err) => {
                  tracing::warn!(path = %path.display(), %err, "failed to parse umu-database CSV");
                  *guard = None;
                  return CsvCoverage::Unknown;
              }
          }
      }

      let Some(entry) = guard.as_ref() else { return CsvCoverage::Unknown; };
      let found = entry.index.contains_key(&(store_key, app_id.to_string()));
      tracing::debug!(app_id, store = ?store, found, "umu-database coverage lookup");
      if found { CsvCoverage::Found } else { CsvCoverage::Missing }
  }

  fn load_index(path: &Path) -> csv::Result<Index> {
      let mut rdr = csv::ReaderBuilder::new()
          .flexible(true)
          .has_headers(true)
          .from_path(path)?;
      let mut out = HashMap::new();
      for row in rdr.deserialize::<CsvRow>() {
          let Ok(row) = row else { continue };
          let key = (row.store.trim().to_ascii_lowercase(), row.codename.trim().to_string());
          out.insert(key, row);
      }
      Ok(out)
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use std::io::Write;

      const FIXTURE: &str = "\
  TITLE,STORE,CODENAME,UMU_ID,COMMON ACRONYM (Optional),NOTE (Optional),EXE_STRINGS (Optional)
  Ghost of Tsushima,steam,546590,umu-546590,GoT,,ghostoftsushima.exe
  Resident Evil 4 Remake,steam,2050650,umu-2050650,RE4R,,re4.exe
  ";

      #[test]
      fn index_contains_fixture_entry() {
          let dir = tempfile::tempdir().unwrap();
          let path = dir.path().join("umu-database.csv");
          writeln!(std::fs::File::create(&path).unwrap(), "{FIXTURE}").unwrap();
          let index = load_index(&path).unwrap();
          assert!(index.contains_key(&("steam".to_string(), "546590".to_string())));
      }
  }
  ```

- **MIRROR**: `OnceLock<Mutex<Option<_>>>` pattern from existing client singletons; rename attributes standard.
- **IMPORTS**: `csv::{ReaderBuilder, Result}`, `serde::Deserialize`, `std::{collections::HashMap, fs, path, sync, time}`.
- **GOTCHA**: Upstream header names contain **spaces and parens** (`COMMON ACRONYM (Optional)`) — Serde's `rename` must match byte-for-byte including the trailing `(Optional)`. Use `flexible(true)` to tolerate rows with fewer columns than the header (upstream occasionally emits short rows). `fs::metadata(...).modified()` can fail on exotic filesystems — degrade to `Unknown`, do not panic.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core umu_database::coverage`. The inline `index_contains_fixture_entry` test passes; richer integration tests land in C6.1.

### Task C1.3: Path resolver with 5-tier precedence — Depends on [none]

- **BATCH**: C1
- **ACTION**: Create `crates/crosshook-core/src/umu_database/paths.rs` with `pub fn resolve_umu_database_path() -> Option<PathBuf>`.
- **IMPLEMENT**:

  ```rust
  use directories::BaseDirs;
  use std::path::PathBuf;

  /// Returns the first readable umu-database CSV path, in precedence order:
  /// 1. CrossHook HTTP cache (data_local_dir()/crosshook/umu-database.csv)
  /// 2. Packaged umu-protonfixes (Arch multilib, Fedora: /usr/share/umu-protonfixes/)
  /// 3. Alternate packaged path (/usr/share/umu/)
  /// 4. Manual installs (/opt/umu-launcher/umu-protonfixes/)
  /// 5. $XDG_DATA_DIRS/umu-protonfixes/
  pub fn resolve_umu_database_path() -> Option<PathBuf> {
      let mut candidates: Vec<PathBuf> = Vec::new();
      if let Some(base) = BaseDirs::new() {
          candidates.push(base.data_local_dir().join("crosshook/umu-database.csv"));
      }
      candidates.push(PathBuf::from("/usr/share/umu-protonfixes/umu-database.csv"));
      candidates.push(PathBuf::from("/usr/share/umu/umu-database.csv"));
      candidates.push(PathBuf::from("/opt/umu-launcher/umu-protonfixes/umu-database.csv"));
      for data_dir in std::env::var("XDG_DATA_DIRS")
          .unwrap_or_default()
          .split(':')
          .filter(|s| !s.is_empty())
      {
          candidates.push(PathBuf::from(data_dir).join("umu-protonfixes/umu-database.csv"));
      }
      for cand in candidates {
          if std::fs::metadata(&cand).map(|m| m.is_file()).unwrap_or(false) {
              tracing::debug!(path = %cand.display(), "resolved umu-database CSV");
              return Some(cand);
          }
      }
      None
  }
  ```

- **MIRROR**: `BaseDirs::new()` pattern from `metadata/mod.rs:60-63` and `settings/mod.rs:339-342`.
- **IMPORTS**: `directories::BaseDirs` (already a workspace dep at `"6.0.0"`).
- **GOTCHA**: Upstream `umu-protonfixes/fix.py` reads the CSV bundled next to itself — distros land the file in different locations. The two `/usr/share/` candidates cover Arch multilib + most community packages; `/opt` covers manual installs. **Flatpak sees none of the `/usr/*` paths without `--filesystem=host-os`** — 3b's HTTP path is the Flatpak escape hatch (PRD Phase 5 owns any manifest change).
- **VALIDATE**: Unit tests: `resolve_returns_none_when_no_candidate_exists` (use `tempfile::TempDir` + `std::env::set_var("XDG_DATA_DIRS", tmp.path())`) and a precedence test (drop a file at candidate #2 via a bind-mount-simulating tempdir + `XDG_DATA_DIRS` injection — or skip precedence tests against `/usr` paths and cover that in C6.1's integration test with a fixture hierarchy).

### Task C2.1: Extend `UmuDecisionPreview` with `csv_coverage` — Depends on [C1]

- **BATCH**: C2
- **ACTION**: Add `pub csv_coverage: CsvCoverage` field to `UmuDecisionPreview` at `preview.rs:165-175`. Extend `build_umu_decision_preview` at `preview.rs:437-467` to compute it.
- **IMPLEMENT**:

  ```rust
  // preview.rs:165-175 (extended)
  #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
  pub struct UmuDecisionPreview {
      pub requested_preference: crate::settings::UmuPreference,
      pub umu_run_path_on_backend_path: Option<String>,
      pub will_use_umu: bool,
      pub reason: String,
      /// Coverage status of the profile's app id in the umu-database CSV.
      /// `Unknown` when no app id is available or no CSV source is reachable.
      pub csv_coverage: crate::umu_database::CsvCoverage,
  }

  // preview.rs:437-467 (extended builder — insert before the final struct literal)
  let app_id = crate::launch::script_runner::resolve_steam_app_id_for_umu(request);
  let csv_coverage = crate::umu_database::check_coverage(app_id, Some("steam"));
  UmuDecisionPreview {
      requested_preference,
      umu_run_path_on_backend_path,
      will_use_umu,
      reason,
      csv_coverage,
  }
  ```

- **MIRROR**: Existing struct-field + builder-assignment pattern.
- **IMPORTS**: `crate::umu_database::CsvCoverage`. If `resolve_steam_app_id_for_umu` is `pub(crate)`, either promote to `pub(super)` or re-export via the `launch` module — confirm visibility with `rg 'pub(.*) fn resolve_steam_app_id_for_umu'` before editing.
- **GOTCHA**: Phase 3a's `resolve_steam_app_id_for_umu` returns `&str`; its fallback is `"umu-0"` (not the empty string). Passing `"umu-0"` to `check_coverage` will correctly return `Missing` (never in CSV), which the UI treats as advisory — perfect semantic for "no app id configured." Do not special-case `"umu-0"`. Also: `UmuDecisionPreview` already derives `Eq` — `CsvCoverage` derives `Eq` as well (see C1.1), so the derive set remains valid.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::preview`. All existing `preview_*` tests that assert `umu_decision` shape must be extended to assert the new field (add `csv_coverage: CsvCoverage::Unknown` where no CSV is present in the test environment).

### Task C2.2: TypeScript type + mock handler — Depends on [C1]

- **BATCH**: C2
- **ACTION**: Update `src/types/launch.ts` `UmuDecisionPreview` at line 144-149 to include `csv_coverage: UmuCsvCoverage`. Update the mock at `src/lib/mocks/handlers/launch.ts:213-325` — which currently does **not** populate `umu_decision` at all (latent gap) — to emit `umu_decision` including `csv_coverage`.
- **IMPLEMENT**:

  ```ts
  // types/launch.ts
  export type UmuCsvCoverage = 'found' | 'missing' | 'unknown';

  export interface UmuDecisionPreview {
    requested_preference: 'auto' | 'umu' | 'proton';
    umu_run_path_on_backend_path: string | null;
    will_use_umu: boolean;
    reason: string;
    csv_coverage: UmuCsvCoverage;
  }
  ```

  In the mock populated branch (`launch.ts:270-304`), set `umu_decision` using the mocked request's preference and a fixed allow-list to fake `csv_coverage`:

  ```ts
  const COVERAGE_FIXTURE_FOUND_NAMES = new Set(['ghost-of-tsushima.exe', 're4.exe']);
  umu_decision: request.umu_preference && request.method === 'proton_run' ? {
      requested_preference: request.umu_preference,
      umu_run_path_on_backend_path: request.umu_preference === 'umu' ? '/usr/bin/umu-run' : null,
      will_use_umu: request.umu_preference === 'umu',
      reason: request.umu_preference === 'umu' ? 'using umu-run at /usr/bin/umu-run' : 'mocked — Auto → Proton',
      csv_coverage: COVERAGE_FIXTURE_FOUND_NAMES.has(gameName) ? 'found' : 'missing',
  } : null,
  ```

- **MIRROR**: Existing Rust-to-TS mirror style in `types/launch.ts`; mock-handler shape at `lib/mocks/handlers/launch.ts`.
- **IMPORTS**: `UmuCsvCoverage` type alias; update the import block in `LaunchPanel.tsx` downstream.
- **GOTCHA**: Mock's failure branch (`launch.ts:241-266`) must also include `umu_decision: null` (not just omit the field) so TS narrowing works. Leaving `umu_decision` unset while the type has `csv_coverage` as required would type-error on the consumer side.
- **VALIDATE**: `npx --prefix src/crosshook-native tsc --noEmit`. `./scripts/dev-native.sh --browser` — toggle mock game between `ghost-of-tsushima.exe` and an arbitrary name; verify the chip flips between green/found and amber/missing.

### Task C2.3: `refresh_umu_database` Tauri command scaffold — Depends on [C1]

- **BATCH**: C2
- **ACTION**: Create `src-tauri/src/commands/umu_database.rs` with a thin Tauri command. Register it in `src-tauri/src/commands/mod.rs` and `src-tauri/src/lib.rs`. Body is a placeholder returning `Err("not implemented yet")` until C4.3 wires the real body.
- **IMPLEMENT**:

  ```rust
  // src-tauri/src/commands/umu_database.rs
  use crosshook_core::umu_database;

  #[tauri::command]
  pub async fn refresh_umu_database() -> Result<umu_database::UmuDatabaseRefreshStatus, String> {
      umu_database::refresh_umu_database().await.map_err(|e| e.to_string())
  }
  ```

  Register in `commands/mod.rs` (`pub mod umu_database;`) and `lib.rs` `tauri::generate_handler![…, commands::umu_database::refresh_umu_database, …]`.

- **MIRROR**: `src-tauri/src/commands/settings.rs` command-signature convention (snake_case name, async where core-side is async, `Result<T, String>`).
- **IMPORTS**: `crosshook_core::umu_database` already re-exports the status type from C1.1.
- **GOTCHA**: Until C4.1 lands, `umu_database::refresh_umu_database` returns a placeholder `Err`. Document this in the PR body. **Invoke handler list ordering**: keep alphabetical/grouped per existing convention — grep `generate_handler!` for the existing layout before inserting.
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/Cargo.toml --workspace` compiles; `tauri::generate_handler!` macro expansion does not error.

### Task C3.1: LaunchPanel chip extension + theme tokens — Depends on [C2]

- **BATCH**: C3
- **ACTION**: Extend the inline-styled chip at `LaunchPanel.tsx:432-458` with a coverage status line and an amber-state remediation sub-line when `will_use_umu && csv_coverage === 'missing'`. Pull container background/border into a derived color.
- **IMPLEMENT**: Inside the existing `{preview.umu_decision ? (…) : null}` block, add below the `reason` line:

  ```tsx
  <div className="crosshook-muted" style={{ marginTop: 4 }}>
    umu protonfix coverage: <code>{preview.umu_decision.csv_coverage}</code>
  </div>;
  {
    preview.umu_decision.will_use_umu && preview.umu_decision.csv_coverage === 'missing' ? (
      <div style={{ marginTop: 6, color: 'var(--crosshook-color-warning)' }}>
        ⚠ umu has no protonfix entry for this app id. umu will apply global defaults, which may crash some games (e.g.
        Witcher 3 on Nvidia). If you hit crashes, override this profile&apos;s Runtime → umu launcher to{' '}
        <code>Proton</code>.
      </div>
    ) : null;
  }
  ```

  Derive the container `background` + `border` by ranking `(will_use_umu, csv_coverage)`: amber when `(true, 'missing')`, green when `(true, 'found' | 'unknown')`, neutral-amber (existing) when `!will_use_umu`.

- **MIRROR**: Warning-color pattern `HealthBadge.tsx:102-116` (`var(--crosshook-color-warning)`). Inline-style approach mirrors what's already in this block (do NOT refactor to `crosshook-status-chip`).
- **IMPORTS**: None new.
- **GOTCHA**: Use `&apos;` in JSX attribute-body text (lint rule). The chip at 432-458 uses `className="crosshook-muted"` — confirm the class exists in `theme.css`; if not, a local style is fine. Keep the chip width bounded (no horizontal overflow); the remediation text is ~2 lines wrapped.
- **VALIDATE**: `./scripts/dev-native.sh --browser` — toggle mock `csv_coverage` between `found`, `missing`, `unknown`; verify three visually-distinct states. `npx --prefix src/crosshook-native @biomejs/biome check src/crosshook-native/src/components/LaunchPanel.tsx`.

### Task C3.2: PinnedProfilesStrip amber badge — Depends on [C2]

- **BATCH**: C3
- **ACTION**: Add an amber ⚠ badge next to the pinned profile chip name when the effective `umu_preference` is `umu` AND the profile's latest cached preview has `csv_coverage === 'missing'` AND `will_use_umu === true`. Do **not** trigger backend previews from the strip.
- **IMPLEMENT**: In `PinnedProfilesStrip.tsx:20-47`, insert inside the existing `<button className="crosshook-pinned-strip__chip">` after the name span:

  ```tsx
  {
    umuCoverageWarn ? (
      <span
        className="crosshook-pinned-strip__badge crosshook-pinned-strip__badge--warn"
        title="umu has no protonfix entry for this app id — launch may crash. Override Runtime → umu launcher to Proton."
        aria-label="umu protonfix missing"
      >
        ⚠
      </span>
    ) : null;
  }
  ```

  Derive `umuCoverageWarn` from the cached preview in `LaunchStateContext` (check whether the existing `usePreviewState` hook exposes per-profile preview or add a memoized selector). If no cached preview exists for a profile, render no badge.
  Add CSS tokens in `src/styles/theme.css`:

  ```css
  .crosshook-pinned-strip__badge {
    margin-left: 6px;
    font-size: 0.85rem;
  }
  .crosshook-pinned-strip__badge--warn {
    color: var(--crosshook-color-warning);
  }
  ```

- **MIRROR**: Badge/chip class precedent at `PinnedProfilesStrip.tsx` (`.crosshook-pinned-strip__chip--active`) + amber color variable from `HealthBadge`.
- **IMPORTS**: Selector or hook for cached preview from `LaunchStateContext`.
- **GOTCHA**: **Performance**: the pinned strip renders all pinned profiles on every mount; do NOT `invoke('preview_launch')` per profile per render. Only render the badge when a cached preview already lives in context (option b in research notes). If `usePreviewState` does not currently cache per-profile results, add a context selector rather than inflating every render with IPC calls.
- **VALIDATE**: `./scripts/dev-native.sh --browser` — pin a profile, toggle mock preference to `umu` + `csv_coverage: 'missing'`; verify amber ⚠ appears. Toggle to `'found'`; verify badge hides. Measure: no extra `preview_launch` IPC calls in the network tab when the strip renders.

### Task C4.1: `umu_database/client.rs` — HTTP fetch + ETag revalidation — Depends on [C1]

- **BATCH**: C4
- **ACTION**: Implement the real HTTP fetcher. Single-URL client targeting `https://raw.githubusercontent.com/Open-Wine-Components/umu-database/main/umu-database.csv` with `OnceLock<reqwest::Client>` singleton; conditional GET via `If-None-Match` (ETag) stored in `external_cache_entries`; body written to `data_local_dir().join("crosshook/umu-database.csv")` for `paths::resolve_umu_database_path()` to pick up.
- **IMPLEMENT**:
  - Singleton + client (6 s timeout, CrossHook user-agent) — mirrors `protondb/client.rs:175-190`.
  - Public API:

    ```rust
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UmuDatabaseRefreshStatus {
        pub refreshed: bool,
        pub cached_at: Option<String>,
        pub source_url: String,
        pub reason: String,
    }

    pub async fn refresh_umu_database() -> Result<UmuDatabaseRefreshStatus, Error> { /* … */ }

    #[cfg(test)]
    pub fn set_source_url_for_test(url: String) { /* OnceLock<Mutex<String>> override */ }
    ```

  - Flow:
    1. Read existing `external_cache_entries` row for `cache_key = "umu-database:csv"` → parse JSON `{etag, last_modified, body_sha256}`.
    2. Build `GET` with `If-None-Match: "<etag>"` and/or `If-Modified-Since: <last_modified>`.
    3. On `304`: update `fetched_at` + `expires_at` (+24 h) via `put_cache_entry`; return `UmuDatabaseRefreshStatus { refreshed: false, cached_at: Some(now), … }`.
    4. On `200`: read body → write atomically (`<target>.tmp` → `fs::rename(&tmp, &target)`) → upsert `external_cache_entries` with new ETag/Last-Modified + `expires_at = now + 24h`; `refreshed: true`.
    5. On network error: log `tracing::warn!`, do NOT touch disk/DB, return `Err`.
  - Cache payload JSON (stored in `external_cache_entries.payload_json`): small header blob with `etag`, `last_modified`, `body_sha256`, `body_bytes` (size) — NOT the body itself.

- **MIRROR**: `SINGLETON_HTTP_CLIENT_PATTERN`, `EXTERNAL_CACHE_PERSIST_PATTERN`, `protondb/client.rs:320-394` for cache read-back.
- **IMPORTS**: `reqwest::{Client, StatusCode, header::{ETAG, IF_NONE_MATCH, IF_MODIFIED_SINCE, LAST_MODIFIED}}`, `chrono::{DateTime, Utc}`, `std::{sync::OnceLock, time::Duration, fs, path::PathBuf}`, `sha2::{Digest, Sha256}`, `crate::metadata::MetadataStore`.
- **GOTCHA**:
  - **Atomic write without `tempfile`**: use `fs::rename(&tmp, &target)` after writing to `<target>.tmp`. `tempfile` stays dev-only.
  - **Body NOT in SQLite**: `MAX_CACHE_PAYLOAD_BYTES` in `metadata/cache_store.rs:37-47` would reject a 900 KB body. Header metadata is <1 KB — safe.
  - **Metadata DB unavailable**: still write CSV to disk (best effort); return `UmuDatabaseRefreshStatus { refreshed: true, cached_at: None, reason: "metadata db unavailable" }`.
  - **No retry/backoff**: single try; the Settings "Refresh" button is the manual retry surface.
  - **Test override**: gate `set_source_url_for_test` behind `#[cfg(test)]` or behind `cfg(feature = "test-override")` — do NOT ship it in release.
  - **Error type**: define a thin `pub enum Error { Network(reqwest::Error), Io(std::io::Error), Metadata(String) }` — do not leak `reqwest::Error` in the public API.
- **VALIDATE**: Unit tests named `client_persists_body_to_disk_and_metadata_on_2xx`, `client_leaves_body_unchanged_on_304`, `client_returns_err_cleanly_on_network_failure`, `client_roundtrips_etag_via_if_none_match` — land in C6.2 via `wiremock`.

### Task C4.2: Startup background refresh — Depends on [C1]

- **BATCH**: C4
- **ACTION**: Hook a non-blocking refresh into app startup. If `external_cache_entries` row for `"umu-database:csv"` is missing or expired, spawn `refresh_umu_database()` via `tauri::async_runtime::spawn` from the Tauri `setup(|app| …)` block in `src-tauri/src/lib.rs`. Do NOT `.await`.
- **IMPLEMENT**:

  ```rust
  // In src-tauri/src/lib.rs inside .setup(|app| { … })
  tauri::async_runtime::spawn(async {
      match crosshook_core::umu_database::refresh_umu_database().await {
          Ok(status) => tracing::info!(?status, "umu-database startup refresh complete"),
          Err(err) => tracing::warn!(%err, "umu-database startup refresh failed"),
      }
  });
  ```

  Optionally gate behind a cache-freshness check (skip the fetch if `expires_at > now`) — the client itself already does conditional GET, so 304 responses are cheap even without gating. Gating is a mild optimization, not required.

- **MIRROR**: Existing startup-spawn patterns in `src-tauri/src/lib.rs` (ProtonDB warm-up, update-check — grep `tauri::async_runtime::spawn`).
- **IMPORTS**: `tauri::async_runtime`.
- **GOTCHA**: Do NOT `.await` the spawn. Logging level: success at `info`, failure at `warn` (not `error`) so offline launches don't paint the log red. First-ever launch with no network leaves `csv_coverage === Unknown` — acceptable.
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/Cargo.toml --workspace`. Manual: run `./scripts/dev-native.sh`, verify log line `"umu-database startup refresh complete"` or the warn variant.

### Task C4.3: Wire real body into `refresh_umu_database` Tauri command — Depends on [C1]

- **BATCH**: C4
- **ACTION**: Replace the C2.3 placeholder body with the real call. Verify `UmuDatabaseRefreshStatus` serde shape matches C2.2's TS counterpart.
- **IMPLEMENT**: The C2.3 scaffold already delegates to `umu_database::refresh_umu_database()`; once C4.1 replaces the placeholder core-side body with the real implementation, C4.3 is effectively a no-op on `src-tauri/` — the scaffold already forwards. This task exists to:
  1. Verify TS `UmuDatabaseRefreshStatus` shape (`{ refreshed: boolean; cached_at: string | null; source_url: string; reason: string }`) matches Rust after C4.1 finalizes field names.
  2. Smoke-test the command round-trip via `cargo test -p crosshook` and a manual browser-dev invocation.
- **MIRROR**: Command-to-core-crate pattern at `src-tauri/src/commands/settings.rs`.
- **IMPORTS**: None new.
- **GOTCHA**: If C4.1 renames any field on `UmuDatabaseRefreshStatus`, this task is where the TS type and mock get aligned.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace`. Manual: `./scripts/dev-native.sh`, click the Settings refresh button, confirm round-trip.

### Task C5.1: Settings panel "Refresh umu protonfix database" button — Depends on [C4]

- **BATCH**: C5
- **ACTION**: Add a button + last-refreshed timestamp row to `SettingsPanel.tsx`, placed below the existing `umu_preference` dropdown (shipped in Phase 3a). Clicking invokes `refresh_umu_database`; disable during pending; render `cached_at`.
- **IMPLEMENT**:

  ```tsx
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [lastStatus, setLastStatus] = useState<UmuDatabaseRefreshStatus | null>(null);
  const onRefresh = async () => {
    setIsRefreshing(true);
    try {
      const status = await invoke<UmuDatabaseRefreshStatus>('refresh_umu_database');
      setLastStatus(status);
    } catch (err) {
      setLastStatus({ refreshed: false, cached_at: null, source_url: '', reason: String(err) });
    } finally {
      setIsRefreshing(false);
    }
  };
  // …
  <button className="crosshook-button" onClick={onRefresh} disabled={isRefreshing}>
    {isRefreshing ? 'Refreshing…' : 'Refresh umu protonfix database'}
  </button>
  <div className="crosshook-muted">
    Last refreshed: {lastStatus?.cached_at ? formatRelative(lastStatus.cached_at) : 'never'}
  </div>
  ```

- **MIRROR**: Existing async-button pattern in `SettingsPanel.tsx` (search for `invoke` + `useState` pairs).
- **IMPORTS**: `invoke` from `@tauri-apps/api/core` (or the project's alias); `UmuDatabaseRefreshStatus` type.
- **GOTCHA**: On failure render the error inline; do NOT toast. If `cached_at === null`, render "Never refreshed" (or "Refreshed (cache unavailable)" when `refreshed: true`).
- **VALIDATE**: `./scripts/dev-native.sh --browser` — click button, verify spinner + success state against the C5.2 mock. Biome clean.

### Task C5.2: Mock `refresh_umu_database` — Depends on [C4]

- **BATCH**: C5
- **ACTION**: Add a `refresh_umu_database` handler to the browser-dev-mode mock layer. Reuse `lib/mocks/handlers/launch.ts` (closest to the umu surface) or add `lib/mocks/handlers/umu_database.ts` and register in the mocks barrel.
- **IMPLEMENT**:

  ```ts
  map.set('refresh_umu_database', async () => ({
    refreshed: true,
    cached_at: new Date(Date.now() - 8 * 3600 * 1000).toISOString(),
    source_url: 'https://raw.githubusercontent.com/Open-Wine-Components/umu-database/main/umu-database.csv',
    reason: 'mocked — no network fetch',
  }));
  ```

- **MIRROR**: Mock-registration pattern at `lib/mocks/handlers/launch.ts`.
- **IMPORTS**: None new.
- **GOTCHA**: CI `verify:no-mocks` sentinel — stay inside `src/lib/mocks/`.
- **VALIDATE**: `./scripts/dev-native.sh --browser`. `./scripts/lint.sh` including the mocks sentinel.

### Task C6.1: `umu_database_coverage` integration test — Depends on [C2, C3, C4, C5]

- **BATCH**: C6
- **ACTION**: Create `crates/crosshook-core/tests/umu_database_coverage.rs`. Use `tempfile` + `std::env::set_var("XDG_DATA_DIRS", …)` (scoped with a file-local `Mutex<()>` to serialize with other env-manipulating tests) to stage a fixture CSV in a path `resolve_umu_database_path` can find. Assert all three `CsvCoverage` variants.
- **IMPLEMENT**: Mirror `crates/crosshook-core/tests/config_history_integration.rs` layout. Fixture CSV inlined as a `const FIXTURE: &str = "…"` with 3 rows (one `found` case, one to test `missing`). Test names: `coverage_found_for_known_app_id`, `coverage_missing_for_absent_app_id`, `coverage_unknown_when_no_csv_source`, `coverage_respects_mtime_cache_invalidation`.
- **MIRROR**: `crates/crosshook-core/tests/config_history_integration.rs` integration-test layout.
- **IMPORTS**: `tempfile::TempDir`, `std::env`, `crosshook_core::umu_database::{check_coverage, CsvCoverage, resolve_umu_database_path}`.
- **GOTCHA**: `std::env::set_var` is process-global. Use `once_cell::sync::Lazy<Mutex<()>>` (or `std::sync::Mutex`) held across each test to serialize; otherwise parallel tests will race each other's env. Also: `coverage.rs`'s OnceLock cache may hold stale state across tests — expose a `#[cfg(test)] pub fn clear_cache_for_test()` helper in `coverage.rs` or use a unique app id per test so cache doesn't collide.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --test umu_database_coverage`. All four tests pass; completes <2 s.

### Task C6.2: `umu_database_http_cache` wiremock integration test — Depends on [C2, C3, C4, C5]

- **BATCH**: C6
- **ACTION**: Create `crates/crosshook-core/tests/umu_database_http_cache.rs`. Stand up `wiremock::MockServer`, inject URL via `umu_database::client::set_source_url_for_test`, exercise: (1) 200 fresh → disk + SQLite; (2) 304 Not Modified on `If-None-Match` → body unchanged, `fetched_at` advanced; (3) network error → existing cache intact, returns `Err`.
- **IMPLEMENT**: Use `tempfile::TempDir` + scoped `HOME` / `XDG_DATA_HOME` env so writes land in a throwaway directory. Mirror existing `wiremock`-based integration tests in `crosshook-core`.
- **MIRROR**: Existing wiremock tests in the crate (grep `wiremock::MockServer`).
- **IMPORTS**: `wiremock::{MockServer, Mock, ResponseTemplate, matchers::{method, path, header}}`, `tempfile::TempDir`.
- **GOTCHA**: Serialize with the same mutex as C6.1 if both run in the same `cargo test` process. **Metadata DB**: the test must get its own isolated `metadata.db` — either via scoped `XDG_DATA_HOME` or by explicitly opening a `MetadataStore` at a tempdir path.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --test umu_database_http_cache`. Completes <3 s. Zero leaked files under `$HOME/.local/share/crosshook/`.

### Task C6.3: `preview.rs` `csv_coverage` tests — Depends on [C2, C3, C4, C5]

- **BATCH**: C6
- **ACTION**: Add preview-level tests mirroring the shipped Phase 3a umu tests. Stub `XDG_DATA_DIRS` with a fixture CSV; build a `LaunchRequest` with a specific `steam.app_id`; call `build_launch_preview`; assert `umu_decision.csv_coverage`.
- **IMPLEMENT**: Test names: `preview_reports_csv_coverage_found_when_app_id_matches`, `preview_reports_csv_coverage_missing_when_app_id_absent`, `preview_reports_csv_coverage_unknown_when_no_csv_source`. Reuse `proton_request()` helper from `preview.rs` tests module.
- **MIRROR**: Existing `preview_*` tests at `preview.rs:1057+`; `ScopedCommandSearchPath` PATH helper.
- **IMPORTS**: Same as C6.1 + `crosshook_core::launch::preview::build_launch_preview`.
- **GOTCHA**: Preview tests already manipulate PATH — stacking with `XDG_DATA_DIRS` requires serialized execution. Reuse the same mutex introduced in C6.1.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::preview::tests::preview_reports_csv_coverage` all green.

### Task C7.1: PRD update + plan closeout + issue housekeeping — Depends on [C6]

- **BATCH**: C7
- **ACTION**: Update `docs/prps/prds/umu-launcher-migration.prd.md`:
  - **Decisions Log**: add row `CSV source precedence | HTTP cache → /usr/share/umu-protonfixes → alternates → XDG_DATA_DIRS → Unknown | dirname-only / HTTP-only | Flatpak-safe + offline-first; no host dependency`.
  - **Storage Boundary** table: add `umu-database CSV body at ~/.local/share/crosshook/umu-database.csv` → **Runtime-only (derived)** with rationale "regenerated from upstream on refresh; never user-edited"; add `external_cache_entries row cache_key="umu-database:csv"` → **SQLite metadata** with rationale "ETag + Last-Modified + body_sha256 for conditional revalidation".
  - **Open Questions** §GAMEID auto-resolve: mark resolved — "#247 covers the cache layer (full CSV); per-id HTTP endpoints remain deferred, #251 closed as duplicate of #247."
  - **Implementation Phases** table: add subrow Phase 3b with link to this plan.
  - **GitHub issues** table: add rows for #263, #247; mark #251 closed as duplicate.

  Close #251 + #247 + #263 via PR body: `Closes #247, #263; closes #251 (duplicate of #247)`.

  When the continuation is fully implemented, move **both** plan files (`umu-migration-phase-3-umu-opt-in.plan.md` and `umu-migration-phase-3b-umu-opt-in.plan.md`) to `docs/prps/plans/completed/` and write `docs/prps/reports/umu-migration-phase-3b-umu-opt-in-report.md` covering just the 3b scope.

- **IMPLEMENT**: Straightforward documentation edits. No code.
- **MIRROR**: Phase 2 closeout at `docs/prps/plans/completed/umu-migration-phase-2-sandbox-allowlist.plan.md`.
- **IMPORTS**: None.
- **GOTCHA**: Do NOT move either plan file to `completed/` until merge — final implementer step.
- **VALIDATE**: `./scripts/lint.sh` passes (prettier + markdown); PRD table still renders correctly.

## Testing Strategy

### Unit tests (Rust)

| Test                                                      | Input                                                                       | Expected Output                                        | Edge Case?               |
| --------------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------ | ------------------------ |
| `resolve_returns_none_when_no_candidate_exists`           | Empty `XDG_DATA_DIRS`, no files at any candidate                            | `None`                                                 | —                        |
| `resolve_returns_http_cache_first`                        | HTTP cache + `/usr/share/umu-protonfixes/` both stubbed via tempdir         | HTTP cache path returned                               | Precedence               |
| `coverage_found_for_known_app_id`                         | Fixture CSV with app id `546590` (Ghost of Tsushima)                        | `CsvCoverage::Found`                                   | —                        |
| `coverage_missing_for_absent_app_id`                      | Same fixture, app id `292030` (Witcher 3)                                   | `CsvCoverage::Missing`                                 | Motivating case (#262)   |
| `coverage_unknown_when_no_csv_source`                     | No CSV on any candidate path                                                | `CsvCoverage::Unknown`                                 | Flatpak degraded state   |
| `coverage_unknown_for_empty_app_id`                       | Any CSV, `app_id = ""`                                                      | `CsvCoverage::Unknown`                                 | Defensive                |
| `coverage_reuses_cached_index_when_mtime_unchanged`       | Two calls, same CSV                                                         | Second call does not re-parse                          | Perf / cache invariant   |
| `coverage_rebuilds_index_when_mtime_changes`              | Two calls, CSV rewritten between                                            | Second call re-parses                                  | Cache invalidation       |
| `preview_reports_csv_coverage_found_when_app_id_matches`  | Fixture CSV, `steam.app_id = "546590"`                                      | `preview.umu_decision.csv_coverage == Found`           | Preview integration      |
| `preview_reports_csv_coverage_missing_when_app_id_absent` | Same, `app_id = "292030"`                                                   | `Missing`                                              | Preview integration      |
| `preview_reports_csv_coverage_unknown_when_no_csv_source` | No CSV anywhere                                                             | `Unknown`                                              | Preview integration      |
| `client_persists_body_to_disk_and_metadata_on_2xx`        | wiremock 200 + body                                                         | CSV written to disk; `external_cache_entries` upserted | HTTP refresh             |
| `client_leaves_body_unchanged_on_304`                     | wiremock 304 Not Modified                                                   | Body on disk unchanged; `fetched_at` advanced          | Conditional revalidation |
| `client_returns_err_cleanly_on_network_failure`           | Server down                                                                 | Returns `Err`; existing cache intact                   | Offline degraded         |
| `client_roundtrips_etag_via_if_none_match`                | 1st call: 200 w/ ETag "A". 2nd call: expect `If-None-Match: "A"` on request | Header present                                         | Header plumbing          |

### Edge cases checklist

- [x] Empty `steam.app_id` → `Unknown`
- [x] Network offline during background refresh → warn log, existing cache intact
- [x] CSV parse error (malformed row) → `Unknown`, launch continues
- [x] `metadata.db` unavailable → HTTP refresh still writes CSV to disk; `cached_at: None`
- [x] Flatpak sandbox (no `/usr` access) → HTTP cache is the only source; `Unknown` until first successful refresh
- [x] `UmuPreference::Umu` + `umu-run` missing → Phase 3a warn fallback already handles; coverage chip stays advisory
- [x] Rapid CSV mtime changes (file being rewritten mid-read) → `OnceLock<Mutex>` serializes; `Unknown` on partial read

### TypeScript / Frontend manual QA

- [ ] `./scripts/dev-native.sh --browser`, toggle mock `csv_coverage: 'missing'` + `will_use_umu: true` — amber chip renders with remediation copy
- [ ] Toggle to `'found'` — green chip, no warning
- [ ] Toggle preference to `Proton` — chip shows `requested preference: proton`; `csv_coverage` may be `unknown` (no app id fetch triggered)
- [ ] Pin a profile with `umu_preference=umu` + mocked `csv_coverage=missing` — amber ⚠ badge appears on pinned chip; title attribute explains
- [ ] Settings panel → "Refresh umu protonfix database" — button disables during invoke, last-refreshed timestamp updates

## Validation Commands

### Static Analysis

```bash
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings
```

EXPECT: Zero format diffs; zero clippy warnings.

### Unit Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core umu_database
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::preview
```

### Integration Tests (new)

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --test umu_database_coverage
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --test umu_database_http_cache
```

### Full Workspace

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace
./scripts/lint.sh
```

### Browser Dev Mode

```bash
./scripts/dev-native.sh --browser
```

## Acceptance Criteria

- [ ] All 15 continuation tasks (C1.1 – C7.1) completed
- [ ] `csv` crate added to `crosshook-core` deps; no other new workspace deps
- [ ] `CsvCoverage { Found, Missing, Unknown }` exists, round-trips via Serde
- [ ] `resolve_umu_database_path()` respects 5-tier precedence (HTTP cache → `/usr/share/umu-protonfixes` → `/usr/share/umu` → `/opt/…` → `$XDG_DATA_DIRS`)
- [ ] `UmuDecisionPreview.csv_coverage` populated for all `proton_run` previews; `Unknown` when app_id empty or no CSV reachable
- [ ] `LaunchPanel` chip surfaces amber warning + remediation copy when `will_use_umu && csv_coverage === 'missing'`
- [ ] `PinnedProfilesStrip` shows amber ⚠ badge under the same condition (and only then — no spurious renders, no per-render backend IPC)
- [ ] `refresh_umu_database` IPC command + `SettingsPanel` button work end-to-end; status round-trips through Rust → TS
- [ ] Background refresh on app startup is non-blocking (`tokio::spawn`, no `.await` at setup)
- [ ] ETag / `If-None-Match` roundtrip verified by a wiremock test
- [ ] Offline startup logs a warn + leaves existing cache intact
- [ ] Browser dev mode renders the chip AND badge with mocked `csv_coverage` (fixes the latent `umu_decision`-always-null mock gap)
- [ ] PRD Decisions Log lists the CSV source precedence; Storage Boundary lists the CSV body (runtime disk) + `external_cache_entries` row (SQLite metadata)
- [ ] #251 closed as duplicate of #247 in PR body; #263 + #247 closed in PR body
- [ ] Zero changes to: `packaging/flatpak/*.yml`, `onboarding/readiness.rs`, `export/launcher.rs`, Phase 4 default-flip code paths, Phase 5 install-nag fields, Steam-profile code paths

## Completion Checklist

- [ ] Code follows discovered patterns (all 8 `MIRROR` references above)
- [ ] Error handling: `umu_database::client` returns a scoped `Error` enum (not `reqwest::Error` directly); Tauri command maps to `String`
- [ ] Logging: `tracing::info!` on successful startup refresh; `tracing::warn!` on degraded paths; `tracing::debug!` on coverage lookups
- [ ] Tests follow existing naming (`<subject>_<verb>_<qualifier>`) and use the existing `command_env_value` / `ScopedCommandSearchPath` helpers where relevant
- [ ] No hardcoded values beyond the documented constants (`"umu-database:csv"`, `"umu-0"` fallback already shipped in 3a, the upstream URL)
- [ ] Documentation: PRD Decisions Log + Storage Boundary rows added in C7.1 — no CLAUDE.md / AGENTS.md churn
- [ ] No speculative scope (all Phase 4/5/6 items remain deferred; per-id auto-resolve not implemented)

## Risks

| Risk                                                                               | Likelihood | Impact                                            | Mitigation                                                                                                                                  |
| ---------------------------------------------------------------------------------- | ---------- | ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `XDG_DATA_DIRS` env manipulation in tests races parallel `std::env` writers        | M          | Medium — flaky CI                                 | Serialize with a file-local `Mutex<()>`; optionally adopt `serial_test`; document in test file header                                       |
| Upstream CSV header names change                                                   | L          | Medium — every lookup `Unknown`                   | `ReaderBuilder::flexible(true)`; `tracing::warn!` on parse failure; unit test on golden fixture. Recovery: update Serde renames.            |
| Distro bundles CSV at a path not in the 5-tier list                                | M          | Medium — native users see `Unknown`               | Document the 5-tier list in PRD footnote so distro PRs can extend; Settings refresh button populates the HTTP cache as a universal fallback |
| Flatpak users never hit 2xx on first launch (network slow)                         | L          | Low — `Unknown` until refresh                     | Non-blocking spawn; subsequent launches pick up cached CSV; Settings manual refresh is the explicit fallback                                |
| Pinned-strip badge triggers backend preview calls per profile per render           | H          | Medium — perf regression                          | C3.2 GOTCHA — only render when cached preview exists; never trigger backend IPC from strip render                                           |
| `external_cache_entries.payload_size` cap rejects metadata header                  | L          | Low — metadata stays <1 KB                        | Metadata payload is a small JSON header (etag + last-modified + sha256); CSV body lives on disk                                             |
| Phase 4 (Auto default-on) ships before 3b — more users hit Witcher-3-class crashes | M          | Medium — user trust hit                           | **Ship 3b before Phase 4 default-flip.** Update PRD Phase 4 gate: "Phase 4 gated on #263 + #247 landed AND 2-week observation clean."       |
| `set_source_url_for_test` leaks into release builds                                | L          | Low — harmless test hook                          | Gate behind `#[cfg(test)]`; do not expose via any public API in release                                                                     |
| `MetadataStore` unavailable during refresh                                         | L          | Low — CSV still written to disk                   | C4.1 GOTCHA — disk write is best-effort independent of metadata DB; `cached_at: None` signals the degraded state                            |
| `std::env::set_var("HOME", …)` in tests races with real-user-config reads          | M          | Medium — test pollutes user home if mutex dropped | Test setup MUST acquire the shared mutex before setting HOME; teardown restores; use `tempfile::TempDir` with `drop` guard                  |
| `tokio::spawn` at app setup competes with Tauri's event loop at cold start         | L          | Low — 6s timeout is small                         | Already acceptable for ProtonDB warm-up precedent                                                                                           |

## Notes

- **#251 duplicate close**: Issue body already states "covered by #247". PR body closes both.
- **Motivating Witcher 3 example verified upstream (2026-04-14)**: App id `292030` is absent from `raw.githubusercontent.com/Open-Wine-Components/umu-database/main/umu-database.csv`. The MD5 `STEAM_COMPAT_APP_ID` overwrite is at `umu/umu_run.py:515` (not `:299` as noted in issue #263 — correct in PR body if relevant). Canonical implementer test case: app id `292030` → `Missing`.
- **Storage boundary classification** (per CLAUDE.md persistence rule):
  - `UmuDecisionPreview.csv_coverage` → **Runtime-only** (computed per preview, never persisted)
  - Cached CSV body at `~/.local/share/crosshook/umu-database.csv` → **Runtime-only (derived)** — regenerated from upstream
  - `external_cache_entries` row for `"umu-database:csv"` → **SQLite metadata** (ETag, Last-Modified, fetched_at, expires_at)
  - No new TOML settings — coverage check is always-on; no user-tunable knob
- **User visibility / editability**:
  - `csv_coverage` visible in Launch Preview + pinned strip (read-only)
  - Last-refreshed timestamp visible in Settings
  - No user-editable CSV path — auto-resolved; a user who drops a file at `~/.local/share/crosshook/umu-database.csv` will have CrossHook prefer it (HTTP cache path is the first-priority readable candidate)
- **Phase 4 coupling**: Shipping 3b makes Phase 4 (`UmuPreference::Auto` default-on) materially safer. Recommend gating Phase 4 ship on this continuation landing + 1-week observation of `area:launch` / `feat:umu-launcher` labels. C7.1 updates PRD Phase 4 prerequisites.
- **Research gaps carried forward**:
  - GitHub raw ETag behavior on 2026-04-14 assumed (infra research flagged as unverified). Implementer should `curl -I` the raw URL at start of C4.1 to confirm.
  - Distro CSV install paths vary — 5-tier list covers Arch multilib, Fedora (where packaged), manual `/opt` installs, and XDG fallback. Implementers on other distros should add a 6th candidate via PR.
  - No benchmark for `csv` crate on ~2K-row files; <10 ms parse is an assumption. Validate with an in-test timer if perf regresses.
- **Rollback plan**: If 3b ships and the chip generates false positives (e.g. upstream CSV regression removes a row), users can (a) use the per-profile `umu_preference = proton` override (3a escape hatch), or (b) ignore the advisory chip — it never blocks launch. No emergency revert needed.
- **Follow-ups after merge**:
  - File upstream umu-database PR for Witcher 3 (tracked as [#262](https://github.com/yandy-r/crosshook/issues/262)) — separate from 3b.
  - Monitor chip false-positives; extend 5-tier path list if new distros surface.
  - When 3b is fully implemented, move **both** phase 3 plan files to `docs/prps/plans/completed/` and write `docs/prps/reports/umu-migration-phase-3b-umu-opt-in-report.md`.
