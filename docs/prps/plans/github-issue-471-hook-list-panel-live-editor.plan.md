# Plan: GitHub Issue 471 HookListPanel Live Editor

## Summary

Implement the Phase 6 Hero Detail Launch-tab hook declaration editor from GitHub issue #471. The feature replaces the disabled pre/post hooks placeholder with a controlled `HookListPanel`, wires pre-launch and post-exit arrays into the existing profile draft save path, and keeps runtime execution explicitly deferred to #482.

This is a frontend-focused change against the Phase 3 hook schema. No Rust schema, SQLite migration, launch-request plumbing, host-gateway call, path probe, or subprocess execution is part of this plan.

## User Story

As a CrossHook user, I want to declare pre-launch and post-exit hooks per profile, so that script/DLL paths and enabled state are saved with the profile and ready for the later runtime executor.

## Problem -> Solution

The Hero Detail Launch tab currently shows a disabled "No pre/post hooks configured yet" placeholder even though `GameProfile` already has `pre_launch_hooks` and `post_exit_hooks`. Replace the placeholder with a banner plus two live hook editors that mutate the profile draft, debounce-save through `persistProfileDraft`, and display the runtime deferral clearly.

## Metadata

- **Complexity**: Medium
- **Source PRD**: `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`
- **PRD Phase**: Phase 6: Pre/post hooks panel
- **Source Issue**: https://github.com/yandy-r/crosshook/issues/471
- **Estimated Files**: 7

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks         | Depends On | Parallel Width |
| ----- | ------------- | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3 | none       | 3              |
| B2    | 2.1           | B1         | 1              |
| B3    | 3.1, 3.2, 3.3 | B2         | 3              |
| B4    | 4.1           | B3         | 1              |

- **Total tasks**: 8
- **Total batches**: 4
- **Max parallel width**: 3

## UX Design

### Before

```text
Hero Detail > Launch
  Launch command / environment / existing launch subtabs
  Pre/post hooks
    "No pre/post hooks configured yet"
    (Add hook) disabled
```

### After

```text
Hero Detail > Launch
  Launch command / environment / existing launch subtabs
  Pre/post hooks
    Info banner: saved, not executed yet, link to #482
    Pre-launch hooks
      [enabled] Name  /path/to/script.sh  pre-launch  [settings]
      [+ Attach script or DLL]
    Post-exit hooks
      [enabled] Name  /path/to/script.sh  post-exit   [settings]
      [+ Attach script or DLL]
```

### Interaction Changes

| Touchpoint              | Before                    | After                                                                        | Notes                                                              |
| ----------------------- | ------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| Hook section            | Disabled placeholder      | Live editor with two stage panels                                            | Replace the existing placeholder block, do not add another surface |
| Add hook                | Disabled                  | Appends a client-minted hook with empty path and `enabled: true`             | Use guarded `crypto.randomUUID()` fallback                         |
| Toggle                  | Not available             | Updates `enabled` for the row                                                | No runtime effect                                                  |
| Settings gear           | Not available             | Opens inline popover with name/path inputs and remove                        | No new dependency                                                  |
| Deferred runtime banner | Not shown                 | Always visible and linked to #482                                            | Use exact copy from issue/PRD                                      |
| Profile mismatch        | Launch sub-tabs are gated | Hook controls must also be disabled or hidden behind the same mismatch guard | Avoid saving to the wrong selected profile                         |

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                                 | Lines                 | Why                                                                           |
| -------------- | ------------------------------------------------------------------------------------ | --------------------- | ----------------------------------------------------------------------------- |
| P0 (critical)  | `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`                    | 363-375               | Phase 6 scope, copy, and files touched                                        |
| P0 (critical)  | `src/crosshook-native/src/components/library/HeroDetailLaunchTab.tsx`                | 31-101                | Current ProfileContext reads, mismatch guard, and placeholder insertion point |
| P0 (critical)  | `src/crosshook-native/src/types/profile.ts`                                          | 1-5, 175-178, 292-293 | Type barrel, hook fields, and normalization to arrays                         |
| P0 (critical)  | `src/crosshook-native/src/types/generated/launch_hooks.ts`                           | 3-25                  | Generated `HookStage` and `LaunchHook` wire contract                          |
| P1 (important) | `src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts`             | 24-90                 | Debounced `persistProfileDraft` pattern for Launch-tab edits                  |
| P1 (important) | `src/crosshook-native/src/components/library/profiles/useHeroProfilesAutosave.ts`    | 29-111                | Shared full-draft autosave status and delay pattern                           |
| P1 (important) | `src/crosshook-native/src/hooks/profile/useProfileCrud.ts`                           | 256-300               | `updateProfile` marks dirty; `persistProfileDraft` calls `profile_save`       |
| P1 (important) | `src/crosshook-native/src/components/collections/CollectionLaunchDefaultsEditor.tsx` | 21-27                 | Guarded client-side id minting pattern                                        |
| P1 (important) | `src/crosshook-native/src/styles/hero-detail.css`                                    | 222-335, 758-805      | Existing launch-tab layout, hook placeholder, and mobile collapse             |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/profile/models/profile.rs`           | 26-60                 | Stage authority, hook normalization, and skip-serializing empty arrays        |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/profile/exchange/utils.rs`           | 38-63                 | Community export strip and import force-disable hardening                     |
| P2 (reference) | `src/crosshook-native/src/components/library/__tests__/HeroDetailLaunchTab.test.tsx` | 31-181                | Existing launch-tab test harness and placeholder assertions to replace        |

## External Documentation

| Topic                  | Source                                          | Key Takeaway                                                                                                                 |
| ---------------------- | ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| GitHub issue #471      | https://github.com/yandy-r/crosshook/issues/471 | The editor is declaration-only, must persist `pre_launch_hooks`/`post_exit_hooks`, and must show the deferred runtime banner |
| Runtime follow-up #482 | https://github.com/yandy-r/crosshook/issues/482 | Runtime hook execution, launch request plumbing, health checks, and consent/strip UX are out of scope here                   |

No third-party API, SDK, or new npm dependency is required.

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION

```tsx
// SOURCE: src/crosshook-native/src/types/profile.ts:1-5
import type { LaunchHook } from './generated/launch_hooks';
export type { HookStage, LaunchHook } from './generated/launch_hooks';
```

Import `LaunchHook` and `HookStage` through `@/types/profile`, not directly from generated files. Name the component `HookListPanel`, export `HookListPanelProps`, and use BEM-like `crosshook-hero-detail__hook-*` classes in `hero-detail.css`.

### ERROR_HANDLING

```tsx
// SOURCE: src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts:59-74
if (!hasSavedSelectedProfile) return;
const scheduledProfileName = latestProfileNameRef.current;
if (latestProfileNameRef.current !== scheduledProfileName) return;
```

Gate hook autosaves to the saved selected profile and skip if the selected profile changed before the debounce fires. For invalid hook rows, render a recoverable `Invalid hook` row with a remove control instead of throwing.

### LOGGING_PATTERN

No logging is needed. This is a UI/profile-draft mutation feature; do not add console logging, backend logs, or runtime diagnostics.

### REPOSITORY_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/models/profile.rs:29-32
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub pre_launch_hooks: Vec<LaunchHook>,
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub post_exit_hooks: Vec<LaunchHook>,
```

Persist `[]` when the last row is removed. The Rust serializer already omits empty TOML sections; the frontend should not invent a separate nullable representation.

### SERVICE_PATTERN

```ts
// SOURCE: src/crosshook-native/src/hooks/profile/useProfileCrud.ts:256-258
const updateProfile = useCallback((updater: (current: GameProfile) => GameProfile) => {
  setProfile((current: GameProfile) => updater(current));
  setDirty(true);
}, []);
```

Use `updateProfile` for immediate draft state and a small hook-specific debounce helper that calls `persistProfileDraft`. Do not call `callCommand('profile_save')` from `HookListPanel` or `HeroDetailLaunchTab`.

### TEST_STRUCTURE

```tsx
// SOURCE: src/crosshook-native/src/components/library/__tests__/HeroDetailLaunchTab.test.tsx:111-130
describe('HeroDetailLaunchTab', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;
  beforeEach(() => {
    vi.clearAllMocks();
  });
  afterEach(() => {
    vi.useRealTimers();
    consoleErrorSpy.mockRestore();
  });
});
```

Use focused Vitest + React Testing Library tests with `userEvent`, fake timers for debounce assertions, and `console.error` guards.

## Files to Change

| File                                                                                 | Action | Justification                                                                           |
| ------------------------------------------------------------------------------------ | ------ | --------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/library/HookListPanel.tsx`                      | CREATE | Controlled editor for one hook stage                                                    |
| `src/crosshook-native/src/components/library/launch/useHeroLaunchHooksAutosave.ts`   | CREATE | Debounced `persistProfileDraft` helper for top-level hook arrays edited from Launch tab |
| `src/crosshook-native/src/components/library/HeroDetailLaunchTab.tsx`                | UPDATE | Replace placeholder with banner and two `HookListPanel` instances                       |
| `src/crosshook-native/src/styles/hero-detail.css`                                    | UPDATE | Add hook rows, stage pills, banner, popover, invalid-row, and mobile styles             |
| `src/crosshook-native/src/components/library/__tests__/HookListPanel.test.tsx`       | CREATE | Focused add/toggle/edit/remove/invalid-row behavior coverage                            |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailLaunchTab.test.tsx` | UPDATE | Replace placeholder assertions and cover merged arrays plus debounced save              |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx`    | UPDATE | Refresh launch-tab mock/expectations away from disabled placeholder text if needed      |

## NOT Building

- Runtime hook execution, `LaunchRequest` fields, shell spawning, timeout/failure semantics, health checks, and host-gateway calls. Those belong to #482.
- New Rust profile schema, SQLite metadata, migrations, ts-rs regeneration, or community exchange behavior. Phase 3 already shipped the fields and hardening.
- File picker integration. The issue asks for path input in the settings popover, not native file selection.
- Generic plugin/hook framework. Build the specific pre/post hook declaration editor only.
- New icon package. The project has local icons and text-button conventions.

## Step-by-Step Tasks

### Task 1.1: Build HookListPanel Component - Depends on none

- **BATCH**: B1
- **ACTION**: Create `src/crosshook-native/src/components/library/HookListPanel.tsx`.
- **IMPLEMENT**: Implement a controlled component with exact props `{ hooks: LaunchHook[]; stage: HookStage; onUpdate: (hooks: LaunchHook[]) => void }`. Add local helpers `newHookId`, `stageLabel`, `defaultHookName`, `coerceStage`, and `isInvalidHook`; each mutation must emit hooks whose `stage` matches the owning panel.
- **MIRROR**: `src/crosshook-native/src/types/profile.ts:1-5` for type imports; `src/crosshook-native/src/components/collections/CollectionLaunchDefaultsEditor.tsx:21-27` for guarded UUID fallback.
- **IMPORTS**: `useState` from React; `type LaunchHook, type HookStage` from `@/types/profile`; `SettingsIcon` from `../icons/SidebarIcons`.
- **GOTCHA**: Missing `pre_launch_hooks`/`post_exit_hooks` normalize to arrays, but props can still contain blank or malformed data in tests. Render `Invalid hook` rows with remove only instead of throwing.
- **VALIDATE**: `npm --prefix src/crosshook-native test -- src/components/library/__tests__/HookListPanel.test.tsx` after Task 3.1 exists.

### Task 1.2: Add Hook Autosave Helper - Depends on none

- **BATCH**: B1
- **ACTION**: Create `src/crosshook-native/src/components/library/launch/useHeroLaunchHooksAutosave.ts`.
- **IMPLEMENT**: Mirror `useLaunchEnvironmentAutosave`: keep refs for `profile`, `profileName`, `persistProfileDraft`, and the latest scheduled hook arrays; clear any existing timer before scheduling. Use `launchOptimizationsAutosaveDelayMs` rather than a hardcoded 400ms so the editor matches current Hero Detail cadence.
- **MIRROR**: `src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts:24-90` and `src/crosshook-native/src/components/library/profiles/useHeroProfilesAutosave.ts:80-111`.
- **IMPORTS**: `useCallback`, `useEffect`, `useRef`; `launchOptimizationsAutosaveDelayMs`; `type GameProfile`; `type LaunchHook`; `type PersistProfileDraft`.
- **GOTCHA**: `updateProfile` alone does not save. The helper must call `persistProfileDraft(profileName, nextProfile)` after the debounce and skip when `hasSavedSelectedProfile` is false or the profile name changed.
- **VALIDATE**: Covered by Task 3.2 fake-timer assertions that `persistProfileDraftSpy` receives updated hook arrays only after the shared delay.

### Task 1.3: Add Hook Styles - Depends on none

- **BATCH**: B1
- **ACTION**: Update `src/crosshook-native/src/styles/hero-detail.css`.
- **IMPLEMENT**: Replace the placeholder-only styling with reusable hook section classes: banner, two-panel stack, row, row text, mono path, stage pill, settings button, popover, invalid row, and responsive collapse. Avoid adding an inner `overflow-y: auto` container; if one becomes necessary, register it in `useScrollEnhance`.
- **MIRROR**: `src/crosshook-native/src/styles/hero-detail.css:222-335` for launch-tab layout and `src/crosshook-native/src/styles/hero-detail.css:758-805` for mobile behavior.
- **IMPORTS**: None.
- **GOTCHA**: Keep text wrapping stable on narrow viewports. Do not let the path text force horizontal overflow outside the row.
- **VALIDATE**: `npm --prefix src/crosshook-native run typecheck` plus manual browser check if implementation changes layout beyond CSS-only row wrapping.

### Task 2.1: Wire Hook Editors Into HeroDetailLaunchTab - Depends on 1.1, 1.2, 1.3

- **BATCH**: B2
- **ACTION**: Update `src/crosshook-native/src/components/library/HeroDetailLaunchTab.tsx`.
- **IMPLEMENT**: Extend the ProfileContext destructure to include `updateProfile` and `persistProfileDraft`. Replace the placeholder section with a `DashboardPanelSection` containing the deferred-runtime banner, pre-launch `HookListPanel`, and post-exit `HookListPanel`; stage-specific update handlers must merge only the edited array while preserving the other.
- **MIRROR**: `src/crosshook-native/src/components/library/HeroDetailLaunchTab.tsx:31-101` for existing computed `hasSavedSelectedProfile` and profile mismatch logic.
- **IMPORTS**: `HookListPanel`; `useHeroLaunchHooksAutosave`; `type LaunchHook`; `type HookStage` if needed.
- **GOTCHA**: The Profile mismatch path exists because the displayed profile can differ from `ProfileContext.selectedProfile`. When `profileMismatch` is true, show the existing disabled-style hint and do not mount active hook controls.
- **VALIDATE**: `npm --prefix src/crosshook-native test -- src/components/library/__tests__/HeroDetailLaunchTab.test.tsx`.

### Task 3.1: Add HookListPanel Focused Tests - Depends on 2.1

- **BATCH**: B3
- **ACTION**: Create `src/crosshook-native/src/components/library/__tests__/HookListPanel.test.tsx`.
- **IMPLEMENT**: Test add, toggle off, edit name/path through the settings popover, remove, stage coercion, empty state, and invalid row removal. Keep tests direct with a local `onUpdate` spy and small local hook fixture helper.
- **MIRROR**: `src/crosshook-native/src/components/library/__tests__/HeroDetailLaunchTab.test.tsx:111-130` for mock/timer cleanup style.
- **IMPORTS**: `render`, `screen`, `within`; `userEvent`; `describe`, `expect`, `it`, `vi`, `beforeEach`, `afterEach`; `HookListPanel`.
- **GOTCHA**: If the component opens a popover inline, query by accessible names rather than CSS selectors so the tests remain resilient to markup changes.
- **VALIDATE**: `npm --prefix src/crosshook-native test -- src/components/library/__tests__/HookListPanel.test.tsx`.

### Task 3.2: Update HeroDetailLaunchTab Tests - Depends on 2.1

- **BATCH**: B3
- **ACTION**: Update `src/crosshook-native/src/components/library/__tests__/HeroDetailLaunchTab.test.tsx`.
- **IMPLEMENT**: Replace assertions for the disabled placeholder with banner/link, two stage panels, and active attach buttons. Add fake-timer tests that add/toggle/remove hooks and assert `updateProfileSpy` receives merged `pre_launch_hooks`/`post_exit_hooks`, then `persistProfileDraftSpy` receives the updated full profile after `launchOptimizationsAutosaveDelayMs`.
- **MIRROR**: `src/crosshook-native/src/components/library/__tests__/HeroDetailLaunchTab.test.tsx:79-108` for ProfileContext mocks and `src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts:72-87` for debounce expectations.
- **IMPORTS**: Existing test imports plus `launchOptimizationsAutosaveDelayMs` if the assertion advances the exact shared delay.
- **GOTCHA**: Existing mocks may use a static profile object. For autosave assertions, either make `updateProfileSpy` apply the updater to a mutable test profile or assert the scheduled profile returned by the hook update path directly.
- **VALIDATE**: `npm --prefix src/crosshook-native test -- src/components/library/__tests__/HeroDetailLaunchTab.test.tsx`.

### Task 3.3: Refresh Panel-Level Mock Expectations - Depends on 2.1

- **BATCH**: B3
- **ACTION**: Update `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx` if it still mocks or asserts the disabled hook placeholder.
- **IMPLEMENT**: Keep the mock light, but make its text match the live hook section contract rather than "No pre/post hooks configured yet" with a disabled button. Do not broaden this file into hook behavior tests; behavior belongs in Tasks 3.1 and 3.2.
- **MIRROR**: `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx:13-20` for the existing launch-tab mock shape.
- **IMPORTS**: Existing imports only.
- **GOTCHA**: Avoid duplicating HookListPanel behavior coverage at the panel level.
- **VALIDATE**: `npm --prefix src/crosshook-native test -- src/components/library/__tests__/HeroDetailPanels.test.tsx`.

### Task 4.1: Run Full Validation - Depends on 3.1, 3.2, 3.3

- **BATCH**: B4
- **ACTION**: Run targeted and project-level validation for the feature.
- **IMPLEMENT**: Execute focused Vitest tests first, then frontend typecheck. Run the Rust core hook tests as a regression gate because the acceptance criteria include TOML empty-array omission.
- **MIRROR**: `src/crosshook-native/package.json:13-26` for frontend scripts and repo AGENTS verification guidance for `cargo test`.
- **IMPORTS**: None.
- **GOTCHA**: This plan should not require `npm install`, Rust codegen, OpenAPI, database generation, or Playwright browser installs.
- **VALIDATE**: Run the commands listed in `Validation Commands`; all must pass or failures must be documented with exact output.

## Testing Strategy

### Unit Tests

| Test                                   | Input                                           | Expected Output                                                                             | Edge Case? |
| -------------------------------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------------- | ---------- |
| `HookListPanel` add                    | Empty pre-launch hooks                          | One hook with `stage: "pre-launch"`, `enabled: true`, client id, empty path                 | No         |
| `HookListPanel` toggle                 | Existing enabled hook                           | `onUpdate` receives same row with `enabled: false`                                          | No         |
| `HookListPanel` edit                   | Name/path changed in popover                    | `onUpdate` receives edited values and stage remains panel stage                             | No         |
| `HookListPanel` remove                 | Single hook removed                             | `onUpdate` receives `[]`                                                                    | Yes        |
| `HookListPanel` invalid                | Hook with blank id/name or mismatched stage     | Renders `Invalid hook` and remove button; no throw                                          | Yes        |
| `HeroDetailLaunchTab` add pre hook     | Saved selected profile                          | `updateProfile` and debounced `persistProfileDraft` receive updated `pre_launch_hooks` only | No         |
| `HeroDetailLaunchTab` remove last hook | One post-exit hook                              | Debounced save receives `post_exit_hooks: []`                                               | Yes        |
| `HeroDetailLaunchTab` mismatch         | Displayed profile differs from selected profile | Hook controls disabled/not mounted; no save occurs                                          | Yes        |

### Edge Cases Checklist

- [ ] Empty hook arrays render empty states and active attach buttons.
- [ ] Existing hook arrays render both stages without cross-stage leakage.
- [ ] Stage is coerced to the containing panel before every `onUpdate`.
- [ ] Blank name/path or NUL-containing values are treated as invalid before save.
- [ ] Identity-less or otherwise malformed rows do not crash the panel.
- [ ] Removing the last row persists `[]`.
- [ ] Profile mismatch prevents writes to the wrong selected profile.
- [ ] Hook edits do not call launch/runtime IPC or host-gateway APIs.

## Validation Commands

### Static Analysis

```bash
npm --prefix src/crosshook-native run typecheck
```

EXPECT: Zero TypeScript errors.

### Unit Tests

```bash
npm --prefix src/crosshook-native test -- src/components/library/__tests__/HookListPanel.test.tsx src/components/library/__tests__/HeroDetailLaunchTab.test.tsx src/components/library/__tests__/HeroDetailPanels.test.tsx
```

EXPECT: Hook panel, launch-tab, and panel-level tests pass.

### Rust Regression Gate

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core profile::models::tests::hooks
```

EXPECT: Existing hook TOML round-trip, empty-array omission, and normalization tests pass.

### Host Gateway Boundary

```bash
./scripts/check-host-gateway.sh
```

EXPECT: No new direct host-tool command violations.

### Full Test Suite

```bash
npm --prefix src/crosshook-native test
```

EXPECT: No frontend regressions.

### Manual Validation

- [ ] Open Hero Detail for a profile, switch to Launch, and verify the Pre/post hooks section is live.
- [ ] Add a pre-launch hook, edit name/path, toggle it off, and remove it.
- [ ] Add a post-exit hook and verify the stage pill says `post-exit`.
- [ ] Confirm the banner is visible and opens issue #482.
- [ ] Confirm no launch behavior changes when pressing Launch or Dry-run.

## Acceptance Criteria

- [ ] `HookListPanel.tsx` exists with props `{ hooks, stage, onUpdate }` and no IPC calls.
- [ ] The Launch tab renders two hook editors: pre-launch and post-exit.
- [ ] Adding, toggling, editing, and removing hooks updates the correct profile arrays.
- [ ] Hook edits persist through `persistProfileDraft` and the existing `profile_save` path after the shared debounce.
- [ ] Removing the last hook persists an empty array; Rust serialization omits redundant TOML hook sections.
- [ ] The deferred-runtime banner is always visible and links to https://github.com/yandy-r/crosshook/issues/482.
- [ ] Hook path text is never executed, probed, normalized for execution, chmodded, or routed through host-gateway APIs.
- [ ] Profile mismatch disables or hides hook controls and does not save.
- [ ] Tests cover add, toggle, edit, remove, invalid rows, merged arrays, and save debounce.

## Completion Checklist

- [ ] All tasks completed.
- [ ] All validation commands pass.
- [ ] No new dependencies added.
- [ ] No Rust schema or migration files changed.
- [ ] No `callCommand('profile_save')` added outside existing profile persistence hooks.
- [ ] No direct launch/runtime/host-gateway hook execution code added.
- [ ] GitHub issue #471 scope is satisfied and #482 remains the runtime execution follow-up.

## Risks

| Risk                                                                     | Likelihood | Impact | Mitigation                                                                                      |
| ------------------------------------------------------------------------ | ---------- | ------ | ----------------------------------------------------------------------------------------------- |
| Hook edits update UI state but never save                                | Medium     | High   | Add `useHeroLaunchHooksAutosave` and fake-timer tests for `persistProfileDraft`                 |
| Hook controls save to the wrong profile during display/selected mismatch | Medium     | High   | Reuse existing mismatch guard and test no-save behavior                                         |
| UI implies hooks execute today                                           | Medium     | Medium | Keep banner always visible and avoid "run", "ready", or "validated" language                    |
| Hook paths become a security vector through scope creep                  | Low        | High   | Do not add launch request fields, path probing, shell calls, or host-gateway calls              |
| Mobile layout overflows on long paths                                    | Medium     | Medium | Add wrapping/min-width styles and manually inspect narrow layout if CSS changes are significant |
| Existing community exchange hardening regresses                          | Low        | High   | Do not change exchange files; run Rust hook tests as a regression gate                          |

## Notes

- The issue text says "400ms", but the current shared profile/launch debounce constant is `launchOptimizationsAutosaveDelayMs = 350`. Use the shared constant to keep Hero Detail consistent unless the implementation deliberately changes the shared cadence.
- `HeroDetailTabs` maps every tab, but Radix inactive-content mount semantics should not be treated as the save contract. The Launch-tab hook editor needs its own explicit autosave helper.
- Storage classification: hook arrays are user-editable TOML settings in `profile.toml`; there is no SQLite metadata and no runtime-only state beyond the temporary popover/open-row UI.
- Enhanced research dispatch used six sub-agents plus local synthesis because the runtime agent-thread limit blocked the seventh researcher. Coverage still included API, business, tech, UX, security, practices, and recommendations dimensions.
