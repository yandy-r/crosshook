# Profile writer technical architecture — YAN-811 / M1-06

## Executive Summary

See overview below.

### Overview and evidence

Move application orchestration into `crosshook-app::services::profiles::ProfileWriter`; keep serialization, validation, durable writes, diff algorithms, revision integrity and SQL in `crosshook-core`. This is a clean cutover of the named mutation paths, not a new profile format, revision schema, collection service, or launcher-export workflow. All signatures below are **proposed**, not existing APIs. Paths below are relative to `src/crosshook-native/` unless prefixed `docs/`.

The acceptance criteria in [issue.md](issue.md) are authoritative. Source was read in the assigned worktree; no tests/builds were run.

### Relevant current implementation

| Location                                                                               | Verified behavior to preserve                                                                                                                                                                                                                              |
| -------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/commands/profile/lifecycle.rs:19–402`                                   | List/load/summaries; full save, new-profile settings and default bundled preset, three art imports; delete cleanup; duplicate metadata ancestry; rename launcher cleanup, display-name and last-used-setting update; legacy import; shareable TOML export. |
| `commands/profile/shared.rs:12–162,223–280`                                            | Launcher path derivation; string event emitter; fail-soft observation/revision helpers; retention fallback; section saves preserve unknown on-disk keys.                                                                                                   |
| `commands/profile/shared.rs:164–221`                                                   | `apply_collection_defaults`: absent/empty ID is no-op, corrupt defaults fail loudly, other metadata errors fail open. **Excluded from this move.**                                                                                                         |
| `commands/profile/optimizations.rs:15–328`                                             | DTOs, optimization/preset updates, bundled/user origin metadata. Several saves currently emit no event.                                                                                                                                                    |
| `commands/profile/command_arguments.rs:12–68`, `favorites.rs:1–27`                     | Argument payload and core validation; favorite is SQLite-only and emits a reason string.                                                                                                                                                                   |
| `commands/profile/config_history.rs:17–376`                                            | History DTOs, capped list/diff, scoped rollback integrity verification, rollback lineage, known-good marking.                                                                                                                                              |
| `commands/protondb.rs:15–48,108–233`                                                   | Two accept branches; revalidate catalog env pairs and direct env suggestion at write time, then observe/capture.                                                                                                                                           |
| `commands/migration.rs:63–265`                                                         | Single replacement existence check; batch validates every replacement and loads every profile before any write; per-item failure isolation; current so-called invalidation actually recomputes health synchronously.                                       |
| `crates/crosshook-core/src/profile/toml_store/store.rs:88–125,129–314,347–491,534–605` | Storage-form TOML, atomic replace, companion-file policies, existing mutation validation/normalization, duplicate naming and rename side effects.                                                                                                          |
| `crates/crosshook-core/src/profile/migration/apply.rs:11–110`                          | Replacement must be a regular file; valid old path means `AlreadyValid`; direct atomic storage-form write deliberately does **not** update MangoHud companion.                                                                                             |
| `crates/crosshook-core/src/metadata/config_history_ops.rs:12–145`                      | Dedupe insertion and HMAC/SHA verification, profile-scoped revision reads and known-good updates.                                                                                                                                                          |

## Ownership and async execution

Existing ownership is unambiguous:

- Plain `App` owns runtime/event bus/jobs, not stores (`crates/crosshook-app/src/app.rs:14–20,105–123`). Do not add optional store accessors or initialize a second set of stores there.
- `BootstrappedApp` owns `Arc<BootstrapData>` and exposes `app()`, `stores()`, `services()`, `startup()`; it dereferences to `App` (`bootstrap/app_bootstrap.rs:17–85`).
- `Stores` exposes cloned `ProfileStore`, `MetadataStore`, `SettingsStore` handles; `Services::new(&Stores)` constructs `HealthService::new(profiles, metadata)` and `SettingsService::new(settings, active_profiles_dir)` (`bootstrap/stores.rs:27–97`). `ProfileStore` is only a path handle, **not** a shared transaction lock; its section methods explicitly document last-completed-write-wins.
- Services clone store handles before `services::run_blocking`; that helper is exactly `tokio::task::spawn_blocking(work).await?`, mapping join failures through `AppError` (`services/mod.rs:32–40`). Follow this pattern; do not call Tauri's blocking pool from commands, create a runtime, or use `App::spawn` merely to perform a store operation.

Add `Services::profiles: ProfileWriter`. Change bundle construction to `Services::new(stores: &Stores, events: EventBus)`, invoked as `Services::new(&stores, app.events().clone())` at `bootstrap/app_bootstrap.rs:125`. Constructor stores the three cloned handles plus an `EventBus`, **not** an `App`/`BootstrappedApp`; this avoids an owner/task lifetime cycle. Update any bundle-construction tests to supply a bus. Tauri registers `.manage(services.profiles.clone())` beside health/settings (`src-tauri/src/lib.rs:29–48`). Keep existing store registrations used by out-of-scope commands.

All operation methods are `async`. `new` remains a synchronous, I/O-free constructor like existing services; this is the sole explicit constructor exception to the issue's async-function rule. `commit` runs validation, store operations, serialization/hash, diff-independent metadata effects and publication in **one** blocking closure. Publishing immediately after successful blocking effects avoids a persisted write losing its notification merely because the awaiting command future was dropped. `run_blocking` is reused, not reimplemented. Do not split load and save into independently scheduled tasks. No new locking/concurrency policy is required by this issue; do not claim path-handle cloning serializes writes.

## Architecture Design

### Module layout (every Rust file below <500 lines)

```text
crates/crosshook-app/src/services/profiles/
  mod.rs                 # ProfileWriter fields, constructor, re-exports
  types.rs               # mutation/result types and existing non-history DTOs
  history_types.rs       # existing history/diff/rollback DTOs, unchanged serde
  writer.rs              # async commit + synchronous internal commit pipeline
  lifecycle.rs           # CRUD/import facades; rename/settings orchestration
  reads.rs               # list/load/summaries/favorites/export operations
  sections.rs            # optimizations/MangoHud/gamescope/arguments facades
  presets.rs             # bundled/manual preset fetch/materialization/origin
  history.rs             # history, diff, rollback preparation, known-good
  revisions.rs           # moved snapshot/hash/retention/insert helper
  launcher_cleanup.rs    # moved derivation + core delete_launcher_for_profile
  defaults_art.rs        # moved creation settings/default preset/art imports
  suggestions.rs         # moved ProtonDB accept validation/mutation
  migration.rs           # moved apply/preflight/result aggregation
  imports.rs             # minimal existing-Lutris post-write integration
  tests/
    mod.rs helpers.rs lifecycle.rs sections.rs history.rs events.rs
    migration.rs suggestions.rs degraded.rs golden.rs
```

Split `writer.rs` into `writer/{mod.rs,effects.rs}` if implementing the operation dispatch would exceed the limit; do not let a single 23-command match absorb all domain logic. Modules remain private; expose only the facade and wire DTOs. No generic repository framework or new store trait is needed.

## Exact proposed public Rust surface

Use owned arguments so they cross the `'static` blocking boundary naturally. Import `GameProfile`, `GamescopeConfig`, `MangoHudConfig`, `DuplicateProfileResult`, migration request/results, and ProtonDB request/results from core. Move the existing payload/response structs into app **without changing field names, optionality or serde casing**: `ProfileSummary` remains camelCase; `LaunchOptimizationsPayload`, `CommandArgumentsPayload`, bundled preset and history responses remain their current representation. `ConfigDiffMode` keeps snake_case and the current unknown/absent mode → unified behavior.

```rust
#[derive(Clone)]
pub struct ProfileWriter { /* profiles, metadata, settings, events */ }

impl ProfileWriter {
    pub fn new(
        profiles: ProfileStore,
        metadata: MetadataStore,
        settings: SettingsStore,
        events: EventBus,
    ) -> Self;

    pub async fn commit(
        &self, name: String, mutation: ProfileMutation, reason: ProfileChangeReason,
    ) -> Result<ProfileCommit, AppError>;

    pub async fn list(&self) -> Result<Vec<String>, AppError>;
    pub async fn load(&self, name: String) -> Result<GameProfile, AppError>;
    pub async fn list_summaries(&self) -> Result<Vec<ProfileSummary>, AppError>;
    pub async fn list_favorites(&self) -> Result<Vec<String>, AppError>;
    pub async fn export_toml(
        &self, name: String, data: GameProfile, include_hooks: Option<bool>,
    ) -> Result<String, AppError>;

    pub async fn save(&self, name: String, data: GameProfile) -> Result<(), AppError>;
    pub async fn delete(&self, name: String) -> Result<(), AppError>;
    pub async fn duplicate(&self, name: String) -> Result<DuplicateProfileResult, AppError>;
    pub async fn rename(&self, old_name: String, new_name: String) -> Result<bool, AppError>;
    pub async fn set_favorite(
        &self, name: String, favorite: bool,
    ) -> Result<(), AppError>;
    pub async fn import_legacy(&self, path: String) -> Result<GameProfile, AppError>;

    pub async fn save_launch_optimizations(
        &self, name: String, optimizations: LaunchOptimizationsPayload,
    ) -> Result<(), AppError>;
    pub async fn save_mangohud_config(
        &self, name: String, config: MangoHudConfig,
    ) -> Result<(), AppError>;
    pub async fn save_gamescope_config(
        &self, name: String, config: GamescopeConfig,
    ) -> Result<(), AppError>;
    pub async fn save_trainer_gamescope_config(
        &self, name: String, config: GamescopeConfig,
    ) -> Result<(), AppError>;
    pub async fn save_command_arguments(
        &self, name: String, command_arguments: CommandArgumentsPayload,
        resolved_launch_method: Option<String>,
    ) -> Result<(), AppError>;
    pub async fn list_bundled_optimization_presets(
        &self,
    ) -> Result<Vec<BundledOptimizationPresetDto>, AppError>;
    pub async fn apply_bundled_optimization_preset(
        &self, name: String, preset_id: String,
    ) -> Result<GameProfile, AppError>;
    pub async fn save_manual_optimization_preset(
        &self, name: String, preset_name: String, enabled_option_ids: Vec<String>,
    ) -> Result<GameProfile, AppError>;

    pub async fn config_history(
        &self, name: String, limit: Option<usize>,
    ) -> Result<Vec<ConfigRevisionSummary>, AppError>;
    pub async fn config_diff(
        &self, name: String, revision_id: i64,
        right_revision_id: Option<i64>, mode: Option<String>,
    ) -> Result<ConfigDiffResult, AppError>;
    pub async fn config_rollback(
        &self, name: String, revision_id: i64,
    ) -> Result<ConfigRollbackResult, AppError>;
    pub async fn mark_known_good(
        &self, name: String, revision_id: i64,
    ) -> Result<(), AppError>;

    pub async fn accept_protondb_suggestion(
        &self, request: AcceptSuggestionRequest,
    ) -> Result<AcceptSuggestionResult, AppError>;
    pub async fn apply_proton_migration(
        &self, request: ApplyMigrationRequest,
    ) -> Result<MigrationApplyResult, AppError>;
    pub async fn apply_batch_migration(
        &self, request: BatchMigrationRequest,
    ) -> Result<BatchMigrationResult, AppError>;

    // Existing Lutris helper consumer only: completes effects for writes the
    // established core importer already performed, without another save.
    pub async fn record_imports(
        &self, imported: Vec<ImportedProfileWrite>,
    ) -> Result<(), AppError>;
}
```

`record_imports` is not an alternative TOML writer: it enters the same internal **post-persistence effects** function used by commit, with `SyncSource::LutrisImport`, `ConfigRevisionSource::Import`, `Imported`, concrete successful names and health invalidation. It replaces `lutris.rs`'s imports of soon-deleted Tauri helpers, not the importer itself. A failed metadata effect is logged and does not reject successful imports. Community imports do not consume these shared helpers and are outside the enumerated cutover.

Public mutation/result data types (Serde is unnecessary for these application-internal types):

```rust
pub enum ProfileMutation {
    Save(GameProfile),
    Delete,
    Duplicate,
    Rename { new_name: String },
    SetFavorite { favorite: bool },
    LaunchOptimizations(LaunchOptimizationsPayload),
    MangoHud(MangoHudConfig),
    Gamescope(GamescopeConfig),
    TrainerGamescope(GamescopeConfig),
    CommandArguments {
        payload: CommandArgumentsPayload,
        resolved_launch_method: Option<String>,
    },
    BundledPreset { preset_id: String },
    ManualPreset { preset_name: String, enabled_option_ids: Vec<String> },
    Rollback { revision_id: i64 },
    ImportLegacy { path: String },
    ProtonDbSuggestion(AcceptSuggestionRequest),
    ProtonMigration(ApplyMigrationRequest),
}

pub enum ProfileCommit {
    Profile { name: String, profile: GameProfile, new_revision_id: Option<i64> },
    Deleted,
    Renamed { had_launcher: bool },
    FavoriteUpdated,
    Suggestion(AcceptSuggestionResult),
    Migration(MigrationApplyResult),
}

pub struct ImportedProfileWrite {
    pub name: String,
    pub profile: GameProfile,
    pub path: std::path::PathBuf,
}
```

Facade methods choose the matching reason, invoke commit once, and project its typed result into the unchanged command response. `commit` validates mutation/reason consistency (including rename `from == name` and request profile names matching `name`) before any side effect; do not let a caller label a launch mutation `FavoriteToggled`. A private operation descriptor carries persistence policy, sync/revision source, rollback ancestry and affected names; callers cannot override them.

### Error contract

Reuse `crosshook_app::{AppError, AppErrorKind}`; do not add `ProfileServiceError`, return raw SQL, or carry a Tauri `String` through the service. Existing conversions cover `ProfileStoreError`, `SettingsStoreError`, `MetadataStoreError`, launch validation, I/O and join errors (`error.rs:7–226`). Map remaining TOML/JSON conversion failures to fixed/sanitized `AppError::new` messages inside the crate. Use `Validation { field }` for invalid input, `NotFound` for scoped missing revisions/presets, `Conflict` for rename collisions, `Unavailable` for metadata-dependent operations. Tauri uses `.map_err(|error| error.to_string())`, as existing migrated service commands do.

`config_history`, `config_diff`, rollback, known-good and applying a bundled preset still require available metadata. Normal TOML saves do not: disabled/read-only metadata skips optional observation/revision/health operations with a warning. Favorite remains metadata-only and errors when unavailable, not a fake successful favorite. `MetadataStatusBanner`/`useMetadataStoreStatus` already surface disabled/read-only metadata (`src/components/layout/MetadataStatusBanner.tsx`, `src/hooks/useMetadataStoreStatus.ts`); preserve/test that UI rather than changing all existing command return shapes just to add a warning field.

## Commit pipeline and exceptional operations

1. **Validate** names through core validation and operation inputs through existing validators. Full-save retains runtime Steam App ID validation, unknown-input-key removal, and on-disk-extra preservation. Pre-save validation is not an invented full health gate.
2. **Load** the latest persisted effective profile where the operation needs one. Full save tolerates absence only for creation; duplicate/section/rollback/rename retain their current missing-profile semantics. Save's `is_new` derives from disk, never frontend state.
3. **Mutate** with core domain helpers. Only full **new** save uses settings creation defaults/default bundled preset and art auto-import behavior from lifecycle. Preserve explicit draft preset selection. Failed art imports retain the original path and warn. Rollback uses the verified snapshot as authoritative, not full-save defaults/extras filtering. Duplicate keeps source extras; UI responses are sanitized only after persistence/effects.
4. **Persist** exactly once through the correct core primitive: normal profile write, delete, rename (+ existing display-name update), duplicate, or metadata-only favorite. Avoid invoking a core load-mutate-save method and then saving again. See core extraction below.
5. **Observe** successful writes with their original `SyncSource`; rename calls `observe_profile_rename` **before** observing the final renamed profile so the stable ID/history/favorite/revision ownership survives. Duplicate records original `source_profile_id` with `AppDuplicate`. Delete soft-deletes metadata and does not insert an empty TOML revision. Resolve/delete's profile ID before soft deletion, since lookup excludes deleted rows (`metadata/profile_sync.rs:71–85,168–181`).
6. **Revision** via moved snapshot capture: `toml::to_string_pretty(profile)` → `sha256_hex` → stable-ID lookup → `insert_config_revision` with existing retention fallback. **Do not silently change snapshot serialization to `storage_profile()`**: current snapshot hashing serializes the supplied `GameProfile`, while disk save serializes storage form. Preserve both contracts and test them separately. Rollback passes `Some(restored_revision_id)` with `RollbackApply`; integrity verification must precede any write. Normal dedupe yields `new_revision_id: None`, not failure.
7. **Health invalidation** deletes the affected cached health row, fail-soft, for health-relevant changes. No validation scan runs inside commit. Add a parameterized core helper, not ad-hoc SQL in app. Favorite skips observation, TOML save, revision and health invalidation entirely. Delete invalidates the captured pre-delete ID. Rename invalidates its preserved ID; duplicate invalidates only the new ID.
8. **Publish** `AppEvent::ProfilesChanged` once after required persistence succeeds, even if optional metadata effects fail. No event on validation/save failure, migration `AlreadyValid`/`Failed`, or true no-op rename. `mark_known_good` modifies history metadata only; no profile-content event or health scan is warranted. Batch migrations publish each successful item with a concrete name; no duplicate batch-summary event is needed.

Reason/revision mapping:

| Mutation                   | Reason                        | Revision source                                             |
| -------------------------- | ----------------------------- | ----------------------------------------------------------- |
| Full/section save          | `Saved`                       | `ManualSave`                                                |
| Optimization save/switch   | `Saved`                       | `LaunchOptimizationSave`                                    |
| Argument save              | `Saved`                       | `LaunchCommandArgumentsSave`                                |
| Bundled/manual preset      | `PresetApplied`               | `PresetApply`                                               |
| Rollback                   | `RolledBack`                  | `RollbackApply` + lineage                                   |
| Legacy import              | `Imported`                    | `Import`                                                    |
| ProtonDB accept            | `Saved`                       | `ProtonDbSuggestionApply`                                   |
| Proton migration           | `Migrated`                    | `Migration`                                                 |
| Duplicate                  | `Duplicated`                  | `ManualSave` (no dedicated existing enum; no schema change) |
| Rename/display-name update | `Renamed { from }`            | `ManualSave`                                                |
| Delete/favorite            | `Deleted` / `FavoriteToggled` | None                                                        |

Duplicate/rename currently do not capture revisions or emit events; adding those is the issue's unified-flow behavior, not an existing fact. Document this explicitly in implementation/release notes.

### Reuse core helpers; do not duplicate domain algorithms

`ProfileStore::save_launch_optimizations`, `save_command_arguments`, `materialize_launch_optimization_preset`, `save_manual_launch_optimization_preset` currently both mutate and persist (`toml_store/store.rs:129–314`), so they cannot be called from a pipeline that also owns saving. Extract their **existing bodies**, including catalog-ID validation, trimming, active-preset synchronization, extra-table handling, method resolution, incompatible/duplicate argument checks and 64-token/512-byte limits, into profile mutation helpers. Both existing store methods and app commit call those helpers; existing core callers/tests continue working, with only one implementation of each rule.

Proposed synchronous core signatures (the async requirement applies to application operations, not core pure/store primitives):

```rust
impl GameProfile {
    pub fn apply_launch_optimizations(
        &mut self, enabled_option_ids: Vec<String>, switch_active_preset: Option<String>,
    ) -> Result<(), ProfileStoreError>;
    pub fn apply_command_arguments(
        &mut self, enabled_argument_ids: Vec<String>, custom_args: Vec<String>,
        resolved_launch_method: Option<&str>,
    ) -> Result<(), ProfileStoreError>;
    pub fn materialize_launch_optimization_preset(
        &mut self, preset_key: &str, enabled_option_ids: Vec<String>, set_as_active: bool,
    ) -> Result<(), ProfileStoreError>;
    pub fn apply_manual_optimization_preset(
        &mut self, preset_display_name: &str, enabled_option_ids: Vec<String>,
    ) -> Result<(), ProfileStoreError>;
}
impl ProfileStore {
    pub fn unique_copy_name(&self, source_name: &str) -> Result<String, ProfileStoreError>;
    pub fn save_without_companion(
        &self, name: &str, profile: &GameProfile,
    ) -> Result<(), ProfileStoreError>;
}
impl MetadataStore {
    pub fn invalidate_health_snapshot(
        &self, profile_id: &str,
    ) -> Result<(), MetadataStoreError>;
}
```

- `unique_copy_name` uses the existing generator and list logic; `duplicate` uses it too. No second `(Copy N)` implementation in app. Existing `ProfileStore::rename/delete` preserve companion rename/removal behavior; app keeps launcher cleanup native skip, generated-file safeguards and best-effort failure semantics.
- `save_without_companion` factors the exact storage-profile + pretty-TOML + atomic-write portion from `save`; `save` calls it then writes MangoHud config. Migration calls only the no-companion variant. This preserves the deliberate exception in `migration/apply.rs` instead of accidentally rewriting companion files.
- Extract migration's current load-independent preparation into `profile::migration::prepare_profile_migration(profile: &mut GameProfile, request: &ApplyMigrationRequest) -> MigrationApplyResult`. It performs the current regular-file check, old-path-valid short-circuit and field mutation without saving. Both `apply_single_migration` and the app use it; core `apply_single_migration` uses `save_without_companion`. Failed/AlreadyValid outcomes are never committed. Batch preflight remains all-before-any-write; later failures remain isolated per item. Keep existing result casing and sanitized returned paths.
- Legacy parsing already exists as public `crosshook_core::profile::load(parent, stem)` and `GameProfile::from(LegacyProfileData)`; reuse them, preserving stem validation and overwrite behavior. Do not call `ProfileStore::import_legacy` then save again.
- `compute_unified_diff`, `cap_diff_output_bytes`, `compute_semantic_diff` already exist. Preserve 2,000 lines/side, 512 KiB UTF-8-safe unified cap, 500 semantic changes, semantic parse fallback (`config_diff.rs:6–93`, `config_semantic_diff.rs:7–84`). Do not replace algorithms with a new diff library.
- `MetadataStore::verify_config_revision` enforces SHA-256 and HMAC trust/epoch rules; app must not reproduce or weaken it. Revision scoping is always `(profile_id, revision_id)`, including right-hand revision and known-good. No SQL or HMAC implementation moves into app.

### Collection boundary

Keep `apply_collection_defaults` and its tests in Tauri for M1-35. `profile_load` delegates raw load to `ProfileWriter::load`, applies the existing excluded helper in a blocking adapter, then clears extras at the IPC boundary. For `profile_list_summaries` with no collection context, delegate directly to `list_summaries`. For a nonempty collection context retain the existing collection-composition branch in a blocking adapter; reuse a pure `impl From<(String, GameProfile)> for ProfileSummary` mapping owned by app, so summary field mapping is not duplicated. `ProfileWriter::load` must therefore retain disk extras internally; command responses clear extras, and summaries never expose them. The retained collection branch is the explicit scope exception, not permission to move collection persistence or redesign precedence.

## Event bus, Tauri and frontend contract

Current bus has only `Resync` and `StartupJobFinished` (`events.rs:36–51`). Add/re-export the following reason enum from app and add the requested event variant. Derives preserve `AppEvent: Clone + PartialEq + Eq`.

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileChangeReason {
    Saved, Deleted, Renamed { from: String }, Duplicated, FavoriteToggled,
    RolledBack, PresetApplied, Imported, Migrated,
}
// Within AppEvent:
ProfilesChanged { reason: ProfileChangeReason, names: Vec<String> }

// Tauri bridge wire DTO, not an AppEvent serde envelope:
#[derive(Debug, Clone, serde::Serialize)]
struct ProfilesChangedPayload {
    reason: ProfileChangeReason,
    names: Vec<String>,
}
```

Exact JSON examples (the issue mandates an object, but leaves this nested tagged representation to design):

```json
{"reason":{"kind":"favorite_toggled"},"names":["Alpha"]}
{"reason":{"kind":"renamed","from":"Alpha"},"names":["Beta"]}
{"reason":{"kind":"saved"},"names":["Alpha"]}
```

Normal events contain normalized concrete profile names. Duplicate names contain only the new copy; delete names contain the removed name; rename names contain the new name and `reason.from` identifies the old key. `names: []` is reserved for conservative resync, never a normal favorite update.

The existing Tauri bridge is `startup_bridge.rs:123–146`: a single event receiver loop, no retained `App`, emits startup terminal/legacy success events, replays startup snapshots on `Resync`. Extend this loop with `ProfilesChanged` → `handle.emit("profiles-changed", ProfilesChangedPayload { reason, names })`; warn on emission failure, do not fail a persisted command. On bus lag preserve startup replay **and** emit a conservative `{reason:{kind:"saved"},names:[]}` profile resync so lost changes do not leave views stale. Keep this existing filename/function because the startup-path script explicitly references it; no second receiver or direct command emitter. Add serialization/resync tests in `startup_bridge/tests.rs`.

Add `src/types/profile-events.ts` and export from `types/index.ts`:

```ts
export type ProfileChangeReason =
  | { kind: 'saved' }
  | { kind: 'deleted' }
  | { kind: 'renamed'; from: string }
  | { kind: 'duplicated' }
  | { kind: 'favorite_toggled' }
  | { kind: 'rolled_back' }
  | { kind: 'preset_applied' }
  | { kind: 'imported' }
  | { kind: 'migrated' };
export interface ProfilesChangedPayload {
  reason: ProfileChangeReason;
  names: string[];
}
```

Add a small shared event-policy/parser utility under `src/lib/profileEvents.ts`: receive `unknown`, validate object/tag/names/from at the adapter boundary and return a typed payload; malformed input conservatively triggers resync, not an exception or health skip. No backwards-compatible string/action format. A shared `profileChangeAffectsHealth(reason)` returns false only for `favorite_toggled`; all other reasons remain conservative. For valid named content changes use existing single-profile health calls rather than a whole batch where practical; empty names and membership changes can batch refresh. The acceptance-critical requirement is **zero batch validation on favorite**, not introducing an unrelated incremental-health system.

Five listeners and their policies:

| Listener               | Current location                            | New behavior                                                                                                                                                                             |
| ---------------------- | ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `useProfileHealth`     | `src/hooks/useProfileHealth.ts:239–266`     | Guard favorite before any validation IPC; retain state; content reasons validate affected names or existing batch path; conservative resync batches. Launch/version listeners unchanged. |
| `useProfile`           | `src/hooks/useProfile.ts:232–254`           | Favorite refreshes favorites only, not profile-name list. Other changes retain list/favorite refresh. Do not reload editor drafts merely because any save event arrived.                 |
| `useLibrarySummaries`  | `src/hooks/useLibrarySummaries.ts:63–77`    | Skip summary refetch for favorite; existing favorite-prop effect updates cards locally (`:80–83`).                                                                                       |
| `useProfileSummaries`  | `src/hooks/useProfileSummaries.ts:38–52`    | Skip favorite; refresh summary data for content/membership changes.                                                                                                                      |
| `useCommunityProfiles` | `src/hooks/useCommunityProfiles.ts:432–454` | Favorite cannot change imported membership: skip; retain refresh on other reasons conservatively.                                                                                        |

Preserve rename's **bool** IPC result. `useProfileCrud.ts:376–406` returns `{ok,hadLauncher}`, and `useProfileNotifications.ts:129–174` sets the existing launcher re-export banner after rename/undo. Do not replace this with automatic backend export or remove the response because the event now has names. Add regression tests for the banner path.

Mocks must stop emitting the current incompatible `{name,action}` shape. Update `src/lib/mocks/handlers/profile-mutations.ts`, `profile-history.ts`, `profile-presets.ts` to the above reasons/names, including dev add/remove helpers (`saved`/`deleted`). Update `protondb.ts` acceptance and `proton.ts` applied migration paths to publish matching events after updating mock profile state; `AlreadyValid`/failed paths do not emit. Update Lutris import mock if its successful imports need the new shared import event. Keep `callCommand`/`subscribeEvent`; do not add raw invoke/listen. A small typed mock publisher can live in existing `profile-utils.ts`; it must use the same `ProfilesChangedPayload` type, not invent another schema.

## Tauri adapter signatures and command inventory

All 25 actual profile commands become async (the issue text says 23; the present exports enumerate **25**). Retain snake*case names and exact JS argument names/response shapes. State parameters become `State<'*, ProfileWriter>`named`service`; no profile command needs `AppHandle` solely for emission. For each method above, the adapter signature is exactly:

```rust
#[tauri::command]
pub async fn profile_save(
    name: String, data: GameProfile, service: State<'_, ProfileWriter>,
) -> Result<(), String>;
```

The full mapping, with all non-state arguments and successful result types, is:

| Command                                     | Arguments before `service`                                                                         | `Ok` type / method                                         |
| ------------------------------------------- | -------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| `profile_list`                              | none                                                                                               | `Vec<String>` / `list`                                     |
| `profile_load`                              | `name: String, collection_id: Option<String>`                                                      | `GameProfile` / `load` + excluded composition              |
| `profile_list_summaries`                    | `collection_id: Option<String>`                                                                    | `Vec<ProfileSummary>` / `list_summaries` + excluded branch |
| `profile_save`                              | `name: String, data: GameProfile`                                                                  | `()` / `save`                                              |
| `profile_delete`                            | `name: String`                                                                                     | `()` / `delete`                                            |
| `profile_duplicate`                         | `name: String`                                                                                     | `DuplicateProfileResult` / `duplicate`                     |
| `profile_rename`                            | `old_name: String, new_name: String`                                                               | `bool` / `rename`                                          |
| `profile_import_legacy`                     | `path: String`                                                                                     | `GameProfile` / `import_legacy`                            |
| `profile_export_toml`                       | `name: String, data: GameProfile, include_hooks: Option<bool>`                                     | `String` / `export_toml`                                   |
| `profile_set_favorite`                      | `name: String, favorite: bool`                                                                     | `()` / `set_favorite`                                      |
| `profile_list_favorites`                    | none                                                                                               | `Vec<String>` / `list_favorites`                           |
| `profile_save_launch_optimizations`         | `name: String, optimizations: LaunchOptimizationsPayload`                                          | `()` / same suffix method                                  |
| `profile_save_mangohud_config`              | `name: String, config: MangoHudConfig`                                                             | `()` / same suffix method                                  |
| `profile_save_gamescope_config`             | `name: String, config: GamescopeConfig`                                                            | `()` / same suffix method                                  |
| `profile_save_trainer_gamescope_config`     | `name: String, config: GamescopeConfig`                                                            | `()` / same suffix method                                  |
| `profile_save_command_arguments`            | `name: String, command_arguments: CommandArgumentsPayload, resolved_launch_method: Option<String>` | `()` / `save_command_arguments`                            |
| `profile_list_bundled_optimization_presets` | none                                                                                               | `Vec<BundledOptimizationPresetDto>` / same suffix method   |
| `profile_apply_bundled_optimization_preset` | `name: String, preset_id: String`                                                                  | `GameProfile` / same suffix method                         |
| `profile_save_manual_optimization_preset`   | `name: String, preset_name: String, enabled_option_ids: Vec<String>`                               | `GameProfile` / same suffix method                         |
| `profile_config_history`                    | `name: String, limit: Option<usize>`                                                               | `Vec<ConfigRevisionSummary>` / `config_history`            |
| `profile_config_diff`                       | `name: String, revision_id: i64, right_revision_id: Option<i64>, mode: Option<String>`             | `ConfigDiffResult` / `config_diff`                         |
| `profile_config_rollback`                   | `name: String, revision_id: i64`                                                                   | `ConfigRollbackResult` / `config_rollback`                 |
| `profile_mark_known_good`                   | `name: String, revision_id: i64`                                                                   | `()` / `mark_known_good`                                   |

**Count correction:** the table is 23 commands, matching the issue; count the exported functions, not their duplicated Tauri macro re-exports. Additional named cutover commands are `protondb_accept_suggestion(request: AcceptSuggestionRequest, service: State<'_, ProfileWriter>) -> Result<AcceptSuggestionResult,String>`, `apply_proton_migration(request: ApplyMigrationRequest, service: State<'_, ProfileWriter>) -> Result<MigrationApplyResult,String>` and `apply_batch_migration(request: BatchMigrationRequest, service: State<'_, ProfileWriter>) -> Result<BatchMigrationResult,String>`, all `pub async fn`. `profile_load`/summary collection branch additionally retain `metadata_store: State<'_, MetadataStore>` for the excluded helper. `lutris_import_profiles` retains its established input/importer stores and adds service state for `record_imports`; making that adapter async is required to await it.

## File-by-file implementation change list

### Application/core

- **Create** the `services/profiles/` modules and tests listed above; move helper logic rather than retaining Tauri copies. DTOs get docs because `crosshook-app/src/lib.rs` denies missing docs.
- **Modify** `crates/crosshook-app/src/services/mod.rs`: declare/re-export profiles API and moved wire DTOs.
- **Modify** `crates/crosshook-app/src/events.rs`, `lib.rs`, `events/tests.rs`: reason type, event variant/re-export, delivery/lag/clone/equality tests.
- **Modify** `crates/crosshook-app/src/bootstrap/stores.rs`, `app_bootstrap.rs`, and constructor callers/tests in `bootstrap/stores/tests.rs` as applicable: inject event bus and register writer in service bundle. `App` itself needs no ownership changes.
- **Modify** `crates/crosshook-app/Cargo.toml`: promote/add `toml = "1.1.0"` and `serde_json = "1"` as normal dependencies; they currently exist only in dev-dependencies. Reuse core `sha256_hex`; no new hashing/diff crate. Update workspace lockfile only as dependency tooling requires.
- **Create** `crates/crosshook-core/src/profile/mutations/{mod.rs,optimizations.rs,command_arguments.rs}`: extracted pure mutation rules; **modify** `profile/mod.rs` to wire them and `toml_store/store.rs` to delegate old methods and share no-companion write/unique-copy-name primitives.
- **Modify** `crates/crosshook-core/src/profile/migration/{apply.rs,mod.rs}`: extract preparation and delegate durable write; add preparation/no-companion regression tests in existing `migration/tests.rs`.
- **Modify** `crates/crosshook-core/src/metadata/{health_ops.rs,health_store.rs}`: parameterized delete-by-profile-ID invalidation. Add tests alongside existing health-store tests or a focused `health_store/tests.rs`; no schema/version changes.
- **Modify** existing core profile store tests under `profile/toml_store/tests/` only as necessary to pin extraction parity; retain domain tests there, not duplicate them all in app.

### Shell

- **Modify** `src-tauri/src/lib.rs`: manage writer; command names/registration unchanged.
- **Modify** `src-tauri/src/startup_bridge.rs` and `startup_bridge/tests.rs`: object emission and lag recovery while preserving startup replay.
- **Modify** all `commands/profile/{lifecycle,optimizations,config_history,command_arguments,favorites}.rs`: async delegates with app-owned DTO imports; remove orchestration/helper bodies.
- **Modify** `commands/profile/shared.rs`: retain only excluded collection composition (and only necessary adapter glue); delete moved cleanup, revision, section, and string-emitter implementations.
- **Modify** `commands/profile/mod.rs`: remove helper re-exports (`capture_config_revision`, retention resolver, emitter); preserve Tauri macro re-exports and command exports.
- **Modify** `commands/protondb.rs`: accept command delegates; remove `observe_suggestion_apply`; lookup/suggestion-fetch/dismiss paths are otherwise out of scope.
- **Modify** `commands/migration.rs`: apply commands delegate; scan-only command stays unchanged.
- **Modify** `commands/lutris.rs`: remove dependency on deleted profile helpers; pass successful imported entries to writer post-write effects. No unrelated Lutris/import-preview redesign.
- **Modify** `commands/profile/tests/{mod.rs,lifecycle.rs,optimizations.rs,helpers.rs}` per test migration below.

### Frontend/mocks/docs

- **Create** `src/types/profile-events.ts`, `src/lib/profileEvents.ts` and focused parser/policy tests `src/lib/__tests__/profileEvents.test.ts`.
- **Modify** `src/types/index.ts` and the five hook files listed in the listener table.
- **Modify** `src/lib/mocks/handlers/{profile-mutations,profile-history,profile-presets,profile-utils,protondb,proton,lutris}.ts` for matching publisher semantics; do not alter production mock isolation.
- **Modify** `src/hooks/__tests__/useProfileSummaries.test.ts` and `src/components/pages/__tests__/LibraryPage.test.tsx`: replace string event fixtures (`useProfileSummaries:51–55`, `LibraryPage:475`) with objects; add no-refetch-on-favorite expectations.
- **Create** `src/hooks/__tests__/useProfileHealth.events.test.ts` (reuse startup test fixture/command counter pattern) and `src/hooks/profile/__tests__/useProfileNotifications.test.tsx` for rename/re-export behavior.
- **Modify** `src/lib/mocks/README.md` to document event shape and parity; `crates/crosshook-app/README.md` for service ownership/API; the final feature plan/implementation notes under `docs/plans/profile-writer/` for compatibility/evidence. Do not hand-edit a generated changelog just for this research.

## Test migration and acceptance coverage

- `src-tauri/src/commands/profile/tests/lifecycle.rs:34–79` has two cleanup tests, not command-level lifecycle coverage. Move these to app `services/profiles/tests/lifecycle.rs`, updating private-helper imports; expand to real writer delete and rename paths with generated/watermarked launcher files, `had_launcher`, native skip, last-used setting, renamed display name, companion handling and no duplicate event.
- `tests/optimizations.rs:10–69` has two optimization helper tests. Move to async writer section tests using tempdir stores and enabled/disabled metadata. Assert non-launch sections remain equal and missing profile yields `AppErrorKind::NotFound`.
- **Leave** `apply_collection_defaults_tests` in `tests/optimizations.rs:73–222` in Tauri; optionally rename that remaining file to `collections.rs` and update `tests/mod.rs` for clarity, but do not move the helper or its tests into app in this issue. Move/copy the small steam fixture into app test helpers only if used by migrated tests; delete unused original helper afterward.
- Existing `commands/profile/tests/mod.rs` currently includes only helpers/lifecycle/optimizations. There is no profile command-name contract test there today. Add a focused Tauri IPC contract test pinning all 23 async exported names and JSON request/result serialization. Follow `commands/health/tests.rs:4–10` and settings tests: async functions cannot be cast to the previous synchronous fn-pointer shape. Keep macro-name re-exports tested by generated-handler compilation; do not migrate IPC tests out of Tauri.
- Keep frontend `src/hooks/profile/__tests__/useProfileLaunchAutosaveIpcContract.test.ts` in place: argument keys/command names must not change. Add event serialization contract tests in the bridge/app tests for all reasons, especially tagged rename; fixtures must agree with TS/mocks.
- Add **pre-refactor golden fixtures** before changing writers: save, duplicate, rename (including display-name update), rollback; deterministic paths, unknown keys/local overrides/presets/hooks and companion cases. Store under app `tests/fixtures/profile_writer/`, compare literal file bytes, never generate expected bytes with the new writer. Existing core serializer fixtures may be reused if they cover the same cases; that coverage is not assumed.
- History tests in app exercise the moved orchestration: observe-before-capture stable ID, repeated same hash dedupe, retention fallback/clamp, rename continuity, rollback lineage, revision ownership and integrity failure before disk change, semantic parse fallback, 2,000-line and 512-KiB caps including UTF-8. Existing core `tests/config_history_integration.rs` and `metadata/config_history_store/tests/{insert_and_list,hmac,validation,pruning}.rs` remain responsible for SQL/HMAC rules.
- Favorite acceptance needs two different observations: app event test verifies only `FavoriteToggled`, concrete names, no changed TOML/revision/health snapshot; a test call counter around health-batch invocation verifies no backend batch work is requested. Vitest mounts the real `useProfileHealth` event listener with startup complete, emits the favorite object, and asserts `batch_validate_profiles` call count does not increase (pattern already in `useProfileHealth.startup.test.ts:54–61`). Also emit `Saved` and assert the expected health path does run so the negative test cannot pass with a dead listener.
- Migration tests preserve all-before-any-write batch preflight, regular-file validation, old path `AlreadyValid`, per-item isolation, no MangoHud rewrite, sanitized output and events only for applied items. ProtonDB tests cover unsafe direct env and unsafe catalog pair rejection at commit time without disk/event side effects.
- Enabled and `MetadataStore::disabled()` app tests prove normal saves complete with unchanged bytes and events while revisions/health writes skip and warnings/degraded UI remain visible. Metadata-required methods fail explicitly.
- Main implementation verification (not performed here): issue-prescribed `cargo test -p crosshook-app`, `npm test`, `npm run test:smoke`; substantive core extraction also requires `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`. Native Tauri smoke remains necessary before merge; browser mocks alone do not validate real store/bridge behavior.

## Data Models

No schema or TOML format change. Event types are runtime-only; see below.

### Storage, compatibility and remaining implementation cautions

- **TOML preferences/profiles:** same profile serializer/storage form; settings only retains existing new-profile defaults and rename's `last_used_profile` update. Use `SettingsStore::update` for that focused last-used change rather than load-and-whole-save overwriting concurrent settings changes.
- **SQLite metadata:** existing profile identity, favorite, preset-origin, revision/lineage and health rows; add a deletion operation, not columns/tables. No migration or schema bump. No binary art data enters metadata payload JSON; existing filesystem art import remains unchanged.
- **Runtime only:** event reasons/names and operation results. No retained event history or persisted event queue. Offline expectations unchanged; no network is introduced in commit.
- True health invalidation removes stale snapshots rather than replacing them synchronously. This deliberately ends migration's misleading recompute-as-invalidate behavior and fulfills the issue's pipeline; test next health read/revalidation behavior and preserve existing trend/history semantics elsewhere.
- Full save currently clears frontend-supplied extras and preserves disk extras, while rollback restores the signed snapshot wholesale. A single unconditional `clear_extra/preserve_extra` policy for every mutation would silently break rollback and golden compatibility.
- Rename is an existing multi-step operation: filesystem rename can succeed before a best-effort display-name/settings/metadata update fails. Preserve warning/fail-soft semantics; do not claim cross-filesystem/SQLite atomic transactions or add retries as part of this refactor.
- `MetadataStatusBanner` is dismissible. Its existing visibility contract satisfies disabled-store surfacing; if product acceptance demands a fresh per-save warning despite dismissal, that is a distinct unresolved UI requirement, not grounds to invent a new mutation response envelope here.
