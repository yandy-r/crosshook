# YAN-811 B0 frozen contracts

Output of Tasks 0.1 and 0.2. B1 slices code against these contracts and **must not edit the shared files listed under "Frozen (B0-owned)"**. If a contract is wrong, message the integration owner. Do not patch it from inside a slice. The paths below are relative to `src/crosshook-native/`.

## Deviation from the plan (approved by Main)

- **D5 landed in B0, not in Task 1.1.** `MetadataStore::invalidate_health_snapshot` (plus `health_store::delete_health_snapshot` and three inline tests in `health_ops.rs`) is implemented and already called by the commit pipeline. **Task 1.1 drops the D5 part** and must not touch `metadata/health_ops.rs` or `metadata/health_store.rs`.
- **The commit API takes a trait, not an enum.** `ProfileWriter::commit` is generic over a crate-private `ProfileMutation` trait. Each slice implements that trait next to its facade, so no slice edits a shared central `ProfileMutation` enum or dispatch `match`. The plan's prose signature `commit(name, mutation, reason)` becomes `commit(mutation, reason)`, because the mutation carries its own name(s). D2 is preserved: `commit` is `pub(crate)`, only facades call it, and `accepts(&reason)` rejects a mismatch before any side effect.
- **`ProfileCommit` is generic.** It is `ProfileCommit<T> { output, new_revision_id, warnings }`. The plan's enum variants (`Profile` / `Deleted` / `Renamed{had_launcher}` / …) become the facade's own `Output` type.

## Task 0.1: goldens (D15)

Directory: `crates/crosshook-app/tests/fixtures/profile_writer/`

| File | Produced by (pre-refactor code, current `main` 994f40e) | SHA-256 |
| --- | --- | --- |
| `seed.toml` | **Input**, copied to `<profiles>/Alpha.toml` before every scenario. Hand-written: unknown keys at the top level, in `game`, in the preset, in `local_override` and in a hook; a preset; a hook; legacy DLL arrays; a `custom_env_vars` BTreeMap; machine paths that `storage_profile()` splits into `local_override` | — |
| `save.toml` | `profile_save` update branch: load → `clear_extra` (IPC) → `game.name = "Alpha Updated"` → `validate_steam_app_id` → `clear_extra` → `preserve_extra_from(load)` → `store.save("Alpha")` | `22c0745dfe0305fe3fcd3dcb0dc1f09e43bb860c6d571d2b009f59e7dd63cb82` |
| `rename.toml` | `profile_rename("Alpha","Alpha Renamed")`: `store.rename` → load new → `steam.launcher.display_name = "Alpha Renamed"` → `store.save` | `1f17f17e1a41ceacd2010a1c8a7feccafa81165f138ca916f0d3bbcff886d487` |
| `duplicate.toml` | `profile_duplicate("Alpha")`: `store.duplicate` → `Alpha (Copy).toml` | `5da9175a504e2127b3f40c52ff9f998008f6bfd7d57361981034a97b54979d3e` |
| `rollback.toml` | Save as in `save.toml` → snapshot `toml::to_string_pretty(&data)` → save `game.name = "Alpha Broken"` → `toml::from_str(snapshot)` → `store.save("Alpha")` | `22c0745dfe0305fe3fcd3dcb0dc1f09e43bb860c6d571d2b009f59e7dd63cb82` (identical to save, as expected: rollback restores the saved state) |

Determinism: the throwaway capture ran each scenario twice per run and asserted that both runs were identical. Two separate runs produced the same SHA-256 for all four files. The throwaway test `tests/capture_profile_writer_goldens.rs` was deleted afterwards. **Never regenerate these files from the new writer.** Task 2.1 must reproduce each scenario through the writer, starting from `seed.toml`, and compare with `fs::read == include_bytes!`.

## Rust contracts (`crosshook-app`)

### Events (`src/events.rs`, re-exported from `lib.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileChangeReason {
    Saved, Deleted, Renamed { from: String }, Duplicated, FavoriteToggled,
    RolledBack, PresetApplied, Imported, Migrated,
}
// in #[non_exhaustive] AppEvent:
ProfilesChanged { reason: ProfileChangeReason, names: Vec<String> }
```

Wire shape: `{"reason":{"kind":"renamed","from":"Alpha"},"names":["Beta"]}`. Rename: `names: [new]`, `from: old`. Delete: `names: [removed]`. Duplicate: `names: [copy]`. `names: []` is reserved for the bridge's resync.

### Service (`src/services/profiles/mod.rs`)

```rust
#[derive(Clone)]
pub struct ProfileWriter {            // fields private to services::profiles
    profiles: ProfileStore, metadata: MetadataStore, settings: SettingsStore,
    events: EventBus, write_lock: Arc<Mutex<()>>,
}
impl ProfileWriter {
    pub fn new(profiles: ProfileStore, metadata: MetadataStore,
               settings: SettingsStore, events: EventBus) -> Self;
}
```

Re-exports: `crosshook_app::ProfileWriter`, `crosshook_app::services::ProfileWriter`, and `crosshook_app::services::profiles::{ProfileSummary, LaunchOptimizationsPayload, CommandArgumentsPayload, BundledOptimizationPresetDto, ImportedProfileWrite, CommitWarning, ProfileCommit, ConfigRevisionSummary, ConfigDiffMode, ConfigSemanticChange, ConfigDiffResult, ConfigRollbackResult}`. The DTO serde shapes are copied verbatim from the Tauri commands. `ConfigDiffMode::parse` is **not** included; Task 1.4 adds it as a private fn in `history.rs`.

`mod.rs` carries `#![expect(dead_code, reason = …)]`. **Task 2.1 deletes it.** Because it is `expect`, it fails the build if nothing is dead any more, which forces the removal.

### Commit pipeline (`writer.rs`, `types.rs`)

```rust
// types.rs, crate-private
pub(crate) trait ProfileMutation: Send + 'static {
    type Output: Send + 'static;
    fn accepts(&self, reason: &ProfileChangeReason) -> bool;
    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<Self::Output>, AppError>;
}
pub(crate) struct Persisted<T> {
    pub(crate) output: T,
    pub(crate) effects: Vec<Effect>,
    pub(crate) names: Option<Vec<String>>,   // None = no event (no-op rename, AlreadyValid, …)
}
pub(crate) enum Effect {
    ObserveWrite { name: String, profile: GameProfile, source: SyncSource, source_profile_id: Option<String> },
    ObserveRename { old_name: String, new_name: String },
    ObserveDelete { name: String },
    Revision { name: String, profile: GameProfile, source: ConfigRevisionSource, source_revision_id: Option<i64> },
    InvalidateHealth { name: String },       // looks up the id by *current* name; order before ObserveDelete
}
pub struct ProfileCommit<T> { pub output: T, pub new_revision_id: Option<i64>, pub warnings: Vec<CommitWarning> }
impl<T> ProfileCommit<T> { pub fn into_output(self) -> T; }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitWarning { MetadataUnavailable, ObserveFailed, RevisionSkipped, HealthInvalidationFailed }

// writer.rs
pub(crate) struct CommitContext<'a> {
    pub(crate) profiles: &'a ProfileStore,
    pub(crate) metadata: &'a MetadataStore,
    pub(crate) settings: &'a SettingsStore,
}
impl ProfileWriter {
    pub(crate) async fn commit<M: ProfileMutation>(&self, mutation: M, reason: ProfileChangeReason)
        -> Result<ProfileCommit<M::Output>, AppError>;
}
```

Pipeline semantics:

1. A mismatched `accepts` returns `Internal` and nothing runs.
2. `run_blocking` runs → write lock (poison is recovered via `into_inner`, D3) → `persist`. On error: return the error, run no effects, publish no event.
3. Effects run in list order and fail soft. If metadata is unavailable, **all** effects are skipped and the single warning is `MetadataUnavailable` (D6). The revision retention cap is resolved once per commit through `revisions::resolve_config_history_max_revisions`. Dedupe produces `None` and is not a warning.
4. If `names` is `Some`, the pipeline publishes `ProfilesChanged { reason, names }` inside the closure (D4).

Validation and load happen **inside `persist`**, under the lock. Reads (list, load, summaries, favorites, history, diff) do not use `commit` and do not take the lock. They call `super::run_blocking` directly, following `services/health/mod.rs`.

Persist rules for slices:

- Write TOML exactly once, through `ctx.profiles.save` / `save_without_companion` / `rename` / `duplicate` / `delete`. Never call a core method that saves and then save again (D8).
- Favorite has no TOML write and no effects. It calls `set_profile_favorite` inside `persist`, returns `Unavailable` when `!metadata.is_available()`, and sets `names: Some(vec![name])`.
- Advisory metadata calls that are not listed in `Effect`, such as the preset-origin upsert or `lookup_profile_id` for a duplicate's `source_profile_id`, run inside `persist`. They fail soft with `tracing::warn!`.

### Moved helpers (B0 moved them; B1 slices only call them)

| Module | Item |
| --- | --- |
| `revisions.rs` | `capture_config_revision(name, &GameProfile, ConfigRevisionSource, Option<i64>, &MetadataStore, max) -> RevisionOutcome { Inserted(i64), Deduped, Skipped }`; `resolve_config_history_max_revisions(&SettingsStore) -> usize`. Disabled-store skip now logs `warn!`. Normally reached through `Effect::Revision`. |
| `launcher_cleanup.rs` | `cleanup_launchers_for_profile_delete(name, &GameProfile) -> Result<Option<LauncherDeleteResult>, LauncherStoreError>` (native → `Ok(None)`); `derive_steam_client_install_path`, `derive_target_home_path` |
| `defaults_art.rs` | `apply_new_profile_defaults(&mut GameProfile, &AppSettingsData, &MetadataStore)` (settings defaults + default bundled preset, only when the draft has none); `auto_import_custom_art(name, &mut GameProfile)` (cover/portrait/background, fail-soft) |

All of these are `pub(super)` inside `services::profiles`.

### Test harness (`tests/helpers.rs`)

`Harness::new(Metadata::Enabled | Metadata::Disabled)` gives you `{ root: TempDir, profiles, settings, metadata, writer, events }`, where `events` is subscribed before any write. `harness.next_event().await` returns `Option<AppEvent>` with a 50 ms timeout. `steam_profile(home) -> GameProfile` is moved from the shell test helpers. `tests/writer.rs` (B0) pins the pipeline: reason gate, publish-once, dedupe without warning, failed persist is silent, disabled metadata warns, health row is deleted, poisoned lock is recovered, and disk bytes come from the store serializer.

### Wiring

- `Services::new(stores: &Stores, events: EventBus)` adds the field `pub profiles: ProfileWriter`. `app_bootstrap.rs` calls `Services::new(&stores, app.events().clone())`.
- `src-tauri/src/lib.rs`: `.manage(services.profiles.clone())` makes `State<'_, ProfileWriter>` available to every command.
- `src-tauri/src/startup_bridge.rs` has a private `ProfilesChangedPayload { reason, names }`. The arm `AppEvent::ProfilesChanged` emits `profiles-changed`. `Resync` replays startup jobs and then emits `{reason:{kind:"saved"},names:[]}` (D14). Tests are in `startup_bridge/tests.rs`.
- `crosshook-app` Cargo: `toml = "1.1.0"` and `serde_json = "1"` are now normal dependencies (removed from dev-deps).

### Core (`crosshook-core`)

Landed in B0:

```rust
impl MetadataStore { pub fn invalidate_health_snapshot(&self, profile_id: &str) -> Result<(), MetadataStoreError>; }
// health_store.rs
pub fn delete_health_snapshot(conn: &Connection, profile_id: &str) -> Result<(), MetadataStoreError>;
```

This is a DELETE by primary key. There is no schema change (schema stays 28). Disabled stores and missing rows are no-ops.

Frozen for Task 1.1 (D8; **not** yet implemented):

```rust
impl GameProfile {
    pub fn apply_launch_optimizations(&mut self, enabled_option_ids: Vec<String>, switch_active_preset: Option<String>) -> Result<(), ProfileStoreError>;
    pub fn apply_command_arguments(&mut self, enabled_argument_ids: Vec<String>, custom_args: Vec<String>, resolved_launch_method: Option<&str>) -> Result<(), ProfileStoreError>;
    pub fn materialize_launch_optimization_preset(&mut self, preset_key: &str, enabled_option_ids: Vec<String>, set_as_active: bool) -> Result<(), ProfileStoreError>;
    pub fn apply_manual_optimization_preset(&mut self, preset_display_name: &str, enabled_option_ids: Vec<String>) -> Result<(), ProfileStoreError>;
}
impl ProfileStore {
    pub fn unique_copy_name(&self, source_name: &str) -> Result<String, ProfileStoreError>;
    pub fn save_without_companion(&self, name: &str, profile: &GameProfile) -> Result<(), ProfileStoreError>;
}
// profile::migration (Task 1.5 implements it, in core)
pub fn prepare_profile_migration(profile: &mut GameProfile, request: &ApplyMigrationRequest) -> MigrationApplyResult;
```

The plan's contract list writes `materialize_launch_optimization_preset(...)` and `apply_manual_optimization_preset(...)` without a return type. Both validate catalog ids and names, so they return `Result<(), ProfileStoreError>`, as in `research-technical.md`. This document is binding on that point.

## TypeScript contracts

- `src/types/profile-events.ts` (re-exported from `src/types/index.ts`) defines `ProfileChangeReason` (a discriminated union over `kind`), `ProfileChangeKind`, and `ProfilesChangedPayload { reason; names: string[] }`.
- `src/lib/profileEvents.ts` provides:
  - `parseProfilesChanged(payload: unknown): ProfilesChangedPayload | null`. It returns `null` for any malformed payload, which callers must treat as a **conservative resync**. That covers legacy strings, `{name, action}`, unknown or inherited kinds, `renamed` without a string `from`, and names that are missing or not strings.
  - `profileChangeAffectsHealth(reason): boolean`, which is false only for `favorite_toggled`.
- Tests: `src/lib/__tests__/profileEvents.test.ts`.

## B1 file ownership

Files marked Frozen are B0-owned. A slice must not edit them; Task 2.1 may.

| Task | Owns (creates or fills) |
| --- | --- |
| Frozen (B0) | `crates/crosshook-app/src/events.rs`, `events/tests.rs`, `lib.rs`, `Cargo.toml`, `services/mod.rs`, `services/profiles/{mod,writer,types,history_types,revisions,launcher_cleanup,defaults_art}.rs`, `services/profiles/tests/{mod,helpers,writer}.rs`, `bootstrap/{stores,app_bootstrap}.rs`, `bootstrap/stores/tests.rs`, `crates/crosshook-core/src/metadata/{health_ops,health_store}.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/startup_bridge.rs` (+`tests.rs`), `src/types/profile-events.ts`, `src/types/index.ts`, `src/lib/profileEvents.ts` (+ test), `tests/fixtures/profile_writer/*` |
| 1.1 | `core/src/profile/mutations/{mod,optimizations,command_arguments}.rs`, `core/src/profile/mod.rs`, `core/src/profile/toml_store/store.rs`, `core/src/profile/toml_store/tests/{optimizations,command_arguments}.rs` (**the D5 part is dropped; it is already done**) |
| 1.2 | `services/profiles/{lifecycle,reads}.rs`, `services/profiles/tests/{lifecycle,events}.rs`, `src-tauri/src/commands/profile/{lifecycle,favorites}.rs`, `src-tauri/src/commands/profile/tests/lifecycle.rs` |
| 1.3 | `services/profiles/{sections,presets}.rs`, `services/profiles/tests/sections.rs`, `src-tauri/src/commands/profile/{optimizations,command_arguments}.rs`, `src-tauri/src/commands/profile/tests/optimizations.rs` |
| 1.4 | `services/profiles/history.rs`, `services/profiles/tests/history.rs`, `src-tauri/src/commands/profile/config_history.rs` |
| 1.5 | `services/profiles/{suggestions,migration,imports}.rs`, `services/profiles/tests/{suggestions,migration}.rs`, `core/src/profile/migration/{apply,mod,tests}.rs`, `src-tauri/src/commands/{protondb,migration,lutris}.rs` |
| 1.6 | `src/hooks/{useProfileHealth,useProfile,useProfileSummaries,useLibrarySummaries,useCommunityProfiles}.ts`, `src/components/pages/HealthDashboardPage.tsx`, `src/components/pages/health-dashboard/useHealthDashboardState.ts`, `src/components/layout/MetadataStatusBanner.tsx` (+ test), `src/hooks/__tests__/{useProfileSummaries.test,useProfileHealth.profilesChanged.test}.ts`, `src/components/pages/__tests__/LibraryPage.test.tsx` |
| 1.7 | `src/lib/mocks/handlers/{profile-utils,profile-mutations,profile-presets,profile-history,protondb,proton,lutris}.ts` |
| 2.1 | `src-tauri/src/commands/profile/{shared,mod}.rs`, `commands/profile/tests/{mod,contract}.rs`, `crates/crosshook-app/tests/profile_writer_golden.rs`, `crates/crosshook-app/README.md` (services row; the `EventBus` row was updated in B0), `src/lib/mocks/README.md`; removes the `#![expect(dead_code)]` in `services/profiles/mod.rs` |

Each facade module (`lifecycle.rs`, `reads.rs`, `sections.rs`, `presets.rs`, `history.rs`, `suggestions.rs`, `migration.rs`, `imports.rs`) and each slice test file is declared already and contains only module docs. Slices add `impl ProfileWriter { pub async fn … }` blocks and their `ProfileMutation` impls there. Use `super::run_blocking`, `super::writer::CommitContext`, and `super::types::{Effect, Persisted, ProfileMutation}`.

The shell helpers in `commands/profile/shared.rs` (`capture_config_revision`, `emit_profiles_changed`, …) still exist in B0 so the shell compiles. B1 slices stop importing them, and Task 2.1 deletes them.
