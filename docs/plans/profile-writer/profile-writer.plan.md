# Plan: ProfileWriter — profile mutation, revision and rollback in `crosshook-app`

Tracking: Linear YAN-811 / GitHub yandy-r/crosshook#532 (M1-06). Target release `crosshook v0.7.0-alpha.1`, trunk-only, base `main`, branch `refactor/yan-811-profile-writer`. Paths are relative to the repo root; Rust/frontend code lives under `src/crosshook-native/` (shortened below as `app/` = `src/crosshook-native/crates/crosshook-app`, `core/` = `src/crosshook-native/crates/crosshook-core`, `tauri/` = `src/crosshook-native/src-tauri`, `ui/` = `src/crosshook-native/src`).

## Contents

- Summary
- User Story
- Problem → Solution
- Metadata
- Batches
- UX Design
- Mandatory Reading
- External Documentation
- Patterns to Mirror
- Files to Change
- NOT Building
- Step-by-Step Tasks
- Testing Strategy
- Validation Commands
- Acceptance Criteria
- Completion Checklist
- Risks
- Notes

## Summary

Move every profile write path (23 profile commands + ProtonDB accept + Proton migration apply + Lutris import effects) behind one async `ProfileWriter::commit` pipeline in `crosshook-app`, add a typed `AppEvent::ProfilesChanged { reason, names }` object payload bridged to `profiles-changed`, and stop favorite toggles from triggering `batch_validate_profiles`. TOML bytes, SQLite schema (28) and IPC shapes stay identical, proven by pre-refactor golden fixtures.

## User Story

As a CrossHook player, I want favorite toggles to stay instant and my saves/renames/duplicates/rollbacks to behave exactly as before, so profile health is only recomputed when content actually changed and my data never shifts format.

## Problem → Solution

Today the write sequence (validate → load → mutate → save → observe → revision → invalidate → emit) is duplicated across sync Tauri commands with a bare-string event; five listeners rescan everything on any emission, including a main-thread health batch for a favorite toggle; duplicate/rename/sections/ProtonDB/migration never emit. → One async service pipeline with a typed reason, a single object event, per-reason listener policy, and a serialized writer lock; `src-tauri` becomes thin delegation.

## Metadata

- **Complexity**: XL
- **Source PRD**: `docs/plans/profile-writer/issue.md` (+ binding `decisions.md` D1–D15, spec `feature-spec.md`)
- **PRD Phase**: M1-06 (v0.7.0-alpha.1)
- **Estimated Files**: ~55 (≈24 create, ≈31 modify)

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order. **B0 is executed strictly sequentially by the single integration owner** (both tasks, in ID order) before any B1 task starts; internal batches may be temporarily incomplete, but the final branch must be fully cut over (one PR).

| Batch | Tasks                             | Depends On | Parallel Width                        |
| ----- | --------------------------------- | ---------- | ------------------------------------- |
| B0    | 0.1, 0.2                          | —          | 1 (sequential; one integration owner) |
| B1    | 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7 | B0         | 7                                     |
| B2    | 2.1                               | B1         | 1 (integration owner)                 |

- **Total tasks**: 10
- **Total batches**: 3
- **Max parallel width**: 7

Ownership rule: no two tasks in the same batch touch the same file. Cross-slice Rust signatures (D8 helpers, D5 primitive) are frozen verbatim in Task 0.2's contract list; B1 slices implement/call them against that contract, and the main agent compiles/verifies once after all of B1 lands (repo hand-off policy: subagents skip builds mid-flight).

**B0 outcome (binding for B1):** [`b0-contracts.md`](./b0-contracts.md) records the final signatures, module files and per-task file ownership. Main approved moving the D5 primitive (`MetadataStore::invalidate_health_snapshot`) into B0, so Task 1.1 no longer includes it. `commit` is generic over a crate-private `ProfileMutation` trait rather than an enum, so slices never share a dispatch file.

## UX Design

### Before

```text
any profiles-changed "reason-string" ──▶ 5 listeners rescan everything
favorite toggle ──▶ batch_validate_profiles (full health recompute + snapshot rewrite)
duplicate/rename/sections/protondb/migration ──▶ no event at all (silent staleness)
```

### After

```text
profiles-changed {reason:{kind,…},names} ──▶ per-reason policy:
  favorite_toggled ─▶ favorites reload only (stars/ordering update, zero health IPC)
  saved/deleted/renamed/… ─▶ list/summary/favorite refresh + health revalidation
  malformed payload ─▶ conservative resync (treat as saved, refresh everything)
```

### Interaction Changes

| Touchpoint                               | Before                                              | After                                                                                                                                | Notes                                               |
| ---------------------------------------- | --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------- |
| Favorite star                            | instant optimistic + full health rescan after event | instant optimistic + favorites-only refresh; Health Dashboard star/ordering update from `ProfileContext.favoriteProfiles` projection | no new optimistic UI                                |
| Save/rename/duplicate/rollback           | some paths silent                                   | one typed event each; no duplicate success toasts                                                                                    | rename keeps `had_launcher` bool → re-export banner |
| Metadata unavailable                     | silent revision skip                                | banner explains "profile files still save, but configuration snapshots are not recorded"                                             | session dismissal respected                         |
| History/diff/rollback with metadata down | explicit error                                      | unchanged explicit `Unavailable` error                                                                                               |                                                     |

## Mandatory Reading

| Priority       | File                                                                         | Lines  | Why                                                |
| -------------- | ---------------------------------------------------------------------------- | ------ | -------------------------------------------------- |
| P0 (critical)  | `docs/plans/profile-writer/issue.md`                                         | all    | Authoritative acceptance criteria                  |
| P0 (critical)  | `docs/plans/profile-writer/decisions.md`                                     | all    | Binding decisions D1–D15                           |
| P0 (critical)  | `docs/plans/profile-writer/feature-spec.md`                                  | all    | Synthesized spec, storage boundary, risks          |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-app/src/services/health/mod.rs`       | all    | Service pattern to mirror exactly                  |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-app/src/services/mod.rs`              | all    | `run_blocking` helper (crate-private)              |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-app/src/events.rs`                    | all    | `AppEvent`/`EventBus` to extend                    |
| P0 (critical)  | `src/crosshook-native/src-tauri/src/commands/profile/shared.rs`              | all    | Helpers being moved/deleted                        |
| P0 (critical)  | `src/crosshook-native/src-tauri/src/commands/profile/lifecycle.rs`           | all    | Save/delete/duplicate/rename/import logic to move  |
| P0 (critical)  | `src/crosshook-native/src-tauri/src/commands/profile/config_history.rs`      | all    | History/diff/rollback/known-good to move           |
| P1 (important) | `src/crosshook-native/src-tauri/src/startup_bridge.rs`                       | all    | Bridge gaining the `ProfilesChanged` arm           |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs` | 79-312 | Single serializer, section saves, duplicate naming |
| P1 (important) | `src/crosshook-native/src-tauri/src/commands/health/batch.rs`                | all    | Thin-command delegate pattern                      |
| P1 (important) | `src/crosshook-native/src/hooks/__tests__/useProfileHealth.startup.test.ts`  | 1-118  | Vitest call-counter harness to copy                |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/tests/launch_request_golden.rs`  | 38-101 | Golden-fixture convention                          |
| P2 (reference) | `docs/plans/profile-writer/research-recommendations.md`                      | all    | Phasing, ownership table, risk register            |
| P2 (reference) | `docs/plans/profile-writer/research-business.md`                             | all    | Per-command behavior inventory + edge cases        |
| P2 (reference) | `src/crosshook-native/crates/crosshook-app/src/bootstrap/stores.rs`          | 42-78  | `Services` bundle to extend                        |

## External Documentation

| Topic                      | Source                                                         | Key Takeaway                                                            |
| -------------------------- | -------------------------------------------------------------- | ----------------------------------------------------------------------- |
| tokio `spawn_blocking`     | https://docs.rs/tokio/1.51.0/tokio/task/fn.spawn_blocking.html | Non-abortable once started → publish the event inside the closure (D4)  |
| tokio `sync::broadcast`    | https://docs.rs/tokio/1.51.0/tokio/sync/broadcast/index.html   | Drop-oldest lag → `RecvError::Lagged` → `AppEvent::Resync`              |
| serde enum representations | https://serde.rs/enum-representations.html                     | Internally tagged: struct variants only; unit variants → `{"kind":"…"}` |
| Tauri v2 events            | https://v2.tauri.app/develop/calling-frontend/                 | Payloads JSON; `unlisten` is a Promise; no compat shim allowed          |

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### SERVICE_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-app/src/services/health/mod.rs:37-41
    pub async fn batch_summary(&self) -> Result<EnrichedHealthSummary, AppError> {
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || Ok(batch::build_enriched_health_summary(&profiles, &metadata))).await
    }
```

### THIN_COMMAND

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/health/batch.rs:6-11
pub async fn batch_validate_profiles(
    service: State<'_, HealthService>,
) -> Result<EnrichedHealthSummary, String> {
    service
        .batch_summary()
        .await
        .map_err(|error| error.to_string())
}
```

### TAGGED_ENUM

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/protondb/suggestions.rs:73-78
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AcceptSuggestionRequest {
    #[serde(rename_all = "camelCase")]
    Catalog {
```

### BRIDGE_LOOP

```rust
// SOURCE: src/crosshook-native/src-tauri/src/startup_bridge.rs:122-129
                AppEvent::StartupJobFinished { job } => {
                    emit_finished(&handle, job, &startup.jobs_snapshot());
                }
                AppEvent::Resync { .. } => {
                    let jobs = startup.jobs_snapshot();
```

### ERROR_SANITIZATION

```rust
// SOURCE: src/crosshook-native/crates/crosshook-app/src/error.rs:36-42
    pub(crate) fn new(kind: AppErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: sanitize_message(&message.into()),
        }
    }
```

### LOCK_RECOVERY (D3 target idiom)

```rust
// SOURCE: std::sync poison-recovery idiom; guard value is `()` so poisoning carries no invariant
let _guard = self.write_lock
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);
```

### TEST_HARNESS

```rust
// SOURCE: src/crosshook-native/crates/crosshook-app/src/services/health/tests.rs:3-7
#[tokio::test]
async fn batch_and_cached_snapshots_work_without_metadata() {
    let root = tempfile::tempdir().unwrap();
    let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
    let service = HealthService::new(profiles, MetadataStore::disabled());
```

### GOLDEN_FIXTURE

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/tests/launch_request_golden.rs:39-40
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/launch_request");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
```

### VITEST_COUNTER

```ts
// SOURCE: src/crosshook-native/src/hooks/__tests__/useProfileHealth.startup.test.ts:60-62
const batchCalls = () => mockCallCommand.mock.calls.filter((c) => c[0] === 'batch_validate_profiles');
```

## Files to Change

CREATE rows are skipped by path validation (files do not exist yet); every UPDATE row exists (verified in the worktree).

| File                                                                                           | Action | Justification                                                                                              |
| ---------------------------------------------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-app/tests/fixtures/profile_writer/save.toml`            | CREATE | D15 pre-refactor golden (Task 0.1)                                                                         |
| `src/crosshook-native/crates/crosshook-app/tests/fixtures/profile_writer/rename.toml`          | CREATE | D15 golden incl. display-name rewrite                                                                      |
| `src/crosshook-native/crates/crosshook-app/tests/fixtures/profile_writer/duplicate.toml`       | CREATE | D15 golden for the copy                                                                                    |
| `src/crosshook-native/crates/crosshook-app/tests/fixtures/profile_writer/rollback.toml`        | CREATE | D15 golden for restored snapshot re-save                                                                   |
| `src/crosshook-native/crates/crosshook-app/tests/profile_writer_golden.rs`                     | CREATE | Byte-for-byte comparison vs fixtures (Task 2.1)                                                            |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/mod.rs`                       | CREATE | `ProfileWriter` struct, `new`, module tree, re-exports                                                     |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/writer.rs`                    | CREATE | Private `commit` pipeline + D3 lock + reason validation                                                    |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/types.rs`                     | CREATE | `ProfileMutation`, `ProfileCommit`, `ImportedProfileWrite`, moved non-history DTOs                         |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/history_types.rs`             | CREATE | Moved history/diff DTOs, serde unchanged                                                                   |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/revisions.rs`                 | CREATE | Moved revision capture + retention resolver + D6 warning                                                   |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/launcher_cleanup.rs`          | CREATE | Moved path derivation + cleanup delegation                                                                 |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/defaults_art.rs`              | CREATE | Moved creation defaults + art auto-import                                                                  |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/lifecycle.rs`                 | CREATE | Save/delete/duplicate/rename/import/export facades                                                         |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/reads.rs`                     | CREATE | List/load/summaries/favorites                                                                              |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/sections.rs`                  | CREATE | Optimizations/MangoHud/gamescope/trainer-gamescope/arguments                                               |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/presets.rs`                   | CREATE | Bundled + manual presets                                                                                   |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/history.rs`                   | CREATE | History/diff/rollback/known-good                                                                           |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/suggestions.rs`               | CREATE | ProtonDB accept                                                                                            |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/migration.rs`                 | CREATE | Proton migration apply/batch                                                                               |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/imports.rs`                   | CREATE | `record_imports` for Lutris (D10)                                                                          |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/tests/mod.rs`                 | CREATE | Test module declarations + shared harness                                                                  |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/tests/helpers.rs`             | CREATE | tempdir writer harness, metadata enabled/disabled, steam fixture                                           |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/tests/events.rs`              | CREATE | Event/reason/wire assertions                                                                               |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/tests/lifecycle.rs`           | CREATE | Lifecycle + launcher cleanup tests                                                                         |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/tests/sections.rs`            | CREATE | Section/preset tests                                                                                       |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/tests/history.rs`             | CREATE | History/rollback tests                                                                                     |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/tests/suggestions.rs`         | CREATE | ProtonDB accept tests                                                                                      |
| `src/crosshook-native/crates/crosshook-app/src/services/profiles/tests/migration.rs`           | CREATE | Migration preflight/isolation tests                                                                        |
| `src/crosshook-native/crates/crosshook-core/src/profile/mutations/mod.rs`                      | CREATE | D8 extraction root                                                                                         |
| `src/crosshook-native/crates/crosshook-core/src/profile/mutations/optimizations.rs`            | CREATE | `apply_launch_optimizations`, `materialize_launch_optimization_preset`, `apply_manual_optimization_preset` |
| `src/crosshook-native/crates/crosshook-core/src/profile/mutations/command_arguments.rs`        | CREATE | `apply_command_arguments`                                                                                  |
| `src/crosshook-native/src/types/profile-events.ts`                                             | CREATE | D13 TS union + payload type                                                                                |
| `src/crosshook-native/src/lib/profileEvents.ts`                                                | CREATE | `parseProfilesChanged`, `profileChangeAffectsHealth`                                                       |
| `src/crosshook-native/src/lib/__tests__/profileEvents.test.ts`                                 | CREATE | Parser/policy unit tests                                                                                   |
| `src/crosshook-native/src/hooks/__tests__/useProfileHealth.profilesChanged.test.ts`            | CREATE | Favorite-skips-batch call-counter test + saved control                                                     |
| `src/crosshook-native/src-tauri/src/commands/profile/tests/contract.rs`                        | CREATE | IPC name contract test                                                                                     |
| `src/crosshook-native/crates/crosshook-app/src/events.rs`                                      | UPDATE | `ProfileChangeReason`, `AppEvent::ProfilesChanged`                                                         |
| `src/crosshook-native/crates/crosshook-app/src/events/tests.rs`                                | UPDATE | Wire-shape/lag/equality tests                                                                              |
| `src/crosshook-native/crates/crosshook-app/src/services/mod.rs`                                | UPDATE | Declare/re-export `profiles`                                                                               |
| `src/crosshook-native/crates/crosshook-app/Cargo.toml`                                         | UPDATE | Promote `toml`, `serde_json` to deps                                                                       |
| `src/crosshook-native/crates/crosshook-app/src/bootstrap/stores.rs`                            | UPDATE | `Services::new(&Stores, EventBus)`, `profiles` field                                                       |
| `src/crosshook-native/crates/crosshook-app/src/bootstrap/app_bootstrap.rs`                     | UPDATE | Pass `app.events().clone()`                                                                                |
| `src/crosshook-native/crates/crosshook-app/src/bootstrap/stores/tests.rs`                      | UPDATE | Supply a bus in bundle test                                                                                |
| `src/crosshook-native/crates/crosshook-app/src/lib.rs`                                         | UPDATE | Re-export `ProfileChangeReason`, `ProfileWriter`                                                           |
| `src/crosshook-native/crates/crosshook-app/README.md`                                          | UPDATE | Services table row + event doc                                                                             |
| `src/crosshook-native/crates/crosshook-core/src/metadata/health_ops.rs`                        | UPDATE | D5 `invalidate_health_snapshot` + tests                                                                    |
| `src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs`                      | UPDATE | Parameterized DELETE                                                                                       |
| `src/crosshook-native/crates/crosshook-core/src/profile/mod.rs`                                | UPDATE | `pub mod mutations;` + re-exports                                                                          |
| `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs`                   | UPDATE | Delegate to mutations; add `save_without_companion`, `unique_copy_name`                                    |
| `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/tests/optimizations.rs`     | UPDATE | Pin extraction parity                                                                                      |
| `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/tests/command_arguments.rs` | UPDATE | Pin extraction parity                                                                                      |
| `src/crosshook-native/crates/crosshook-core/src/profile/migration/apply.rs`                    | UPDATE | Extract `prepare_profile_migration`; use `save_without_companion`                                          |
| `src/crosshook-native/crates/crosshook-core/src/profile/migration/mod.rs`                      | UPDATE | Re-export `prepare_profile_migration`                                                                      |
| `src/crosshook-native/crates/crosshook-core/src/profile/migration/tests.rs`                    | UPDATE | Preparation/no-companion regression tests                                                                  |
| `src/crosshook-native/src-tauri/src/lib.rs`                                                    | UPDATE | `.manage(services.profiles.clone())`                                                                       |
| `src/crosshook-native/src-tauri/src/startup_bridge.rs`                                         | UPDATE | `ProfilesChanged` arm + Resync sentinel (D14)                                                              |
| `src/crosshook-native/src-tauri/src/startup_bridge/tests.rs`                                   | UPDATE | Object emission + lag tests                                                                                |
| `src/crosshook-native/src-tauri/src/commands/profile/lifecycle.rs`                             | UPDATE | Async delegates to writer                                                                                  |
| `src/crosshook-native/src-tauri/src/commands/profile/favorites.rs`                             | UPDATE | Async delegates                                                                                            |
| `src/crosshook-native/src-tauri/src/commands/profile/optimizations.rs`                         | UPDATE | Async delegates                                                                                            |
| `src/crosshook-native/src-tauri/src/commands/profile/command_arguments.rs`                     | UPDATE | Async delegate                                                                                             |
| `src/crosshook-native/src-tauri/src/commands/profile/config_history.rs`                        | UPDATE | Async delegates                                                                                            |
| `src/crosshook-native/src-tauri/src/commands/profile/mod.rs`                                   | UPDATE | Drop helper re-exports (B2)                                                                                |
| `src/crosshook-native/src-tauri/src/commands/profile/shared.rs`                                | UPDATE | Keep only `apply_collection_defaults` (+ glue) after B2                                                    |
| `src/crosshook-native/src-tauri/src/commands/profile/tests/mod.rs`                             | UPDATE | `mod contract;`, drop moved test mods                                                                      |
| `src/crosshook-native/src-tauri/src/commands/profile/tests/lifecycle.rs`                       | UPDATE | Migrate cleanup tests to service harness                                                                   |
| `src/crosshook-native/src-tauri/src/commands/profile/tests/optimizations.rs`                   | UPDATE | Migrate two helper tests; keep collection tests                                                            |
| `src/crosshook-native/src-tauri/src/commands/protondb.rs`                                      | UPDATE | Accept delegates; delete `observe_suggestion_apply`                                                        |
| `src/crosshook-native/src-tauri/src/commands/migration.rs`                                     | UPDATE | Apply commands delegate                                                                                    |
| `src/crosshook-native/src-tauri/src/commands/lutris.rs`                                        | UPDATE | Use `record_imports`                                                                                       |
| `src/crosshook-native/src/hooks/useProfileHealth.ts`                                           | UPDATE | Reason policy, favorite skip                                                                               |
| `src/crosshook-native/src/hooks/useProfile.ts`                                                 | UPDATE | Favorite → favorites-only reload                                                                           |
| `src/crosshook-native/src/hooks/useProfileSummaries.ts`                                        | UPDATE | Skip favorite refetch                                                                                      |
| `src/crosshook-native/src/hooks/useLibrarySummaries.ts`                                        | UPDATE | Skip favorite refetch                                                                                      |
| `src/crosshook-native/src/hooks/useCommunityProfiles.ts`                                       | UPDATE | Skip favorite rescan                                                                                       |
| `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`                            | UPDATE | Favorite overlay from `favoriteProfiles`                                                                   |
| `src/crosshook-native/src/components/pages/health-dashboard/useHealthDashboardState.ts`        | UPDATE | Overlay projection without health IPC                                                                      |
| `src/crosshook-native/src/components/layout/MetadataStatusBanner.tsx`                          | UPDATE | D6 copy (snapshots skipped)                                                                                |
| `src/crosshook-native/src/components/layout/__tests__/MetadataStatusBanner.test.tsx`           | UPDATE | Copy assertions                                                                                            |
| `src/crosshook-native/src/types/index.ts`                                                      | UPDATE | Re-export profile-events types                                                                             |
| `src/crosshook-native/src/hooks/__tests__/useProfileSummaries.test.ts`                         | UPDATE | Object payloads, favorite no-refetch                                                                       |
| `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx`                     | UPDATE | Object payloads, favorite assertions                                                                       |
| `src/crosshook-native/src/lib/mocks/handlers/profile-mutations.ts`                             | UPDATE | Typed publisher, real reasons/names                                                                        |
| `src/crosshook-native/src/lib/mocks/handlers/profile-presets.ts`                               | UPDATE | `preset_applied` events                                                                                    |
| `src/crosshook-native/src/lib/mocks/handlers/profile-history.ts`                               | UPDATE | `rolled_back` event                                                                                        |
| `src/crosshook-native/src/lib/mocks/handlers/protondb.ts`                                      | UPDATE | Accept emits `saved`                                                                                       |
| `src/crosshook-native/src/lib/mocks/handlers/proton.ts`                                        | UPDATE | Applied migrations emit `migrated`                                                                         |
| `src/crosshook-native/src/lib/mocks/handlers/lutris.ts`                                        | UPDATE | Successful imports emit `imported`                                                                         |
| `src/crosshook-native/src/lib/mocks/handlers/profile-utils.ts`                                 | UPDATE | One typed `emitProfilesChanged` helper                                                                     |
| `src/crosshook-native/src/lib/mocks/README.md`                                                 | UPDATE | Event shape + parity doc                                                                                   |

`CLAUDE.md` / `AGENTS.md` are intentionally **not** modified: no agent rule changes (no new startup contract, no schema bump, no new label).

## NOT Building

- Collections / `apply_collection_defaults` move (M1-35) — stays in `src-tauri` with its tests.
- Any SQLite schema change or migration (schema stays 28; `invalidate_health_snapshot` is a DELETE over the existing table).
- Any TOML format change, second serializer, or shareable-export redesign.
- String/event compatibility shims, dual emission, or legacy `"saved"` payloads.
- New Rust/npm dependencies (`insta`, `mockall`, etc.).
- Optimistic-UI redesign, toasts per autosave, new spinners, scroll-container work.
- Per-name incremental health scheduling; general event/cache framework; public caller-supplied mutation/reason API.
- ProtonDB network lookup/dismiss redesign; Lutris import-preview redesign; community import rewiring.
- Security redesigns beyond D12 (export name validation) — follow-ups filed separately.

## Step-by-Step Tasks

### Task 0.1: Capture pre-refactor golden fixtures (D15) — Depends on [none]

- **BATCH**: B0
- **ACTION**: Freeze today's on-disk TOML bytes for save, rename, duplicate and rollback into committed fixtures under `src/crosshook-native/crates/crosshook-app/tests/fixtures/profile_writer/`, generated by the **current** (pre-refactor) code.
- **IMPLEMENT**: Write a throwaway integration test in `crates/crosshook-app/tests/` that builds a deterministic `GameProfile` (fixed paths, unknown extra TOML keys, a preset, a hook, machine-path fields that `storage_profile()` splits into `local_override`), then drives the current paths: `ProfileStore::save` (new + update), `profile_rename`'s sequence (`store.rename` + display-name `store.save`), `ProfileStore::duplicate`, and rollback (`store.save` of a parsed snapshot). Copy the four produced files to the fixtures dir as `save.toml`, `rename.toml`, `duplicate.toml`, `rollback.toml`; run twice and assert identical SHA-256 to prove determinism; then delete the throwaway test. Never regenerate fixtures from the new writer later.
- **MIRROR**: GOLDEN_FIXTURE (`crates/crosshook-core/tests/launch_request_golden.rs:38-46` — `CARGO_MANIFEST_DIR`-relative fixtures dir).
- **IMPORTS**: `crosshook_core::profile::{GameProfile, ProfileStore}`, `tempfile`.
- **GOTCHA**: The revision snapshot string (`toml::to_string_pretty(&profile)`) is NOT the disk bytes (`storage_profile()` first) — freeze disk bytes only; avoid `HashMap`-typed fields with >1 entry unless output is confirmed sorted.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app --test <throwaway>` passes twice with byte-identical output; four fixture files committed; throwaway test removed.

### Task 0.2: Freeze contracts — event, writer skeleton, wiring, bridge, TS types — Depends on [0.1]

- **BATCH**: B0
- **ACTION**: Land every cross-slice contract so B1 slices never edit shared files: reason enum + `AppEvent` variant, `ProfileWriter` skeleton with the D3 lock and a working private `commit`, the full `services/profiles/` module tree, `Services` bus injection, Tauri state registration, the bridge arm (D14), and the D13 TS type/parser. Also promote `toml` + `serde_json` to `crosshook-app` dependencies.
- **IMPLEMENT**: In `events.rs` add `ProfileChangeReason` (`Debug, Clone, PartialEq, Eq, Serialize`, `#[serde(tag = "kind", rename_all = "snake_case")]`, variants `Saved, Deleted, Renamed { from }, Duplicated, FavoriteToggled, RolledBack, PresetApplied, Imported, Migrated`) and `AppEvent::ProfilesChanged { reason, names }` (keeps `Clone + PartialEq + Eq`; no serde on `AppEvent`); extend `events/tests.rs` with `serde_json::to_value` wire assertions for all nine reasons and a lag test. Create `services/profiles/`: `mod.rs` (`ProfileWriter { profiles, metadata, settings, events, write_lock: Arc<std::sync::Mutex<()>> }`, sync `new`, submodule declarations, DTO re-exports), `writer.rs` (private `commit(name, mutation, reason) -> Result<ProfileCommit, AppError>` that validates mutation/reason consistency, takes the lock inside `run_blocking`, runs validate → load → mutate → persist-once → observe → revision → health-invalidate → `events.publish(AppEvent::ProfilesChanged …)` inside the closure, and returns typed `warnings` for skipped advisory steps), `types.rs`/`history_types.rs` (moved DTOs, serde field names/casing unchanged), `revisions.rs` (moved `capture_config_revision` + retention resolver, now `tracing::warn!` + a `Warnings::RevisionSkipped` signal when `!is_available()` — dedupe stays silent), `launcher_cleanup.rs` + `defaults_art.rs` (pure moves from Tauri `shared.rs`/`lifecycle.rs`), `tests/{mod,helpers}.rs` (harness: tempdir `ProfileStore::with_base_path`, `SettingsStore::with_base_path`, `MetadataStore::open_in_memory()`/`disabled()`, subscribed `EventBus`, steam fixture), and doc-comment module files for the B1 facade modules (per recommendations: internal batches may be temporarily incomplete; B2 completes the cutover). Change `Services::new(stores: &Stores, events: EventBus)` adding `pub profiles: ProfileWriter`, called as `Services::new(&stores, app.events().clone())` in `app_bootstrap.rs`; update `stores/tests.rs`. In `src-tauri/src/lib.rs` add `.manage(services.profiles.clone())`. In `startup_bridge.rs` add a private `ProfilesChangedPayload { reason, names }` (`Serialize + Clone`) and the arm `AppEvent::ProfilesChanged { reason, names } => if let Err(error) = handle.emit("profiles-changed", ProfilesChangedPayload { reason, names }) { tracing::warn!(…) }`, and on `AppEvent::Resync` additionally emit `{ reason: ProfileChangeReason::Saved, names: vec![] }` after the startup replay (D14); extend `startup_bridge/tests.rs`. Create `src/types/profile-events.ts` (union + `ProfilesChangedPayload`), export from `src/types/index.ts`, and `src/lib/profileEvents.ts` (`parseProfilesChanged(unknown): ProfilesChangedPayload | null` — malformed → null = resync; `profileChangeAffectsHealth(reason)` false only for `favorite_toggled`) with `src/lib/__tests__/profileEvents.test.ts`. **Frozen signatures for B1**: `GameProfile::apply_launch_optimizations(&mut self, enabled_option_ids: Vec<String>, switch_active_preset: Option<String>) -> Result<(), ProfileStoreError>`; `GameProfile::apply_command_arguments(&mut self, enabled_argument_ids: Vec<String>, custom_args: Vec<String>, resolved_launch_method: Option<&str>) -> Result<(), ProfileStoreError>`; `GameProfile::materialize_launch_optimization_preset(&mut self, preset_key: &str, enabled_option_ids: Vec<String>, set_as_active: bool)`; `GameProfile::apply_manual_optimization_preset(&mut self, preset_display_name: &str, enabled_option_ids: Vec<String>)`; `ProfileStore::unique_copy_name(&self, source_name: &str) -> Result<String, ProfileStoreError>`; `ProfileStore::save_without_companion(&self, name: &str, profile: &GameProfile) -> Result<(), ProfileStoreError>`; `profile::migration::prepare_profile_migration(profile: &mut GameProfile, request: &ApplyMigrationRequest) -> MigrationApplyResult`; `MetadataStore::invalidate_health_snapshot(&self, profile_id: &str) -> Result<(), MetadataStoreError>`.
- **MIRROR**: SERVICE_PATTERN (`services/health/mod.rs:37-41`), THIN_COMMAND (`commands/health/batch.rs`), TAGGED_ENUM (`protondb/suggestions.rs:73-78`), BRIDGE_LOOP (`startup_bridge.rs:122-129`), LOCK_RECOVERY above.
- **IMPORTS**: `crosshook_core::{metadata::MetadataStore, profile::ProfileStore, settings::SettingsStore}`, `crate::events::{AppEvent, EventBus}`, `std::sync::{Arc, Mutex}`.
- **GOTCHA**: `run_blocking` is private to `services` — call it as `super::run_blocking` from `services/profiles/*` exactly like `services/health/mod.rs:11`; never capture an `App`/`BootstrappedApp` clone in the service (README shutdown rule); `ProfileChangeReason` must stay `Eq` (strings only) or `AppEvent` derives break.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app` (new events/bridge/wire tests); `cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app -p crosshook-native --all-targets -- -- -D warnings`; `cd src/crosshook-native && npm run typecheck && npm test`.

### Task 1.1: Core extractions (D8) + health invalidation (D5) — Depends on [0.2]

- **BATCH**: B1
- **ACTION**: Extract the domain mutation rules into `core/src/profile/mutations/` so the writer can mutate-then-persist-once, add `MetadataStore::invalidate_health_snapshot`, and make the existing store methods delegate (no behavior change).
- **IMPLEMENT**: Move the pure bodies of `save_launch_optimizations` (preset switch validation, id filtering, active-preset mirroring, extra-table carry), `save_command_arguments` (trim, token validation via `validate_profile_command_arguments`, 64-token/512-byte limits), `materialize_launch_optimization_preset` and the manual-preset name rule into `GameProfile` methods per the frozen signatures; rewrite the `ProfileStore` methods as `load → apply_* → save` so existing callers and core tests keep passing unchanged. Factor the storage-serialize + `write_atomic` portion of `save` into `save_without_companion` (MangoHud companion write stays only in `save`), and expose `unique_copy_name` wrapping the existing generator + `list()`. Add `invalidate_health_snapshot(profile_id)` to `health_ops.rs` as a `with_conn` DELETE by `profile_id` (fail-soft at call sites; no schema change), with SQL in `health_store.rs` and tests inline in `health_ops.rs` (present row removed, missing row no-op, disabled store no-op).
- **MIRROR**: SERVICE_PATTERN structure (`services/health/mod.rs` — thin public fn over a focused impl module); TEST_HARNESS (`health/tests.rs:3-7`).
- **IMPORTS**: `crate::profile::models::GameProfile`, `crate::profile::toml_store::ProfileStore`, `crate::metadata::health_store`.
- **GOTCHA**: Do NOT change error variants, ordering, or TOML bytes — `toml_store/tests/{load_save,optimizations,command_arguments,manual_presets,duplicate}.rs` must pass unmodified except where they pin the new delegation; `save` must still write the companion after `save_without_companion` succeeds.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` (existing suites green); golden risk zero (no `save` byte path touched beyond factoring).

### Task 1.2: Lifecycle, reads and favorites slice — Depends on [0.2]

- **BATCH**: B1
- **ACTION**: Implement `ProfileWriter` lifecycle/read/favorite facades and convert their Tauri commands to async delegates.
- **IMPLEMENT**: In `services/profiles/lifecycle.rs` implement `save` (steam-app-id validation, `clear_extra` + `preserve_extra_from`, new-profile creation defaults + default bundled preset via `defaults_art.rs`, three fail-soft art imports, persist, observe `AppWrite`, revision `ManualSave`, health invalidate, publish `Saved`), `delete` (best-effort cleanup via `launcher_cleanup.rs`, `store.delete`, resolve the profile id before soft-delete observe, publish `Deleted`), `duplicate` (`lookup_profile_id` fail-open, `store.duplicate`, observe `AppDuplicate` + `source_profile_id`, revision `ManualSave`, invalidate the new id, publish `Duplicated { names: [copy] }`), `rename` (load old, `store.rename`, `observe_profile_rename`, cleanup on the old name → `had_launcher`, display-name rewrite save + `ManualSave` revision, `SettingsStore::update` for `last_used_profile`, publish `Renamed { from: old } names:[new]`; same-name rename publishes nothing), `import_legacy` (stem-based, observe `Import`, revision `Import`, publish `Imported`), `export_toml` (`validate_name` first per D12, then `profile_to_shareable_toml_with_options`). In `reads.rs` implement `list`, `load` (raw, keeps disk extras; shell applies collection defaults), `list_summaries` (reuse the moved `ProfileSummary` mapping), `list_favorites`, `set_favorite` (`validate_name`, `Unavailable` when metadata down — D6, SQLite-only, no TOML/revision/health, publish `FavoriteToggled`). Convert `commands/profile/{lifecycle,favorites}.rs` to async delegates (`State<'_, ProfileWriter>`; `profile_load`/`profile_list_summaries` keep `State<'_, MetadataStore>` for the collection branch) and migrate `tests/lifecycle.rs` cleanup tests into `services/profiles/tests/lifecycle.rs` using the harness (native skip, fail-soft cleanup, `had_launcher`, display name, last-used setting, companion handling, no duplicate event). Add `tests/events.rs`: favorite publishes exactly one `FavoriteToggled` with concrete names and touches no TOML/revision/health row (enabled + disabled metadata variants); rename publishes `Renamed { from }`.
- **MIRROR**: SERVICE_PATTERN, THIN_COMMAND, TEST_HARNESS; `SettingsStore::update` for the focused last-used change (`crates/crosshook-core/src/settings/store.rs:136`).
- **IMPORTS**: `crosshook_core::profile::{GameProfile, ProfileStore}`, `crosshook_core::metadata::{MetadataStore, SyncSource, ConfigRevisionSource}`, `super::{types::ProfileMutation, writer, revisions, launcher_cleanup, defaults_art}`.
- **GOTCHA**: Response profiles are `clear_extra()`'d only after persistence/effects; duplicate keeps source extras on disk; `is_new` derives from `profile_exists`, never frontend state; D6 warnings ride the commit result, IPC shapes stay `()`/`bool`/`GameProfile`/`DuplicateProfileResult`.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::profiles` (slice tests); main-agent full-suite gate after B1 lands.

### Task 1.3: Sections and presets slice — Depends on [0.2, 1.1]

- **BATCH**: B1
- **ACTION**: Implement optimization/overlay/argument/preset facades and convert their commands.
- **IMPLEMENT**: In `sections.rs` implement `save_launch_optimizations` (blank-name guard, `load → apply_launch_optimizations → save`, post-save reload gate for observe `AppWrite` + revision `LaunchOptimizationSave`, publish `Saved`), `save_mangohud_config` / `save_gamescope_config` / `save_trainer_gamescope_config` (load → clone → mutate section → `clear_extra` + `preserve_extra_from` → save → observe → revision `ManualSave` → publish `Saved`; name not trimmed, matching today), and `save_command_arguments` (store-rejection warn log, `apply_command_arguments`, reload-gated observe + `LaunchCommandArgumentsSave`, publish `Saved`). In `presets.rs` implement `list_bundled_optimization_presets` (`Unavailable` when metadata down, DTO mapping unchanged) and `apply_bundled_optimization_preset` (metadata required, blank guards, `get_bundled_optimization_preset`, `load → materialize_launch_optimization_preset(…, true) → save`, observe, revision `PresetApply`, `upsert_profile_launch_preset_metadata(Bundled, Some(preset_id))` fail-soft, publish `PresetApplied`) and `save_manual_optimization_preset` (`apply_manual_optimization_preset`, revision `PresetApply`, origin `User`/`None` upsert only when metadata available, publish `PresetApplied`). Convert `commands/profile/{optimizations,command_arguments}.rs`; migrate the two optimization helper tests from `tests/optimizations.rs` into `services/profiles/tests/sections.rs` (section isolation, missing profile → `NotFound`), keeping `apply_collection_defaults_tests` in place.
- **MIRROR**: SERVICE_PATTERN, THIN_COMMAND; today's section-save helper semantics (`commands/profile/shared.rs:242-277`).
- **IMPORTS**: `super::types::{LaunchOptimizationsPayload, CommandArgumentsPayload, BundledOptimizationPresetDto}`, `crosshook_core::profile::mutations` helpers, `crosshook_core::metadata::ProfileLaunchPresetOrigin`.
- **GOTCHA**: Name trimming differs per command (optimizations/arguments trim; section saves do not) — preserve exactly; never call a core method that saves AND save again (D8 exists for this).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::profiles::tests::sections`; collection tests still compile in `src-tauri`.

### Task 1.4: History, diff and rollback slice — Depends on [0.2]

- **BATCH**: B1
- **ACTION**: Implement history/diff/rollback/known-good facades and convert `config_history.rs`.
- **IMPLEMENT**: In `history.rs` implement `config_history` (`Unavailable` guard, id lookup → empty list when absent, `MAX_HISTORY_LIST_LIMIT` cap, summaries omit `snapshot_toml`), `config_diff` (positive-id guards, profile-scoped `get_config_revision` for both sides, right side = other revision or `toml::to_string_pretty` of `store.load`, `ConfigDiffMode::parse`, `compute_unified_diff` + `cap_diff_output_bytes`, semantic mode with parse-failure fallback forcing `mode: Unified` and OR-ed `truncated`), `config_rollback` (availability + positive-id + ownership, **`verify_config_revision` before any write** — SHA-256 + HMAC, parse snapshot, `store.save`, observe `AppWrite`, revision `RollbackApply` with `source_revision_id: Some(id)`, health invalidate, publish `RolledBack`; deduped insert ⇒ `new_revision_id: None`, not a warning), `mark_known_good` (`Unavailable` guard, `set_known_good_revision` with the `Corrupt` → not-found remap; no event, no TOML). Response DTOs moved to `history_types.rs` unchanged. Convert `commands/profile/config_history.rs` to async delegates. Tests in `tests/history.rs`: observe-before-capture stable id, same-hash dedupe (1 row after two identical commits), retention fallback when settings unreadable, foreign revision id rejection, tampered-signature rollback leaves disk unchanged, semantic fallback + 2000-line/512 KiB caps via large inputs, lineage parent survives pruning.
- **MIRROR**: SERVICE_PATTERN, THIN_COMMAND; today's ordering in `commands/profile/config_history.rs:268-331`.
- **IMPORTS**: `crosshook_core::metadata::{ConfigRevisionSource, MAX_HISTORY_LIST_LIMIT}`, `crosshook_core::profile::{compute_unified_diff, cap_diff_output_bytes, compute_semantic_diff}`, `super::{history_types, revisions}`.
- **GOTCHA**: Snapshot hashing serializes the supplied `GameProfile` (NOT `storage_profile()`) — keep both contracts distinct; rollback restores the verified snapshot wholesale (no `clear_extra`/`preserve_extra` filtering).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::profiles::tests::history`; core `config_history_integration` untouched and green.

### Task 1.5: ProtonDB, migration and Lutris slice — Depends on [0.2, 1.1]

- **BATCH**: B1
- **ACTION**: Implement suggestion/migration/import facades, extract `prepare_profile_migration` in core, and convert the three command files.
- **IMPLEMENT**: In core `migration/apply.rs`: split load-independent preparation (regular-file check, old-path-valid `AlreadyValid` short-circuit, field mutation) into `prepare_profile_migration(profile, request) -> MigrationApplyResult`; `apply_single_migration` becomes load → prepare → `save_without_companion` (preserving the deliberate no-MangoHud-companion policy); extend `migration/tests.rs`. In `suggestions.rs` implement `accept_protondb_suggestion`: Catalog branch (trim guards, `global_catalog().find_by_id`, re-validate **every** env pair with `validate_env_suggestion` before load, append-if-missing, persist, observe + revision `ProtonDbSuggestionApply`, publish `Saved`) and EnvVar branch (write-time re-validation, `custom_env_vars` insert last-write-wins, same effects); result shapes unchanged. In `migration.rs` implement `apply_proton_migration` (TOCTOU `try_exists` recheck — keep today's raw-path error strings, single: only `Applied` observes `AppMigration` + health invalidate (D5 DELETE, replacing today's inline recompute) + publishes `Migrated`; sanitize returned paths) and `apply_batch_migration` (preflight all paths + loads before any write, per-item isolation, counts, one `Migrated` event carrying only applied names). In `imports.rs` implement `record_imports(Vec<ImportedProfileWrite>)` running shared post-persistence effects (observe `LutrisImport`, revision `Import`, health invalidate) with reason `Imported` and concrete names (D10). Convert `commands/{protondb,migration,lutris}.rs`: accept/apply commands delegate; `lutris_import_profiles` maps `Imported` results into `ImportedProfileWrite { name, profile: entry.mapped, path }` and calls `record_imports`; delete `observe_suggestion_apply`; `protondb_dismiss_suggestion` and `check_proton_migrations` stay as-is. Tests in `tests/{suggestions,migration}.rs`: unsafe catalog pair / unsafe raw env rejected with no disk/event effects; preflight rejection ⇒ zero writes + no events; mixed applied/skipped/failed accounting; applied-only names; no companion rewrite.
- **MIRROR**: SERVICE_PATTERN, THIN_COMMAND; today's batch preflight (`commands/migration.rs:162-187`).
- **IMPORTS**: `crosshook_core::protondb::{validate_env_suggestion, AcceptSuggestionRequest, AcceptSuggestionResult}`, `crosshook_core::launch::catalog::global_catalog`, `crosshook_core::profile::migration::{prepare_profile_migration, ApplyMigrationRequest, BatchMigrationRequest, MigrationApplyResult, MigrationOutcome}`, `crate::presenter::sanitize_display_path`.
- **GOTCHA**: Never route migration through `ProfileStore::save` (companion rewrite = behavior change); `AlreadyValid`/`Failed` never observe/invalidate/publish; community import stays untouched.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core profile::migration` and `-p crosshook-app services::profiles::tests::migration`.

### Task 1.6: Frontend listeners + health dashboard favorite overlay — Depends on [0.2]

- **BATCH**: B1
- **ACTION**: Switch the five listeners to the typed payload with per-reason policy and update the Health Dashboard favorite presentation without health IPC.
- **IMPLEMENT**: All five hooks subscribe via `subscribeEvent<ProfilesChangedPayload | null>` and run the payload through `parseProfilesChanged` (null ⇒ conservative resync). `useProfileHealth.ts`: skip `batchValidate()` when `profileChangeAffectsHealth` is false (favorite only); other reasons keep the existing batch path; launch/version listeners unchanged. `useProfile.ts`: `favorite_toggled` ⇒ `loadFavorites()` only; others ⇒ `refreshProfiles()` + `loadFavorites()`; never reload editor drafts off the event. `useProfileSummaries.ts` / `useLibrarySummaries.ts`: favorite ⇒ return before `fetchSummaries()` (the `favoriteProfiles` effect already patches cards). `useCommunityProfiles.ts`: favorite ⇒ skip `refreshImportedProfileNames()`. In `HealthDashboardPage.tsx` + `health-dashboard/useHealthDashboardState.ts`, overlay `is_favorite` from `ProfileContext.favoriteProfiles` onto the retained health summary (pure projection; no `checked_at`/status/count resets, no health command) so stars and favorite-first ordering update after a toggle on both a mounted dashboard and route re-entry. Update `MetadataStatusBanner.tsx` copy to state that profile files still save but configuration snapshots are not recorded (D6) and its test. Replace `''` payloads in `useProfileSummaries.test.ts` (also assert favorite ⇒ no summary IPC) and `LibraryPage.test.tsx`; create `useProfileHealth.profilesChanged.test.ts` copying the startup harness: settle startup first, emit `{reason:{kind:'favorite_toggled'},names:['Alpha']}` inside `act` + timer drain ⇒ `batchCalls()` length unchanged; emit `{reason:{kind:'saved'},names:['Alpha']}` ⇒ +1 (positive control); also table the malformed-payload ⇒ resync case.
- **MIRROR**: VITEST_COUNTER (`useProfileHealth.startup.test.ts:60-62`), the module-mock capture pattern (`useProfileSummaries.test.ts:11-18`).
- **IMPORTS**: `@/types/profile-events`, `@/lib/profileEvents`, `@/lib/events`, `@/lib/ipc`.
- **GOTCHA**: A negative assertion needs a drain (`vi.runAllTimersAsync`) or it passes vacuously; hooks keep the `unlisten.then(fn => fn())` cleanup shape; no raw `invoke()`/`listen()`.
- **VALIDATE**: `cd src/crosshook-native && npm test && npm run typecheck`.

### Task 1.7: Browser mock producers — Depends on [0.2]

- **BATCH**: B1
- **ACTION**: Replace every `{ name, action }` emission with the real object payload through one typed helper.
- **IMPLEMENT**: Add `emitProfilesChanged(payload: ProfilesChangedPayload)` to `profile-utils.ts` importing the shared type (no second schema). Update all 14 sites in `profile-mutations.ts` (`saved` ×sections/arguments/optimizations, `deleted`, `duplicated` with the copy name, `renamed` with `from` + new name, `favorite_toggled`, plus import/export variants ⇒ `imported`/`saved`), 3 in `profile-presets.ts` (`preset_applied`), 2 in `profile-history.ts` (`rolled_back`), and add emissions after successful state mutation in `protondb.ts` (`saved` on accept) and `proton.ts` (`migrated`, applied-only) and `lutris.ts` (`imported`, imported-only) — mirroring the new backend semantics; failure/no-op paths emit nothing. Mock `profile_rename` starts returning a realistic `hadLauncher` boolean instead of constant `false`.
- **MIRROR**: THIN_COMMAND spirit (one seam); existing `emitMockEvent` usage (`profile-mutations.ts:29`).
- **IMPORTS**: `emitMockEvent` from `../eventBus`, `ProfilesChangedPayload` from `../../../types/profile-events`.
- **GOTCHA**: Mock error strings keep the `[dev-mock]` prefix (release `verify:no-mocks` sentinel); do not edit hooks/tests (1.6 owns them); `emitMockEvent` is a no-op under Tauri by design.
- **VALIDATE**: `cd src/crosshook-native && npm test && npm run test:smoke`.

### Task 2.1: Clean cutover — helper deletion, IPC contract, goldens, docs — Depends on [1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7]

- **BATCH**: B2
- **ACTION**: Delete the moved shell orchestration, pin the IPC contract, land the golden byte-comparison test, and update docs.
- **IMPLEMENT**: In `commands/profile/shared.rs` delete `emit_profiles_changed`, `capture_config_revision`, `resolve_config_history_max_revisions`, `observe_profile_write_launch_change`, `save_profile_section`, the cleanup/path-derivation helpers and `map_error`, keeping only `apply_collection_defaults` (+ minimal glue); in `commands/profile/mod.rs` drop the `pub(crate) use shared::emit_profiles_changed` and `pub use shared::{capture_config_revision, resolve_config_history_max_revisions}` re-exports and the now-dead local DTO definitions; verify no remaining importer (`grep -rn "capture_config_revision\|emit_profiles_changed\|save_profile_section" src-tauri/src`). Add `commands/profile/tests/contract.rs` pinning all 23 async exported command names (async fns can't cast to fn pointers — mirror `commands/health/tests.rs`). Create `crates/crosshook-app/tests/profile_writer_golden.rs` asserting `fs::read` bytes == `include_bytes!` of the four B0 fixtures through the new writer (`save`, `rename`, `duplicate`, `config_rollback`), with a lossy-UTF-8 failure message. Update `crates/crosshook-app/README.md` (services table row for `ProfileWriter`, event section) and `src/lib/mocks/README.md` (event shape + parity). Confirm `CLAUDE.md`/`AGENTS.md` need no rule change (they don't). Then run the full acceptance gates below.
- **MIRROR**: GOLDEN_FIXTURE (`launch_request_golden.rs:38-46`), THIN_COMMAND contract test (`commands/health/tests.rs:3-10`).
- **IMPORTS**: `std::fs`, `crosshook_app::services::profiles::ProfileWriter`.
- **GOTCHA**: Never regenerate fixture bytes from the new writer; keep `apply_collection_defaults` and its tests in the shell (M1-35); startup-path/schema-version guard scripts must stay green.
- **VALIDATE**: Full gate — all commands in Validation Commands, plus `./scripts/lint.sh` and `./scripts/check-schema-version.sh`; manual `./scripts/dev-native.sh` re-verification of real Tauri event payload and rename re-export before merge.

## Testing Strategy

### Unit Tests

| Test                                                           | Input                                                               | Expected Output                                                                             | Edge Case?                |
| -------------------------------------------------------------- | ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------- |
| Reason wire shapes (app `events/tests.rs`)                     | each `ProfileChangeReason`                                          | exact `serde_json::to_value` objects incl. `{"kind":"renamed","from":…}`                    | yes — tag collision guard |
| Bridge emission (`startup_bridge/tests.rs`)                    | `ProfilesChanged`, `Resync`                                         | object payload emitted; resync ⇒ `{kind:"saved"},names:[]` + startup replay                 | yes                       |
| Favorite purity (`tests/events.rs`)                            | `set_favorite` enabled+disabled metadata                            | exactly one `FavoriteToggled`; no TOML/revision/health row change; disabled ⇒ `Unavailable` | yes                       |
| Golden bytes (`tests/profile_writer_golden.rs`)                | writer save/rename/duplicate/rollback                               | bytes == pre-refactor fixtures                                                              | yes                       |
| Dedupe/lineage (`tests/history.rs`)                            | two identical commits; rollback to older; foreign id; tampered HMAC | 1 row; lineage row retained; scoped rejection; disk unchanged on tamper                     | yes                       |
| Diff caps (`tests/history.rs`)                                 | >2000-line sides, >512 KiB diff, invalid TOML semantic              | `truncated` true, byte cap respected, unified fallback + `semantic_parse_failed`            | yes                       |
| Degraded metadata (`tests/lifecycle.rs`, `tests/sections.rs`)  | `MetadataStore::disabled()`                                         | save succeeds, revision skipped with warning surfaced, banner test renders                  | yes                       |
| Launcher cleanup (`tests/lifecycle.rs`)                        | native / watermarked / adversarial names / failing cleanup          | skip, `had_launcher` bool, deletion confined to launcher dirs, primary op succeeds          | yes                       |
| Migration (`core` + `tests/migration.rs`)                      | preflight miss; mixed outcomes; already-valid                       | zero writes; isolated counts; no event/no companion rewrite                                 | yes                       |
| Extraction parity (`core toml_store/tests/*`)                  | same payloads as before                                             | identical behavior/bytes via delegated methods                                              | no                        |
| TS parser (`profileEvents.test.ts`)                            | valid/unknown/malformed payloads                                    | typed payload / null resync; health predicate table                                         | yes                       |
| Listener counters (`useProfileHealth.profilesChanged.test.ts`) | favorite vs saved vs malformed                                      | 0 / +1 / +1 `batch_validate_profiles` calls                                                 | yes                       |
| Mock parity                                                    | every changed mock handler                                          | emits exact object payload; no-op paths silent                                              | yes                       |

### Edge Cases Checklist

- [x] Empty/blank names (guards per command, trimming parity)
- [x] Maximum sizes (512 KiB diff cap, 256 KiB snapshot cap, 64 tokens/512 B args, 1000 copy-name cap)
- [x] Invalid types (malformed event payload ⇒ resync; invalid TOML snapshot ⇒ unified fallback / rollback abort)
- [x] Concurrent access (D3 writer lock; documented last-completed-write-wins for external writers)
- [x] Network failure — n/a (no network in commit)
- [x] Permission denied / read-only metadata (degraded saves, `Unavailable` where required)
- [x] Bus lag (`Resync` sentinel), dropped caller future (publish in closure), no-op rename, migration `AlreadyValid`

## Validation Commands

### Static Analysis

```bash
cargo clippy --manifest-path src/crosshook-native/Cargo.toml \
  -p crosshook-app -p crosshook-core -p crosshook-native --all-targets -- -- -D warnings
```

EXPECT: Zero warnings (src-tauri package name is `crosshook-native` per its `Cargo.toml`).

### Unit Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native
```

EXPECT: All pass (tempdir stores, metadata enabled + disabled, goldens, dedupe/lineage/diff/launcher/migration).

### Full Test Suite

```bash
./scripts/lint.sh
./scripts/check-schema-version.sh
```

EXPECT: No lint/guard failures; schema stays 28.

### Frontend

```bash
cd src/crosshook-native && npm run typecheck && npm test && npm run test:smoke && npm run lint
```

EXPECT: Typecheck clean (both tsconfigs), Vitest green (listener/parser/banner/mock tests), Playwright smoke green, Biome clean.

### Manual Validation

- [ ] `./scripts/dev-native.sh` (real Tauri): toggle a favorite ⇒ no health "Checking…" pill, dashboard star/ordering updates; rename with a launcher ⇒ re-export banner appears and exports; rollback round-trips; metadata-disabled banner copy shows snapshots skipped.

## Acceptance Criteria

- [ ] All 10 tasks completed across B0→B1→B2 (ownership table respected).
- [ ] **Golden TOML**: `tests/profile_writer_golden.rs` proves byte-identical save/rename/duplicate/rollback vs D15 fixtures (Task 2.1; fixtures from 0.1).
- [ ] **Dedupe/diff preserved**: `tests/history.rs` covers SHA-256 latest-row dedupe, 2000-line/512 KiB/500-change caps, semantic fallback (Task 1.4).
- [ ] **Favorite ⇒ no batch**: Rust favorite-purity test (Task 1.2) + Vitest counter with `saved` positive control (Task 1.6).
- [ ] **Rename/delete keep cleanup/re-export**: `tests/lifecycle.rs` + `had_launcher` bool preserved end-to-end (Task 1.2).
- [ ] **Delegation + smoke**: all commands async delegates (Tasks 1.2–1.5), `npm run test:smoke` green with typed mocks (Task 1.7); IPC contract test pins names (Task 2.1).
- [ ] All validation commands pass; no type/lint errors.

## Completion Checklist

- [x] Code follows discovered patterns (service/thin-command/tagged-enum/bridge/harness mirrored)
- [x] Error handling matches codebase style (`AppError`/`AppErrorKind`, sanitized messages)
- [x] Logging follows codebase conventions (`tracing::warn!` fail-soft, no paths)
- [x] Tests follow test patterns (tempdir harness, golden bytes, Vitest counters with controls)
- [x] No hardcoded values (limits stay in core constants)
- [x] Documentation updated (app README, mocks README; CLAUDE/AGENTS intentionally unchanged)
- [x] No unnecessary scope additions (NOT Building enforced)
- [x] Self-contained — frozen signatures + mandatory reading make slices implementable without questions

## Risks

| Risk                                                                                                 | Likelihood | Impact | Mitigation                                                                                              |
| ---------------------------------------------------------------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------------------------- |
| TOML byte drift (wrong serializer, extras lost, defaults on existing profiles, rollback re-filtered) | Medium     | High   | single serializer via core; D15 goldens; rollback restores wholesale; revision hash asserted separately |
| Event contract breakage (serde mismatch, bridge wildcard, dual emission)                             | Medium     | High   | wire tests all 9 reasons both sides; D14 arm + sentinel; all string producers deleted in-PR             |
| Favorite still triggers batch                                                                        | Medium     | High   | SQLite-only facade; Vitest counter + saved control; Rust purity test                                    |
| Mock/real payload divergence                                                                         | High       | Medium | one typed publisher; smoke exercises parser; parity documented                                          |
| Revision/rollback integrity regression                                                               | Low        | High   | core SQL/HMAC untouched; dedupe/foreign/tamper/lineage tests                                            |
| Migration drift (companion rewrite, preflight bypass)                                                | Low        | High   | `save_without_companion` only; preflight order tests                                                    |
| Shared-file collisions in B1                                                                         | Medium     | Medium | exclusive ownership (audited); B0 pre-declares module tree; frozen signatures                           |
| Concurrent write reordering after async move                                                         | Medium     | Medium | D3 lock inside closure; publish-in-closure (D4)                                                         |
| Coverage silently dropped with helper deletion                                                       | Medium     | Medium | tests migrate with slices; collection tests stay; B2 greps for dead imports                             |
| Rename event-before-response auto-selection race                                                     | Medium     | Medium | `useProfileCrud` keeps explicit post-command selection; listeners never auto-select on `Renamed`        |
| Schema/version accidental bump                                                                       | Low        | High   | `check-schema-version.sh` in gates; no migration files touched                                          |

## Notes

- **Decisions D1–D15 are binding** (`decisions.md`); the spec (`feature-spec.md`) maps them to behavior, storage classification and severity-ranked risks.
- **Storage boundary**: TOML byte-compatible; SQLite unchanged (schema 28; D5 is a DELETE over the existing table); events/warnings runtime-only. No migration; offline unchanged; users edit exactly as before.
- **Parallel model**: B0 sequential (one owner) → B1 seven exclusive slices → B2 single integration owner; main agent compiles/verifies once after B1 and again after B2 (subagents skip builds mid-flight). Internal batches may be temporarily incomplete; the final branch is fully cut over — no shims, no dual paths.
- **Favorite skip scope**: only `favorite_toggled` (UX research: other reasons can affect name-keyed/enriched health; conservative default).
- **Security**: D12 lands `validate_name` before export serialization (W-2) and favorite-name validation (A-5); everything else in the security report becomes Linear follow-ups — the refactor is behavior-preserving elsewhere.
- Trunk-only: single PR off `main`, squash merge, Conventional Commit title (`refactor(profiles): move profile mutation, revision and rollback into crosshook-app`).
