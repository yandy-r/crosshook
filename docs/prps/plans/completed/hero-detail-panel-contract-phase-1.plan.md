# Plan: Hero Detail panel contract expansion (Phase 1)

## Summary

Extend `HeroDetailPanelsProps` with three new optional fields (`updateProfile`, `profileList`, `onSetActiveTab`) and thread them through `GameDetail` + `HeroDetailTabs`, without touching any panel body. Add `data-testid="hero-detail-profiles-tab"` / `"hero-detail-launch-tab"` to the Radix `<Tabs.Content>` roots so Phase 4/5 smoke tests have stable selectors. No user-visible change; all new fields are optional so every existing callsite keeps compiling.

## User Story

As a maintainer, I want the Hero Detail panel prop pipeline ready to mutate the active profile before any sub-tab rework lands, so subsequent phases (Profiles tab editor, Launch tab editor, Overview deep-links) don't need to refactor the pipeline on top of their own work.

## Problem → Solution

**Current state**: `HeroDetailPanelsProps` (`HeroDetailPanels.tsx:18-34`) has no write callbacks and no tab-switch callback. Panels are read-only. `HeroDetailPanelsProps.profile` already exists. Panel roots have no stable test selectors, so the Phase 4/5 Playwright rewrite would target brittle CSS classes.

**Desired state**: `HeroDetailPanelsProps` carries three new _optional_ fields — `updateProfile?: (draft: GameProfile) => Promise<void>`, `profileList?: ProfileSummary[]`, `onSetActiveTab?: (tab: HeroDetailTabId) => void` — threaded through `GameDetail.tsx:117-150` `panelProps` memo and forwarded by `HeroDetailTabs` via the existing `Omit<HeroDetailPanelsProps, 'mode'>` spread. Each tab root div carries a `data-testid`. All four existing test factories (`HeroDetailPanels.test.tsx`, `GameDetail.test.tsx`, `GameInspector.test.tsx`, `components.a11y.test.tsx`) accept the new fields with sensible defaults, and one new test asserts that omitting `updateProfile` still renders the read-only panels.

## Metadata

- **Complexity**: Small (6 files, ~40 LOC of functional change + test updates)
- **Source PRD**: `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`
- **PRD Phase**: Phase 1 — Hero Detail panel contract expansion + test harness
- **GitHub Issue**: [#466](https://github.com/yandy-r/crosshook/issues/466)
- **Estimated Files**: 6 source + 3 tests = 9 files total
- **Scope boundary**: Phase 1 only ships the prop shape and testids. Panel bodies still ignore the new props. Phases 4, 5, 6, 7 are the consumers — this plan deliberately leaves them unwired.

---

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order. Dependencies are enforced by the type system — Task 2.x reads the `HeroDetailPanelsProps` shape Task 1.2 emits, and Task 3.x reads both the testid and the shape.

| Batch | Tasks    | Depends On | Parallel Width |
| ----- | -------- | ---------- | -------------- |
| B1    | 1.1, 1.2 | —          | 2              |
| B2    | 2.1, 2.2 | B1         | 2              |
| B3    | 3.1, 3.2 | B2         | 2              |

- **Total tasks**: 6
- **Total batches**: 3
- **Max parallel width**: 2

Same-file collision check: no two tasks in the same batch touch the same file. B1 splits `hero-detail-model.ts` (1.1) vs. `HeroDetailPanels.tsx` (1.2). B2 splits `HeroDetailTabs.tsx` (2.1) vs. `GameDetail.tsx` (2.2). B3 splits `HeroDetailPanels.test.tsx` + `components.a11y.test.tsx` (3.1) vs. `GameDetail.test.tsx` + `GameInspector.test.tsx` (3.2).

---

## Worktree Setup

- **Parent**: `~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1/` (branch: `feat/hero-detail-panel-contract-phase-1`)
- **Children** (per parallel task; merged back at end of each batch):
  - Task 1.1 → `~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1-1-1/` (branch: `feat/hero-detail-panel-contract-phase-1-1-1`)
  - Task 1.2 → `~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1-1-2/` (branch: `feat/hero-detail-panel-contract-phase-1-1-2`)
  - Task 2.1 → `~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1-2-1/` (branch: `feat/hero-detail-panel-contract-phase-1-2-1`)
  - Task 2.2 → `~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1-2-2/` (branch: `feat/hero-detail-panel-contract-phase-1-2-2`)
  - Task 3.1 → `~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1-3-1/` (branch: `feat/hero-detail-panel-contract-phase-1-3-1`)
  - Task 3.2 → `~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1-3-2/` (branch: `feat/hero-detail-panel-contract-phase-1-3-2`)

**Worktree setup prerequisites** (per `CLAUDE.md` § Worktrees): both the parent and every child worktree need local `node_modules` before `./scripts/lint.sh` / `./scripts/format.sh` will work. Run once per worktree root:

```bash
npm install -D --no-save typescript@5.6.3 biome
cd src/crosshook-native && npm ci
```

---

## UX Design

### Before

No UI change.

### After

Identical pixels. `data-testid` attributes are non-visible DOM attributes and do not affect styling or behavior.

### Interaction Changes

| Touchpoint                                   | Before                     | After                                    | Notes                                                                                                      |
| -------------------------------------------- | -------------------------- | ---------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Hero Detail panel rendering                  | Receives 15 required props | Receives 15 required + 3 optional props  | Existing callers keep working; new callers opt in                                                          |
| `<Tabs.Content value="profiles">` root       | No `data-testid`           | `data-testid="hero-detail-profiles-tab"` | Stable smoke-test selector                                                                                 |
| `<Tabs.Content value="launch-options">` root | No `data-testid`           | `data-testid="hero-detail-launch-tab"`   | Stable smoke-test selector (note: PRD uses `launch` in the testid but the tab id remains `launch-options`) |

---

## Mandatory Reading

Files that MUST be read before implementing. All paths relative to repo root.

| Priority       | File                                                                              | Lines          | Why                                                                                                    |
| -------------- | --------------------------------------------------------------------------------- | -------------- | ------------------------------------------------------------------------------------------------------ |
| P0 (critical)  | `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`                | 1-40, 395-460  | Current `HeroDetailPanelsProps` shape + function-signature destructure + panel-mode switch             |
| P0 (critical)  | `src/crosshook-native/src/components/library/HeroDetailTabs.tsx`                  | 1-45           | Forwarder using `Omit<HeroDetailPanelsProps, 'mode'>` + Radix `<Tabs.Content>` tab-root location       |
| P0 (critical)  | `src/crosshook-native/src/components/library/GameDetail.tsx`                      | 40-50, 115-180 | `activeTab` owner + `panelProps` memo + `setActiveTab` wiring                                          |
| P0 (critical)  | `src/crosshook-native/src/components/library/hero-detail-model.ts`                | 1-15           | `HeroDetailTabId` + `HERO_DETAIL_TABS` catalog — helper additions land here                            |
| P1 (important) | `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx` | 1-170          | Inline `renderHeroDetailPanels` factory at L144-165 — must be extended with defaults                   |
| P1 (important) | `src/crosshook-native/src/__tests__/a11y/components.a11y.test.tsx`                | 170-200        | Second `HeroDetailPanels` factory used by a11y tests — must mirror L144-165 extension                  |
| P1 (important) | `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx`       | 1-60           | `renderGameDetail` helper (wraps `ProfileProvider`) — panelProps smoke updates plug in here            |
| P1 (important) | `src/crosshook-native/src/components/library/__tests__/GameInspector.test.tsx`    | 1-30           | `mount()` wraps `ProfileProvider` — reference for optional-prop-in-provider rendering idioms           |
| P2 (reference) | `src/crosshook-native/src/types/profile.ts`                                       | 90-175         | `GameProfile` hand-written interface; exact import path is `@/types/profile`                           |
| P2 (reference) | `src/crosshook-native/src/types/library.ts`                                       | 1-20           | `ProfileSummary` hand-written interface; exact import path is `@/types/library`                        |
| P2 (reference) | `src/crosshook-native/src/hooks/useProfile.ts`                                    | 75-95          | Pre-existing `updateProfile` signature (sync updater-fn form) — naming collision documented as gotcha  |
| P2 (reference) | `src/crosshook-native/src/context/ProfileContext.tsx`                             | 15-85          | `ProfileContextValue` surface + fail-fast provider-missing error                                       |
| P2 (reference) | `src/crosshook-native/src/test/fixtures.ts`                                       | 60-170         | `makeLibraryCardData` + `makeProfileDraft` — sources for `profileList` / `updateProfile` test defaults |
| P2 (reference) | `src/crosshook-native/biome.json`                                                 | 1-60           | Format rules: single-quote, `trailingCommas: es5`, semicolons, 120-col width, `useImportType`          |

## External Documentation

| Topic | Source | Key Takeaway                                                      |
| ----- | ------ | ----------------------------------------------------------------- |
| —     | —      | Pure internal prop-pipeline extension; no external docs required. |

---

## Patterns to Mirror

Code patterns discovered during research. Follow these exactly. Snippets trimmed to ≤5 lines each.

### NAMING_CONVENTION (optional props, ?: syntax)

```tsx
// SOURCE: src/crosshook-native/src/components/library/LibraryGrid.tsx:8-16
interface LibraryGridProps {
  launchingName?: string;
  onSelect?: (name: string) => void;
  onContextMenu?: (...) => void;
}
```

Convention: `?:` on the field, no `= () => {}` destructuring default, callers null-check or callees use optional chaining. `tsconfig.json` has `strict: true` but **not** `exactOptionalPropertyTypes` — so `?:` alone is safe.

### NAMING_CONVENTION (callback naming — NOTE the deliberate deviation)

```tsx
// SOURCE: src/crosshook-native/src/components/library/HeroDetailTabs.tsx:7
onActiveTabChange: (tab: HeroDetailTabId) => void;
```

**Gotcha**: Phase 1 intentionally adds `onSetActiveTab` (per the PRD / issue #466) alongside the existing `onActiveTabChange`. Repo convention would normally use `setX` or `onXChange`, but the PRD fixed the name to signal a **second channel** (panel-body → shell request) distinct from the existing Radix controlled-tabs callback. Document in code comments so later phases don't "normalize" it.

### NAMING_CONVENTION (imperative mutator, non-`on*`)

```ts
// SOURCE: src/crosshook-native/src/hooks/useProfile.ts:80
updateProfile: (updater: (current: GameProfile) => GameProfile) => void;
```

**Gotcha — signature collision**: Phase 1's `updateProfile: (draft: GameProfile) => Promise<void>` is a **new shape** that differs from the hook's existing sync updater-fn form. Document in a prop-level JSDoc so downstream phases know the Hero-Detail shape intentionally matches `profile_save` (async draft persist), not the `useProfile` in-memory updater.

### REPOSITORY_PATTERN (panelProps memo + dep array)

```tsx
// SOURCE: src/crosshook-native/src/components/library/GameDetail.tsx:117-134
const panelProps = useMemo(
  () => ({
    summary,
    steamAppId: steamAppIdForHooks,
    meta,
    profile,
    loadState,
    profileError: errorMessage,
    healthReport,
    healthLoading,
    /* …existing 14 keys… */
  }),
  [summary, steamAppIdForHooks, meta, profile, loadState /* …14-dep array… */]
);
```

Every new key must appear in **both** the object literal and the dep array — `useExhaustiveDependencies` biome rule enforces this.

### SERVICE_PATTERN (tabs root forwarder via Omit)

```tsx
// SOURCE: src/crosshook-native/src/components/library/HeroDetailTabs.tsx:5-9,37
panelProps: Omit<HeroDetailPanelsProps, 'mode'>;
// …inside render:
<HeroDetailPanels mode={tab.id} {...panelProps} />;
```

Adding optional fields to `HeroDetailPanelsProps` flows through the `Omit` automatically — no type edit needed in `HeroDetailTabs`. Testid injection is a separate concern: add a per-tab conditional on the `<Tabs.Content>` element.

### TEST_STRUCTURE (factory with partial overrides, no providers)

```tsx
// SOURCE: src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx:144-165
function renderHeroDetailPanels(overrides: Partial<HeroDetailPanelsProps> = {}) {
  const props: HeroDetailPanelsProps = {
    mode: 'launch-options', summary: makeLibraryCardData(), /* …defaults… */,
    ...overrides,
  };
  return render(<HeroDetailPanels {...props} />);
}
```

Factory spreads a full default `props` object (no `<ProfileProvider>` wrap). Phase 1 optional fields can be omitted from the defaults without breaking type checking, but it's cleaner to add explicit `undefined` defaults + a helper builder for `profileList` that uses `makeLibraryCardData()` (recall `LibraryCardData extends ProfileSummary`, so it's assignable).

### TEST_STRUCTURE (provider-wrapped factory)

```tsx
// SOURCE: src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx:22-40
renderWithMocks(
  <ProfileProvider>
    <PreferencesProvider>
      <GameDetail onBack={vi.fn()} onLaunch={vi.fn()} {...props} />
    </PreferencesProvider>
  </ProfileProvider>,
  options
);
```

Uses `renderWithMocks` from `src/test/render.tsx` which installs the IPC mock handler map. Callbacks default to `vi.fn()`. Mirror this idiom in any Phase 1 assertion that touches `profile_save` via `updateProfile`.

### ERROR_HANDLING (inline warn text, no console/throw)

```tsx
// SOURCE: src/crosshook-native/src/components/library/HeroDetailPanels.tsx:420-423
{
  loadState === 'error' ? (
    <p className="crosshook-hero-detail__warn">{profileError ?? 'Failed to load profile.'}</p>
  ) : null;
}
```

No `console.*`, no toast, no throw anywhere in the library tree. Phase 1 ships the **shape** of `updateProfile`; its future callers must catch rejected Promises and surface via an existing error-string prop — do **not** introduce a logger.

### LOGGING_PATTERN

Not applicable — library components have zero logging today. Don't introduce one.

### DATA_TESTID_CONVENTION

```tsx
// SOURCE: src/crosshook-native/src/components/library/GameDetail.tsx:153
<div className="crosshook-hero-detail" data-testid="game-detail">
```

Kebab-case, scoped to component/area, applied to the root wrapper element. Phase 1 targets: `hero-detail-profiles-tab`, `hero-detail-launch-tab` (literal strings mirroring the issue #466 body — note the shortened `launch` vs. the `launch-options` tab id).

---

## Files to Change

| File                                                                              | Action | Justification                                                                                  |
| --------------------------------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/library/hero-detail-model.ts`                | UPDATE | Add `heroDetailTabTestId(tabId)` helper + `HERO_DETAIL_TAB_TESTIDS` constant map               |
| `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`                | UPDATE | Add 3 optional fields to `HeroDetailPanelsProps`; destructure in function signature            |
| `src/crosshook-native/src/components/library/HeroDetailTabs.tsx`                  | UPDATE | Inject `data-testid` on `<Tabs.Content>` using the helper (only `profiles` + `launch-options`) |
| `src/crosshook-native/src/components/library/GameDetail.tsx`                      | UPDATE | Thread the 3 new values through `panelProps` memo object + dep array                           |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx` | UPDATE | Extend inline factory with defaults; add no-op-default test; assert testid presence            |
| `src/crosshook-native/src/__tests__/a11y/components.a11y.test.tsx`                | UPDATE | Mirror factory extension so a11y tests still compile with the new type shape                   |
| `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx`       | UPDATE | Smoke-assert new fields survive the panelProps hop (no behavior change)                        |
| `src/crosshook-native/src/components/library/__tests__/GameInspector.test.tsx`    | UPDATE | Only if type-checker flags it (`GameInspector` indirectly consumes `HeroDetailPanelsProps`)    |

## NOT Building

- **Consumer logic inside panel bodies** — `ProfilesPanel` still reads `useProfileContext()`; `launch-options` case still renders the current structured preview. Phases 4 and 5 migrate these to props.
- **`ProfilesProvider` refactors** or new context surfaces — the singleton `ProfileProvider` stays. Phase 1 does not touch context.
- **Changes to `useProfile.ts` or `useProfileContext`** — the existing `updateProfile: (updater) => void` stays. Phase 1's Hero-Detail-prop `updateProfile: (draft) => Promise<void>` is a new, separate surface; wiring it to `profile_save` is a Phase 4/5 concern.
- **Testid on other tabs** (`overview`, `trainer`, `history`, `compatibility`) — only `profiles` and `launch-options` get testids, per issue #466. Do not pre-emptively add the rest.
- **New test fixtures** beyond what `makeLibraryCardData` / `makeProfileDraft` already provide.
- **New IPC commands, new Rust code, new Tauri commands** — backend untouched.
- **Changes to `biome.json`, `tsconfig.json`, `package.json`, `vitest.config.ts`** — Phase 1 fits within existing tooling.
- **Smoke-test updates** — Playwright `tests/smoke.spec.ts` is explicitly a Phase 11 concern. Phase 1 only ships the testids so Phase 11 has stable selectors to target.
- **Documentation outside the code** — no `docs/internal-docs/design-tokens.md` entry (Phase 12 handles that). No `CHANGELOG.md` edit (cliff-generated on release).

---

## Step-by-Step Tasks

### Task 1.1: Add `heroDetailTabTestId` helper to `hero-detail-model.ts` — Depends on [none]

- **BATCH**: B1
- **Worktree**: `~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1-1-1/` (branch: `feat/hero-detail-panel-contract-phase-1-1-1`)
- **ACTION**: Extend `src/crosshook-native/src/components/library/hero-detail-model.ts` with a testid helper so `HeroDetailTabs` can look up the stable attribute without string duplication, and so future phases have a single source of truth for testids.
- **IMPLEMENT**: After the existing `HERO_DETAIL_TABS` export, add a `Partial<Record<HeroDetailTabId, string>>` constant `HERO_DETAIL_TAB_TESTIDS` with entries only for `profiles: 'hero-detail-profiles-tab'` and `'launch-options': 'hero-detail-launch-tab'`. Export a helper `heroDetailTabTestId(tabId: HeroDetailTabId): string | undefined` that returns the map entry. Do **not** add entries for `overview`, `trainer`, `history`, `compatibility` (per PRD Phase 1 scope).
- **MIRROR**: Module-level hand-written const exports in this same file (`HERO_DETAIL_TABS as const`). Single-quote strings, trailing commas, semicolons (biome `trailingCommas: es5`, single-quote, semicolons-always).
- **IMPORTS**: No new imports. `HeroDetailTabId` is already declared in this file.
- **GOTCHA**: The issue body writes `hero-detail-launch-tab` (shortened) while the tab id remains `'launch-options'`. Do not rename the tab id. Keep the mapping deliberate and comment the asymmetry.
- **VALIDATE**:
  - `cd src/crosshook-native && npx tsc --noEmit` — zero errors.
  - `(cd src/crosshook-native && npx @biomejs/biome check src/components/library/hero-detail-model.ts)` — zero warnings.

### Task 1.2: Extend `HeroDetailPanelsProps` shape + destructure — Depends on [none]

- **BATCH**: B1
- **Worktree**: `~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1-1-2/` (branch: `feat/hero-detail-panel-contract-phase-1-1-2`)
- **ACTION**: Add three optional fields to `HeroDetailPanelsProps` (`HeroDetailPanels.tsx:18-34`) and destructure them at the function signature (`HeroDetailPanels.tsx:399-415`). No panel-body changes — the new values are unconsumed, solely to unblock downstream phases.
- **IMPLEMENT**:
  - Add to the interface:
    - `updateProfile?: (draft: GameProfile) => Promise<void>;` — JSDoc above: "Phase 1 channel: intentionally async-draft shape, differs from `useProfile.ts#updateProfile` (sync updater). Consumed by Phase 4/5."
    - `profileList?: ProfileSummary[];` — JSDoc: "Phase 1 channel: left-list cards source for Phase 4 Profiles tab."
    - `onSetActiveTab?: (tab: HeroDetailTabId) => void;` — JSDoc: "Phase 1 channel: panel-body → shell request, distinct from `HeroDetailTabs#onActiveTabChange`. Consumed by Phase 7 Overview deep-links."
  - Add `ProfileSummary` import from `@/types/library` if not present. Use `import type`.
  - Destructure the three new names in the function signature; do **not** assign defaults (`?.()` at call sites is the library convention).
  - Do **not** add `data-testid` to any `<div>` in this file — testid work lives in Task 2.1 on `HeroDetailTabs`.
- **MIRROR**: Destructure-at-signature pattern (`HeroDetailPanels.tsx:399-415`). Optional-field syntax from `LibraryGrid.tsx:8-16`.
- **IMPORTS**: `import type { ProfileSummary } from '@/types/library';` (biome `useImportType: warn` forces `import type`). `GameProfile` and `HeroDetailTabId` are already imported in this file.
- **GOTCHA**: The existing destructured list is long — biome will reformat it automatically if line-width >120 cols. Do not hand-format; let `biome format --write` handle wrapping.
- **VALIDATE**:
  - `cd src/crosshook-native && npx tsc --noEmit` — zero errors (both tsconfigs via `npm run typecheck`).
  - `(cd src/crosshook-native && npx @biomejs/biome check src/components/library/HeroDetailPanels.tsx)` — zero warnings; `useImportType` passing.
  - `grep -n "updateProfile\|profileList\|onSetActiveTab" src/crosshook-native/src/components/library/HeroDetailPanels.tsx` — 3 new symbols appear in both the interface and the destructure.

### Task 2.1: Inject `data-testid` on `<Tabs.Content>` in `HeroDetailTabs.tsx` — Depends on [1.1]

- **BATCH**: B2
- **Worktree**: `~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1-2-1/` (branch: `feat/hero-detail-panel-contract-phase-1-2-1`)
- **ACTION**: Use the `heroDetailTabTestId` helper (Task 1.1) to apply `data-testid` to the `<Tabs.Content>` root for each tab where the helper returns a string. Tabs with no mapped testid (overview, trainer, history, compatibility) get no attribute.
- **IMPLEMENT**:
  - Import `heroDetailTabTestId` from `./hero-detail-model`.
  - Inside the `HERO_DETAIL_TABS.map(...)` iteration (`HeroDetailTabs.tsx:29-40`), compute `const testId = heroDetailTabTestId(tab.id);` once per tab, then spread `{...(testId ? { 'data-testid': testId } : {})}` onto the `<Tabs.Content>` element (keep existing `value`, `className`, other attrs).
  - Do **not** add testids to the inner `<div className="crosshook-subtab-content__inner ...">` — the issue body says "tab root divs" which maps to `<Tabs.Content>` (Radix renders a `div` by default for this primitive).
- **MIRROR**: Existing `data-testid="game-detail"` on the wrapper element (`GameDetail.tsx:153`). Conditional spread pattern (common React idiom; no project-specific code to mirror).
- **IMPORTS**: `import { heroDetailTabTestId } from './hero-detail-model';` alongside the existing `HERO_DETAIL_TABS` import — combine into one line.
- **GOTCHA**: Radix `<Tabs.Content>` forwards unknown props to the underlying DOM element, so `data-testid` lands on the real DOM — no `asChild` needed. Keep the attribute conditional; don't emit `data-testid={undefined}` because that pollutes snapshots and `@testing-library` queries.
- **VALIDATE**:
  - `cd src/crosshook-native && npx tsc --noEmit` — zero errors.
  - `(cd src/crosshook-native && npx @biomejs/biome check src/components/library/HeroDetailTabs.tsx)` — zero warnings.
  - `npm test -- --run src/components/library/__tests__/HeroDetailPanels.test.tsx` — existing tests still green (no behavioral change).

### Task 2.2: Thread new props through `GameDetail.panelProps` — Depends on [1.2]

- **BATCH**: B2
- **Worktree**: `~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1-2-2/` (branch: `feat/hero-detail-panel-contract-phase-1-2-2`)
- **ACTION**: Extend the `panelProps` `useMemo` in `GameDetail.tsx` (L117-150) with three new optional entries. Since Phase 1 has no caller that provides real values yet, each new key resolves to `undefined` — this is the explicit "shape-only" Phase 1 contract.
- **IMPLEMENT**:
  - In the `panelProps` object literal, after `previewError`, add three new keys assigned to `undefined`: `updateProfile: undefined`, `profileList: undefined`, `onSetActiveTab: undefined`. Keep trailing commas per biome `es5`.
  - Add TODO comments above each line referencing the phase that will populate it: `// TODO(phase-4): wire via useProfileContext().updateProfile wrapper`, `// TODO(phase-4): wire via useProfileSummaries()`, `// TODO(phase-7): wire via setActiveTab from L42 useState`.
  - **Do NOT** add them to the dep array — `undefined` literals are not reactive; `useExhaustiveDependencies` will not flag them. (Verify with biome after edit.)
  - **Alternative path**: if biome's `useExhaustiveDependencies` flags `undefined` literals, wrap them in `useMemo(() => undefined, [])` or (cleaner) move the placeholders into a `useMemo`-external `const PHASE_1_PLACEHOLDERS = { updateProfile: undefined, profileList: undefined, onSetActiveTab: undefined } as const;` at module scope and spread it. Pick whichever form biome accepts.
- **MIRROR**: Existing `panelProps` assembly (`GameDetail.tsx:117-134`). Match exact key ordering convention (data fields first, then async state, then callbacks — follow the established cluster).
- **IMPORTS**: None — no new symbols referenced.
- **GOTCHA**: The type check in Task 1.2 must be merged (or the parent branch must carry Task 1.2's change) before this worktree's `tsc` will pass. The `Depends on [1.2]` annotation is enforced by this constraint.
- **VALIDATE**:
  - `cd src/crosshook-native && npx tsc --noEmit` — zero errors.
  - `(cd src/crosshook-native && npx @biomejs/biome check src/components/library/GameDetail.tsx)` — zero warnings; no `useExhaustiveDependencies` flag.
  - Manually confirm the new keys flow through: `grep -n "updateProfile\|profileList\|onSetActiveTab" src/crosshook-native/src/components/library/GameDetail.tsx` — three matches inside `panelProps`.

### Task 3.1: Update `HeroDetailPanels.test.tsx` + a11y factory; add no-op-default test — Depends on [1.2, 2.1]

- **BATCH**: B3
- **Worktree**: `~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1-3-1/` (branch: `feat/hero-detail-panel-contract-phase-1-3-1`)
- **ACTION**: Extend both inline factories (`HeroDetailPanels.test.tsx:144-165` and `components.a11y.test.tsx:170-200`) with defaults for the three new optional fields. Add a new test case asserting that omitting `updateProfile` still renders the read-only panels without calling it. Add a test asserting `data-testid="hero-detail-profiles-tab"` and `data-testid="hero-detail-launch-tab"` appear on the respective tab roots when rendered via `HeroDetailTabs`.
- **IMPLEMENT**:
  - In `HeroDetailPanels.test.tsx` factory at L144-165, add to the `props` literal: `updateProfile: undefined`, `profileList: undefined`, `onSetActiveTab: undefined`. Keep them optional in the factory (no override of the Partial pattern — sensible defaults, still overridable).
  - Add a new `describe('no-op defaults')` block with a test:
    ```tsx
    it('renders read-only panels when updateProfile is omitted', () => {
      renderHeroDetailPanels({ mode: 'profiles', updateProfile: undefined, profileList: undefined });
      // Assertion: no throw, panel root exists, no call to any mock mutation
      expect(screen.getByText(/active profile/i)).toBeInTheDocument();
    });
    ```
    Tune the assertion to text currently rendered by `ProfilesPanel` (e.g. the "No active profile loaded..." branch). The goal is a smoke that says "render succeeds with the Phase 1 contract unwired".
  - Add a second test for testid presence — but this requires rendering `HeroDetailTabs`, not `HeroDetailPanels` directly (testids live on `<Tabs.Content>`, not inside panels). Either:
    1. Add the testid test to `components.a11y.test.tsx` where `HeroDetailTabs` is already rendered, or
    2. Add a small new test file `HeroDetailTabs.test.tsx` at `src/crosshook-native/src/components/library/__tests__/HeroDetailTabs.test.tsx` with one `describe('data-testid')` block.
       **Prefer option 1** (no new file) — in `components.a11y.test.tsx`, add two assertions: `screen.getByTestId('hero-detail-profiles-tab')` for `activeTab='profiles'` render and `screen.getByTestId('hero-detail-launch-tab')` for `activeTab='launch-options'` render.
  - In `components.a11y.test.tsx` factory, mirror the L144-165 default additions so the a11y test inputs compile against the new `HeroDetailPanelsProps` shape.
- **MIRROR**: `HeroDetailPanels.test.tsx:229-238` (existing read-only / null-inputs idiom using `renderHeroDetailPanels({ launchRequest: null, preview: null })`). Apply same "omit and assert render succeeds" pattern.
- **IMPORTS**: Already-present `render, screen, within` from `@testing-library/react`; no new imports unless adding `userEvent` for a negative "no-call" assertion (if doing so, `expect(mockFn).not.toHaveBeenCalled();`).
- **GOTCHA**: Radix `<Tabs.Content>` only mounts its children when `value === activeTab`. Testid assertions must select the active tab first (or pass `forceMount` — but don't; we want the real behavior). If running two testid assertions in one test, re-render with `activeTab` changed between them, or use two separate `it(...)` blocks.
- **VALIDATE**:
  - `npm test -- --run src/components/library/__tests__/HeroDetailPanels.test.tsx src/__tests__/a11y/components.a11y.test.tsx` — all green, including new cases.
  - `cd src/crosshook-native && npx tsc -p tsconfig.test.json --noEmit` — zero errors.
  - `(cd src/crosshook-native && npx @biomejs/biome check src/components/library/__tests__/HeroDetailPanels.test.tsx src/__tests__/a11y/components.a11y.test.tsx)` — zero warnings.

### Task 3.2: Update `GameDetail.test.tsx` + `GameInspector.test.tsx` smoke — Depends on [2.2]

- **BATCH**: B3
- **Worktree**: `~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1-3-2/` (branch: `feat/hero-detail-panel-contract-phase-1-3-2`)
- **ACTION**: Confirm existing `GameDetail.test.tsx` still compiles and passes with the extended `panelProps` shape. Add one focused regression case asserting the three new fields propagate through `panelProps` without error when `<GameDetail>` renders. Update `GameInspector.test.tsx` only if the type-checker flags it (indirect consumer).
- **IMPLEMENT**:
  - In `GameDetail.test.tsx`, add a single new test to the existing `describe` block:
    ```tsx
    it('forwards phase-1 panel-contract placeholders through panelProps', async () => {
      renderGameDetail({ summary: makeLibraryCardData() });
      const root = await screen.findByTestId('game-detail');
      expect(root).toBeInTheDocument();
      // No direct assertion on the placeholder values — their presence is enforced by TS.
      // If any placeholder leaks as a visible string, the test will fail on render.
    });
    ```
    The point is a runtime-render smoke; compile-time enforcement is already handled by `panelProps: Omit<HeroDetailPanelsProps, 'mode'>`.
  - Run `npm run typecheck` — if `GameInspector.test.tsx` fails because of its `ProfileProvider`-wrapped render calling a surface that changed shape, add the same three default-`undefined` entries to whatever factory is flagged. If it doesn't fail, leave the file untouched (no speculative edits).
  - Do **not** add a `profile_save` mock assertion — `updateProfile` is intentionally unwired in Phase 1.
- **MIRROR**: `GameDetail.test.tsx:18-40` (`renderGameDetail` helper, `vi.fn()` defaults). `await screen.findByTestId('game-detail')` matches the existing idiom for awaiting hydration.
- **IMPORTS**: Already-present imports; `makeLibraryCardData` from `@/test/fixtures` is already imported.
- **GOTCHA**: `GameDetail` hydrates `profile` via `useGameDetailsProfile` which calls `profile_load`. The IPC mock handler map must already cover `profile_load` — confirm in `src/lib/mocks/handlers/profile-core.ts` before relying on it. If the handler is missing, the test will throw `[test-mock] Unhandled command: profile_load`; the fix is to add a handler to `handlerOverrides` in the `renderWithMocks` call, not to disable the test.
- **VALIDATE**:
  - `npm test -- --run src/components/library/__tests__/GameDetail.test.tsx src/components/library/__tests__/GameInspector.test.tsx` — all green.
  - `cd src/crosshook-native && npm run typecheck` — both tsconfigs zero-error.
  - `(cd src/crosshook-native && npx @biomejs/biome check src/components/library/__tests__/GameDetail.test.tsx)` — zero warnings.

---

## Testing Strategy

### Unit Tests

| Test                                                         | Input                                                                 | Expected Output                                            | Edge Case?          |
| ------------------------------------------------------------ | --------------------------------------------------------------------- | ---------------------------------------------------------- | ------------------- |
| `renderHeroDetailPanels` factory accepts new optional fields | `overrides = { profileList: undefined }`                              | Renders without throw                                      | No                  |
| Omitting `updateProfile` — no-op default path                | `overrides = { mode: 'profiles', updateProfile: undefined }`          | Panel renders; no throw; no mock call                      | **Yes** (new idiom) |
| `data-testid="hero-detail-profiles-tab"` on profiles tab     | `<HeroDetailTabs activeTab='profiles' panelProps={defaults} />`       | `getByTestId('hero-detail-profiles-tab')` succeeds         | No                  |
| `data-testid="hero-detail-launch-tab"` on launch-options tab | `<HeroDetailTabs activeTab='launch-options' panelProps={defaults} />` | `getByTestId('hero-detail-launch-tab')` succeeds           | No                  |
| `heroDetailTabTestId` returns undefined for unmapped tabs    | `heroDetailTabTestId('overview')`                                     | `undefined`                                                | Yes (negative path) |
| `GameDetail` forwards phase-1 panel-contract placeholders    | `renderGameDetail({ summary: makeLibraryCardData() })`                | `data-testid="game-detail"` root present; no compile error | No                  |
| A11y factory still compiles with extended type               | `components.a11y.test.tsx` suite                                      | `jest-axe` run passes as before                            | No                  |

### Edge Cases Checklist

- [x] **Empty input** — all three new fields accept `undefined`; render paths verified.
- [x] **Omitted callback** — `updateProfile` undefined; no-op-default test asserts render succeeds and no mock fires.
- [ ] **Maximum size input** — N/A (Phase 1 adds shape only, no list rendering).
- [x] **Invalid types** — prevented by TypeScript (strict mode).
- [ ] **Concurrent access** — N/A (no async state wired in Phase 1).
- [ ] **Network failure** — N/A (no IPC wired in Phase 1; `profile_save` is a Phase 4/5 concern).
- [ ] **Permission denied** — N/A.

### Manual Validation

- [ ] Open `./scripts/dev-native.sh --browser` and confirm Hero Detail opens without regression (click a library card, verify the existing Overview / Profiles / Launch tabs render their current content).
- [ ] Inspect DOM: confirm `document.querySelector('[data-testid="hero-detail-profiles-tab"]')` returns an element when Profiles tab is active.
- [ ] Confirm zero visible UI change by eye.

---

## Validation Commands

### Static Analysis

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native
npm run typecheck
```

EXPECT: Zero type errors across both `tsconfig.json` and `tsconfig.test.json`.

### Lint

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook
./scripts/lint.sh
```

EXPECT: Biome + tsc + shellcheck + host-gateway + legacy-palette all green. (Rust clippy and cargo fmt also run but Phase 1 makes no Rust changes, so they should be no-ops.)

### Format check

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook
./scripts/format.sh
```

EXPECT: Idempotent — if Phase 1 code is formatted correctly, this is a no-op. Run with `--fix` semantics (biome writes in place), then re-run `git diff --stat` to confirm zero unexpected reformats outside the edited files.

### Unit Tests

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native
npm test -- --run src/components/library/__tests__/HeroDetailPanels.test.tsx \
  src/components/library/__tests__/GameDetail.test.tsx \
  src/components/library/__tests__/GameInspector.test.tsx \
  src/__tests__/a11y/components.a11y.test.tsx
```

EXPECT: All files green, including the new no-op-default and testid cases.

### Full Test Suite

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native
npm test
```

EXPECT: No regressions across the full Vitest suite. Phase 1 is purely additive; pre-existing tests should stay untouched.

### Rust (sanity only — no changes expected)

```bash
cargo test --manifest-path /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: Green. Phase 1 makes no Rust changes; this is a regression-sanity check only.

### Browser Validation (optional smoke)

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook
./scripts/dev-native.sh --browser
# In another terminal:
npm --prefix src/crosshook-native run test:smoke
```

EXPECT: Playwright smoke passes with existing assertions. Phase 1 does **not** add new smoke — testids are dormant until Phase 11.

---

## Acceptance Criteria

- [ ] All 6 tasks completed across 3 batches.
- [ ] `HeroDetailPanelsProps` compiles with three new optional fields: `updateProfile`, `profileList`, `onSetActiveTab`.
- [ ] `heroDetailTabTestId('profiles')` returns `'hero-detail-profiles-tab'`; `heroDetailTabTestId('launch-options')` returns `'hero-detail-launch-tab'`; other tabs return `undefined`.
- [ ] `<Tabs.Content value="profiles">` renders `data-testid="hero-detail-profiles-tab"`; same for `launch-options`.
- [ ] `GameDetail.panelProps` memo includes all three new keys (as `undefined` placeholders in Phase 1).
- [ ] All pre-existing RTL tests untouched and still green.
- [ ] New "no-op default" test passes (`updateProfile` omitted → render succeeds).
- [ ] New testid-presence tests pass.
- [ ] `npm test` green.
- [ ] `npm run typecheck` green across both tsconfigs.
- [ ] `./scripts/lint.sh` green.
- [ ] No visible UI change (manual dev-native browser check).

## Completion Checklist

- [ ] Code follows discovered patterns (optional `?:` syntax; no destructuring defaults; single-quote; trailing-comma-es5).
- [ ] Error handling matches codebase style (N/A in Phase 1 — no error paths introduced).
- [ ] Logging follows codebase conventions (N/A — library tree has zero logging, no change).
- [ ] Tests follow test patterns (partial-override factory; `renderWithMocks` where `ProfileProvider` needed; `vi.fn()` defaults for callbacks).
- [ ] No hardcoded values outside the two testid literals (kept in `HERO_DETAIL_TAB_TESTIDS` constant, not sprinkled).
- [ ] Documentation updated: JSDoc on each new `HeroDetailPanelsProps` field naming the consuming phase and the collision (for `updateProfile`) with the existing `useProfile` shape.
- [ ] No unnecessary scope additions — testid on **only** `profiles` + `launch-options`; no other tabs pre-emptively instrumented.
- [ ] Self-contained — no questions needed during implementation. All decisions resolved in the PRD Phase 1 section + research table above.

## Risks

| Risk                                                                                                                  | Likelihood    | Impact                          | Mitigation                                                                                                                                                                                                                                                           |
| --------------------------------------------------------------------------------------------------------------------- | ------------- | ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `useExhaustiveDependencies` flags `undefined`-placeholder keys in `panelProps` memo                                   | M             | L (easy to re-shape)            | Task 2.2 documents the fallback: hoist placeholders to a module-level `as const` object and spread; or keep inline if biome accepts primitives. Decide at implementation time.                                                                                       |
| Naming collision — new `updateProfile` shape (async draft) shadows existing `useProfile.updateProfile` (sync updater) | M             | M (readability / refactor cost) | JSDoc on the new field spelling out the deliberate split. Later phases consuming both must rename one (likely `persistProfile` or `saveProfile` for the async shape) — flagged as Phase 4/5 decision, not Phase 1.                                                   |
| Naming collision — `onSetActiveTab` vs. existing `onActiveTabChange`                                                  | L             | L                               | JSDoc explaining the two-channel model (Radix controlled-tabs vs. panel-body → shell request).                                                                                                                                                                       |
| Factory-order mismatch between `HeroDetailPanels.test.tsx` and `components.a11y.test.tsx`                             | M             | L                               | Apply the exact same `undefined` defaults in both; add a comment pointing to the sibling file so future edits stay in sync. A test fixture extraction to `src/test/fixtures/heroDetailPanels.ts` is tempting but explicitly **out of scope** (flagged NOT Building). |
| Radix `<Tabs.Content>` unmounts inactive tabs — testid assertion for hidden tab fails                                 | M             | L                               | Task 3.1 mandates one-active-tab-per-assertion (either separate `it` blocks or re-render with changed `activeTab`). Do not use `forceMount`.                                                                                                                         |
| Worktree dependency chain bleed — Task 2.2 requires 1.2's type change, Task 2.1 requires 1.1's helper                 | H (by design) | M                               | Batch orchestrator (`/ycc:prp-implement --parallel`) merges B1 children back into parent before B2 starts (per `worktree-strategy.md` fan-in protocol). If running tasks manually, confirm B1 merged first.                                                          |
| `profile_load` unhandled mock in Task 3.2's new `GameDetail` render                                                   | L             | L                               | Task 3.2 GOTCHA flags this; fix is adding handler to `handlerOverrides`, not disabling the test.                                                                                                                                                                     |

## Notes

- **Why three fields, not four**: the issue body and PRD Phase 1 list four (`profile`, `updateProfile`, `profileList`, `onSetActiveTab`). Research confirms `profile: GameProfile | null` already exists on `HeroDetailPanelsProps:22`. Phase 1 adds three; the fourth is a no-op for the shape, though the plan treats the `profile` field as "already present, document it". No behavior change either way.
- **Why a helper for testids**: two string literals duplicated across `HeroDetailPanels.tsx` and `HeroDetailTabs.tsx` is minor today, but Phases 4/5/11 will introduce more testids on hero-detail panels and smoke tests. A single-source helper in `hero-detail-model.ts` is consistent with how `HERO_DETAIL_TABS` already centralizes tab metadata.
- **Why testid on `<Tabs.Content>`, not on panels**: Radix `<Tabs.Content>` is the primitive that owns the tab-root semantics (role=`tabpanel`, data-state, etc.). Phase 11 smoke tests need a stable selector for "the active tab's panel container". Putting it on `<Tabs.Content>` means Phase 11 doesn't need to drill into per-panel JSX — a `screen.getByTestId('hero-detail-profiles-tab')` always returns the currently-mounted profiles panel container.
- **Why no new Rust / no new IPC**: Phase 1 is a frontend-only prop-pipeline extension. Backend work for the PRD lives in Phase 3 (hook schema) and is unrelated to this plan.
- **Why placeholder `undefined` values in `panelProps`**: deliberate — Phase 1 is the "shape-only" phase. Wiring live values (from `useProfileContext`, `useProfileSummaries`, `setActiveTab`) lands in Phases 4/5/7 where each consumer exists. Populating them in Phase 1 without consumers would force a decision on naming (e.g. `updateProfile` vs. `saveProfile` vs. `persistProfile`) that the PRD intentionally defers.
- **Parallel-batch safety**: task-to-file mapping audit completed:
  - B1: `hero-detail-model.ts` (1.1) ≠ `HeroDetailPanels.tsx` (1.2) — no collision
  - B2: `HeroDetailTabs.tsx` (2.1) ≠ `GameDetail.tsx` (2.2) — no collision
  - B3: `HeroDetailPanels.test.tsx` + `components.a11y.test.tsx` (3.1) ≠ `GameDetail.test.tsx` + `GameInspector.test.tsx` (3.2) — no collision
- **Commit style** (per `CLAUDE.md`): PRs for this plan should use `feat(ui): extend Hero Detail panel contract (phase 1)` or similar Conventional-Commits form, with `Part of #466` in the body.
- **Rollback**: if anything goes wrong, revert is trivial — the three new fields are optional, the helper is self-contained, and the testid injection is a single-file conditional. Each task is independently revertable.
