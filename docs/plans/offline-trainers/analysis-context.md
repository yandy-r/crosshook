# Context Analysis: offline-trainers

## Executive Summary

The offline-trainers feature adds trainer type classification (data-driven TOML catalog), SHA-256 hash caching with stat-based invalidation, a 0-100 readiness scoring system, and pre-flight launch checks — integrating into the existing profile/health/launch pipeline. CrossHook is already ~80% offline-capable; the remaining work extends existing modules rather than building greenfield infrastructure. No new Rust crate dependencies are required.

## Architecture Context

- **System Structure**: Rust `crosshook-core` library → Tauri IPC commands (`src-tauri/src/commands/`) → React frontend via `invoke()`. New offline logic extends existing `profile/`, `metadata/`, `launch/`, and `community/` modules. A new `commands/offline.rs` Tauri command file exposes the offline API surface. `feature-spec.md` (authoritative) uses a data-driven TOML catalog (pattern: `launch/catalog.rs`) for trainer type definitions — the **only compiled enum is `OfflineCapability`**; vendor entries live in `assets/default_trainer_type_catalog.toml`.

- **Data Flow**:
  1. Profile save → hash trainer binary via `hash_trainer_file()` → store in `version_snapshots.trainer_file_hash` (existing) OR new `trainer_hash_cache` table (migration 13) — **decision: feature-spec uses new table; practices research says reuse existing column; must resolve before implementation**
  2. Launch → offline pre-flight check (file existence + hash compare via `compute_correlation_status()`) → warning if score < threshold → proceed (non-blocking)
  3. Health dashboard → `batch_offline_readiness()` → reads from `offline_readiness_snapshots` SQLite table → surfaces per-profile scores
  4. Community tap sync failure → `CommunityTapError::Git` caught → fallback to `local_path` on disk (already cloned) → show staleness notice

- **Integration Points**:
  - `profile/models.rs::TrainerSection`: add `trainer_type: Option<String>` (catalog id ref); `kind` stays as display-only string
  - `profile/health.rs`: offline readiness produces `HealthIssue { field: "offline_readiness", severity: Warning }` entries pushed into the **existing** `check_profile_health()` issues vec — not a separate function or report. This means health dashboard displays offline issues with zero UI changes
  - `launch/request.rs`: add `ValidationError::OfflineReadinessInsufficient` variant; severity = Warning (not Fatal, follows GamescopeNestedSession pattern)
  - `metadata/migrations.rs`: add migration 13 (`trainer_hash_cache`, `offline_readiness_snapshots`, `community_tap_offline_state`)
  - `community/taps.rs`: wrap `git fetch` non-zero exit as graceful degradation; add `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0` env vars
  - `settings/mod.rs::AppSettingsData`: add `#[serde(default)] pub offline_mode: bool`
  - `startup.rs`: compute initial offline readiness on app start

## Critical Files Reference

- `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`: **Exact template** for trainer type catalog — data-driven TOML with embedded default + user override + `OnceLock`
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`: `hash_trainer_file()` (SHA-256, line ~215), `compute_correlation_status()`, `VersionCorrelationStatus::TrainerChanged` — reuse, don't duplicate
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Sequential migrations 0→12; add migration 13 as `migrate_12_to_13(conn)` following exact naming convention
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: `MetadataStore` facade with `with_conn()` / `with_conn_mut()` — ALL new store methods MUST use these helpers for fail-soft behavior
- `src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs`: Pattern for `offline_readiness_snapshots` CRUD (~100 LOC, upsert + load functions)
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `TrainerSection` (`kind: String`, `loading_mode: TrainerLoadingMode`); `effective_profile()` / `storage_profile()` / `portable_profile()` — all path checks MUST use `effective_profile()`
- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`: `evaluate_checks()` pattern returning `ReadinessCheckResult { checks: Vec<HealthIssue>, all_passed: bool, ... }` — offline pre-flight should return this same type
- `src/crosshook-native/crates/crosshook-core/src/profile/health.rs`: `check_profile_health()`, `HealthIssue`, `ProfileHealthReport` — extend here, not parallel implementation
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`: `TrainerLoadingMode` as exact enum pattern template (FromStr + as_str + serde rename_all + #[default] + #[serde(other)])
- `src/crosshook-native/crates/crosshook-core/src/community/taps.rs`: `CommunityTapStore`, `sync_tap()`, `git_command()` — must add git hardening env vars
- `src/crosshook-native/src-tauri/src/lib.rs`: Tauri state registration + `invoke_handler!` flat list — two edits required: (1) `pub mod offline;` in `commands/mod.rs`, (2) add each new command name to the `tauri::generate_handler![...]` list in `lib.rs`. No per-module shortcut exists. Also: startup scan spawned here at ~500ms delay (health) → mirror offline readiness scan at ~700ms
- `src/crosshook-native/src/hooks/useProfileHealth.ts`: Template for `useOfflineReadiness` hook (`useReducer` + `invoke()` + `listen()`)
- `src/crosshook-native/src/components/HealthBadge.tsx`: CSS pattern (`crosshook-status-chip crosshook-compatibility-badge--{rating}`) to reuse for `OfflineReadinessBadge`

## Patterns to Follow

- **Data-driven TOML Catalog**: `launch/catalog.rs` — `include_str!()` embedded default + `parse_catalog_toml` + `merge_catalogs` + `static GLOBAL_CATALOG: OnceLock<OptimizationCatalog> = OnceLock::new()`. No `lazy_static`. Initialization called at Tauri startup in `lib.rs`. Mirror exactly for the trainer type catalog; call `initialize_trainer_type_catalog()` from `lib.rs` startup
- **MetadataStore with_conn()**: All `MetadataStore` public methods call `self.with_conn(action_label, |conn| {...})`. Both `with_conn` and `with_conn_mut` open with `if !self.available { return Ok(T::default()); }` before lock acquisition — fail-soft is automatic. Never check `is_available()` explicitly inside store methods
- **Enum Serialization**: `#[serde(rename_all = "snake_case")]` + `pub fn as_str(self) -> &'static str` + `#[default]` variant + `#[serde(other)]` Unknown catch-all. Pattern: `TrainerLoadingMode` in `metadata/models.rs`
- **TOML Struct Fields**: `#[serde(default)]` on all fields; `#[serde(skip_serializing_if = "Option::is_none")]` for optional fields; `#[serde(rename = "type")]` for TOML key aliasing
- **SQLite Migration**: `fn migrate_12_to_13(conn: &Connection) -> Result<(), MetadataStoreError>` + `execute_batch` + `action: "run metadata migration 12 to 13"` imperative-tense string
- **IPC Command Convention**: `#[tauri::command]` returns `Result<T, String>`; errors via `.map_err(|e| e.to_string())`; CPU-heavy work via `tauri::async_runtime::spawn_blocking`
- **Sequential Migration Seeding**: Migration 13 can bootstrap `trainer_hash_cache` via `INSERT INTO ... SELECT` from `version_snapshots.trainer_file_hash` (already populated for launched profiles)
- **Pre-flight Readiness Pattern**: `evaluate_checks(already_resolved_values)` — pure function, no I/O inside; I/O happens at call site in Tauri command layer
- **React Hook**: `useReducer` with typed `HookStatus` (idle/loading/loaded/error) + `invoke()` + optional `listen()` for push events

## Cross-Cutting Concerns

- **Portable vs Machine-Local Boundary (CRITICAL)**: `trainer_type` string → TOML (portable, travels with exports). `trainer_activated`, `readiness_state`, `trainer_hash` → SQLite only (machine-local). `offline_activated` flag MUST NOT appear in `portable_profile()`. This is enforced via the `storage_profile()` / `portable_profile()` distinction in `profile/models.rs`
- **Always Use `effective_profile()`**: Every offline path check must call `profile.effective_profile()` to resolve `local_override` paths. Raw profile fields (especially `trainer.path`) will be empty on the originating machine unless resolved
- **MetadataStore Disabled Mode**: All offline queries must degrade to `readiness_state = "unconfigured"` / score contribution = 0 when `!store.is_available()`. The `with_conn()` pattern handles this automatically with `T: Default` bound
- **`spawn_blocking` for Hash Computation (MANDATORY)**: `hash_trainer_file()` uses `std::fs::read(path).ok()?` — full file into memory, no streaming path exists in the codebase. `spawn_blocking` is not optional. For the offline feature's `offline/hash.rs`, implement a streaming variant using `sha2::Update` + `BufReader` to handle large trainers safely
- **Security W-2**: SQLite DB created with 0644 by default — `fs::set_permissions(path, 0o600)` immediately after creation in `metadata/db.rs`. Already implemented per integration research; verify WAL/SHM sidecar files also get 0600
- **Security A-6**: `git_command()` in `community/taps.rs` must add three env vars: `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0`
- **Aurora Steam Deck Hard Constraint**: Aurora offline keys do NOT work on Steam Deck (HWID requires Windows). The `isSteamDeck` boolean from `useGamepadNav` drives platform-aware UI: Steam Deck → "ONLINE ONLY" badge only; desktop Linux → offline key setup modal
- **Scoring Is Informational Only — Never Blocks Launch**: `ValidationError::OfflineReadinessInsufficient` is Warning severity — launch proceeds regardless of score. The offline readiness score must NOT modify the existing `HealthStatus` enum values (Healthy/Stale/Broken) — it is additive metadata only, surfaced via `HealthIssue` entries. Missing trainer file IS blocking. Hash mismatch is advisory
- **Startup Scan Pattern**: `lib.rs` spawns the health scan at 500ms delay and emits `"profile-health-batch-complete"` event. The offline readiness scan MUST mirror this pattern: spawn at ~700ms (after health completes), emit `"offline-readiness-scan-complete"`. Frontend hooks listen on this event — do not poll. This is a named task in the implementation plan (not just a startup hook detail)
- **TypeScript Mirror Discipline**: Every Rust struct field crossing IPC must have exact mirror in TypeScript with same `snake_case` field names. Add to `src/types/health.ts` or new `src/types/trainer.ts`, then re-export from `src/types/index.ts`
- **New `offline/` Module Is Confirmed**: `feature-spec.md` (authoritative) supersedes `research-practices.md`. Create `crosshook-core/src/offline/` with `mod.rs`, `trainer_type.rs`, `readiness.rs`, `network.rs`, `hash.rs`. Core entry point: `pub fn check_offline_preflight(profile: &GameProfile, conn: &Connection) -> ReadinessCheckResult` — keeps scoring logic cohesive without bloating `profile/health.rs`. Precedent: `lib.rs` already calls `commands::health::build_enriched_health_summary` directly (line 126)

## Parallelization Opportunities

- **Phase 1A + 1B (fully parallel)**: Trainer type model + catalog implementation (`offline/trainer_type.rs`, assets/, TOML catalog) can run simultaneously with trainer hash caching + migration 13 (`offline/hash.rs`, `metadata/offline_store.rs`, `metadata/migrations.rs`). **1A is the critical path** — it must complete before 1C, 3B, and TypeScript type scaffolding
- **Phase 2A + 2B + 2C (parallel after Phase 1)**: Pre-launch hash verification, offline-aware validation errors, and launch history preservation are independent within Phase 2
- **Phase 3A + 3B (parallel)**: Community tap offline cache UI and Aurora/WeMod info modal are independent; both need Phase 1's TrainerType
- **Phase 4A + 4B (parallel)**: Status badges and graceful degradation UI are independent; both need Phases 1-3
- **TypeScript types start after 1A only**: Frontend type definitions (`OfflineCapability` union, `TrainerOfflineStatusReport`) need only the `OfflineCapability` variant names from 1A — not full Phase 1 convergence. This allows frontend scaffolding to begin while 1B (migration/hash store) is still in progress
- **Testing**: Unit tests for `migrate_12_to_13` and hash store round-trips can be written in-parallel with implementation using `db::open_in_memory()` fixtures

## Implementation Constraints

- **No New Rust Crate Dependencies**: `sha2`, `rusqlite`, `chrono`, `uuid`, `directories` are all already present. `keyring` crate (offline key storage) is a future enhancement only — Phase 1-4 do NOT require it
- **Migration v13 Ownership**: No other in-flight feature should claim v13. Must coordinate to avoid migration conflicts
- **Hash Table Decision**: `feature-spec.md` uses dedicated `trainer_hash_cache` table; `research-practices.md` recommends reusing `version_snapshots.trainer_file_hash`. **Feature-spec is authoritative** — use new table with stat-based fields (file_size, file_modified_at). Bootstrap from `version_snapshots` at migration time via `INSERT INTO ... SELECT`
- **`trainer.kind` Stays as Display String — TOML key is `type`**: The existing `kind: String` field uses `#[serde(rename = "type")]` — in TOML it appears as `[trainer] type = "fling"`. The Rust field name is `kind`; the TOML key is `type`. Do NOT replace it. The NEW field `trainer_type: Option<String>` holds the catalog `id` reference (e.g., `"standalone"`). Both coexist as separate TOML keys in the `[trainer]` section. Existing profiles that have `type = "fling"` are unaffected — `kind` continues to hold that string
- **Score Caps by Type**: `standalone`/`cheat_engine` (with CE present) → max 100; `aurora`/`wemod`/`unknown`/`custom` → cap 90; `plitch` → cap 80. Scoring weights: trainer_present=30, hash_valid=15, game_present=20, proton_available=15, prefix_exists=10, network_not_required=10
- **Never-Launched Profiles**: Profiles without a version snapshot have no cached hash. Pre-flight must prompt: "Launch once with internet to record baseline hash." Do not block, do warn
- **Stat-Based Cache False Hit**: ext4 mtime has 1-second resolution — a binary replaced within the same second won't be detected. Accepted limitation; consequence is a warning missed at one launch, caught at next sync
- **`AppSettingsData` Field Default**: `AppSettingsData` derives `#[serde(default)]` at the **struct level**. Adding `offline_mode: bool` requires zero extra annotation — `bool::default()` = `false` is applied automatically for existing `settings.toml` files that lack the key

## Key Recommendations

- **Read `feature-spec.md` first** — it supersedes all individual research files where they conflict. Especially: catalog architecture (data-driven TOML, not raw enum), table schemas for migration 13, API command surface
- **Start with Phase 1A (TrainerType + catalog) and 1B (hash cache + migration)** in parallel — these unblock all subsequent phases
- **Trainer type catalog as TOML asset**: Create `src/crosshook-native/src-tauri/assets/default_trainer_type_catalog.toml` and load via `include_str!()` same as `launch/catalog.rs`
- **Reuse `ReadinessCheckResult`** from `onboarding/readiness.rs` as the offline pre-flight return type — do not create a new type for the same shape
- **Inject offline readiness into existing `check_profile_health()` issues vec**: push `HealthIssue { field: "offline_readiness", severity: Warning }` entries from within the existing function — not a separate call. Run base health check first; append offline issues only for profiles that pass path checks. Health dashboard displays these with zero UI changes
- **Task breakdown suggestion**: 7 independent implementation tasks across 4 phases; Phase 1 has max parallelism (1A + 1B); backend Rust + frontend TypeScript can be split across agents in Phases 3-4 once the IPC contract is agreed
- **SQLite DB path is `~/.local/share/crosshook/metadata.db`** (verified): `MetadataStore::try_new()` uses `BaseDirs::data_local_dir()`. `paths.rs` in `src-tauri/src/` is for helper script resolution only — unrelated. The `~/.config/crosshook/metadata.db` reference in `research-security.md` is wrong. W-2 chmod 600 applies to `~/.local/share/crosshook/metadata.db` and its `-wal`/`-shm` sidecars
