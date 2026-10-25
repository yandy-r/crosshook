# Feature Spec: ProfileWriter — profile mutation, revision and rollback in `crosshook-app`

Tracking: Linear YAN-811 / GitHub #532 (M1-06). Target release `crosshook v0.7.0-alpha.1`, trunk-only off `main`.
Paths are relative to `src/crosshook-native/` unless prefixed `docs/`. Authoritative acceptance criteria: [issue.md](./issue.md); binding decisions D1–D15: [decisions.md](./decisions.md).

## Executive Summary

Every profile write today repeats validate → load → mutate → save → observe → revision → emit, spread across 23 sync Tauri commands plus ProtonDB accept and Proton migration. This feature moves that orchestration into one async `ProfileWriter::commit` pipeline in `crates/crosshook-app/src/services/profiles/`, adds a typed `AppEvent::ProfilesChanged { reason, names }` object payload bridged to `profiles-changed`, and makes favorite toggles stop triggering `batch_validate_profiles`. TOML bytes, SQLite schema and IPC shapes stay byte-identical; byte compatibility is proven by pre-refactor golden fixtures. Main risks are event-contract breakage across five listeners and mocks, and silent drift of revision/rollback semantics.

## External Dependencies

### APIs and Services

No external service APIs are introduced. The refactor uses only in-workspace crates and the existing bounded `EventBus`; commit performs no network I/O (offline behaviour unchanged).

#### Tauri v2 event emission (internal contract)

- **Documentation**: https://v2.tauri.app/develop/calling-frontend/ and docs.rs `tauri::Emitter` (2.11.2, pinned in `Cargo.lock`)
- **Authentication**: none (in-process webview events)
- **Key surface**: `Emitter::emit(event, payload)` with any `Serialize + Clone` payload; JS `listen<T>` receives `event.payload` already JSON-parsed
- **Rate limits**: none applicable
- **Pricing**: n/a

### Libraries and SDKs

All already in the workspace; **no new Rust or npm dependencies** (verified against `Cargo.lock` and `package.json`).

| Library                                           | Version | Purpose                                                                                               | Installation                           |
| ------------------------------------------------- | ------- | ----------------------------------------------------------------------------------------------------- | -------------------------------------- |
| `tokio`                                           | 1.51.0  | `spawn_blocking` commit pipeline, broadcast bus, `#[tokio::test]`                                     | present (`crosshook-app` deps)         |
| `serde` + derive                                  | 1       | `#[serde(tag = "kind", rename_all = "snake_case")]` on `ProfileChangeReason`                          | present                                |
| `serde_json`                                      | 1.0.149 | wire-shape assertions; promoted from dev-dep to dep in `crosshook-app`                                | edit `crates/crosshook-app/Cargo.toml` |
| `toml`                                            | 1.1.x   | revision snapshot serialization (`to_string_pretty`); promoted from dev-dep to dep in `crosshook-app` | edit `crates/crosshook-app/Cargo.toml` |
| `sha2` via `crosshook_core::metadata::sha256_hex` | —       | revision content hashing; do **not** add `sha2` to `crosshook-app`                                    | re-export exists                       |
| `tempfile`                                        | 3 (dev) | tempdir `ProfileStore` + in-memory/disabled `MetadataStore` tests                                     | present                                |
| `vitest`                                          | ^4.1.12 | listener call-counter tests                                                                           | present                                |
| `@playwright/test`                                | ^1.60.0 | `npm run test:smoke` against browser mocks                                                            | present                                |

Considered and rejected: `insta`, `expect-test`, `goldenfile` (not in `Cargo.lock`; the repo's golden convention is plain fixture files + byte equality), `mockall`/`tokio-test` (call counters are satisfied on both sides without them).

### External Documentation

- tokio `spawn_blocking` (https://docs.rs/tokio/1.51.0/tokio/task/fn.spawn_blocking.html): non-abortable once started — the event must be published inside the blocking closure.
- tokio `sync::broadcast` (https://docs.rs/tokio/1.51.0/tokio/sync/broadcast/index.html): drop-oldest lag semantics; lag maps to `AppEvent::Resync`.
- serde enum representations (https://serde.rs/enum-representations.html): internally tagged enums reject tuple variants; `Renamed { from }` is a struct variant.
- Tauri v2 events (https://v2.tauri.app/develop/calling-frontend/): payload JSON rules, `unlisten` promise pitfall, listener ordering caveat.

## Business Requirements

### User Stories

**Primary user: CrossHook player**

- As a player, I want favorite toggles to stay instant, so the UI does not rescan profile health for a change that cannot affect it.
- As a player, I want saves, renames, duplicates and rollbacks to behave exactly as before, so my TOML profiles, history and launcher re-export flow stay intact.
- As a player with metadata unavailable, I want saves to keep succeeding with a visible explanation that snapshots are skipped, so I am never blocked or misled.

**Secondary users: maintainers**

- As a native-UI developer, I want one profile write pipeline in `crosshook-app`, so the iced UI reuses it without duplicating Tauri command logic.
- As a shell maintainer, I want `src-tauri` profile commands to be thin delegates, so business logic lives in one place.

### Business Rules

1. **One commit pipeline (D2)**: every mutating facade funnels through one private `commit` (validate → load → mutate → persist once → observe → revision → health invalidate → publish). The facade picks the reason; callers cannot mislabel provenance.
2. **Write serialization (D3)**: `ProfileWriter` holds an `Arc<std::sync::Mutex<()>>`; every mutating closure takes it inside `run_blocking` (poison recovered via `into_inner`). Reads do not take it. This preserves today's main-thread-serialized ordering now that commands are async.
3. **Publish inside the closure (D4)**: the event is published inside the blocking closure after primary persistence succeeds, even if advisory metadata effects fail. No event for validation/save failure, a no-op rename, or migration `AlreadyValid`/`Failed`.
4. **Favorite is metadata-only (D5)**: `FavoriteToggled` skips TOML save, revision and health entirely; health "invalidation" for other reasons is a new parameterized `MetadataStore::invalidate_health_snapshot(profile_id)` (DELETE of the cached row, fail-soft, no schema change); no validation runs inside commit.
5. **Reason/names mapping (D1)**: `Saved`, `Deleted`, `Renamed { from }`, `Duplicated`, `FavoriteToggled`, `RolledBack`, `PresetApplied`, `Imported`, `Migrated`. Rename: `names: [new]`, `reason.from: old`. Delete: `names: [removed]`. Duplicate: `names: [copy]`. `names: []` means only "conservative resync" (bus lag). No legacy string form, no dual emission.
6. **Revision semantics preserved (D9)**: snapshot stays `toml::to_string_pretty(&profile)` then `sha256_hex`; latest-row dedupe, retention fallback (`MAX_CONFIG_REVISIONS_PER_PROFILE` = 20, clamp 5..=100), 256 KiB snapshot cap; rollback verifies SHA-256 + HMAC via `verify_config_revision` before any write and records lineage (`source_revision_id`). Duplicate and rename now capture revisions and emit events (new, issue-mandated).
7. **Migration keeps its write policy (D8)**: `apply_single_migration` deliberately does not rewrite the MangoHud companion; extraction keeps `save_without_companion` for it. Batch preflight validates all paths/loads before any write; per-item failures stay isolated.
8. **Launcher cleanup and re-export (preserved)**: rename/delete keep best-effort watermarked-file cleanup with the native-method skip; rename still returns the `had_launcher` bool that drives the frontend re-export banner. Cleanup failure never fails the primary operation.
9. **Degraded metadata (D6)**: TOML saves succeed; observe/revision/health steps are skipped with `tracing::warn!` plus typed warnings in the commit result; the classic UI surfaces the state through the existing `MetadataStatusBanner` (`useMetadataStoreStatus`). Metadata-required operations (history, diff, rollback, known-good, bundled preset lookup, favorite) return explicit `Unavailable` errors instead of fake success.
10. **Collections stay out (D7)**: `apply_collection_defaults` and the collection branch of summaries remain in `src-tauri` for M1-35; `profile_load` calls `ProfileWriter::load`, then the shell helper.
11. **Errors (D11)**: reuse `AppError`/`AppErrorKind`; Tauri maps `error.to_string()`. No new error type.
12. **Security hardening in moved code only (D12)**: `profile_export_toml` validates the name (`validate_name`) before interpolating it into the TOML header (closes header injection, W-2). Other security findings become Linear follow-ups; no behaviour change.
13. **Blocking + async (issue)**: blocking store and diff work runs in `spawn_blocking` inside the service; every public fn is async; `new` stays sync (constructor exception).
14. **Imports under the writer lock (D10)**: `ProfileWriter::import_lutris(Vec<LutrisImportEntry>)` and `ProfileWriter::import_community(PathBuf)` run the core importers inside the commit, under the write lock, so unique-name derivation and TOML writes cannot interleave with other profile writes. Each queues the shared effects (observe, `Import` revision, health invalidation; community also seeds its display-only version snapshot) and publishes one `Imported` event with the written names. Lutris validates and trims each `suggested_name` (an invalid entry is reported `Failed` without failing the import) and publishes nothing when nothing was written. The Tauri helpers are deleted.

### Edge Cases

| Scenario                                                          | Expected Behavior                                                                                         | Notes                                              |
| ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| Same-name rename                                                  | Store no-op `Ok(())`; observe/cleanup/display-name steps still run; **no event**                          | D4: no event for no-op rename                      |
| Duplicate of missing source                                       | `NotFound`; nothing written                                                                               | `ProfileStore::duplicate` semantics                |
| Rollback to snapshot whose hash equals the latest                 | File restored, event emitted, `new_revision_id: None` (dedupe)                                            | Absent id is not an error; never warn for dedupe   |
| Rollback with tampered snapshot / HMAC mismatch                   | Fails before any disk write (`IntegrityCheckFailed`)                                                      | A-2; disk unchanged                                |
| Section/optimization/command-arg save when post-save reload fails | TOML already written; observe/revision skipped; command still `Ok`                                        | Preserve today's fail-soft order                   |
| Favorite of an unknown or disabled-metadata profile               | Explicit `Unavailable` error (D6), not silent success                                                     | Today it silently succeeds and writes nothing      |
| Migration `AlreadyValid` / `Failed`                               | No observe, no health step, no event                                                                      | Batch counts preserved; applied-only names emitted |
| Batch migration preflight failure                                 | `Err`, zero writes, no events                                                                             | W-4 all-before-any-write                           |
| Bus lag (receiver behind)                                         | Bridge re-emits startup jobs **and** emits `{reason:{kind:"saved"},names:[]}` (D14)                       | Conservative resync; never masquerades as favorite |
| Malformed `profiles-changed` payload in TS                        | `parseProfilesChanged` returns resync semantics; health revalidates                                       | D13 safe default                                   |
| New-profile save with settings load failure                       | Save fails (existing rule); creation defaults never clobber non-empty fields or an explicit wizard preset | `creation_defaults.rs`                             |
| Custom-art import failure                                         | Original path kept, warn; save proceeds                                                                   | Three independent fail-soft paths                  |
| Legacy import overwrites an existing `{stem}.toml`                | Preserved (pre-existing behavior); revision uses the stem                                                 | A-7 recorded as known edge                         |
| `profile_export_toml` with newline in name                        | `InvalidName` validation error                                                                            | D12 / W-2 fix                                      |

### Success Criteria

- [ ] Golden-file tests show save, rename, duplicate and rollback produce byte-identical TOML compared with pre-refactor fixtures (D15).
- [ ] Revision SHA-256 dedupe, diff output cap (2000 lines/side, 512 KiB, 500 semantic changes, semantic fallback) preserved and tested through the service.
- [ ] A favorite toggle triggers no `batch_validate_profiles` — asserted by a call counter in Rust tests and by a Vitest listener test with a `saved` positive control.
- [ ] Rename and delete keep launcher cleanup and re-export behaviour, with tests.
- [ ] Tauri profile commands delegate to the service; React smoke passes (`npm run test:smoke`).

## Technical Specifications

### Architecture Overview

````text
React listeners (5)                src-tauri (thin)                    crosshook-app
┌──────────────────┐   invoke    ┌────────────────────┐   async     ┌──────────────────────────┐
│ useProfileHealth │ ──────────▶ │ profile_save …     │ ──────────▶ │ services/profiles/       │
│ useProfile       │             │ protondb_accept…   │             │ ProfileWriter::commit    │
│ useProfileSumm.  │             │ apply_*_migration  │             │  lock → run_blocking     │
│ useLibrarySumm.  │             │ lutris_import…     │             │  validate→load→mutate    │
│ useCommunityProf.│             └─────────┬──────────┘             │  →persist→observe→revision│
└────────▲─────────┘                       │                        │  →health→publish        │
         │ profiles-changed                │ manage(State)          └───────┬──────┬───────────┘
         │ {reason:{kind,…},names}         │                                │      │
┌────────┴─────────────────────────────────┴────────────────┐       EventBus   ProfileStore /
│ startup_bridge.rs  (single bus receiver; D14 arm)        │◀────── publish   MetadataStore /
│ AppEvent::ProfilesChanged → app.emit(object payload)     │                   SettingsStore
│ AppEvent::Resync → replay + {kind:"saved"},names:[]      │                   (SQLite/TOML)
└──────────────────────────────────────────────────────────┘
```text

### Binding decisions D1–D15

The orchestrator decisions in [decisions.md](./decisions.md) are binding for this spec and the implementation plan. Summary of their effect: D1 wire shape; D2 typed facades over one private `commit`; D3 writer-wide `Arc<Mutex<()>>`; D4 publish-in-closure; D5 fail-soft `invalidate_health_snapshot` and favorite bypass; D6 degraded metadata warnings via existing banner, `Unavailable` for metadata-required ops; D7 collections excluded; D8 core extraction (not duplication) of `apply_launch_optimizations`, `apply_command_arguments`, `materialize_launch_optimization_preset`, `apply_manual_optimization_preset`, `ProfileStore::unique_copy_name`, `ProfileStore::save_without_companion`, `profile::migration::prepare_profile_migration` with store methods delegating; D9 byte-compatible revision hashing + verified rollback; D10 `import_lutris`/`import_community` under the write lock; D11 `AppError` reuse; D12 export-name validation only; D13 shared TS type + parser + health predicate; D14 bridge arm + resync sentinel; D15 pre-refactor golden fixtures under `crates/crosshook-app/tests/fixtures/profile_writer/`.

### Data Models

#### Storage boundary and persistence classification

| Datum | Layer | Change |
| ----- | ----- | ------ |
| Game profiles (`~/.local/share/crosshook/profiles/*.toml`) | TOML via `ProfileStore::save` | byte-identical; single serializer (`storage_profile()` → `toml::to_string_pretty` → `write_atomic`) |
| `settings.toml` (creation defaults, `last_used_profile`, `config_history.max_revisions`) | TOML via `SettingsStore` | unchanged; rename's last-used update uses `SettingsStore::update` for a focused change |
| Revision rows, HMAC, lineage, known-good, favorites, preset origin, health snapshots | SQLite `MetadataStore` (schema **unchanged**, `SUPPORTED_MAX_VERSION` = 28) | add `invalidate_health_snapshot(profile_id)` (DELETE by existing PK; no schema/version bump) |
| `AppEvent::ProfilesChanged`, `ProfileChangeReason`, `names`, commit warnings | runtime only (bounded bus) | never persisted; no event log |

#### Persistence and usability

- **Migration/backward compatibility**: none required — no TOML format, SQLite schema or IPC argument/response shape changes. Existing profiles, revisions and favorites open untouched.
- **Offline expectations**: unchanged; commit introduces no network calls. ProtonDB *accept* only re-validates cached suggestion data; no lookup is added.
- **Degraded/failure fallback**: with `MetadataStore::disabled()` saves still succeed; observe/revision/health are skipped with `tracing::warn!` and typed warnings surfaced through the existing `MetadataStatusBanner`; history/diff/rollback/known-good/bundled-preset/favorite fail closed with `Unavailable`. Revision dedupe (`new_revision_id: None`) is **not** a warning. Launcher cleanup failure never fails delete/rename.
- **User visibility/editability**: users keep editing profiles in TOML or the UI exactly as before; history/diff/rollback/known-good controls are unchanged; the metadata banner copy should state that profile files still save but snapshots are not recorded.

#### Event wire contract (D1/D13/D14)

```json
{"reason":{"kind":"favorite_toggled"},"names":["Alpha"]}
{"reason":{"kind":"renamed","from":"Old"},"names":["New"]}
{"reason":{"kind":"saved"},"names":[]}
```json

`ProfileChangeReason` derives `Debug, Clone, PartialEq, Eq, Serialize` with `#[serde(tag = "kind", rename_all = "snake_case")]` (repo precedent: `AcceptSuggestionRequest`, `JobState`, `MetadataStatus`). TS mirror: discriminated union on `reason.kind` in `src/types/profile-events.ts`; `parseProfilesChanged(unknown)` treats malformed input as resync; `profileChangeAffectsHealth(reason)` is false only for `favorite_toggled`.

### API Design

#### `ProfileWriter` public surface (application crate)

Typed async facade (D2): `save`, `delete`, `duplicate`, `rename` (returns `bool` `had_launcher`), `set_favorite`, `save_launch_optimizations`, `save_mangohud_config`, `save_gamescope_config`, `save_trainer_gamescope_config`, `save_command_arguments`, `list_bundled_optimization_presets`, `apply_bundled_optimization_preset`, `save_manual_optimization_preset`, `config_history`, `config_diff`, `config_rollback`, `mark_known_good`, `accept_protondb_suggestion`, `apply_proton_migration`, `apply_batch_migration`, `import_legacy`, `import_lutris`, `import_community`, `export_toml`, `list`, `load`, `list_summaries`, `list_favorites`. Errors: `AppError`/`AppErrorKind` (D11); kinds `Validation { field }`, `NotFound`, `Conflict`, `Unavailable` per existing conversions. Reason/revision mapping: full/section save → `Saved`/`ManualSave`; optimization → `Saved`/`LaunchOptimizationSave`; arguments → `Saved`/`LaunchCommandArgumentsSave`; presets → `PresetApplied`/`PresetApply`; rollback → `RolledBack`/`RollbackApply` + lineage; legacy, Lutris and community → `Imported`/`Import`; ProtonDB accept → `Saved`/`ProtonDbSuggestionApply`; migration → `Migrated`/`Migration`; duplicate → `Duplicated` (revision `ManualSave`); rename → `Renamed { from }` (revision `ManualSave`); delete/favorite → no revision.

#### Tauri commands (unchanged names/shapes)

All 23 profile commands plus `protondb_accept_suggestion`, `apply_proton_migration`, `apply_batch_migration` and `lutris_import_profiles` become `async` delegates: `service.op(args).await.map_err(|e| e.to_string())`. `profile_load`/`profile_list_summaries` keep `State<'_, MetadataStore>` only for the excluded collection composition (D7).

### System Integration

#### Files to Create

- `crates/crosshook-app/src/services/profiles/` module tree: `mod.rs`, `types.rs`, `history_types.rs`, `writer.rs` (commit + D3 lock), `revisions.rs` (moved capture), `lifecycle.rs`, `reads.rs`, `sections.rs`, `presets.rs`, `history.rs`, `launcher_cleanup.rs`, `defaults_art.rs`, `suggestions.rs`, `migration.rs`, `imports.rs`, `tests/{mod,helpers,events,lifecycle,sections,history,migration,suggestions}.rs`
- `crates/crosshook-app/tests/fixtures/profile_writer/*.toml` + `crates/crosshook-app/tests/profile_writer_golden.rs` (D15)
- `crates/crosshook-core/src/profile/mutations/{mod,optimizations,command_arguments}.rs` (D8 extraction)
- `src/types/profile-events.ts`, `src/lib/profileEvents.ts`, `src/lib/__tests__/profileEvents.test.ts`
- `src/hooks/__tests__/useProfileHealth.profilesChanged.test.ts`
- `src-tauri/src/commands/profile/tests/contract.rs` (IPC name contract)

#### Files to Modify

- App: `events.rs` (+tests), `lib.rs` re-exports, `services/mod.rs`, `Cargo.toml` (promote `toml`, `serde_json`), `bootstrap/{stores.rs,app_bootstrap.rs,stores/tests.rs}` (`Services::new(&Stores, EventBus)`), `crates/crosshook-app/README.md`
- Core: `metadata/{health_ops.rs,health_store.rs}` (D5), `profile/mod.rs`, `profile/toml_store/store.rs` (D8 delegation + `save_without_companion` + `unique_copy_name`), `profile/migration/{apply.rs,mod.rs,tests.rs}` (`prepare_profile_migration`)
- Shell: `src-tauri/src/lib.rs` (manage writer), `startup_bridge.rs` (+tests, D14), `commands/profile/{lifecycle,favorites,optimizations,command_arguments,config_history,shared,mod}.rs`, `commands/profile/tests/{mod,lifecycle,optimizations,helpers}.rs`, `commands/{protondb,migration,lutris}.rs`
- Frontend: five listener hooks, `HealthDashboardPage.tsx` + `health-dashboard/useHealthDashboardState.ts` (favorite overlay), `src/lib/mocks/handlers/{profile-mutations,profile-presets,profile-history,protondb,proton,lutris,profile-utils}.ts`, `src/lib/mocks/README.md`, `src/types/index.ts`, affected tests; `CLAUDE.md`/`AGENTS.md` only if a rule changes.

#### Configuration

No new configuration keys. Retention still reads `config_history.max_revisions` (clamp 5..=100, fallback 20) through the moved resolver.

## UX Considerations

### User Workflows

#### Primary Workflow: Favorite toggle without health rescan

1. **Toggle** — User clicks the library star; UI applies the existing optimistic update.
2. **Persist** — `profile_set_favorite` → `set_favorite` (SQLite only; `Unavailable` when metadata is down); `FavoriteToggled` event published; `useProfile` reloads favorites only.
3. **Success State** — Stars, Favorites filter, collection cards and the Health Dashboard favorite column/ordering update from `ProfileContext.favoriteProfiles` (overlay projection) with zero `batch_validate_profiles` calls, no loading flicker, health `checked_at` untouched.

#### Error Recovery Workflow

1. **Error occurs** — e.g. metadata read-only during save: TOML persists, revision skipped.
2. **User sees** — the session-dismissible `MetadataStatusBanner` explaining that profile files still save but snapshots are not recorded; no per-autosave toast storm.
3. **Recovery** — restore metadata availability; next save records revisions again. Validation/save failures keep today's inline error surfaces; rename keeps its Undo and launcher re-export banner driven by the `had_launcher` bool.

### UI Patterns

| Component | Pattern | Notes |
| --------- | ------- | ----- |
| `MetadataStatusBanner` | existing app-level dismissible status banner | copy clarified for skipped snapshots; `role="status"`; session dismissal respected |
| Library favorite star | existing optimistic toggle + revert on rejection | unchanged; no new optimistic UI (D13 projection is not optimistic state) |
| Rename toast + re-export | existing `ProfilesOverlays` polite status with Undo | preserved; event must not create a second success toast |
| History panel | existing success/warning split (restore ok, history refresh failed) | preserved; unavailable vs empty stays distinct |

### Accessibility Requirements

- Existing `role="status"` / `aria-live="polite"` toast conventions preserved; no focus changes or extra modals for degraded metadata (history modal keeps its local warning route).
- Existing a11y test suites (`modals.a11y`, `components.a11y`, `routes.a11y`) must keep passing; banner copy change updates its own test.

### Performance UX

- **Loading states**: no new spinners; favorite-only updates must not flip summary/health loading flags.
- **Optimistic updates**: existing library toggle only; nothing added.
- **Error feedback**: unchanged timing/format; degraded metadata is a persistent banner, not per-save toasts.

## Recommendations

### Implementation Approach

**Recommended strategy**: one complete cutover PR, dependency-ordered batches — freeze behavior first (goldens + contracts), migrate operation families in parallel with exclusive file ownership, then delete shell helpers and run acceptance gates. Never regenerate expected bytes with the new writer.

**Phasing:**

1. **Phase 0 — Characterize (Batch 0, single integration owner)**: capture pre-refactor golden fixtures (D15); freeze contracts (reason enum, `AppEvent` variant, `ProfileWriter` skeleton with D3 lock and working private `commit`, module tree, `Services` wiring + bus injection, bridge arm D14, TS types D13, D5 core primitive, manifest deps).
2. **Phase 1 — Parallel migration (Batch 1)**: core extractions (D8); lifecycle/reads/favorites; sections/presets; history/rollback; ProtonDB/migration/Lutris; frontend listeners + dashboard overlay; mocks. Each slice owns distinct files and its tests.
3. **Phase 2 — Clean cutover (Batch 2)**: delete `capture_config_revision`, string `emit_profiles_changed`, `save_profile_section`, cleanup helpers and their re-exports; keep `apply_collection_defaults`; IPC contract tests; docs (`crates/crosshook-app/README.md`, `src/lib/mocks/README.md`, `CLAUDE.md`/`AGENTS.md` only if a rule changes); full acceptance gates.

### Technology Decisions

| Decision | Recommendation | Rationale |
| --------- | -------------- | --------- |
| Event tagging | internally tagged `kind`, snake_case | maps Rust enum 1:1; TS discriminated union; repo precedent |
| Snapshot testing crates | none; plain fixture files + byte equality | acceptance is byte-identical TOML; no new deps |
| Write serialization | writer-wide `Arc<Mutex<()>>` inside closure | preserves today's serialized ordering; rename/duplicate touch two names so per-name maps add complexity |
| Health invalidation | core DELETE, fail-soft; frontend revalidates on non-favorite reasons | matches issue pipeline; keeps commit cheap |
| Favorite health skip scope | only `favorite_toggled` | other reasons can affect name-keyed reports/enriched health; conservative default |
| Bridge topology | extend `startup_bridge.rs` | single receiver keeps `Resync` handling in one place; startup-path script references the filename |

### Quick Wins

- `profile_export_toml` name validation (D12) closes a real header-injection risk in moved code.
- Removing the mock/real event-contract mismatch makes browser smoke actually exercise the new parser.

### Future Enhancements

- Linear follow-ups from security research (D12): legacy-import app-id validation symmetry, env reserved-key symmetry across save paths, launcher slug-collision detection, art-import source confinement.
- Targeted per-name health batching and community-membership skips for `PresetApplied`/`Migrated`/`RolledBack` once invariants are explicit (nice-to-have, not this issue).

## Risk Assessment

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
| ---- | ---------- | ------ | ---------- |
| TOML behavior drift (wrong representation serialized, lost unknown keys, defaults on existing profiles, rollback re-filtered through full-save policy) | Medium | High | keep `ProfileStore::save`/core mutators as the only serializer; D15 pre-refactor goldens for save/rename/duplicate/rollback; distinct extras policy for rollback; assert revision hash separately from disk bytes |
| Event payload/bridge breakage (serde shape mismatch, bridge wildcard swallows event, string+object dual emission) | Medium | High | pin JSON for all nine reasons in Rust bridge tests and TS parser tests; D14 arm + resync sentinel; delete every string producer/consumer in the same PR |
| Favorite still triggers validation (unconditional `batchValidate`, or commit invalidates health for favorites) | Medium | High | favorite is SQLite-only; Vitest call counter with `saved` positive control; Rust event test asserts exactly one `FavoriteToggled` and no snapshot row change |
| Browser mocks diverge from real payload | High | Medium | one typed publisher helper in `profile-utils.ts`; all handlers table-tested against the TS union; smoke exercises the parser |
| Revision dedupe/rollback integrity regression (global dedupe, lost ownership scoping, unverified restore, lost lineage) | Low | High | reuse `insert_config_revision`/`verify_config_revision` untouched; tests for latest-only dedupe, foreign revision id, tampered HMAC, retained lineage parents |
| Migration drift (companion rewrite, preflight bypass, aborted batch on one failure) | Low | High | D8 `save_without_companion`; keep preflight order and per-item isolation; tests assert zero writes on preflight rejection and no companion rewrite |
| Concurrent write reordering after async move | Medium | Medium | D3 lock inside the closure; document last-completed-write-wins for external writers |
| Test migration silently drops coverage (helper tests deleted without equivalents; collection tests moved by accident) | Medium | Medium | move launcher/section tests into owning slices; keep `apply_collection_defaults` tests in the shell; core suites stay |
| Shared-file edit collisions across parallel slices | Medium | Medium | exclusive file ownership; integration owner pre-declares the whole module tree in Batch 0 so no slice edits `mod.rs` |
| Dropped caller future loses the notification | Low | Medium | D4 publish inside the blocking closure |
| Event arrives before invoking command resolves (rename auto-selection race) | Medium | Medium | `useProfileCrud` keeps explicit post-command refresh/selection; listeners must not auto-select on `Renamed`; test rename with open detail |

### Integration Challenges

- **Five listeners + ~19 mock emit sites + two legacy tests must switch atomically**: shared parser with resync-on-malformed default; no compatibility shims.
- **Lutris, community and ProtonDB commands import the helpers being deleted**: migrate `lutris_import_profiles` to `import_lutris` and `community_import_profile` to `import_community` (D10), and `protondb_accept_suggestion` to the facade, in the same PR or the build breaks.
- **`profile_load`/summaries keep a shell-side collection branch**: service owns raw reads/projection; the temporary boundary is explicit and documented (D7).

### Security Considerations

#### Critical — Hard Stops

| Finding | Risk | Required Mitigation |
| -------- | ---- | ------------------- |
| None identified | — | — |

#### Warnings — Must Address

| Finding | Risk | Mitigation | Alternatives |
| -------- | ---- | ----------- | ------------ |
| W-2 export header injection: `profile_to_shareable_toml_with_options` interpolates an unvalidated name into the TOML header | newline-bearing name prepends arbitrary TOML lines to "valid" output | D12: `validate_name` before serialize in `export_toml`; test rejects `x\n[[pre_launch_hooks]]…` | Linear follow-up to harden the core formatter |
| W-4 error strings leak absolute paths today (`ProfileStoreError::NotFound(path)`, migration `error` field) | home layouts exposed to webview/logs | route through `AppError` sanitization; keep `sanitize_display_path` on migration result fields and apply it to the error field; test no `/home/` in command errors | document that `sanitize_message` only redacts home-style paths |
| W-5 launcher slug collisions (`"Test 1"` vs `"Test-1"`) can delete another profile's launcher | cross-profile interference from rename/delete | do not loosen slug sanitization; regression-test that derived delete paths stay inside the two launcher dirs for adversarial names | Linear follow-up: collision detection/warning |
| W-6 art auto-import copies any readable ≤5 MB image into asset-scope-served `media/` | exfiltration bridge from a compromised webview (bounded to images) | keep `validate_image_bytes` as the mandatory gate in moved code; document the trusted-webview stance | follow-up: restrict to `$HOME` sources, atomic destination write |
| W-1 legacy import bypasses `validate_steam_app_id` | arbitrary `SteamAppId=` text reaches disk and app-id-keyed lookups | behavior-preserving move; add Linear follow-up (D12 scope discipline) | land the gate with the writer only if trivially confined to moved code |
| A-5 favorite emits without name validation | unvalidated names in event payloads/SQL binds | `validate_name` before `set_profile_favorite` and before publish in the facade | — |
| A-6 panics in moved code (`SettingsStore` poisoned lock expect) | one panic degrades later metadata ops until restart | no `unwrap`/`expect` added in closures; `run_blocking` contains panics into sanitized `AppError` | — |

#### Advisories — Best Practices

- Keep launcher deletion delegated to `core::export::delete_launcher_for_profile` (watermark + symlink refusal); never inline `fs::remove_file` (A-1).
- Rollback order stays verify → parse → save → observe → capture lineage → publish, exactly once (A-2).
- One serialization path (`ProfileStore::save` / `save_without_companion`) + `write_atomic`; the service never writes TOML itself (A-3).
- Disabled-store revision skip must become visible (warn + surfaced), while insert/serialize failures stay fail-open (A-4).
- Community shareable export stays denylist-hardened; never route it through the save path (A-10).

## Task Breakdown Preview

### Phase 0: Characterize and freeze (Batch 0)

**Focus**: prove byte compatibility pre-change and freeze every cross-slice contract.
**Tasks**: capture pre-refactor golden fixtures; land reason enum + `AppEvent` variant + bridge arm; `ProfileWriter` skeleton with D3 lock and working private `commit`; module tree; `Services` wiring + bus injection; D5 core primitive; TS types + parser; manifest deps.
**Parallelization**: one integration owner, strict order (fixtures before any writer code); nothing else starts.

### Phase 1: Parallel operation-family migration (Batch 1)

**Focus**: move every operation family behind the facade with exclusive file ownership.
**Dependencies**: Batch 0 contracts.
**Tasks**: core extractions (D8); lifecycle/reads/favorites; sections/presets; history/rollback; ProtonDB/migration/Lutris; frontend listeners + dashboard favorite overlay; mocks. Each slice lands its own service module, thin Tauri delegates and tests.

### Phase 2: Clean cutover and gates (Batch 2)

**Focus**: delete shell orchestration, pin the IPC contract, update docs, run acceptance.
**Tasks**: helper deletion and re-export cleanup; collection-branch boundary retained; IPC contract test; README/mock docs; full validation suite.

## Decisions Needed

All architectural decisions are settled by [decisions.md](./decisions.md) (D1–D15, binding). Remaining follow-ups to file (not blocking):

1. **Security follow-ups (D12)** — legacy-import app-id validation, env reserved-key symmetry on the generic save path, slug-collision detection, art-import source confinement: Linear issues, not this PR.
2. **Health dashboard favorite copy (D6)** — banner wording ("profile files still save, but configuration snapshots are not recorded") needs a final copy pass against `MetadataStatusBanner` tests.
3. **Per-name health batching** — explicitly deferred; `names` carries the future capability without building a scheduler now.

## Research References

- [issue.md](./issue.md) — authoritative acceptance criteria and scope.
- [decisions.md](./decisions.md) — binding orchestrator decisions D1–D15.
- [research-business.md](./research-business.md) — the 23 commands, mutation-path inventory, listeners, revision/deduge/disabled-store behavior, edge cases.
- [research-technical.md](./research-technical.md) — module layout, public Rust surface, commit pipeline, core extraction signatures, frontend contract, file-by-file change list.
- [research-external.md](./research-external.md) — tokio/serde/Tauri semantics, golden-file approach, Vitest patterns, constraints and code examples.
- [research-ux.md](./research-ux.md) — listener refresh matrix, favorite/health consistency risk, error and warning surfacing, verification guidance.
- [research-security.md](./research-security.md) — threat model, findings W-1…W-6, advisories, invariants checklist, test hooks.
- [research-practices.md](./research-practices.md) — reuse inventory, module boundaries, KISS assessment, testability patterns, persistence classification.
- [research-recommendations.md](./research-recommendations.md) — phasing, wire shape, risk register, parallel ownership table, acceptance-to-test allocation.
````
