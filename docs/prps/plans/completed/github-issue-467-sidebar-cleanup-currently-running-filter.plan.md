# Plan: GitHub Issue 467 sidebar cleanup + currently running filter

## Summary

Implement PRD Phase 2 by simplifying the sidebar Game section to `Library` only, then promoting `Favorites` and `Currently Playing` into first-class sidebar entries that open Library with an explicit filter state. The change adds a new `currentlyRunning` library filter key, exposes the existing runtime-only `LaunchSessionRegistry` as a lightweight read surface for active game-profile names, threads navigation options through shell routing, and introduces a dedicated running-profile hook so filtering is data-driven instead of hardcoded.

## User Story

As a CrossHook user, I want Favorites and Currently Playing to be directly selectable from the sidebar, so I can jump to focused Library views without extra chip clicks or route detours.

## Problem -> Solution

Current state: sidebar still exposes duplicate top-level `Profiles` and `Launch` entries in the Game section, while Favorites only exists as an in-page chip and there is no currently-running library filter.

Desired state: sidebar Game section contains only `Library`; Collections includes `Favorites` and `Currently Playing` as library-filter entries; AppShell forwards route options; Library page can apply `favorites` and `currentlyRunning` filters immediately; running-profile state is read from the in-memory launch-session registry with a safe empty-set fallback.

## Metadata

- **Complexity**: Medium
- **Source PRD**: `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`
- **PRD Phase**: Phase 2 - Sidebar cleanup + Favorites + Currently Playing
- **GitHub Issue**: [#467](https://github.com/yandy-r/crosshook/issues/467)
- **Estimated Files**: 19

---

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks         | Depends On | Parallel Width |
| ----- | ------------- | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3 | -          | 3              |
| B2    | 2.1, 2.2      | B1         | 2              |
| B3    | 3.1           | B2         | 1              |
| B4    | 4.1, 4.2      | B3         | 2              |

- **Total tasks**: 8
- **Total batches**: 4
- **Max parallel width**: 3

Same-file collision check: no two tasks in the same batch modify the same file.

---

## UX Design

### Before

- Sidebar Game section shows `Library`, `Profiles`, and `Launch`.
- Favorites is reachable only through the Library toolbar chip.
- No dedicated `Currently Playing` entry or running-status filter.

### After

- Sidebar Game section shows only `Library`.
- Sidebar Collections includes `Favorites` and `Currently Playing`.
- Clicking either entry routes to Library and preselects its matching filter chip.

### Interaction Changes

| Touchpoint                  | Before                          | After                                  | Notes                                            |
| --------------------------- | ------------------------------- | -------------------------------------- | ------------------------------------------------ |
| Sidebar Game section        | `Library`, `Profiles`, `Launch` | `Library` only                         | Aligns with consolidation direction in PRD       |
| Sidebar Collections section | Collection list only            | Collection list + fixed filter entries | Adds quick filters without replacing collections |
| Favorites access            | Toolbar chip only               | Sidebar entry + toolbar chip           | Both resolve to same `LibraryFilterKey`          |
| Running games view          | Not available as a filter       | `currentlyRunning` filter + chip       | Driven by a hook-backed `Set<string>`            |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                            | Lines           | Why                                                                        |
| -------------- | ------------------------------------------------------------------------------- | --------------- | -------------------------------------------------------------------------- |
| P0 (critical)  | `src/crosshook-native/src/components/layout/Sidebar.tsx`                        | 1-263           | Sidebar section model, route typing, and trigger rendering                 |
| P0 (critical)  | `src/crosshook-native/src/components/layout/AppShell.tsx`                       | 65-242, 326-462 | Central route state, sidebar wiring, and command routing                   |
| P0 (critical)  | `src/crosshook-native/src/components/layout/ContentArea.tsx`                    | 16-67           | `onNavigate` contract passed into pages                                    |
| P0 (critical)  | `src/crosshook-native/src/components/pages/LibraryPage.tsx`                     | 33-158, 281-340 | Filter state lifecycle and page-level routing handlers                     |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/session/registry.rs`     | 1-220           | In-memory launch-session source of truth                                   |
| P0 (critical)  | `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`                 | 1-75            | Lightweight launch read-command pattern                                    |
| P0 (critical)  | `src/crosshook-native/src-tauri/src/lib.rs`                                     | 430-475         | Tauri managed `LaunchSessionRegistry` and command registry                 |
| P1 (important) | `src/crosshook-native/src/components/library/LibraryToolbar.tsx`                | 1-87            | Filter chip options and aria-pressed behavior                              |
| P1 (important) | `src/crosshook-native/src/types/library.ts`                                     | 1-32            | Library filter type contract used across page + toolbar                    |
| P1 (important) | `src/crosshook-native/src/components/icons/SidebarIcons.tsx`                    | 1-140           | Existing sidebar icon style for Heart/Play additions                       |
| P1 (important) | `src/crosshook-native/src/lib/mocks/handlers/launch.ts`                         | 1-490           | Browser-dev launch mock state and running-game helpers                     |
| P1 (important) | `src/crosshook-native/src/lib/mocks/wrapHandler.ts`                             | 38-86           | Read-command allowlist for browser-dev error toggles                       |
| P1 (important) | `src/crosshook-native/src/components/layout/__tests__/Sidebar.test.tsx`         | 25-54           | Sidebar structure and section-order invariants                             |
| P1 (important) | `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`        | 1-140           | AppShell integration harness and sidebar-layout assertions                 |
| P1 (important) | `src/crosshook-native/src/components/library/__tests__/LibraryToolbar.test.tsx` | 18-85           | Filter event and keyboard/tab-order assertions                             |
| P1 (important) | `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx`      | 41-213          | Library harness pattern and filter/shell behavior test style               |
| P1 (important) | `src/crosshook-native/tests/smoke.spec.ts`                                      | 32-60, 190-335  | Browser smoke assertions currently expect Profiles/Launch sidebar triggers |
| P2 (reference) | `src/crosshook-native/src/hooks/useLaunchState.ts`                              | 230-270         | Existing `check_game_running` command usage pattern                        |
| P2 (reference) | `src/crosshook-native/package.json`                                             | 9-27            | Canonical typecheck/lint/test/smoke commands                               |

## External Documentation

| Topic | Source | Key Takeaway                                              |
| ----- | ------ | --------------------------------------------------------- |
| N/A   | N/A    | Internal codebase patterns are sufficient for this issue. |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION

```ts
// SOURCE: src/crosshook-native/src/components/pages/LibraryPage.tsx
const [filterKey, setFilterKey] = useState<LibraryFilterKey>('all');
```

Use `*Key` naming for filter state and `set*` setter naming for local page state.

### TYPE_DEFINITION

```ts
// SOURCE: src/crosshook-native/src/types/library.ts
export type LibraryFilterKey = 'all' | 'favorites' | 'installed' | 'recentlyLaunched';
```

Extend narrow unions in one source-of-truth type file, then consume that type from toolbar/page props.

### SIDEBAR_SECTION_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/layout/Sidebar.tsx
type SidebarSection = SidebarRouteSection | SidebarCollectionsSection;
const SIDEBAR_SECTIONS: SidebarSection[] = [
  /* ... */
];
```

Sidebar behavior is declarative via typed section variants plus a single render switch.

### RUNTIME_REGISTRY_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/session/registry.rs
pub fn sessions_for_profile(
    &self,
    profile_key: &str,
    kind_filter: Option<SessionKind>,
) -> Vec<SessionId> {
```

Keep launch-session state in `LaunchSessionRegistry`; add read-only accessors under the same short-lock pattern rather than duplicating runtime state elsewhere.

### IPC_READ_PATTERN

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/launch/queries.rs
#[tauri::command]
pub fn check_game_running(exe_name: String) -> bool {
    let name = exe_name.trim();
```

Expose running-session reads as snake_case `#[tauri::command]` functions in `queries.rs`, registered through `launch/mod.rs` and `src-tauri/src/lib.rs`.

### ROUTE_CONTRACT_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/layout/AppShell.tsx
<Sidebar activeRoute={route} onNavigate={setRoute} ... />
<ContentArea route={route} onNavigate={setRoute} ... />
```

Route updates originate in AppShell and are forwarded consistently into Sidebar + ContentArea.

### ERROR_HANDLING

```ts
// SOURCE: src/crosshook-native/src/hooks/useLaunchState.ts
void callCommand<boolean>('check_game_running', { exeName })
  .then((running) => {
    if (!cancelled) setIsGameRunning(running);
  })
  .catch(() => {
```

Runtime-status checks fail open and retry later. The `useRunningProfiles` hook should fall back to an empty set on command errors rather than throwing.

### LOGGING_PATTERN

```rust
// SOURCE: src/crosshook-native/src-tauri/src/lib.rs
if let Err(error) =
    startup::run_metadata_reconciliation(&metadata_for_startup, &profile_store)
{
    tracing::warn!(%error, "startup metadata reconciliation failed");
```

Use structured `tracing` warnings for recoverable backend degradation; do not print ad-hoc strings from Tauri commands.

### TEST_STRUCTURE

```tsx
// SOURCE: src/crosshook-native/src/components/library/__tests__/LibraryToolbar.test.tsx
await user.click(screen.getByRole('button', { name: 'Favorites' }));
expect(onFilterChange).toHaveBeenCalledWith('favorites');
```

Test filter behavior through accessible button labels and callback payload assertions.

### CONFIGURATION

```json
// SOURCE: src/crosshook-native/package.json
"typecheck": "tsc --noEmit && tsc -p tsconfig.test.json --noEmit",
"test": "vitest run",
"test:smoke": "playwright test"
```

Use package-defined scripts for verification; avoid ad-hoc command variants.

---

## Files to Change

| File                                                                            | Action | Justification                                                                                       |
| ------------------------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/launch/session/registry.rs`     | UPDATE | Add read-only active game-profile accessor plus core tests                                          |
| `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`                 | UPDATE | Add `list_running_profiles` Tauri read command over `LaunchSessionRegistry`                         |
| `src/crosshook-native/src-tauri/src/commands/launch/mod.rs`                     | UPDATE | Re-export generated command symbols for the new query                                               |
| `src/crosshook-native/src-tauri/src/lib.rs`                                     | UPDATE | Register `list_running_profiles` in `generate_handler!`                                             |
| `src/crosshook-native/src/lib/mocks/handlers/launch.ts`                         | UPDATE | Add browser-dev/test mock state for running profile names                                           |
| `src/crosshook-native/src/lib/mocks/wrapHandler.ts`                             | UPDATE | Treat `list_running_profiles` as read-only under `?errors=true`                                     |
| `src/crosshook-native/src/components/icons/SidebarIcons.tsx`                    | UPDATE | Add `HeartIcon` and `PlayIcon` matching existing sidebar icon style                                 |
| `src/crosshook-native/src/components/layout/Sidebar.tsx`                        | UPDATE | Add `library-filter` section-item variant and remove Game-section `profiles`/`launch` items         |
| `src/crosshook-native/src/components/layout/AppShell.tsx`                       | UPDATE | Expand route navigation contract to accept optional route intent payload                            |
| `src/crosshook-native/src/components/layout/ContentArea.tsx`                    | UPDATE | Forward richer `onNavigate` signature to Library and other page callers                             |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx`                     | UPDATE | Accept initial filter intent, apply `currentlyRunning` filtering, preserve existing detail behavior |
| `src/crosshook-native/src/components/library/LibraryToolbar.tsx`                | UPDATE | Add `Running` chip mapped to `currentlyRunning`                                                     |
| `src/crosshook-native/src/types/library.ts`                                     | UPDATE | Extend `LibraryFilterKey` with `currentlyRunning`                                                   |
| `src/crosshook-native/src/hooks/useRunningProfiles.ts`                          | CREATE | New reusable hook that returns running profile names as `Set<string>`                               |
| `src/crosshook-native/src/components/layout/__tests__/Sidebar.test.tsx`         | UPDATE | Assert Profiles/Launch removal and presence of new fixed filter entries                             |
| `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`        | UPDATE | Assert sidebar quick-filter navigation reaches Library with the requested filter                    |
| `src/crosshook-native/src/components/library/__tests__/LibraryToolbar.test.tsx` | UPDATE | Assert `Running` filter chip emits `currentlyRunning`                                               |
| `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx`      | UPDATE | Assert initial-filter navigation behavior and running-filter path                                   |
| `src/crosshook-native/tests/smoke.spec.ts`                                      | UPDATE | Remove Profiles/Launch from sidebar route loop and add sidebar quick-filter smoke coverage          |

## NOT Building

- AppRoute union shrink (`'profiles' | 'launch'` removal) - explicitly deferred to Phase 8.
- Deleting `ProfilesPage` / `LaunchPage` routes - handled by later PRD phases.
- Persisting running-profile state to TOML or SQLite - state stays runtime-only in `LaunchSessionRegistry`.
- Full Playwright route-suite rewrite - only update assertions affected by removing Profiles/Launch sidebar triggers.
- Any hero-detail tabs, hook schema, or route-deletion work from Phases 3+.

---

## Step-by-Step Tasks

### Task 1.1: LaunchSessionRegistry running-profile read surface - Depends on [none]

- **BATCH**: B1
- **ACTION**: Expose active game-profile names from the existing in-memory launch-session registry.
- **IMPLEMENT**: Add a read-only `LaunchSessionRegistry::active_profile_keys(kind_filter: Option<SessionKind>) -> Vec<String>` method that locks briefly, filters by kind, de-duplicates profile keys, and returns deterministic output. Add `list_running_profiles(session_registry: State<'_, Arc<LaunchSessionRegistry>>) -> Vec<String>` in `src-tauri/src/commands/launch/queries.rs`, filtering `SessionKind::Game`; re-export/register the command in `commands/launch/mod.rs` and `src-tauri/src/lib.rs`.
- **MIRROR**: `RUNTIME_REGISTRY_PATTERN` and `IPC_READ_PATTERN`.
- **IMPORTS**: `std::sync::Arc`, `tauri::State`, `LaunchSessionRegistry`, and `SessionKind` where needed.
- **GOTCHA**: Do not persist this data. It is runtime-only and should not touch metadata migrations or settings TOML.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::session::registry`

### Task 1.2: Library filter contract update (`currentlyRunning`) - Depends on [none]

- **BATCH**: B1
- **ACTION**: Extend library filter typing and toolbar chip options to expose running-state filtering.
- **IMPLEMENT**: Update `types/library.ts` to append `'currentlyRunning'` to `LibraryFilterKey`; update `LibraryToolbar.tsx` `FILTER_OPTIONS` with `{ key: 'currentlyRunning', label: 'Running' }`; add `HeartIcon` and `PlayIcon` in `SidebarIcons.tsx` using the existing `defaults` object and 20x20 stroke-only style.
- **MIRROR**: `TYPE_DEFINITION`, `CONFIGURATION`, and existing icon component style in `SidebarIcons.tsx`.
- **IMPORTS**: No new package imports expected.
- **GOTCHA**: Keep existing `'recentlyLaunched'` key even if not surfaced in toolbar; do not remove unrelated union members.
- **VALIDATE**: `cd src/crosshook-native && npm test -- --run src/components/library/__tests__/LibraryToolbar.test.tsx`

### Task 1.3: Browser-dev running-profile mock support - Depends on [none]

- **BATCH**: B1
- **ACTION**: Add deterministic browser-dev and test coverage support for the new running-profile command.
- **IMPLEMENT**: In `lib/mocks/handlers/launch.ts`, add a module-scope `runningProfiles: Set<string>`, register `list_running_profiles` to return a sorted string array, and add a dev/test helper like `_mock_set_profile_running` for targeted tests. Clear the set in `resetLaunchMockState`; add `list_running_profiles` to `EXPLICIT_READ_COMMANDS` in `wrapHandler.ts`.
- **MIRROR**: Existing `runningGames` and `_mock_set_game_running` mock-state pattern.
- **IMPORTS**: Existing mock handler types only.
- **GOTCHA**: Keep `_mock_set_profile_running` dev-only; production code must call only `list_running_profiles`.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck`

### Task 2.1: Sidebar section model + fixed library-filter entries - Depends on [1.2]

- **BATCH**: B2
- **ACTION**: Refactor sidebar section typing so Collections can render dynamic collections plus fixed `library-filter` entries, then remove Game `profiles`/`launch`.
- **IMPLEMENT**: In `Sidebar.tsx`, add a `SidebarLibraryFilterItem` variant with `{ type: 'library-filter'; filterKey: LibraryFilterKey; label; icon; badge? }`, keep route items distinct, and update render logic to call `onNavigate('library', { libraryFilter: filterKey })` for filter items. Game section keeps only `library`; Collections section includes `Favorites` and `Currently Playing`; render optional badge counts from props such as `libraryFilterBadges`.
- **MIRROR**: `SIDEBAR_SECTION_PATTERN` and existing `SidebarTrigger` accessible trigger pattern.
- **IMPORTS**: `LibraryFilterKey` type from `@/types/library`; `HeartIcon` and `PlayIcon` from the sidebar icon module.
- **GOTCHA**: Do not remove `AppRoute` union values in this phase; Phase 8 owns route shrink.
- **VALIDATE**: `cd src/crosshook-native && npm test -- --run src/components/layout/__tests__/Sidebar.test.tsx`

### Task 2.2: Add `useRunningProfiles` hook - Depends on [1.1, 1.3]

- **BATCH**: B2
- **ACTION**: Introduce a hook that derives currently running profile names from `LaunchSessionRegistry` through IPC.
- **IMPLEMENT**: Create `src/hooks/useRunningProfiles.ts` returning `Set<string>`. On mount, call `callCommand<string[]>('list_running_profiles')`, convert to a `Set`, refresh on a modest interval, and also refresh after `launch-complete` events through `subscribeEvent`; on command or subscription failure, keep or reset to an empty set without throwing.
- **MIRROR**: `ERROR_HANDLING`, `IPC_READ_PATTERN`, and the `subscribeEvent` wrapper usage in existing hooks.
- **IMPORTS**: `useEffect`, `useState`, `callCommand`, and `subscribeEvent`.
- **GOTCHA**: Return a `Set<string>` as the issue requires; if a `refresh` helper is useful internally, keep it inside the hook unless tests require exposure.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck`

### Task 3.1: Route intent plumbing + LibraryPage running-filter behavior - Depends on [2.1, 2.2]

- **BATCH**: B3
- **ACTION**: Thread optional navigation intent from sidebar through shell/content/page and apply filter intent in LibraryPage.
- **IMPLEMENT**: Add a shared navigation options type with `{ libraryFilter?: LibraryFilterKey; heroDetailTab?: HeroDetailTabId; profileName?: string }`; expand `onNavigate` in `Sidebar.tsx`, `AppShell.tsx`, `ContentArea.tsx`, and `LibraryPage.tsx`. In AppShell, store library filter intent with a monotonically changing token so repeated sidebar clicks on the same filter re-apply after local chip changes; pass that intent to LibraryPage. In `LibraryPage.tsx`, set `filterKey` from incoming intent and add `case 'currentlyRunning': list = list.filter((p) => runningProfiles.has(p.name));`.
- **MIRROR**: `ROUTE_CONTRACT_PATTERN`, `NAMING_CONVENTION`, and existing LibraryPage filter switch style.
- **IMPORTS**: `LibraryFilterKey`, `HeroDetailTabId`, and `useRunningProfiles`.
- **GOTCHA**: Preserve current launch/edit flows to `'profiles'` and `'launch'`; these routes still exist even though sidebar triggers are removed.
- **VALIDATE**: `cd src/crosshook-native && npm test -- --run src/components/pages/__tests__/LibraryPage.test.tsx src/components/layout/__tests__/AppShell.test.tsx`

### Task 4.1: Focused RTL and smoke coverage updates - Depends on [3.1]

- **BATCH**: B4
- **ACTION**: Expand existing test suites to lock the new behavior and prevent navigation regression.
- **IMPLEMENT**: In `Sidebar.test.tsx`, assert Game section no longer includes Profiles/Launch tabs and Collections exposes Favorites/Currently Playing controls with badge text when supplied. In `LibraryToolbar.test.tsx`, assert clicking `Running` emits `currentlyRunning` and update tab-order expectations. In `LibraryPage.test.tsx` and/or `AppShell.test.tsx`, assert intent-driven filter selection and running-filter behavior with `list_running_profiles` mocked to one profile. In `tests/smoke.spec.ts`, remove `profiles`/`launch` from the sidebar `ROUTE_ORDER`, use command-palette navigation for any remaining direct Profiles/Launch route smoke, and add a browser smoke for the Favorites/Currently Playing sidebar entries.
- **MIRROR**: `TEST_STRUCTURE` and existing `renderWithMocks` role-based assertions.
- **IMPORTS**: `userEvent`, `waitFor`, and existing test helper imports only.
- **GOTCHA**: Keep assertions role/label-driven; avoid brittle className selectors unless no accessible role exists.
- **VALIDATE**: `cd src/crosshook-native && npm test -- --run src/components/layout/__tests__/Sidebar.test.tsx src/components/layout/__tests__/AppShell.test.tsx src/components/library/__tests__/LibraryToolbar.test.tsx src/components/pages/__tests__/LibraryPage.test.tsx`

### Task 4.2: End-to-end regression check + command verification - Depends on [3.1]

- **BATCH**: B4
- **ACTION**: Confirm this phase does not regress baseline quality gates and route smoke.
- **IMPLEMENT**: Run project standard checks (`typecheck`, targeted tests, Rust registry tests, lint, smoke). If smoke assertions fail solely due to explicit sidebar label changes, update only the affected assertions while preserving route coverage intent.
- **MIRROR**: `CONFIGURATION` and the repository command reference in `AGENTS.md`.
- **IMPORTS**: N/A (verification task).
- **GOTCHA**: Do not preemptively rewrite smoke scenarios that belong to later PRD phases; keep this scoped to phase-2 behavior.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::session::registry && cd src/crosshook-native && npm run typecheck && npm test && npm run test:smoke`

---

## Testing Strategy

### Unit Tests

| Test                             | Input                                   | Expected Output                                                                       | Edge Case? |
| -------------------------------- | --------------------------------------- | ------------------------------------------------------------------------------------- | ---------- |
| Registry active profiles         | Register game + trainer sessions        | Game-profile accessor returns only active game profile names                          | Yes        |
| Running-profile command mock     | `_mock_set_profile_running` helper      | `list_running_profiles` returns deterministic profile-name array                      | Yes        |
| Sidebar fixed entries render     | Render `Sidebar` in full variant        | `Favorites` + `Currently Playing` present; `Profiles`/`Launch` absent in Game section | Yes        |
| Toolbar running chip             | Click `Running` chip                    | `onFilterChange('currentlyRunning')`                                                  | No         |
| Library currentlyRunning filter  | Mock running set with one profile       | Only running profile cards remain visible                                             | Yes        |
| Navigation intent filter handoff | Trigger sidebar library-filter navigate | Library route active + matching chip `aria-pressed="true"`                            | Yes        |
| Browser sidebar quick filters    | Click Favorites / Currently Playing     | Matching Library chip is active in browser smoke                                      | Regression |
| Existing favorite behavior       | Toggle Favorites entry/chip             | Favorites flow still works unchanged                                                  | Regression |

### Edge Cases Checklist

- [x] Empty running set shows an empty/filtered library state without crash
- [x] Running filter with no favorites still renders toolbar and page controls
- [x] Invalid command/error in running-status fetch falls back to empty set
- [x] Existing route-only sidebar entries still navigate as before
- [x] Repeated click on the same sidebar filter re-applies after user changes the toolbar chip
- [ ] High-frequency running-state churn (can be covered with lightweight polling debounce assertions)

---

## Validation Commands

### Static Analysis

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native
npm run typecheck
```

EXPECT: Zero type errors in app and test tsconfigs.

### Unit / Integration Tests

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native
npm test -- --run src/components/layout/__tests__/Sidebar.test.tsx \
  src/components/layout/__tests__/AppShell.test.tsx \
  src/components/library/__tests__/LibraryToolbar.test.tsx \
  src/components/pages/__tests__/LibraryPage.test.tsx
```

EXPECT: New + existing behavior tests pass.

### Rust Core Tests

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::session::registry
```

EXPECT: LaunchSessionRegistry active-profile accessor tests pass.

### Full Test Suite

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native
npm test
```

EXPECT: No regressions outside phase-2 scope.

### Lint

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook
./scripts/lint.sh
```

EXPECT: Biome/type checks pass for touched frontend files.

### Browser Validation

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native
npm run test:smoke
```

EXPECT: Existing smoke remains green; only explicitly affected assertions need updates.

---

## Acceptance Criteria

- [ ] Sidebar Game section renders with `Library` only.
- [ ] Sidebar includes `Favorites` and `Currently Playing` quick-filter entries.
- [ ] Clicking `Favorites` opens Library with Favorites chip active.
- [ ] Clicking `Currently Playing` opens Library with Running chip active.
- [ ] `LibraryFilterKey` includes `currentlyRunning`.
- [ ] Library toolbar exposes a `Running` filter chip.
- [ ] Library page correctly filters cards by running profile names.
- [ ] `list_running_profiles` reads active game-profile names from `LaunchSessionRegistry`.
- [ ] Browser-dev mocks expose deterministic running-profile state for tests and smoke.
- [ ] Existing typecheck, lint, unit tests, and smoke checks pass.

## Completion Checklist

- [ ] All 8 tasks completed with batch dependency order preserved.
- [ ] No AppRoute deletion/scope creep beyond phase-2 requirements.
- [ ] New hook and navigation contract are documented in code comments where non-obvious.
- [ ] Tests cover both positive path and fallback/error path for running filter data.
- [ ] No new dependency introduced.
- [ ] Plan remains self-contained for `/prp-implement` execution.

## Risks

| Risk                                                             | Likelihood | Impact | Mitigation                                                                    |
| ---------------------------------------------------------------- | ---------- | ------ | ----------------------------------------------------------------------------- |
| Running-status source is inconsistent in webdev/mock mode        | Medium     | Medium | Fail-open to empty set; assert deterministic mocks in tests                   |
| Navigation intent payload breaks existing `onNavigate` callers   | Medium     | High   | Keep payload optional and backwards compatible; typecheck all route callsites |
| Sidebar item model refactor introduces accessibility regressions | Low        | Medium | Preserve `Tabs.Trigger` role/aria patterns and verify with existing tests     |
| Smoke assertions tied to old labels/routes become flaky          | Medium     | Low    | Update only directly impacted assertions and keep scope narrow                |

## Notes

- Research dispatch used parallel `prp-researcher` passes across patterns/quality/infra categories.
- External research is not required for this issue because all contracts are internal.
- This plan intentionally aligns to PRD Phase 2 and defers route deletion + AppRoute shrink to later phases.
