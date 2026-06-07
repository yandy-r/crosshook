# Plan: Hero Detail Trainer Tab Editor Upgrade

## Summary

Replace the read-only Hero Detail Trainer tab with an in-context editor for per-profile loaded DLL hook declarations, injection configuration, and a bounded recent injection/trainer lifecycle log. This plan keeps trainer editing inside the existing Hero Detail `trainer` tab, stores user-editable hook/config data in profile TOML, and uses runtime-only structured events for the recent log.

The implementation does not build a native DLL injection engine or process-attach runtime. Until a runtime engine exists, Method/Stage/Timeout/Fallback are persisted and clearly marked as stored configuration, while scoped trainer/injection telemetry reports the trainer lifecycle and the fact that declared DLL hooks are not actively injected.

## User Story

As a per-game CrossHook user configuring trainer behavior, I want the Hero Detail Trainer tab to manage DLL hook declarations, injection settings, and recent trainer/injection lifecycle feedback, so that I can stay in the per-game flow without trusting a read-only summary.

## Problem → Solution

The current Trainer tab only displays path and loading mode while injection profile data is limited to `dll_paths` and `inject_on_launch`. → The Trainer tab becomes a guarded three-section editor that persists canonical DLL hook/config TOML data, exposes unsupported runtime status honestly, and tails scoped trainer/injection events through the repo event adapter.

## Metadata

- **Complexity**: Large
- **Source PRD**: N/A
- **Source Spec**: `docs/prps/specs/hero-detail-trainer-tab-editor-upgrade.spec.md`
- **PRD Phase**: standalone
- **Estimated Files**: 29

---

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks              | Depends On | Parallel Width |
| ----- | ------------------ | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3      | —          | 3              |
| B2    | 2.1, 2.2, 2.3, 2.4 | B1         | 4              |
| B3    | 3.1, 3.2, 3.3, 3.4 | B2         | 4              |
| B4    | 4.1, 4.2, 4.3      | B3         | 3              |

- **Total tasks**: 14
- **Total batches**: 4
- **Max parallel width**: 4

---

## UX Design

### Before

- Hero Detail has a `Trainer` tab id, but its panel is a summary card with only trainer path and loading mode.
- DLL hook editing does not exist in the Trainer tab; script pre/post hooks live under Launch options.
- Runtime feedback is only generic `launch-log`/diagnostic console output or launch history, not a scoped Trainer tab tail.

### After

- Hero Detail `Trainer` opens a three-section editor: Loaded DLL hooks, Injection config, and Recent injection log.
- Loaded DLL hooks use add/remove/rename/path/toggle row affordances, but remain separate from script lifecycle hooks.
- Injection config fields are persisted and visibly marked as stored-only until runtime support lands.
- Recent injection log shows the latest scoped trainer/injection lifecycle events and caps the in-memory tail.

### Interaction Changes

| Touchpoint              | Before                                 | After                                                           | Notes                                                                 |
| ----------------------- | -------------------------------------- | --------------------------------------------------------------- | --------------------------------------------------------------------- |
| Hero Detail Trainer tab | Read-only path/loading summary         | Editable Loaded hooks, Injection config, Recent injection log   | Keep existing `trainer` tab id, do not add a route                    |
| Loaded hook rows        | No DLL-specific rows                   | Add/remove/rename/retarget/toggle DLL hook declarations         | Do not feed DLL rows into `LaunchHook` script execution               |
| Injection config        | Only legacy arrays exist               | Method/Stage/Timeout/Fallback profile fields                    | Controls are stored-only or disabled until runtime consumption exists |
| Autosave                | Launch tab has guarded hook autosave   | Trainer tab uses same selected-profile guard and visible status | No displayed-vs-selected profile writes                               |
| Recent injection log    | Generic console stream                 | Scoped bounded log in Trainer tab                               | Subscribe through `subscribeEvent`, mock through `emitMockEvent`      |
| Scroll                  | Hero Detail body handles tab scrolling | Log tail may own bounded overflow                               | Register new scroll selector if overflow is added                     |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                                 | Lines                     | Why                                                                   |
| -------------- | ------------------------------------------------------------------------------------ | ------------------------- | --------------------------------------------------------------------- |
| P0 (critical)  | `docs/prps/specs/hero-detail-trainer-tab-editor-upgrade.spec.md`                     | all                       | Source requirements, storage boundary, and open runtime questions     |
| P0 (critical)  | `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`                   | 287-318                   | Current read-only Trainer tab branch to replace                       |
| P0 (critical)  | `src/crosshook-native/src/components/library/HeroDetailLaunchTab.tsx`                | 34-92, 121-138            | Selected-profile mismatch guard and hook autosave write path          |
| P0 (critical)  | `src/crosshook-native/src/components/library/launch/useHeroLaunchHooksAutosave.ts`   | 24-82                     | Debounced guarded `persistProfileDraft` pattern                       |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/profile/models/game_meta.rs`         | 29-35                     | Current `InjectionSection` schema                                     |
| P0 (critical)  | `src/crosshook-native/src/types/profile.ts`                                          | 115-118, 233-252, 297-307 | TS injection shape, normalizer, and default profile                   |
| P0 (critical)  | `src/crosshook-native/src/lib/events.ts`                                             | 7-29                      | Required event adapter for browser-dev parity                         |
| P1 (important) | `src/crosshook-native/src/components/library/HookListPanel.tsx`                      | 38-188                    | Row affordances to mirror without script/DLL conflation               |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs`         | 75-87                     | Profile load normalization path                                       |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/profile/exchange/utils.rs`           | 17-63                     | Community export/import path stripping and disable-on-import contract |
| P1 (important) | `src/crosshook-native/src-tauri/src/commands/launch/execution.rs`                    | 200-398                   | Trainer launch/session lifecycle surface                              |
| P1 (important) | `src/crosshook-native/src-tauri/src/commands/launch/streaming.rs`                    | 87-139, 426-442           | Existing launch log and completion event emission                     |
| P1 (important) | `src/crosshook-native/src/lib/mocks/handlers/launch.ts`                              | 108-137, 217-225          | Browser-dev launch event sequence to extend                           |
| P1 (important) | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                                 | 8-11, 49-64               | Scroll selector contract for bounded log tails                        |
| P2 (reference) | `src/crosshook-native/src/components/library/__tests__/HeroDetailLaunchTab.test.tsx` | 204-280                   | Autosave and mismatch test pattern                                    |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/profile/models/tests/hooks.rs`       | all                       | Rust TOML/default/normalization test pattern                          |

## External Documentation

| Topic                   | Source | Key Takeaway                                                                                                             |
| ----------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------ |
| External APIs/libraries | N/A    | No external research needed; use repo-local `callCommand`, `subscribeEvent`, `emitMockEvent`, React, and Vitest patterns |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION

```ts
// SOURCE: src/crosshook-native/src/components/library/hero-detail-model.ts:5
export type HeroDetailTabId = 'overview' | 'profiles' | 'launch-options' | 'trainer' | 'history' | 'compatibility';
```

```ts
// SOURCE: src/crosshook-native/src/lib/events.ts:7-13
export async function subscribeEvent<T>(name: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  if (isTauri()) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<T>(name, handler);
  }
```

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/models/game_meta.rs:29-35
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InjectionSection {
    #[serde(rename = "dll_paths", default)]
    pub dll_paths: Vec<String>,
```

### ERROR_HANDLING

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs:134-148
pub fn validate(request: &LaunchRequest) -> Result<(), ValidationError> {
    match request.method.trim() {
        "" | METHOD_STEAM_APPLAUNCH | METHOD_PROTON_RUN | METHOD_NATIVE => {}
        value => return Err(ValidationError::UnsupportedMethod(value.to_string())),
    }
```

```ts
// SOURCE: src/crosshook-native/src/components/library/HeroDetailLaunchTab.tsx:56-65
const profileMismatch = useMemo(() => {
  if (selectedTrimmed.length === 0) {
    return false;
  }
  const displayedName = displayProfileName?.trim() ?? '';
```

### LOGGING_PATTERN

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/launch/streaming.rs:91-95
for ui_line in transform_launch_log_line_for_ui(&mut relay_state, line) {
    if let Err(error) = app.emit("launch-log", ui_line) {
        tracing::warn!(%error, "failed to emit launch log line; continuing stream");
    }
```

```ts
// SOURCE: src/crosshook-native/src/lib/mocks/handlers/launch.ts:115-119
logLines.forEach((line, index) => {
  scheduleLaunchTimeout(
    () => {
      emitMockEvent('launch-log', line);
```

### REPOSITORY_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs:81-87
let content = fs::read_to_string(&path)?;
let profile: GameProfile = toml::from_str(&content)?;
let mut effective = profile.effective_profile();
effective.local_override = LocalOverrideSection::default();
effective.launch.normalize_preset_selection();
```

```ts
// SOURCE: src/crosshook-native/src/types/profile.ts:248-252
injection: {
  ...profile.injection,
  dll_paths: [...profile.injection.dll_paths],
  inject_on_launch: [...profile.injection.inject_on_launch],
},
```

### SERVICE_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/library/launch/useHeroLaunchHooksAutosave.ts:51-56
const scheduleHookAutosave = useCallback(
  (nextProfile: GameProfile) => {
    if (!hasSavedSelectedProfile) {
      return;
    }
```

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/exchange/utils.rs:21-25
/// **Denylist invariant**: any new path-bearing field added to `GameProfile` MUST be cleared here.
/// This function is fail-open — new fields silently survive export unless explicitly enumerated.
pub(super) fn sanitize_profile_for_community_export(profile: &GameProfile) -> GameProfile {
    let mut out = profile.portable_profile();
```

### TEST_STRUCTURE

```ts
// SOURCE: src/crosshook-native/src/components/library/__tests__/HeroDetailLaunchTab.test.tsx:204-215
it('adds a pre-launch hook and persists through the profile draft save path', async () => {
  vi.useFakeTimers();
  try {
    const { profile } = renderLaunchTab();
```

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/models/tests/hooks.rs:70-76
fn legacy_profile_without_hook_keys_defaults_to_empty() {
    let parsed: GameProfile = toml::from_str(
        r#"[game]
name = "No Hooks"
executable_path = "/games/no-hooks.exe"
```

---

## Files to Change

| File                                                                                   | Action | Justification                                                                         |
| -------------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/profile/models/game_meta.rs`           | UPDATE | Add canonical loaded DLL hook/config model, serde defaults, and legacy mirror helpers |
| `src/crosshook-native/crates/crosshook-core/src/profile/models/profile.rs`             | UPDATE | Add `normalize_injection()` and call it with existing hook normalization              |
| `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs`           | UPDATE | Normalize injection data after TOML load and before downstream use                    |
| `src/crosshook-native/src/types/profile.ts`                                            | UPDATE | Add TS `LoadedDllHook`/injection config types and default normalization               |
| `src/crosshook-native/src/hooks/profile/createEmptyProfile.ts`                         | UPDATE | Seed default injection config and empty loaded hook declarations                      |
| `src/crosshook-native/src/hooks/profile/profileNormalize.ts`                           | UPDATE | Keep edit/save normalization aligned with canonical injection shape                   |
| `src/crosshook-native/src/test/fixtures.ts`                                            | UPDATE | Add fixture defaults and helpers for trainer tab tests                                |
| `src/crosshook-native/src/types/injection.ts`                                          | CREATE | Shared frontend structured injection log event and type guards                        |
| `src/crosshook-native/src-tauri/src/commands/launch/shared.rs`                         | UPDATE | Add structured injection event payload used by trainer launch telemetry               |
| `src/crosshook-native/crates/crosshook-core/src/profile/exchange/utils.rs`             | UPDATE | Strip new path-bearing DLL hook fields on export and disable imported declarations    |
| `src/crosshook-native/crates/crosshook-core/src/profile/health/profile.rs`             | UPDATE | Validate canonical loaded DLL hook paths while retaining legacy array checks          |
| `src/crosshook-native/src/hooks/profile/useProfileCrud.ts`                             | UPDATE | Include canonical DLL hook paths in recent-files sync                                 |
| `src/crosshook-native/src/components/library/trainer/useHeroTrainerAutosave.ts`        | CREATE | Debounced selected-profile-safe trainer editor autosave with status                   |
| `src/crosshook-native/src/components/library/trainer/LoadedDllHookListPanel.tsx`       | CREATE | DLL-specific hook declaration row editor                                              |
| `src/crosshook-native/src/components/library/trainer/InjectionLogTail.tsx`             | CREATE | Bounded scoped event-tail component                                                   |
| `src/crosshook-native/src/components/library/trainer/InjectionConfigPanel.tsx`         | CREATE | Method/Stage/Timeout/Fallback controls with stored-only runtime status                |
| `src/crosshook-native/src-tauri/src/commands/launch/execution.rs`                      | UPDATE | Emit trainer/injection lifecycle events around `launch_trainer`                       |
| `src/crosshook-native/src-tauri/src/commands/launch/streaming.rs`                      | UPDATE | Emit completion/failure lifecycle events using the structured payload                 |
| `src/crosshook-native/src/lib/mocks/handlers/launch.ts`                                | UPDATE | Add browser-dev injection event sequence for trainer launch                           |
| `src/crosshook-native/src/components/library/HeroDetailTrainerTab.tsx`                 | CREATE | New three-section Trainer tab container                                               |
| `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`                     | UPDATE | Replace read-only `case 'trainer'` with `HeroDetailTrainerTab`                        |
| `src/crosshook-native/src/styles/hero-detail.css`                                      | UPDATE | Add trainer editor/log layout styles, reusing existing hook/list tokens               |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`                                   | UPDATE | Register any bounded trainer log scroll container selector                            |
| `src/crosshook-native/crates/crosshook-core/src/profile/models/tests/injection.rs`     | CREATE | Rust model/default/legacy migration tests                                             |
| `src/crosshook-native/crates/crosshook-core/src/profile/models/tests/mod.rs`           | UPDATE | Register new injection model test module                                              |
| `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/tests/load_save.rs` | UPDATE | Add store load/save coverage for legacy arrays and canonical hooks                    |
| `src/crosshook-native/crates/crosshook-core/src/profile/exchange/mod.rs`               | UPDATE | Extend exchange tests for new DLL hook sanitization/import disabling                  |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailTrainerTab.test.tsx`  | CREATE | Component tests for sections, edits, autosave, mismatch guard, and log cap/filter     |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx`      | UPDATE | Update Trainer branch expectations                                                    |

## NOT Building

- No native DLL injection engine, process attach loop, `LoadLibrary` bridge, or memory patching runtime.
- No new SQLite injection-history table; recent injection rows are runtime-only and capped in memory.
- No raw Tauri `invoke()`/`listen()` usage; frontend code must use `callCommand()` and `subscribeEvent()`.
- No broad `src-tauri` business logic expansion; validation and model decisions stay in `crosshook-core`.
- No community export of machine-local DLL hook paths or auto-enabled imported DLL hooks.
- No new external frontend or Rust dependencies for basic editor controls/log tail behavior.

---

## Step-by-Step Tasks

### Task 1.1: Define Canonical Rust Injection Model — Depends on none

- **BATCH**: B1
- **ACTION**: Add canonical profile TOML fields for loaded DLL hooks and injection config.
- **IMPLEMENT**: In `game_meta.rs`, add `LoadedDllHook`, `InjectionMethod`, `InjectionStage`, and `InjectionFallback` with serde defaults and `skip_serializing_if` where appropriate. Keep legacy `dll_paths` and `inject_on_launch` as backward-compatible mirror arrays, then add `GameProfile::normalize_injection()` in `profile.rs` and call it from `ProfileStore::load` after `normalize_hooks()`.
- **MIRROR**: `InjectionSection` serde defaults in `game_meta.rs` and `GameProfile::normalize_hooks()` in `profile.rs`.
- **IMPORTS**: `serde::{Deserialize, Serialize}` and existing profile model module exports.
- **GOTCHA**: Do not extend `LaunchHook` for DLL rows; script hooks execute host commands and must stay separate.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core profile::models::tests::injection`

### Task 1.2: Add Frontend Injection Types And Defaults — Depends on none

- **BATCH**: B1
- **ACTION**: Mirror the canonical injection shape in TypeScript and all frontend profile defaults.
- **IMPLEMENT**: Update `types/profile.ts`, `createEmptyProfile.ts`, `profileNormalize.ts`, and `test/fixtures.ts` so sparse profiles always expose `loaded_hooks`, `method`, `stage`, `timeout_ms`, and `fallback`. Normalize legacy arrays into canonical `LoadedDllHook[]` for edit state and mirror canonical rows back to arrays only where backward compatibility requires it.
- **MIRROR**: `normalizeSerializedGameProfile()` default expansion pattern in `types/profile.ts`.
- **IMPORTS**: Existing `GameProfile` type exports; no new dependencies.
- **GOTCHA**: Browser mocks and tests often use sparse profile objects; every UI read must be safe after normalization.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck`

### Task 1.3: Define Structured Injection Log Contract — Depends on none

- **BATCH**: B1
- **ACTION**: Add a structured runtime-only event payload for trainer/injection lifecycle rows.
- **IMPLEMENT**: Create `src/types/injection.ts` with `InjectionLogEvent` and a narrow type guard. Add the matching serializable payload in launch shared Rust code with fields for timestamp, profile name, session id, session kind, level, source, message, optional hook id/name, and optional unsupported-runtime marker.
- **MIRROR**: `LaunchResult` serde payload style in `src-tauri/src/commands/launch/shared.rs` and `isLaunchValidationIssue()` style in `types/launch.ts`.
- **IMPORTS**: `serde::Serialize` in Rust; local TS types only.
- **GOTCHA**: Keep the event payload display-safe; do not pass raw helper output, environment dumps, or unsanitized paths as injection log messages.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck`

### Task 2.1: Update Security, Exchange, Health, And Recent Files — Depends on 1.1, 1.2

- **BATCH**: B2
- **ACTION**: Carry the canonical DLL hook model through security boundaries and health/recent-file helpers.
- **IMPLEMENT**: Update community export to strip loaded DLL hook paths and disable imported loaded hooks. Update profile health to validate enabled canonical DLL hook paths while retaining legacy `injection.dll_paths[i]` coverage during migration, and update recent-files sync to include canonical hook paths.
- **MIRROR**: Denylist invariant in `profile/exchange/utils.rs` and `check_profile_health()` field-specific path issue pattern.
- **IMPORTS**: Existing profile model exports and health check helpers.
- **GOTCHA**: Any new path-bearing field that survives community export is a local path leak and a potential execution-vector leak.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core profile::exchange profile::health`

### Task 2.2: Build Guarded Trainer Autosave Hook — Depends on 1.2

- **BATCH**: B2
- **ACTION**: Create a Trainer-tab autosave hook with visible status and selected-profile safety.
- **IMPLEMENT**: Add `useHeroTrainerAutosave.ts` that accepts `hasSavedSelectedProfile`, `profile`, `profileName`, and `persistProfileDraft`. Debounce saves with `launchOptimizationsAutosaveDelayMs`, guard against profile-name drift before persisting, and expose a `LaunchAutoSaveStatus`-shaped status for idle/saving/success/error.
- **MIRROR**: `useHeroLaunchHooksAutosave()` timer/ref guard and `LaunchAutoSaveStatus` tone labels.
- **IMPORTS**: `useCallback`, `useEffect`, `useRef`, `useState`, `launchOptimizationsAutosaveDelayMs`, `PersistProfileDraft`, `GameProfile`, `LaunchAutoSaveStatus`.
- **GOTCHA**: Do not autosave when the displayed game profile differs from `ProfileContext.selectedProfile`.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/library/__tests__/HeroDetailTrainerTab.test.tsx`

### Task 2.3: Build DLL-Specific Hook List Panel — Depends on 1.2

- **BATCH**: B2
- **ACTION**: Create a Loaded DLL hook declaration editor that reuses row affordances without script-stage semantics.
- **IMPLEMENT**: Add `LoadedDllHookListPanel.tsx` with add/remove/rename/path/toggle behavior for `LoadedDllHook[]`. Use DLL-specific labels and validation (`id`, NUL-free name/path, optional `.dll` hint) and do not render pre-launch/post-exit stage pills.
- **MIRROR**: `HookListPanel` row, popover, invalid-row removal, and client-generated id behavior.
- **IMPORTS**: React state, `LoadedDllHook` type, existing icon/button CSS classes.
- **GOTCHA**: The existing `HookListPanel` button text says script or DLL; do not reuse that copy in the Trainer tab.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/library/__tests__/HeroDetailTrainerTab.test.tsx`

### Task 2.4: Build Bounded Injection Log Tail — Depends on 1.3

- **BATCH**: B2
- **ACTION**: Create a scoped live-tail component for recent trainer/injection events.
- **IMPLEMENT**: Add `InjectionLogTail.tsx` that subscribes through `subscribeEvent<InjectionLogEvent>('injection-log', ...)`, filters by current profile/session fields when present, and caps rows to the latest 200 entries. Render empty, live, warning, and unsupported-runtime rows without subscribing through raw Tauri APIs.
- **MIRROR**: `ConsoleView` event cleanup, `PrefixDepsPanel` 200-row bounded tail, and `normalizeLogMessage()` defensive payload parsing.
- **IMPORTS**: `subscribeEvent`, `InjectionLogEvent`, `isInjectionLogEvent`, React effects/state.
- **GOTCHA**: Generic `launch-log` strings are not profile-scoped; use the structured event for Trainer tab display.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/library/__tests__/HeroDetailTrainerTab.test.tsx`

### Task 3.1: Build Injection Config Panel — Depends on 1.2, 2.2

- **BATCH**: B3
- **ACTION**: Add Method, Stage, Timeout, and Fallback controls bound to profile TOML.
- **IMPLEMENT**: Add `InjectionConfigPanel.tsx` with select/number/toggle-style controls for the canonical injection config fields. Because this PRP does not build a DLL injection engine, show a persistent stored-only status and ensure tests assert that controls do not imply launch-time behavior.
- **MIRROR**: `TrainerSection` select/help text pattern and `ThemedSelect` usage.
- **IMPORTS**: `ThemedSelect`, `GameProfile` injection types, autosave callback props.
- **GOTCHA**: Requirement F5 is satisfied by honest stored-only/disabled status; do not add `LaunchRequest` fields unless runtime consumption is implemented in the same patch.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/library/__tests__/HeroDetailTrainerTab.test.tsx`

### Task 3.2: Emit Trainer/Injection Lifecycle Telemetry — Depends on 1.3, 2.4

- **BATCH**: B3
- **ACTION**: Emit structured runtime-only events for the current trainer launch lifecycle.
- **IMPLEMENT**: Update `launch_trainer` and stream finalization to emit `injection-log` rows for trainer launch requested, process started, unsupported DLL injection engine status, completion, and failure/teardown. Extend the browser-dev launch mock to emit the same structured sequence alongside existing trainer `launch-log` lines.
- **MIRROR**: `app.emit("launch-log", ...)` tracing warning pattern and `scheduleLaunchLogSequence()` mock event scheduling.
- **IMPORTS**: `tauri::Emitter`, structured injection event payload, `emitMockEvent`.
- **GOTCHA**: Do not emit raw helper log lines into the injection log; messages must be scoped and sanitized.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/library/__tests__/HeroDetailTrainerTab.test.tsx`

### Task 3.3: Wire Hero Detail Trainer Tab — Depends on 2.2, 2.3, 2.4, 3.1

- **BATCH**: B3
- **ACTION**: Replace the Trainer panel branch with the new three-section editor.
- **IMPLEMENT**: Create `HeroDetailTrainerTab.tsx` using `useProfileContext()` like `HeroDetailLaunchTab`, compute `hasSavedSelectedProfile` and `profileMismatch`, and wire Loaded hooks, Injection config, and Recent injection log sections. Update `HeroDetailPanels.tsx` to render the new component for `case 'trainer'` while preserving loading/error states and `displayProfileName`.
- **MIRROR**: `HeroDetailLaunchTab` guard comments, `DashboardPanelSection` composition, and `HeroDetailPanels` branch structure.
- **IMPORTS**: `HeroDetailTrainerTab`, `useProfileContext`, trainer subcomponents, profile types.
- **GOTCHA**: The Trainer tab should remain a Hero Detail tab id, not a route or legacy page.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/library/__tests__/HeroDetailPanels.test.tsx src/components/library/__tests__/HeroDetailTrainerTab.test.tsx`

### Task 3.4: Add Trainer Styles And Scroll Registration — Depends on 2.3, 2.4, 3.1, 3.3

- **BATCH**: B3
- **ACTION**: Style the editor and register any bounded log scroll container.
- **IMPLEMENT**: Add `crosshook-hero-detail__trainer-*` styles in `hero-detail.css` using the existing hook row, card, muted text, mono text, and mobile-collapse vocabulary. If `InjectionLogTail` owns `overflow-y: auto`, add its selector to `SCROLL_ENHANCE_SELECTORS` and use `overscroll-behavior: contain`.
- **MIRROR**: Existing hook row styles in `hero-detail.css` and `useScrollEnhance` selector list.
- **IMPORTS**: CSS only; no JS imports unless adding a selector constant test.
- **GOTCHA**: Do not create nested page cards or one-off viewport height chains; stay inside Hero Detail's existing scroll owner.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck`

### Task 4.1: Add Backend Model, Store, Exchange, And Health Tests — Depends on 1.1, 2.1, 3.2

- **BATCH**: B4
- **ACTION**: Cover Rust serialization, normalization, sanitization, health, and telemetry behavior.
- **IMPLEMENT**: Add `profile/models/tests/injection.rs`, register it in the test module, and extend store/exchange/health tests for legacy arrays, canonical loaded hooks, export stripping, import disabling, and enabled path validation. Add launch telemetry unit coverage where practical for structured event construction.
- **MIRROR**: `profile/models/tests/hooks.rs`, `toml_store/tests/load_save.rs`, and existing exchange sanitizer tests.
- **IMPORTS**: Existing test fixture helpers, `GameProfile`, new injection model types.
- **GOTCHA**: Unknown enum values should reject when unsafe, while missing fields must default for legacy profiles.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

### Task 4.2: Add Frontend Trainer Tab Tests — Depends on 2.2, 2.3, 2.4, 3.1, 3.3, 3.4

- **BATCH**: B4
- **ACTION**: Test the Trainer tab editor behavior in the existing component test style.
- **IMPLEMENT**: Add `HeroDetailTrainerTab.test.tsx` covering section rendering, DLL hook add/edit/remove/toggle, config edits, debounced save success/error status, selected-profile mismatch no-write behavior, native/unsupported state messaging, event filtering, and 200-row cap. Update `HeroDetailPanels.test.tsx` to expect the new Trainer branch.
- **MIRROR**: `HeroDetailLaunchTab.test.tsx` fake-timer/mocked `ProfileContext` setup and `HookListPanel.test.tsx` row interaction assertions.
- **IMPORTS**: Testing Library, `userEvent`, `vi`, `emitMockEvent`, profile fixtures.
- **GOTCHA**: Let `subscribeEvent` promises resolve before asserting event-driven log rows.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/library/__tests__/HeroDetailTrainerTab.test.tsx src/components/library/__tests__/HeroDetailPanels.test.tsx`

### Task 4.3: Run Cross-Cutting Validation Gates — Depends on 4.1, 4.2

- **BATCH**: B4
- **ACTION**: Run focused and repo-native validation for the full feature surface.
- **IMPLEMENT**: Run frontend typecheck/tests, core Rust tests, mock coverage check, and host-gateway check because launch telemetry touches the trainer launch path. Run smoke tests only if the implementation changes browser-dev navigation or the visible mock lifecycle flow beyond component coverage.
- **MIRROR**: Repo command reference in `AGENTS.md` and `package.json` scripts.
- **IMPORTS**: N/A.
- **GOTCHA**: `src/crosshook-native` owns frontend `typecheck`, `test`, and `test:smoke`; the repo root does not expose `typecheck`.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck && npm test`; `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`; `./scripts/check-mock-coverage.sh`; `./scripts/check-host-gateway.sh`

---

## Testing Strategy

### Unit Tests

| Test                    | Input                                             | Expected Output                                                        | Edge Case? |
| ----------------------- | ------------------------------------------------- | ---------------------------------------------------------------------- | ---------- |
| Rust injection defaults | TOML profile missing new injection fields         | Loaded profile has default config and empty loaded hooks               | Yes        |
| Rust legacy migration   | TOML with `dll_paths` and `inject_on_launch` only | Canonical `loaded_hooks` derives stable rows and mirrors enabled state | Yes        |
| Rust export sanitizer   | Profile with loaded DLL hook paths                | Community export clears paths and disables/import sanitizes hooks      | Yes        |
| Rust health             | Enabled loaded hook with missing DLL path         | Health issue references canonical injection field with remediation     | Yes        |
| Trainer tab render      | Ready profile in Hero Detail Trainer tab          | Three sections render with stored-only status                          | No         |
| Loaded hook editing     | Add, rename, retarget, toggle, remove             | `updateProfile` receives canonical `loaded_hooks` changes              | No         |
| Autosave safety         | Displayed profile differs from selected profile   | Editor controls disabled or not mounted; no update/save occurs         | Yes        |
| Autosave status         | Persist succeeds or fails                         | Visible status changes to success or error                             | Yes        |
| Injection log tail      | 205 scoped events plus unrelated profile events   | Latest 200 scoped rows render, unrelated rows ignored                  | Yes        |
| Browser mocks           | Mock trainer launch emits `injection-log`         | Trainer tab log updates without Tauri                                  | No         |

### Edge Cases Checklist

- [ ] Legacy profile has `dll_paths` longer than `inject_on_launch`.
- [ ] Legacy profile has enabled flag without a matching DLL path.
- [ ] Loaded hook has empty id, empty path, NUL in name/path, or duplicate id.
- [ ] Profile is native launch method and trainer injection editing is unsupported.
- [ ] Displayed Hero Detail profile does not match `ProfileContext.selectedProfile`.
- [ ] `persistProfileDraft` rejects during autosave.
- [ ] `injection-log` payload is malformed or belongs to another profile/session.
- [ ] Log tail exceeds 200 rows.
- [ ] Community import contains enabled DLL hook declarations.
- [ ] Flatpak/host-visible path checks differ from raw `Path::exists()`.

---

## Validation Commands

### Static Analysis

```bash
cd src/crosshook-native && npm run typecheck
```

EXPECT: Zero TypeScript errors for app and test configs.

### Focused Frontend Tests

```bash
cd src/crosshook-native && npm test -- src/components/library/__tests__/HeroDetailTrainerTab.test.tsx src/components/library/__tests__/HeroDetailPanels.test.tsx
```

EXPECT: Trainer tab render/edit/autosave/log tests pass.

### Core Rust Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: Profile model, store, exchange, health, and launch-related tests pass.

### Browser Mock Coverage

```bash
./scripts/check-mock-coverage.sh
```

EXPECT: No missing browser-dev handlers if new IPC commands are added; event-only mock additions should not break the sentinel.

### Host Gateway Contract

```bash
./scripts/check-host-gateway.sh
```

EXPECT: No direct denylisted host-tool `Command::new` usage outside the platform gateway.

### Full Frontend Test Suite

```bash
cd src/crosshook-native && npm test
```

EXPECT: No regressions in library, launch, profile, and event tests.

### Optional Smoke Test

```bash
cd src/crosshook-native && npm run test:smoke
```

EXPECT: Browser-dev Hero Detail navigation and mock trainer lifecycle remain usable when the visible mock flow changes.

### Manual Validation

- [ ] Open Hero Detail and select the Trainer tab for a Proton/Steam profile.
- [ ] Confirm Loaded hooks, Injection config, and Recent injection log sections are visible.
- [ ] Add a DLL hook, rename it, set a path, toggle it off and on, then confirm autosave status.
- [ ] Switch to a different selected profile and confirm Trainer controls cannot write the displayed profile.
- [ ] Launch trainer in browser-dev mode and confirm scoped injection log rows appear and remain bounded.
- [ ] Confirm native profiles show disabled/unsupported trainer injection messaging.

---

## Acceptance Criteria

- [ ] Ready-state Hero Detail Trainer tab renders Loaded hooks, Injection config, and Recent injection log sections instead of the old read-only card.
- [ ] Users can add, remove, rename, retarget, enable, and disable per-profile loaded DLL hook declarations.
- [ ] Loaded DLL hooks are modeled separately from script lifecycle `LaunchHook` rows and never flow into pre/post hook execution.
- [ ] Injection Method, Stage, Timeout, and Fallback fields are stored in profile TOML with backward-compatible defaults.
- [ ] UI clearly marks injection config and loaded DLL hooks as stored-only until a runtime DLL injection engine consumes them.
- [ ] Trainer tab edits autosave through the selected profile guard and surface success/error status.
- [ ] Displayed-vs-selected profile mismatch cannot write to the wrong profile.
- [ ] Recent injection log uses `subscribeEvent`, filters scoped structured events, and caps its tail to 200 rows.
- [ ] Browser-dev mocks can exercise Trainer tab log behavior without Tauri.
- [ ] Community export/import strips or disables new path-bearing DLL hook declarations.
- [ ] Rust and frontend tests cover legacy profile loading and sparse mock/default profiles.
- [ ] Validation commands listed above pass or failures are documented with exact output.

## Completion Checklist

- [ ] Code follows discovered Hero Detail, profile model, event adapter, and mock patterns.
- [ ] Error handling and validation use existing profile/launch validation styles.
- [ ] Logging/telemetry emits structured sanitized `injection-log` rows, not raw helper output.
- [ ] Tests cover Rust model defaults, legacy migration, exchange sanitization, frontend editing, autosave, mismatch safety, and event tail bounds.
- [ ] No hardcoded local filesystem paths ship in exports, fixtures, or telemetry beyond existing mock paths.
- [ ] No new external dependencies are added.
- [ ] No `## Worktree Setup` section or per-task worktree annotations are present because `--no-worktree` was requested.
- [ ] Self-contained implementation path remains clear without resolving the original open questions interactively.

## Risks

| Risk                                                                 | Likelihood | Impact | Mitigation                                                                                    |
| -------------------------------------------------------------------- | ---------- | ------ | --------------------------------------------------------------------------------------------- |
| UI implies DLL injection occurs even though no runtime engine exists | Med        | High   | Stored-only status, disabled unsupported controls, tests asserting no launch-time implication |
| Legacy array migration creates unstable hook ids                     | Med        | Med    | Deterministic id derivation from index/path or explicit migration tests documenting behavior  |
| New path-bearing fields leak through community export                | Med        | High   | Extend sanitizer and import-disable tests in the same batch as model changes                  |
| Recent log shows generic noise from other profiles                   | Med        | Med    | Use structured scoped `injection-log`; do not display unscoped `launch-log` strings           |
| Autosave writes the wrong profile                                    | Med        | High   | Mirror `HeroDetailLaunchTab` selected-profile guard and fake-timer mismatch tests             |
| Scroll jank from nested log container                                | Low        | Med    | Keep editor in existing scroll owner and register only bounded log tail selector              |
| Host gateway regression while adding telemetry near trainer launch   | Low        | High   | Run `check-host-gateway.sh`; route host-tool work through `platform.rs` if introduced         |

## Notes

- Persistence boundary: loaded DLL hook declarations and injection config are user-editable profile TOML data; recent injection log rows are runtime-only; no SQLite schema change is planned.
- Runtime boundary: this plan intentionally does not add a DLL injection engine. Trainer/injection telemetry reports lifecycle and unsupported runtime status so the UI remains honest.
- Enhanced research dispatch: requested enhanced mode was attempted, but the local YCC preflight reported a missing agents directory. Equivalent enhanced coverage was gathered through local scan plus six sidecar researchers; the recommendations role was synthesized locally.
- Worktree mode: disabled via `--no-worktree`; this plan is written for the current checkout.
