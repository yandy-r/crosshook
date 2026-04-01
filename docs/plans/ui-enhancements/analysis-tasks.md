# Task Structure Analysis: ui-enhancements

## Executive Summary

The ui-enhancements feature spans four executable phases (0–3) with a deferred Phase 4. The backend Rust work (`steam_metadata/`, `game_images/`, SQLite v14 migration, `GameImageStore`) is fully independent of the frontend restructuring (`ProfilesPage` card layout, `ProfileFormSections` extraction) — these two tracks can be developed in parallel up until the IPC integration point in Phase 2. The highest-risk task is the `ProfileFormSections` extraction in Phase 3 because it touches a 41k-line shared component used by both `ProfilesPage` and `InstallPage`; all other tasks are well-isolated with clear file boundaries. Define IPC command signatures and TypeScript types as the shared contract before parallelizing Rust and frontend work in Phase 2.

---

## Recommended Phase Structure

### Phase 0 — Foundation (gate for all phases)

**Goal**: Eliminate circular imports, add `infer` dependency, run SQLite migration v14, implement `GameImageStore`, extend `AppSettingsData`.

**Rationale**: No UI visual output but unblocks everything. The circular import (`ui/ProtonPathField.tsx` → `ProfileFormSections.tsx` for `formatProtonInstallLabel`) causes extraction failures in Phase 3 if not resolved first. The v14 migration must exist before `GameImageStore` tests can use `open_in_memory()`. All Phase 0 work is additive and reversible.

**Key constraint**: `AppSettingsData` change (`settings/mod.rs`) must be paired with a matching `types/settings.ts` update — the type crosses the IPC boundary and a mismatch silently drops the field on `settings_save`.

**Parallelism**: UI cleanup tasks and backend infrastructure tasks share zero files — run two workstreams fully in parallel.

**Estimated tasks**: 7 (4 UI cleanup + 3 backend infra)

---

### Phase 1 — ProfilesPage Card Layout (after Phase 0)

**Goal**: Remove the single `CollapsibleSection("Advanced", defaultOpen=false)` wrapper; promote each logical section group into its own `CollapsibleSection` + `crosshook-panel` card; move `ProfileActions` outside any card; promote health badges to profile selector bar; add cover art CSS stub classes.

**Rationale**: Highest-value change (immediately visible to users) with lowest regression risk — touches only `ProfilesPage.tsx` and CSS, makes zero changes to `ProfileFormSections`. Adding `crosshook-profile-cover-art` CSS class here as a conditional stub prevents layout rework when art is wired in Phase 2.

**Key constraint**: `ProfileActions` must never be inside a tab panel or collapsible section. Verify after restructure. Also verify `InstallPage` (OnboardingWizard `reviewMode`) and keyboard/controller navigation still work.

**Estimated tasks**: 2 (1 ProfilesPage restructuring + 1 CSS)

---

### Phase 2 — Steam Metadata + Cover Art Integration (after Phase 1; Rust modules can start in parallel with Phase 1)

**Goal**: Wire Steam Store API backend, implement `useGameMetadata` and `useGameCoverArt` hooks, display cover art and genre chips in the Core card, configure Tauri asset protocol + CSP.

**Rationale**: Phase 1 creates the cover art slot; Phase 2 fills it. The Rust backend modules (`steam_metadata/`, `game_images/`) start in parallel with Phase 1 once Phase 0 is done — they share zero files with frontend work. Frontend hooks can be stubbed against a mock before Tauri commands are complete, using the agreed IPC type signatures as the contract.

**Key constraint**: SVG rejection (`validate_image_bytes`) and path traversal prevention (`safe_image_cache_path`) from `research-security.md` are required mitigations in P2-C (not advisory). Tauri CSP and capabilities config (P2-D) must be in place before Phase 2 cover art display can be manually tested.

**Estimated tasks**: 8 (IPC contract + 1 steam_metadata Rust + 1 game_images Rust + 1 Tauri commands + 1 Tauri config + 1 TS hooks + 1 UI components + 1 ProfilesPage wiring)

---

### Phase 3 — ProfileFormSections Extraction + Sub-Tab Navigation (after Phase 1; gate on Phase 1 user feedback)

**Goal**: Extract 6 section components from `ProfileFormSections.tsx`; add `ProfileSubTabs` using `@radix-ui/react-tabs`; CSS `display: none` for inactive panels; optionally add SteamGridDB Rust client and Settings API key field.

**Rationale**: Phase 3 is the highest-risk phase because `ProfileFormSections` is shared with `InstallPage`. Gating on Phase 1 user feedback avoids a second structural restructuring if the navigation model needs to change. The 6 section extractions are parallel-safe with each other.

**Key constraint**: CSS `display: none` for inactive tab panels is a hard correctness requirement — `CustomEnvironmentVariablesSection` holds local `rows` draft state that is silently lost on conditional unmount (W1). `ProtonDB + EnvVars must stay co-located` (business rule BR3) — they must live in the same card/tab.

**Estimated tasks**: 10 (6 section extractions + 1 thin ProfileFormSections + 1 ProfileSubTabs + 1 SteamGridDB + 1 Settings API key)

---

### Phase 4 — Visual Polish (after Phase 2; deferred)

Gradient overlays, portrait card layout option, grid/list view toggle. Fully deferred — no tasks until Phase 2 ships and user feedback is collected.

---

## Task Granularity Recommendations

Target: 1–3 files per task. This keeps diffs reviewable and enables parallel execution without merge conflicts.

### Phase 0 — UI Cleanup Workstream (all parallel)

| Task ID | Task                                                                    | Files Touched                                                              | Notes                                       |
| ------- | ----------------------------------------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------- |
| P0-A    | Extract `formatProtonInstallLabel` (fix circular dep)                   | `ProfileFormSections.tsx`, new `utils/proton.ts`, `ui/ProtonPathField.tsx` | Must complete before any Phase 3 extraction |
| P0-B    | Deduplicate `FieldRow` → `InstallField` (add `id` prop)                 | `ui/InstallField.tsx` + callers                                            | Additive prop; audit for `FieldRow` usages  |
| P0-C    | Consolidate `ProtonPathField` (make `ui/` version canonical)            | `ui/ProtonPathField.tsx`, callers                                          | Canonical file already exists               |
| P0-D    | Replace `OptionalSection` with `CollapsibleSection defaultOpen={false}` | Callers of `OptionalSection`                                               | Remove wrapper component                    |

### Phase 0 — Backend Infrastructure Workstream (all parallel)

| Task ID | Task                                                     | Files Touched                                                | Notes                                                                    |
| ------- | -------------------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------ |
| P0-E    | Add `infer ~0.16` to Cargo.toml + SQLite v14 migration   | `crosshook-core/Cargo.toml`, `metadata/migrations.rs`        | Additive; sequential `if version < 14` guard                             |
| P0-F    | Implement `metadata/game_image_store.rs`                 | New `metadata/game_image_store.rs`, update `metadata/mod.rs` | Mirror `health_store.rs` — `&Connection` params, MetadataStore delegates |
| P0-G    | Add `steamgriddb_api_key` to settings (Rust + TS paired) | `settings/mod.rs`, `types/settings.ts`                       | Must update both in same task — IPC round-trip                           |

### Phase 1 Tasks

| Task ID | Task                                                                                 | Files Touched                              | Notes                                                                                  |
| ------- | ------------------------------------------------------------------------------------ | ------------------------------------------ | -------------------------------------------------------------------------------------- |
| P1-A    | Restructure ProfilesPage — remove Advanced wrapper, card layout, move ProfileActions | `ProfilesPage.tsx`                         | Largest single-file change; verify `InstallPage` + keyboard/controller nav             |
| P1-B    | Cover art CSS stub classes + variables                                               | `styles/theme.css`, `styles/variables.css` | Add `crosshook-profile-cover-art`, `crosshook-skeleton`, aspect-ratio + animation vars |

### Phase 2 Tasks

| Task ID | Task                                            | Files Touched                                                             | Notes                                                                             |
| ------- | ----------------------------------------------- | ------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| P2-A    | Define IPC contract: types + command signatures | New `types/game-metadata.ts`, sketch command signatures                   | **Define first** — shared contract for parallel Rust/TS tracks                    |
| P2-B    | Rust `steam_metadata/` module                   | New `steam_metadata/mod.rs`, `client.rs`, `models.rs`                     | Copy protondb module as scaffold; same state enum, same cache key pattern         |
| P2-C    | Rust `game_images/` module                      | New `game_images/mod.rs`, `client.rs`, `cache.rs`, `models.rs`            | Must implement `validate_image_bytes` + `safe_image_cache_path` (security gates)  |
| P2-D    | Tauri IPC commands                              | New `commands/game_metadata.rs`, update `commands/mod.rs`, `lib.rs`       | 13-line template per command; zero business logic                                 |
| P2-E    | Tauri config + capabilities                     | `tauri.conf.json`, `capabilities/default.json`                            | CSP `img-src` + narrow `$LOCALDATA/cache/images/**` scope; do before UI testing   |
| P2-F    | Frontend hooks                                  | `hooks/useGameMetadata.ts`, `hooks/useGameCoverArt.ts`                    | Mirror `useProtonDbLookup.ts` state machine and `requestIdRef` race guard exactly |
| P2-G    | UI display components                           | New `components/profile-sections/GameCoverArt.tsx`, `GameMetadataBar.tsx` | Shimmer skeleton + loading states; depends on P2-F types                          |
| P2-H    | Wire into ProfilesPage                          | `ProfilesPage.tsx`                                                        | Integration; depends on P1-A + P2-F + P2-G                                        |

### Phase 3 Tasks

| Task ID | Task                                                     | Files Touched                                                | Notes                                                                           |
| ------- | -------------------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------- |
| P3-A    | Extract `ProfileIdentitySection`                         | New `components/profile-sections/ProfileIdentitySection.tsx` | Stateless; parallel-safe; exclude `injection.*`                                 |
| P3-B    | Extract `GameSection`                                    | New `components/profile-sections/GameSection.tsx`            | Stateless; parallel-safe; exclude `injection.*`                                 |
| P3-C    | Extract `RunnerMethodSection`                            | New `components/profile-sections/RunnerMethodSection.tsx`    | Stateless; parallel-safe                                                        |
| P3-D    | Extract `TrainerSection`                                 | New `components/profile-sections/TrainerSection.tsx`         | Conditional on launch method; parallel-safe                                     |
| P3-E    | Extract `RuntimeSection`                                 | New `components/profile-sections/RuntimeSection.tsx`         | Most complex — runner-method conditionals; run last in parallel window          |
| P3-F    | Verify `EnvironmentSection` co-location (no new file)    | `ProfileFormSections.tsx` (checklist only)                   | ProtonDB + EnvVars stay together per BR3; do NOT create a separate section file |
| P3-G    | Reduce `ProfileFormSections` to thin composition wrapper | `ProfileFormSections.tsx`                                    | Blocks on P3-A–E complete; preserve `reviewMode` prop contract                  |
| P3-H    | Add `ProfileSubTabs` component                           | New `components/ProfileSubTabs.tsx`                          | CSS `display:none` panels required (W1); blocks on P3-G                         |
| P3-I    | SteamGridDB Rust client                                  | New `game_images/steamgriddb.rs`                             | Fully parallel with P3-A–H                                                      |
| P3-J    | SteamGridDB Settings UI (API key field)                  | `SettingsPage.tsx`                                           | Mask input; UX warning about plaintext; blocks on P0-G + P3-I                   |

---

## Dependency Analysis

```
Phase 0 (Foundation)
  P0-A  [no deps]              — circular dep fix (first task to run)
  P0-B  [no deps]              — InstallField id prop
  P0-C  [no deps]              — ProtonPathField consolidation
  P0-D  [no deps]              — OptionalSection replacement
  P0-E  [no deps]              — Cargo.toml + v14 migration
  P0-F  [blocks on P0-E]       — GameImageStore (needs migration schema for tests)
  P0-G  [no deps]              — AppSettingsData + types/settings.ts (paired task)

Phase 1 (Cards layout)
  P1-A  [blocks on P0-A]       — ProfilesPage restructure
  P1-B  [no deps within phase] — CSS stub classes

Phase 2 (Steam integration)
  P2-A  [blocks on Phase 0]              — IPC contract types (define before any parallel work)
  P2-B  [blocks on P0-E, P0-F, P2-A]    — steam_metadata Rust module
  P2-C  [blocks on P0-E, P0-F, P2-A]    — game_images Rust module (parallel with P2-B)
  P2-D  [blocks on P2-B, P2-C]          — Tauri IPC commands
  P2-E  [no code deps; start after P2-A] — Tauri config + capabilities
  P2-F  [blocks on P2-A]                — Frontend hooks (can stub against mock IPC)
  P2-G  [blocks on P2-F]                — UI display components
  P2-H  [blocks on P1-A + P2-F + P2-G]  — Wire into ProfilesPage

  Note: P2-B and P2-C can run in parallel with P1-A (zero shared files)

Phase 3 (Extraction + Tabs)  [all blocks on Phase 1; gate on Phase 1 user feedback]
  P3-A  [blocks on P0-A]       — ProfileIdentitySection
  P3-B  [blocks on P0-A]       — GameSection
  P3-C  [blocks on P0-A]       — RunnerMethodSection
  P3-D  [blocks on P0-A]       — TrainerSection
  P3-E  [blocks on P0-A]       — RuntimeSection (extract last)
  P3-F  [no file changes]      — EnvironmentSection co-location checklist
  P3-G  [blocks on P3-A–E]     — Thin ProfileFormSections wrapper
  P3-H  [blocks on P3-G]       — ProfileSubTabs
  P3-I  [no deps in phase]     — SteamGridDB Rust client (fully parallel)
  P3-J  [blocks on P0-G + P3-I] — SteamGridDB Settings UI
```

---

## File-to-Task Mapping

### New Files

| File                                                                              | Phase | Task |
| --------------------------------------------------------------------------------- | ----- | ---- |
| `src/crosshook-native/src/utils/proton.ts`                                        | P0    | P0-A |
| `crosshook-core/src/metadata/game_image_store.rs`                                 | P0    | P0-F |
| `src/crosshook-native/src/types/game-metadata.ts`                                 | P2    | P2-A |
| `crosshook-core/src/steam_metadata/mod.rs`                                        | P2    | P2-B |
| `crosshook-core/src/steam_metadata/client.rs`                                     | P2    | P2-B |
| `crosshook-core/src/steam_metadata/models.rs`                                     | P2    | P2-B |
| `crosshook-core/src/game_images/mod.rs`                                           | P2    | P2-C |
| `crosshook-core/src/game_images/client.rs`                                        | P2    | P2-C |
| `crosshook-core/src/game_images/cache.rs`                                         | P2    | P2-C |
| `crosshook-core/src/game_images/models.rs`                                        | P2    | P2-C |
| `src-tauri/src/commands/game_metadata.rs`                                         | P2    | P2-D |
| `src/crosshook-native/src/hooks/useGameMetadata.ts`                               | P2    | P2-F |
| `src/crosshook-native/src/hooks/useGameCoverArt.ts`                               | P2    | P2-F |
| `src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx`           | P2    | P2-G |
| `src/crosshook-native/src/components/profile-sections/GameMetadataBar.tsx`        | P2    | P2-G |
| `src/crosshook-native/src/components/profile-sections/ProfileIdentitySection.tsx` | P3    | P3-A |
| `src/crosshook-native/src/components/profile-sections/GameSection.tsx`            | P3    | P3-B |
| `src/crosshook-native/src/components/profile-sections/RunnerMethodSection.tsx`    | P3    | P3-C |
| `src/crosshook-native/src/components/profile-sections/TrainerSection.tsx`         | P3    | P3-D |
| `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx`         | P3    | P3-E |
| `src/crosshook-native/src/components/ProfileSubTabs.tsx`                          | P3    | P3-H |
| `crosshook-core/src/game_images/steamgriddb.rs`                                   | P3    | P3-I |

### Modified Files (hotspots — watch for sequential edit dependencies)

| File                            | Tasks          | Change Summary                                                                                   |
| ------------------------------- | -------------- | ------------------------------------------------------------------------------------------------ |
| `ProfileFormSections.tsx`       | P0-A then P3-G | Remove `formatProtonInstallLabel` in P0-A; reduce to thin wrapper in P3-G — two sequential edits |
| `ProfilesPage.tsx`              | P1-A then P2-H | Card layout in P1-A; cover art wiring in P2-H — sequential across phases                         |
| `metadata/mod.rs`               | P0-F           | Expose `game_image_store` submodule                                                              |
| `metadata/migrations.rs`        | P0-E           | Add `if version < 14` sequential migration block                                                 |
| `crosshook-core/Cargo.toml`     | P0-E           | Add `infer ~0.16`                                                                                |
| `settings/mod.rs`               | P0-G           | Add `steamgriddb_api_key: Option<String>` with `#[serde(default)]`                               |
| `types/settings.ts`             | P0-G           | Add `steamgriddb_api_key?: string \| null` — paired with Rust change                             |
| `ui/ProtonPathField.tsx`        | P0-A then P0-C | Fix import in P0-A; make canonical in P0-C                                                       |
| `ui/InstallField.tsx`           | P0-B           | Add `id` prop                                                                                    |
| `styles/theme.css`              | P1-B           | Add `crosshook-profile-cover-art`, `crosshook-skeleton`                                          |
| `styles/variables.css`          | P1-B           | Add cover art aspect-ratio + skeleton animation vars                                             |
| `src-tauri/src/commands/mod.rs` | P2-D           | Add `pub mod game_metadata`                                                                      |
| `src-tauri/src/lib.rs`          | P2-D           | Register new commands in `invoke_handler!`                                                       |
| `tauri.conf.json`               | P2-E           | CSP `img-src 'self' asset: http://asset.localhost`                                               |
| `capabilities/default.json`     | P2-E           | Asset protocol scope + `fs:allow-read-file` scoped to `$LOCALDATA/cache/images/**`               |
| `SettingsPage.tsx`              | P3-J           | Add SteamGridDB API key field                                                                    |

---

## Optimization Opportunities

### Parallel Execution Windows

**Window 1 (Phase 0 — two workstreams, full parallel)**
UI cleanup (P0-A, P0-B, P0-C, P0-D) runs concurrently with backend infra (P0-E, P0-G). Only P0-F waits for P0-E (needs migration schema for `open_in_memory()` tests).

**Window 2 (Phase 2 Rust backend concurrent with Phase 1 frontend)**
P2-B and P2-C share zero files with `ProfilesPage.tsx`. Assign Rust modules to one track while Phase 1 card restructuring happens on another. Requires P2-A (IPC contract) to be written first as the shared interface.

**Window 3 (Phase 2 frontend mock-first)**
P2-F (hooks) and P2-G (components) can develop against hardcoded mock data before P2-D (real Tauri commands) is ready. The `types/game-metadata.ts` from P2-A is the only prerequisite.

**Window 4 (Phase 3 section extractions — up to 5 parallel agents)**
P3-A, P3-B, P3-C, P3-D each create one new file with zero conflicts. P3-E (RuntimeSection, most complex) can overlap with the others but should be the last to complete. P3-I (SteamGridDB) runs fully independent of all P3 extraction work.

### Serial Bottlenecks (must not be parallelized)

| Bottleneck                      | Reason                                                                       |
| ------------------------------- | ---------------------------------------------------------------------------- |
| P0-E before P0-F                | `GameImageStore` tests require v14 schema via `open_in_memory()`             |
| P0-A before P1-A                | ProfilesPage imports from ProfileFormSections; circular dep must be resolved |
| P2-A before P2-B, P2-C, P2-F    | IPC contract defines types used on both Rust and TS sides                    |
| P1-A before P2-H                | Cover art wiring requires the card slot to exist in the layout               |
| P2-F + P2-G before P2-H         | Hooks and components must exist before ProfilesPage integration              |
| P3-A–E all complete before P3-G | Thin wrapper cannot be written until all section extractions are done        |
| P3-G before P3-H                | `ProfileSubTabs` composes the extracted section components                   |

---

## Cross-Cutting Checklist Items

These concerns affect multiple tasks and must be explicitly verified on each relevant task:

1. **`injection.*` exclusion** — Every new section component (P3-A through P3-E) must explicitly exclude `injection.dll_paths` and `injection.inject_on_launch`. Add as a checklist item on each extraction task definition.

2. **CSS `display:none` for tab panels** — P3-H (`ProfileSubTabs`) must use `display: none` on inactive panels, not conditional rendering. Verify `CustomEnvironmentVariablesSection` draft state is preserved across tab switches.

3. **`ProfileActions` outside all panels** — P1-A restructuring must render Save/Delete/Duplicate/Rename outside any `CollapsibleSection` or tab panel. Verify in markup and visually.

4. **`ProtonDB + EnvVars co-location`** — When laying out cards in P1-A and assigning sections to tabs in P3-H, ProtonDB lookup and `CustomEnvironmentVariablesSection` must be in the same card/tab (BR3).

5. **Security gates for P2-C** — `game_images/cache.rs` must implement both `validate_image_bytes` (SVG rejection, `infer` crate magic-byte) and `safe_image_cache_path` (numeric-only `steam_app_id` + `canonicalize` + prefix assertion). Implementation-ready code is in `research-security.md`. P2-C must not merge without both.

6. **`InstallPage` compatibility check** — Run OnboardingWizard `reviewMode` flow after P1-A and again after P3-G. `ProfileFormSections` with `reviewMode={true}` must render identically before and after each change.

7. **Keyboard + controller navigation** — Verify F2 rename, focus zones, and gamepad D-pad after P1-A (layout change) and P3-H (tab introduction).

8. **IPC type pairing** — P0-G pairs `settings/mod.rs` and `types/settings.ts`. P3-J must also ensure both are in sync for any new Settings fields.

9. **MetadataStore mutex not held across awaits** — New Rust async code in `steam_metadata/client.rs` and `game_images/client.rs` must acquire and release the mutex in discrete operations. Do not hold the lock across HTTP requests.

---

## Implementation Strategy Recommendations

### 1. P0-A is the first task (highest Phase 3 unlock value)

The circular import in `ui/ProtonPathField.tsx → ProfileFormSections.tsx` is the highest-friction blocker for Phase 3. Extract `formatProtonInstallLabel` to `utils/proton.ts` first. It is a mechanical rename with no logic change.

### 2. Run Phase 2 Rust backend in parallel with Phase 1

Assign `steam_metadata/` and `game_images/` Rust modules to a second track while Phase 1 does the ProfilesPage card restructure. These tracks share zero files. The backend track can be tested independently with `cargo test` and `MetadataStore::open_in_memory()` before any frontend code exists.

### 3. Write P2-A (IPC contract) before splitting Phase 2 into parallel tracks

`types/game-metadata.ts` and the `fetch_game_metadata` / `fetch_game_cover_art` signatures are the shared contract. Writing this first means both the Rust and TypeScript tracks can proceed independently with a clear interface — no ambiguity mid-flight.

### 4. Use `protondb/` as the scaffold for `steam_metadata/`, not just a reference

For P2-B, copy `protondb/client.rs` and `protondb/models.rs` as starting scaffolds and adapt names, endpoints, and cache key (`steam:appdetails:v1:{app_id}`). Same file names, same state enum values, same OnceLock pattern. This is not a design-from-scratch problem.

### 5. Security items for P2-C are implementation tasks, not design tasks

`research-security.md` contains code-ready `validate_image_bytes()` and `safe_image_cache_path()` Rust functions. Task P2-C definition must include both as explicit acceptance criteria. Review must verify both are present before merge.

### 6. Phase 3 section extraction order

Recommend this sequencing discipline within the parallel window:

- First pass (parallel): extract stateless sections P3-A (Identity), P3-B (Game), P3-C (RunnerMethod), P3-D (Trainer)
- Final pass: extract P3-E (RuntimeSection) — most complex due to runner-method conditionals
- `CustomEnvironmentVariablesSection` is already extracted — do not touch it

### 7. CSS `display:none` is a correctness requirement in P3-H

Inactive tab panels must use `display: none`, not conditional rendering. Draft state in `CustomEnvironmentVariablesSection` is silently discarded on unmount. Verify by entering text in env vars, switching tabs, switching back, and confirming text is preserved.

### 8. Gate Phase 3 on Phase 1 user feedback

The `@radix-ui/react-tabs` infrastructure is ready and `crosshook-subtab-*` CSS classes already exist (unused). Shipping Phase 1 cards first and collecting feedback before committing to sub-tab navigation reduces the risk of a second structural restructuring.

### 9. Phase 4 is strictly out of scope for the initial plan

Do not stub, plan, or create files for Phase 4 (gradient overlays, portrait cards, grid/list toggle) until Phase 2 ships. Premature Phase 4 work risks layout assumptions becoming stale.
