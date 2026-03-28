# Architecture Research: Profile Health Dashboard

The health dashboard uses a two-layer architecture: a filesystem-first core module (`profile/health.rs`) in `crosshook-core` that accepts `Option<&MetadataStore>` for optional enrichment, and an orchestration layer (`commands/health.rs`) in `src-tauri` that composes both stores following the established dual-store Tauri command pattern. `commands/profile.rs` is the verbatim reference implementation. No new Rust crate dependencies are required.

---

## Relevant Components

### Core Library (`crosshook-core`)

- `/src/crosshook-native/crates/crosshook-core/src/lib.rs` — Module root; declares `pub mod profile`, `pub mod metadata`, `pub mod launch`
- `/src/crosshook-native/crates/crosshook-core/src/profile/mod.rs` — Re-exports `ProfileStore`, `GameProfile`, and all profile types; add `pub mod health` and re-exports here
- `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — `GameProfile` struct with sections: `GameSection` (`executable_path`), `TrainerSection` (`path`), `InjectionSection` (`dll_paths`), `SteamSection` (`compatdata_path`, `proton_path`), `RuntimeSection` (`prefix_path`, `proton_path`), `LaunchSection` (`method`)
- `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs` — `ProfileStore` (filesystem TOML CRUD); `base_path: PathBuf` field is `pub`
- `/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — `MetadataStore` API; `with_conn()` returns `T::default()` when store is disabled; `query_failure_trends()`, `query_last_success_per_profile()`, `lookup_profile_id()`
- `/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` — `DriftState`, `FailureTrendRow`, `LaunchOutcome`, `MetadataStoreError`, `SyncSource`
- `/src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs` — manages `launchers.drift_state` column; no method yet exposes per-profile drift state for health queries (gap — see Gotchas)
- `/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — sequential migrations v0→v5; Phase D `health_snapshots` table will be migration v6
- `/src/crosshook-native/crates/crosshook-core/src/launch/request.rs` — `ValidationError`, `LaunchValidationIssue`, `ValidationError::issue()`, and the private helpers `require_directory`, `require_executable_file`, `is_executable_file` that health.rs will reuse after promotion to `pub(crate)`

### Tauri Shell (`src-tauri`)

- `/src/crosshook-native/src-tauri/src/lib.rs` — App entry: initializes all stores, `.manage()`s them, registers commands in `invoke_handler!`; `tauri::Emitter` already imported
- `/src/crosshook-native/src-tauri/src/commands/mod.rs` — `pub mod` list; add `pub mod health` here
- `/src/crosshook-native/src-tauri/src/commands/profile.rs` — **Canonical reference** for dual-store command pattern: `State<'_, ProfileStore>` + `State<'_, MetadataStore>` params, fail-soft `tracing::warn!` on metadata errors
- `/src/crosshook-native/src-tauri/src/startup.rs` — `run_metadata_reconciliation()` (sync) + `tauri::async_runtime::spawn` for `auto-load-profile` event; Phase C health scan follows the same async spawn + `app_handle.emit()` shape

### React Frontend

- `/src/crosshook-native/src/App.tsx` — App shell; routes: `profiles`, `launch`, `install`, `community`, `compatibility`, `settings`; `VALID_APP_ROUTES` must be updated if a `health` route is added
- `/src/crosshook-native/src/components/layout/ContentArea.tsx` — Route → page mapping with `const _exhaustive: never` exhaustiveness guard; any new route requires an entry in both `VALID_APP_ROUTES` and the `renderPage()` switch
- `/src/crosshook-native/src/components/pages/ProfilesPage.tsx` — Profile list page; consumes `useProfileContext()`; health badges integrate here alongside the existing profile list
- `/src/crosshook-native/src/hooks/useProfile.ts` — Pattern for new `useProfileHealth` hook: `invoke` + loading/error state + typed result
- `/src/crosshook-native/src/components/layout/PageBanner.tsx` — Existing banner component for startup broken-profile notification

---

## Data Flow

### Profile Data (existing, health reads from this)

```
~/.config/crosshook/*.toml
  → ProfileStore::list() / load()             (crosshook-core, TOML deserialization)
    → commands/profile.rs Tauri command        (src-tauri, State<ProfileStore>)
      → invoke('profile_load')                (React, @tauri-apps/api/core)
        → useProfile hook / ProfileContext
          → ProfilesPage / ProfileFormSections
```

### MetadataStore Data (parallel, enriches health)

```
~/.local/share/crosshook/metadata.db         (SQLite, WAL mode)
  → MetadataStore methods                    (crosshook-core, Arc<Mutex<Connection>>)
    → with_conn() fail-soft guard            (returns T::default() when disabled)
      → commands/*.rs via State<MetadataStore>
        → React via invoke()
```

### Health Data Flow (new)

```
commands/health.rs::batch_validate_profiles(
    store: State<ProfileStore>,
    metadata_store: State<MetadataStore>
)
  → store.list()  →  for each name:
      → store.load(name)  [on error: emit Broken sentinel, continue]
      → profile/health.rs::check_profile_health(
            &GameProfile,
            Option<&MetadataStore>   ← pass Some(&metadata_store) from command layer
        )
          → resolve_launch_method() to determine which fields to check
          → require_directory / require_executable_file  [promoted to pub(crate)]
          → ValidationError::issue() to construct HealthIssue values
          → if metadata available:
              → query_failure_trends(30) → tag Degraded
              → query_last_success_per_profile() → attach last_success
              → query launcher drift per profile_id → attach DriftState
          → returns ProfileHealthReport { name, status, issues, checked_at, metadata? }

  → emit("profile-health-batch-complete", EnrichedHealthCheckSummary)  [Phase C]
  → return EnrichedHealthCheckSummary to frontend

React:
  useProfileHealth hook
    → invoke('batch_validate_profiles') or listen("profile-health-batch-complete")
    → HealthBadge renders status per profile
    → ProfileHealthDashboard renders summary counts + per-profile table
    → PageBanner renders startup notification for broken profiles
```

---

## Integration Points

### `profile/health.rs` (new file)

- **Declare**: Add `pub mod health;` to `crates/crosshook-core/src/profile/mod.rs`
- **Re-export**: Add `pub use health::{HealthStatus, ProfileHealthReport, HealthCheckSummary, ...}` in the same `mod.rs`
- **Function signature**: `pub fn check_profile_health(name: &str, profile: &GameProfile, metadata: Option<&MetadataStore>) -> ProfileHealthReport`
- **Batch function**: `pub fn batch_check_health(store: &ProfileStore, metadata: Option<&MetadataStore>) -> HealthCheckSummary` — iterates `store.list()`, handles per-profile load errors without aborting
- **Path check helpers**: Requires `require_directory`, `require_executable_file`, `is_executable_file` promoted from `fn` → `pub(crate) fn` in `launch/request.rs`
- **Issue construction**: Use `ValidationError::issue()` — not ad-hoc string literals

### `commands/health.rs` (new file)

- **Declare**: Add `pub mod health;` to `src-tauri/src/commands/mod.rs`
- **Register**: Add entries to `invoke_handler!` in `src-tauri/src/lib.rs` (line ~85):
  ```rust
  commands::health::batch_validate_profiles,
  commands::health::get_profile_health,
  ```
- **State signature**: `(store: State<'_, ProfileStore>, metadata_store: State<'_, MetadataStore>)` — both already managed, no new `.manage()` calls
- **Metadata enrichment**: Call `batch_check_health(&store, Some(&metadata_store))` — pass through to core; commands layer handles Tauri-specific concerns (serialization, emit)
- **Startup integration (Phase C)**: In `lib.rs` setup closure, after the existing `auto-load-profile` spawn block (~line 61), add a second `tauri::async_runtime::spawn` that clones `profile_store` and `metadata_store` then calls the health scan and emits `"profile-health-batch-complete"`

### Frontend

- **New hook**: `src/hooks/useProfileHealth.ts` — mirrors `useProfile.ts`: `invoke` call, `loading`/`error` state, typed `HealthCheckSummary` result, `listen` for startup push event
- **New components**: `src/components/ProfileHealthDashboard.tsx`, `src/components/ui/HealthBadge.tsx`
- **ProfilesPage integration**: Import `useProfileHealth` result; render `HealthBadge` inline with each profile list entry
- **Startup banner**: In `ProfilesPage.tsx` or `App.tsx`, render `PageBanner` with broken-profile count on `"profile-health-batch-complete"` event

---

## Key Dependencies

### Existing APIs used by health (no new tables Phase A/B)

| API                                               | Location                              | Purpose                                       |
| ------------------------------------------------- | ------------------------------------- | --------------------------------------------- |
| `ProfileStore::list()`                            | `profile/toml_store.rs`               | Enumerate all profiles for batch scan         |
| `ProfileStore::load(name)`                        | `profile/toml_store.rs`               | Load `GameProfile` per profile                |
| `ProfileStore::base_path` (pub)                   | `profile/toml_store.rs`               | Derive TOML path for MetadataStore lookup     |
| `resolve_launch_method()`                         | `profile/models.rs`                   | Determine which fields to validate            |
| `ValidationError::issue()`                        | `launch/request.rs`                   | Construct `HealthIssue` values consistently   |
| `require_directory` (→ `pub(crate)`)              | `launch/request.rs:700`               | Path existence + type check                   |
| `require_executable_file` (→ `pub(crate)`)        | `launch/request.rs:721`               | Executable path check                         |
| `is_executable_file` (→ `pub(crate)`)             | `launch/request.rs:742`               | `PermissionsExt::mode() & 0o111` check        |
| `MetadataStore::query_failure_trends(days)`       | `metadata/mod.rs`                     | Degraded classification (BR-NEW-1)            |
| `MetadataStore::query_last_success_per_profile()` | `metadata/mod.rs`                     | Last-success timestamp (US-10)                |
| `MetadataStore::lookup_profile_id(name)`          | `metadata/mod.rs`                     | UUID for rename-stable persistence (Phase D)  |
| `DriftState` enum                                 | `metadata/models.rs`                  | Launcher drift secondary indicator (BR-NEW-3) |
| `AppHandle::emit()`                               | `tauri::Emitter` (imported in lib.rs) | Push startup scan result to frontend          |

### Fail-soft pattern (from `commands/profile.rs`)

Metadata observation calls (fire-and-forget):

```rust
if let Err(e) = metadata_store.some_operation(...) {
    tracing::warn!(%e, "metadata enrichment failed — health result unaffected");
}
```

Metadata query calls (enrichment, degrade gracefully):

```rust
// with_conn() returns Vec::default() when MetadataStore is disabled — no explicit guard needed
let failure_trends = metadata_store.query_failure_trends(30).unwrap_or_default();
let last_success = metadata_store.query_last_success_per_profile().unwrap_or_default();
```

### Migration Note

Current schema level: v5. Phase D `health_snapshots` table requires migration v6, following the sequential `if version < N` pattern in `metadata/migrations.rs`. Phase A/B require no schema changes.

---

## Gotchas

- **`profile/health.rs` accepts `Option<&MetadataStore>`, it is not metadata-free**: The spec's "no MetadataStore dependency" refers to the module not _requiring_ metadata — it accepts `Option<&MetadataStore>` and degrades silently when `None`. The metadata module is still an import; it is just optional at runtime.

- **`require_directory`, `require_executable_file`, `is_executable_file` are private**: All three are `fn` (no visibility modifier) at `launch/request.rs:700,721,742`. They must be promoted to `pub(crate)` before `profile/health.rs` can use them. This is a required code change in the implementation phase.

- **`ValidationError::issue()` must be used for HealthIssue construction**: Do not construct `LaunchValidationIssue` (or any `HealthIssue`) with ad-hoc strings — use `ValidationError::issue()` which calls the canonical `.message()`, `.help()`, and `.severity()` implementations.

- **`MetadataStore::available` is private**: The field `available: bool` is not accessible from `src-tauri`. The fail-soft mechanism is built into `with_conn()` returning `T::default()` — use `.unwrap_or_default()` on enrichment queries rather than an explicit availability guard.

- **No existing MetadataStore method returns per-profile `DriftState`**: `launcher_sync.rs` manages the `launchers.drift_state` column but no public `MetadataStore` method exposes it for health queries. Querying drift state for health enrichment requires either a new `MetadataStore::query_launcher_drift_by_profile()` method or an inline SQL query in `commands/health.rs` joining `launchers` on `profile_id`.

- **`ProfileStore::base_path` is `pub`**: `store.base_path.join(format!("{name}.toml"))` is the established pattern for deriving a profile's TOML path (see `commands/profile.rs:108`).

- **Single-profile parse failure must not abort batch**: `ProfileStore::load()` returns `Result`; the batch loop in `batch_check_health()` must `continue` on error and emit a synthetic `Broken` `ProfileHealthReport` with a "Profile data could not be read" issue for the failed entry.

- **Method-aware validation scope**: Health checks only validate fields required by the resolved launch method. Call `resolve_launch_method()` (exported from `profile/models.rs`) before deciding which paths to inspect. `steam.proton_path` is only checked for `steam_applaunch`; `runtime.prefix_path` only for `proton_run`.

- **Degraded is a sub-state of filesystem-Healthy**: Only apply `Degraded` when the primary filesystem check returns `Healthy` AND `FailureTrendRow.failures >= 2 && successes == 0` over the last 30 days. `clean_exit` outcomes are `LaunchOutcome::Succeeded` — not counted as failures.

- **Drift is a separate dimension, not a primary badge modifier**: `DriftState::Missing | Moved | Stale` produces an amber secondary indicator alongside the primary `HealthStatus` badge. Do not map drift to `Stale` or `Broken` health status.

- **`query_failure_trends` uses SQLite `FILTER` clause**: Requires SQLite 3.25+. Bundled SQLite 3.51.1 via rusqlite 0.38.0 satisfies this.

- **Startup scan must be async**: `run_metadata_reconciliation()` in `startup.rs` is synchronous and runs in the setup closure. The Phase C health scan must use `tauri::async_runtime::spawn` (matching the `auto-load-profile` pattern at `lib.rs:61-70`) to avoid blocking UI render.

- **`AppRoute` exhaustive match**: `ContentArea.tsx` has `const _exhaustive: never = route` — any new `health` route requires updates to both `VALID_APP_ROUTES` and the `renderPage()` switch in `ContentArea.tsx`, plus the `AppRoute` type in `Sidebar.tsx`.

- **`GameProfile → LaunchRequest` divergence**: Health checks validate at-rest filesystem state only. A filesystem-healthy profile can still fail `validate_launch()`. Do not conflate health status with launch-readiness.
