# Plan: Unified Desktop Redesign — Phase 4 (Library + Inspector Rail + routeMetadata Contract)

## Summary

Phase 4 of the Unified Desktop Redesign: redesign the Library surface (`LibraryCard` chrome,
`LibraryGrid`, `LibraryToolbar`) and introduce a persistent right-side `Inspector` rail driven
by a new optional `inspectorComponent` entry on `routeMetadata.ts`. Library opts in with a
`GameInspector` (hero · pills · actions · active profile · recent launches · health); other
routes ship `null` in v1. Extends `useScrollEnhance` with two new scroll selectors so the rail
and sidebar nav participate in WebKitGTK scroll contracts.

## User Story

As a Linux gamer with a 1080p-or-wider display, I want a persistent inspector rail alongside
the library grid so I can scan games and inspect the selected one without opening a blocking
modal.

## Problem → Solution

**Current**: Library is a single-column grid; game details open in a blocking modal
(`GameDetailsModal.tsx`, 479 lines) that takes over focus, dims the app, and re-mounts on each
open. Toolbar is search + 2-button view toggle only. The shell has no concept of a per-route
side panel — only `Sidebar` + main content. Routes have no way to contribute a side-panel slot.
→
**Desired**: Library shows a hover-gradient-reveal card grid on the left with a persistent
inspector on the right (`360 / 320 / 280 / 0` by breakpoint `uw · desk · narrow · deck`).
Selecting a card (click or keyboard) fills the inspector; the sidebar and inspector stay
mounted. `LibraryToolbar` adds sort/filter/view chips and a ⌘K palette-trigger placeholder
(palette itself lands in Phase 6). `routeMetadata.ts` gains an optional
`inspectorComponent?: ComponentType<{ selection?: SelectedGame }>`; only `library` opts in.

## Metadata

- **Complexity**: Large
- **Source PRD**: `docs/prps/prds/unified-desktop-redesign.prd.md`
- **PRD Phase**: Phase 4 — Library redesign + cards + inspector rail
- **GitHub scope**: Issue [#443](https://github.com/yandy-r/crosshook/issues/443) (tracking) ·
  Issue [#416](https://github.com/yandy-r/crosshook/issues/416) (deliverable) · feat label
  `feat:library-inspector`
- **Estimated Files**: ~16 files changed, ~8 files created
- **Dependencies (PRD phase graph)**: Phase 1 (`useBreakpoint` + layout unlock + `AppShell`
  extraction) and Phase 2 (token swap) must be shipped. Phase 3 (sidebar variants + Collections
  formalization) lands in parallel with Phase 2 and must also be merged before this plan runs.

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run
concurrently; batches run in order.

| Batch | Tasks         | Depends On | Parallel Width |
| ----- | ------------- | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3 | —          | 3              |
| B2    | 2.1, 2.2, 2.3 | B1         | 3              |
| B3    | 3.1, 3.2, 3.3 | B2         | 3              |
| B4    | 4.1           | B3         | 1              |
| B5    | 4.2           | B4         | 1              |
| B6    | 4.3           | B5         | 1              |

- **Total tasks**: 10
- **Total batches**: 4
- **Max parallel width**: 3

---

## Worktree Setup

- **Parent**: `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector/` (branch: `feat/unified-desktop-phase-4-library-inspector`)
- **Children** (per parallel task; merged back at end of each batch):
  - Task 1.1 → `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-1-1/` (branch: `feat/unified-desktop-phase-4-library-inspector-1-1`)
  - Task 1.2 → `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-1-2/` (branch: `feat/unified-desktop-phase-4-library-inspector-1-2`)
  - Task 1.3 → `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-1-3/` (branch: `feat/unified-desktop-phase-4-library-inspector-1-3`)
  - Task 2.1 → `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-2-1/` (branch: `feat/unified-desktop-phase-4-library-inspector-2-1`)
  - Task 2.2 → `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-2-2/` (branch: `feat/unified-desktop-phase-4-library-inspector-2-2`)
  - Task 2.3 → `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-2-3/` (branch: `feat/unified-desktop-phase-4-library-inspector-2-3`)
  - Task 3.1 → `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-3-1/` (branch: `feat/unified-desktop-phase-4-library-inspector-3-1`)
  - Task 3.2 → `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-3-2/` (branch: `feat/unified-desktop-phase-4-library-inspector-3-2`)
  - Task 3.3 → `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-3-3/` (branch: `feat/unified-desktop-phase-4-library-inspector-3-3`)

Task 4.1 is sequential and runs in the parent worktree directly.

---

## UX Design

### Before

```
┌───────────────────────────────────────────────────────────────┐
│  Sidebar │  Library grid (full width)                         │
│          │  ┌──────┐┌──────┐┌──────┐┌──────┐                  │
│          │  │ card ││ card ││ card ││ card │   search + view  │
│          │  └──────┘└──────┘└──────┘└──────┘   toggle only    │
│          │                                                    │
│          │  click → opens blocking modal (createPortal)       │
│          │          sidebar dims, focus trapped in modal      │
└───────────────────────────────────────────────────────────────┘
```

### After (1920×1080 — `desk`)

```
┌───────────────────────────────────────────────────────────────┐
│  Sidebar │  Toolbar: search · sort · filter · view · ⌘K      │ Inspector
│          │  ┌──────┐┌──────┐┌──────┐                         │ (320px)
│          │  │ card ││ card*││ card │   hover-gradient reveal │ ┌──────┐
│          │  └──────┘└──────┘└──────┘   favorite heart badge  │ │ hero │
│          │  *selected → fills inspector                      │ ├──────┤
│          │                                                    │ │pills │
│          │  sidebar + inspector stay mounted                 │ │actions│
│          │                                                    │ │profile│
│          │                                                    │ │health │
└───────────────────────────────────────────────────────────────┘ └──────┘
```

### After (1280×800 — `deck`)

Inspector collapses to 0px (not rendered). Library grid is full-width inside the content Panel.
Sidebar is the rail variant from Phase 3.

### Interaction Changes

| Touchpoint                  | Before                                         | After                                                                                                                | Notes                                                             |
| --------------------------- | ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Click card                  | Opens blocking `GameDetailsModal`              | Single-click fills inspector; details modal still exists for this phase (deprecated in Phase 5)                      | `onOpenDetails` prop remains; selection is new parallel channel   |
| Keyboard select             | Arrow / focus selects card (no visible change) | Arrow / focus selects card → inspector populates                                                                     | Selection state decoupled from `ProfileContext.selectedProfile`   |
| Favorite toggle             | Heart in footer button group                   | Favorite heart badge in card top-right corner (hover-reveal) + footer button kept for gamepad/keyboard accessibility | Both trigger same `onToggleFavorite`                              |
| Library toolbar — sort      | Absent                                         | Chip group: Recent · Name · Last Played · Playtime (sort by)                                                         | New local state on `LibraryPage`                                  |
| Library toolbar — filter    | Absent                                         | Chip group: All · Favorites · Installed · Recently Launched                                                          | New local state on `LibraryPage`                                  |
| Library toolbar — palette   | Absent                                         | `⌘K` trigger button (placeholder — emits `onOpenCommandPalette` noop in Phase 4)                                     | Palette ships in Phase 6; button is wired but dormant             |
| Inspector width (by bp)     | N/A                                            | 360 (uw) · 320 (desk) · 280 (narrow) · 0 (deck)                                                                      | Derived — not stored; mirrors sidebar variant derivation          |
| Inspector content — library | N/A                                            | Hero art · pills · quick actions (Launch / Edit / Favorite) · active profile · recent launches · health              | Launches section is placeholder in v1 (no IPC — see NOT BUILDING) |
| Inspector content — other   | N/A                                            | Nothing rendered (routeMetadata entry omits `inspectorComponent`)                                                    | Future phases opt other routes in                                 |
| Focus ring                  | Per-card card-level ring                       | Card ring stays; inspector elements follow existing `crosshook-*` ring tokens                                        | No token changes in this phase                                    |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                         | Lines   | Why                                                                                   |
| -------------- | ---------------------------------------------------------------------------- | ------- | ------------------------------------------------------------------------------------- |
| P0 (critical)  | `docs/prps/prds/unified-desktop-redesign.prd.md`                             | all     | Full PRD — Phase 4 goal, success signals, decisions log                               |
| P0 (critical)  | `CLAUDE.md`                                                                  | all     | Repo policies; scroll-container rule; persistence classification; file-size cap       |
| P0 (critical)  | `AGENTS.md`                                                                  | 77-143  | Stack overview, commands, testing, pre-commit                                         |
| P0 (critical)  | `src/crosshook-native/src/components/layout/routeMetadata.ts`                | all     | The contract Phase 4 extends                                                          |
| P0 (critical)  | `src/crosshook-native/src/components/layout/AppShell.tsx`                    | 46-222  | Shell structure; Panel/Group layout; derivation pattern for variant+width             |
| P0 (critical)  | `src/crosshook-native/src/hooks/useBreakpoint.ts`                            | all     | Breakpoint source of truth (`uw/desk/narrow/deck`)                                    |
| P0 (critical)  | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                         | 1-50    | `SCROLLABLE` selector — append site                                                   |
| P0 (critical)  | `src/crosshook-native/src/components/layout/sidebarVariants.ts`              | all     | Mirror for new `inspectorVariants.ts`                                                 |
| P0 (critical)  | `src/crosshook-native/src/components/pages/LibraryPage.tsx`                  | all     | Current selection semantics; where new state lands                                    |
| P1 (important) | `src/crosshook-native/src/components/library/LibraryCard.tsx`                | all     | Chrome redesign target; props contract stays stable                                   |
| P1 (important) | `src/crosshook-native/src/components/library/LibraryGrid.tsx`                | all     | Forwarding `selectedName`/`onSelect` prop                                             |
| P1 (important) | `src/crosshook-native/src/components/library/LibraryToolbar.tsx`             | all     | Toolbar chip additions                                                                |
| P1 (important) | `src/crosshook-native/src/components/library/GameDetailsModal.tsx`           | 381-440 | Existing hero+section composition — visual template for `GameInspector`               |
| P1 (important) | `src/crosshook-native/src/components/library/__tests__/LibraryCard.test.tsx` | all     | Test conventions (`renderWithMocks`, `mockCallCommand`, `triggerIntersection`)        |
| P1 (important) | `src/crosshook-native/src/components/library/__tests__/LibraryGrid.test.tsx` | all     | Child-component mock pattern                                                          |
| P1 (important) | `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`     | all     | Integration-scope breakpoint simulation (stays — unit tests use new `mockBreakpoint`) |
| P1 (important) | `src/crosshook-native/src/components/layout/Sidebar.tsx`                     | 186-232 | Existing persistent side rail — skeleton to mirror                                    |
| P1 (important) | `src/crosshook-native/src/types/library.ts`                                  | all     | `LibraryCardData` / `ProfileSummary` — selection payload                              |
| P1 (important) | `src/crosshook-native/src/test/render.tsx`                                   | all     | `renderWithMocks` contract                                                            |
| P1 (important) | `src/crosshook-native/src/test/fixtures.ts`                                  | all     | Where to add new fixture factories                                                    |
| P2 (reference) | `src/crosshook-native/src/hooks/useProfileHealth.ts`                         | all     | `useProfileHealthContext` — inspector health section                                  |
| P2 (reference) | `src/crosshook-native/src/components/HealthBadge.tsx`                        | all     | Reuse existing badge component inside inspector                                       |
| P2 (reference) | `src/crosshook-native/src/hooks/useGameCoverArt.ts`                          | all     | Inspector hero art fetch                                                              |
| P2 (reference) | `src/crosshook-native/src/components/OfflineReadinessPanel.tsx`              | 32-60   | Loading/error/empty fallback idiom                                                    |
| P2 (reference) | `src/crosshook-native/src/hooks/useOfflineReadiness.ts`                      | all     | Inspector readiness section                                                           |

## External Documentation

| Topic                                  | Source                                                         | Key Takeaway                                                                                         |
| -------------------------------------- | -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `react-resizable-panels` — fixed Panel | Library source already in tree; used in `AppShell.tsx:182-196` | Fixed-width pattern: `defaultSize = minSize = maxSize = N` (numeric px). Do **not** set `maxSize=0`. |
| React `ComponentType<Props>`           | React TypeScript handbook                                      | Use `ComponentType<{ selection?: SelectedGame }>` for the registry-stored component reference.       |
| `prefers-reduced-motion`               | MDN CSS media feature                                          | Hover-gradient-reveal animations must respect `@media (prefers-reduced-motion: reduce)`.             |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION — component files + CSS BEM

```
// SOURCE: src/crosshook-native/src/components/layout/Sidebar.tsx:188-197
<aside
  className="crosshook-sidebar"
  style={{ width: `${width}px` }}
  data-collapsed={collapsed ? 'true' : 'false'}
  data-crosshook-focus-zone="sidebar"
  data-sidebar-variant={variant}
  aria-label="CrossHook navigation"
>
```

PascalCase file → default+named export. BEM-like `crosshook-<block>__<element>--<modifier>`.
`Inspector.tsx` mirrors this root structure (swap `crosshook-inspector`, drop `data-sidebar-variant`,
keep `data-crosshook-focus-zone="inspector"`).

### FROZEN_WIDTH_PANEL — AppShell reuse

```
// SOURCE: src/crosshook-native/src/components/layout/AppShell.tsx:182-196
<Panel
  className="crosshook-shell-panel"
  defaultSize={sidebarWidth}
  minSize={sidebarWidth}
  maxSize={sidebarWidth}
>
  <Sidebar ... variant={sidebarVariant} />
</Panel>
```

Inspector Panel uses identical pattern: `defaultSize = minSize = maxSize = inspectorWidth`.
**When `inspectorWidth === 0` (deck), skip rendering the Panel entirely** — do not use a
`maxSize={0}` Panel (undefined behavior in `react-resizable-panels`).

### DERIVED_WIDTH — no stored state for breakpoint-driven width

```
// SOURCE: src/crosshook-native/src/components/layout/AppShell.tsx:46-56
const [route, setRoute] = useState<AppRoute>('library');
const shellRef = useRef<HTMLDivElement>(null);
const breakpoint = useBreakpoint(shellRef);
const sidebarVariant = sidebarVariantFromBreakpoint(breakpoint.size, breakpoint.height);
const sidebarWidth = sidebarWidthForVariant(sidebarVariant);
```

Mirror: `const inspectorWidth = inspectorWidthForBreakpoint(breakpoint.size);` — derived in
`AppShell`, passed to `<Inspector>` as `width`. Never `useState` it.

### VARIANT_HELPER_MODULE — sidebarVariants.ts as template

```
// SOURCE: src/crosshook-native/src/components/layout/sidebarVariants.ts:3-25
export type SidebarVariant = 'rail' | 'mid' | 'full';
export const SIDEBAR_VARIANT_WIDTHS = { rail: 56, mid: 68, full: 240 } as const
  satisfies Record<SidebarVariant, number>;
export function sidebarWidthForVariant(variant: SidebarVariant): number {
  return SIDEBAR_VARIANT_WIDTHS[variant];
}
```

New `inspectorVariants.ts` exports `INSPECTOR_WIDTHS` (`Record<BreakpointSize, number>`) and
`inspectorWidthForBreakpoint(size: BreakpointSize): number`.

### ROUTE_METADATA_REGISTRY — extend, don't replace

```
// SOURCE: src/crosshook-native/src/components/layout/routeMetadata.ts:17-25
export interface RouteMetadataEntry {
  navLabel: string;
  sectionEyebrow: string;
  bannerTitle: string;
  bannerSummary: string;
  Art: ComponentType<SVGProps<SVGSVGElement>>;
}
```

Add a single optional field: `inspectorComponent?: ComponentType<{ selection?: SelectedGame }>;`.
Export a new `SelectedGame` type alias from the same file. Only `library` sets a value;
`Record<AppRoute, RouteMetadataEntry>` keeps compiling because the field is optional.

### ERROR_HANDLING — try/catch → state flag

```
// SOURCE: src/crosshook-native/src/hooks/useLibrarySummaries.ts:33-54
try {
  const result = await callCommand<ProfileSummary[]>('profile_list_summaries', { collectionId: cid });
  setSummaries(...);
} catch (err) {
  console.error('Failed to fetch profile summaries', err);
  setError(String(err));
}
```

Inspector sub-fetches mirror this shape: local `loading`/`error` state; `console.error('<ctx>', err)`.
Never throw to render.

### LOADING_ERROR_EMPTY_FALLBACK — inspector panel idiom

```
// SOURCE: src/crosshook-native/src/components/OfflineReadinessPanel.tsx:32-56
{error ? <p className="crosshook-launch-panel__feedback-help" role="status">{error}</p> : null}
{loading && !report ? <p ... role="status">Loading offline readiness…</p> : null}
{report && report.blocking_reasons.length > 0 ? (<div ...> ... </div>) : null}
```

Each `GameInspector` section (hero / pills / actions / profile / launches / health) uses this
three-branch pattern. The inspector root must tolerate `selection == null` by short-circuiting
to an empty-state `<p role="status">` or `null`.

### OPTIMISTIC_WITH_REVERT — user actions

```
// SOURCE: src/crosshook-native/src/components/pages/LibraryPage.tsx:188-199
setSummaries((prev) => prev.map(s => (s.name === name ? { ...s, isFavorite: !current } : s)));
void toggleFavorite(name, !current).catch(() => {
  setSummaries((prev) => prev.map(s => (s.name === name ? { ...s, isFavorite: current } : s)));
});
```

Inspector Favorite action reuses `handleToggleFavorite` from `LibraryPage` — no new error path.

### LOGGING_PATTERN — bare console, no central logger

```
// SOURCE: src/crosshook-native/src/hooks/useLibrarySummaries.ts:50
console.error('Failed to fetch profile summaries', err);
```

No logger module is introduced in this phase. Use `console.error('<context>', err)`. Do not
add a telemetry dependency.

### SCROLLABLE_SELECTOR — single string append

```
// SOURCE: src/crosshook-native/src/hooks/useScrollEnhance.ts:8-9
const SCROLLABLE =
  '.crosshook-route-card-scroll, .crosshook-page-scroll-body, .crosshook-subtab-content__inner--scroll, .crosshook-console-drawer__body, .crosshook-modal__body, .crosshook-prefix-deps__log-output, .crosshook-discovery-results, .crosshook-collections-sidebar__list, .crosshook-collection-assign-menu__list, .crosshook-route-stack__body--scroll';
```

Append `, .crosshook-sidebar__nav--scroll, .crosshook-inspector__body` to this string. Both
classes must be applied in markup (sidebar nav node already exists; new `.crosshook-inspector__body`
lives inside the new `Inspector.tsx`). Add `overscroll-behavior: contain` on each per CLAUDE.md
scroll-container contract.

### TEST_STRUCTURE — renderWithMocks + co-located tests

```
// SOURCE: src/crosshook-native/src/components/library/__tests__/LibraryCard.test.tsx:1-11
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeLibraryCardData } from '@/test/fixtures';
import { mockCallCommand, renderWithMocks } from '@/test/render';
import { LibraryCard } from '../LibraryCard';
vi.mock('@/lib/ipc', () => ({ callCommand: mockCallCommand }));
```

Every new test file in `__tests__/` follows this preamble. Path alias `@/*` resolves via
`tsconfig.json:19-21` + `vite.config.ts:17-21` + `vitest.config.ts:20`.

### TEST_PATTERN_BREAKPOINT_UNIT — new helper introduced here

```
// NEW — src/crosshook-native/src/test/breakpoint.ts (see Task 1.3)
export function mockBreakpoint(size: BreakpointSize) {
  vi.mock('@/hooks/useBreakpoint', () => ({
    useBreakpoint: () => ({ size, width: PX[size], height: 800, ...FLAGS[size] }),
  }));
}
```

Unit-scope inspector/library tests call `mockBreakpoint('desk')` (or `'deck'`) in `beforeEach`.
`AppShell.test.tsx` keeps its existing `setInnerWidth` + `mockAppShellRect` **integration** pattern.

### CHILD_COMPONENT_MOCK — grid-level test strategy

```
// SOURCE: src/crosshook-native/src/components/library/__tests__/LibraryGrid.test.tsx:6-23
vi.mock('../LibraryCard', () => ({
  LibraryCard: ({ profile, isSelected, isLaunching }: ...) => (
    <li data-testid={`card-${profile.name}`}
        data-selected={String(Boolean(isSelected))}
        data-launching={String(Boolean(isLaunching))}>{profile.name}</li>
  ),
}));
```

Grid test keeps this strategy after prop additions. Any new `onSelect`/`selectedName` prop
Phase 4 adds is asserted via `data-selected` + `userEvent.click(getByTestId('card-foo'))`.

---

## Files to Change

| File                                                                            | Action | Justification                                                                                                              |
| ------------------------------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/layout/routeMetadata.ts`                   | UPDATE | Add `inspectorComponent?` field + `SelectedGame` type; register `GameInspector` under `library`                            |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`                            | UPDATE | Append `.crosshook-sidebar__nav--scroll, .crosshook-inspector__body` to `SCROLLABLE`                                       |
| `src/crosshook-native/src/test/breakpoint.ts`                                   | CREATE | `mockBreakpoint(size)` helper for unit-scope inspector + library tests                                                     |
| `src/crosshook-native/src/components/layout/inspectorVariants.ts`               | CREATE | `INSPECTOR_WIDTHS` + `inspectorWidthForBreakpoint(size)` helper (mirrors `sidebarVariants.ts`)                             |
| `src/crosshook-native/src/components/layout/Inspector.tsx`                      | CREATE | Layout shell component (root `<aside>`, breakpoint-width container, empty/error/loading states, `InspectorErrorBoundary`)  |
| `src/crosshook-native/src/components/layout/__tests__/Inspector.test.tsx`       | CREATE | Unit tests: renders `null` body when `selection` undefined; `data-testid="inspector"`; error boundary catches thrown child |
| `src/crosshook-native/src/styles/layout.css`                                    | UPDATE | New `.crosshook-inspector`, `.crosshook-inspector__body` rules (width, scroll, `overscroll-behavior`)                      |
| `src/crosshook-native/src/styles/library.css`                                   | UPDATE | New `crosshook-library-card__favorite-heart`, hover-gradient reveal, toolbar chip styles, palette-trigger styles           |
| `src/crosshook-native/src/styles/sidebar.css`                                   | UPDATE | Add `.crosshook-sidebar__nav--scroll` selector on the nav (hook registers it but class must be applied in markup)          |
| `src/crosshook-native/src/components/library/LibraryCard.tsx`                   | UPDATE | Chrome redesign: hover-gradient reveal layer, favorite heart badge, preserved action-button group                          |
| `src/crosshook-native/src/components/library/__tests__/LibraryCard.test.tsx`    | UPDATE | Add assertions for the heart badge + hover-reveal; preserve existing Launch/Edit/Unfavorite tests                          |
| `src/crosshook-native/src/components/library/LibraryToolbar.tsx`                | UPDATE | Add `sortBy`/`filter`/`onOpenCommandPalette?` props + chip groups + palette-trigger button                                 |
| `src/crosshook-native/src/components/library/__tests__/LibraryToolbar.test.tsx` | CREATE | Unit tests: chip groups emit change events; palette-trigger emits `onOpenCommandPalette`                                   |
| `src/crosshook-native/src/components/library/GameInspector.tsx`                 | CREATE | Library's opt-in inspector body: hero · pills · quick actions · active profile · recent launches (placeholder) · health    |
| `src/crosshook-native/src/components/library/__tests__/GameInspector.test.tsx`  | CREATE | Unit tests: each section renders on selection; empty-state when selection undefined; health uses `useProfileHealthContext` |
| `src/crosshook-native/src/components/library/LibraryGrid.tsx`                   | UPDATE | Add `onSelect(name)` prop; forward `selectedName` to `LibraryCard` as `isSelected`                                         |
| `src/crosshook-native/src/components/library/__tests__/LibraryGrid.test.tsx`    | UPDATE | Assert `onSelect` fires on card click; `data-selected` reflects `selectedName`                                             |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx`                     | UPDATE | Add `inspectorName`/`sortBy`/`filter` local state; wire toolbar chips + grid `onSelect`; pass selection payload downstream |
| `src/crosshook-native/src/components/layout/Sidebar.tsx`                        | UPDATE | Apply `.crosshook-sidebar__nav--scroll` class on the nav element; add `data-testid="sidebar"` to root `<aside>`            |
| `src/crosshook-native/src/components/layout/AppShell.tsx`                       | UPDATE | Compute `inspectorWidth`; conditionally insert 3rd `<Panel>` with `<Inspector>` before `</Group>` when width > 0           |
| `src/crosshook-native/src/test/fixtures.ts`                                     | UPDATE | Add `makeProfileHealthReport()` fixture factory (used by `GameInspector.test.tsx`)                                         |

## NOT Building

- **Hero Detail mode (card click → full-screen takeover)** — lands in **Phase 5**. Phase 4
  selecting a card only fills the inspector; `GameDetailsModal` is still the path for
  "open details", unchanged. `GameDetailsModal.tsx` is **not** deprecated in Phase 4.
- **⌘K command palette itself** — lands in **Phase 6**. Phase 4 wires only the
  **palette-trigger placeholder button** in `LibraryToolbar`; clicking it calls the
  `onOpenCommandPalette?: () => void` prop, which is currently wired to a no-op in
  `LibraryPage.tsx` (or logs a `console.debug`). The button is visible and focusable.
- **Recent launches IPC surface** — `launch_operations` table has no `#[tauri::command]`
  accessor today. `GameInspector`'s "Recent launches" section ships as an empty-state
  placeholder (`"History will appear after launches"`) in v1. **Follow-up GitHub issue
  must be filed** during Phase 4 for a `list_launch_history_for_profile` IPC that both
  this section and the Phase 7 Context Rail activity chart will consume. Phase 4 **does
  not add backend code** — PRD is explicit that this redesign is frontend-only.
- **Context rail (4th pane) on ultrawide** — lands in **Phase 7**. Phase 4 does not add it.
- **Sidebar variant rewiring** — already complete in Phase 3. Phase 4 only _adds_ the
  `.crosshook-sidebar__nav--scroll` class to the existing nav element so the scroll hook
  picks it up; no variant-logic changes.
- **Token/palette changes** — already complete in Phase 2. Phase 4 uses only existing
  `--crosshook-color-*` tokens.
- **Route-level inspector content for non-Library routes** — every entry except `library`
  omits `inspectorComponent`. Profiles/Proton Manager/etc. opt in over the routes-rework
  phases (9-11); Phase 4 only establishes the contract.
- **Per-route `inspectorCollapsed` persistence / user-pinnable inspector width** — PRD
  defers to follow-up: _"Inspector-collapsed state persistence per route"_ is in the PRD's
  Could column, not Must.
- **Gamepad zones for the inspector** — inspector is a sibling of the sidebar; existing
  focus-zone primitives stay on `data-crosshook-focus-zone="sidebar"` and a new
  `data-crosshook-focus-zone="inspector"`. n-zone gamepad-nav refactor remains a
  follow-up (PRD Technical Risks table).
- **Replacing `GameDetailsModal` with inspector** — the modal still handles the
  "open full details" intent in Phase 4. Inspector is an adjunct preview, not a
  modal replacement, until Phase 5 ships Hero Detail mode.
- **Any `src-tauri` / `crosshook-core` / Rust changes** — zero in Phase 4.

---

## Step-by-Step Tasks

### Task 1.1: Extend `routeMetadata.ts` with `inspectorComponent` contract — Depends on [none]

- **BATCH**: B1
- **Worktree**: `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-1-1/` (branch: `feat/unified-desktop-phase-4-library-inspector-1-1`)
- **ACTION**: Add an optional `inspectorComponent?: ComponentType<{ selection?: SelectedGame }>` field to `RouteMetadataEntry` and export a new `SelectedGame` type alias. Do **not** set any values on the existing 11 route entries in this task — Task 3.3 will wire `library` to `GameInspector` once the component exists.
- **IMPLEMENT**: Add `import type { LibraryCardData } from '@/types/library';` at the top. Export `export type SelectedGame = LibraryCardData;` right before `RouteMetadataEntry`. Append the optional field to the interface body. Leave `ROUTE_METADATA` unchanged — every entry still compiles because the field is optional.
- **MIRROR**: `ROUTE_METADATA_REGISTRY` pattern from _Patterns to Mirror_. Keep the interface docblock style (JSDoc `/** ... */` on each field) consistent with the existing `navLabel`/`sectionEyebrow` lines.
- **IMPORTS**: `import type { ComponentType, SVGProps } from 'react';` (already present); add `import type { LibraryCardData } from '@/types/library';`.
- **GOTCHA**: The file sits at `src/crosshook-native/src/components/layout/routeMetadata.ts` — **not** `src/crosshook-native/src/routeMetadata.ts` as a few PRD bullets suggest. Do not relocate the file. The issue description's path is outdated; keep the existing path and everything that imports from `'./routeMetadata'` or `'../layout/routeMetadata'` continues to compile. Also — **do not** use a generic `RouteMetadataEntry<TSelection>` in this phase; v1 only opts one route in and a concrete `SelectedGame = LibraryCardData` alias is sufficient. Generic parameterization is a follow-up if a second route opts in with a different selection type.
- **VALIDATE**:
  - `cd src/crosshook-native && npm run typecheck` — zero errors.
  - `rg -n 'inspectorComponent' src/crosshook-native/src/components/layout/routeMetadata.ts` — one match in the interface, none in the registry (Task 3.3 adds the registry entry).
  - `rg -n 'export type SelectedGame' src/crosshook-native/src/components/layout/routeMetadata.ts` — one match.

### Task 1.2: Register new scroll containers in `useScrollEnhance` — Depends on [none]

- **BATCH**: B1
- **Worktree**: `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-1-2/` (branch: `feat/unified-desktop-phase-4-library-inspector-1-2`)
- **ACTION**: Append `.crosshook-sidebar__nav--scroll, .crosshook-inspector__body` to the `SCROLLABLE` string literal in `src/crosshook-native/src/hooks/useScrollEnhance.ts`. No other code changes in this task. The actual markup + CSS that attaches these classes land in Task 2.1 (`Inspector.tsx`) and Task 4.1 (Sidebar nav class attachment alongside AppShell wiring) — registering early is safe because a selector that matches nothing is a no-op.
- **IMPLEMENT**: Open `src/crosshook-native/src/hooks/useScrollEnhance.ts`. Replace the `SCROLLABLE` constant's right-hand side with the existing list plus the two new selectors at the end, comma-separated.
- **MIRROR**: `SCROLLABLE_SELECTOR` from _Patterns to Mirror_.
- **IMPORTS**: None needed (constant edit only).
- **GOTCHA**: Keep it a single-line (or clean multiline) comma-separated string. Do **not** change the hook's behavior or return type. Preserving the exact existing formatting around the literal prevents noisy Biome diffs.
- **VALIDATE**:
  - `rg -n 'crosshook-sidebar__nav--scroll' src/crosshook-native/src/hooks/useScrollEnhance.ts` — one match.
  - `rg -n 'crosshook-inspector__body' src/crosshook-native/src/hooks/useScrollEnhance.ts` — one match.
  - `cd src/crosshook-native && npx biome check src/hooks/useScrollEnhance.ts` — no issues.

### Task 1.3: Create `mockBreakpoint` test helper — Depends on [none]

- **BATCH**: B1
- **Worktree**: `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-1-3/` (branch: `feat/unified-desktop-phase-4-library-inspector-1-3`)
- **ACTION**: Create `src/crosshook-native/src/test/breakpoint.ts` exporting a `mockBreakpoint(size: BreakpointSize)` factory that registers a `vi.mock('@/hooks/useBreakpoint', ...)` with a stubbed return value matching the real hook shape (`size`, `width`, `height`, `isDeck`, `isNarrow`, `isDesk`, `isUw`). Also export a `BREAKPOINT_PX` table for deterministic width values (`{ uw: 2560, desk: 1920, narrow: 1440, deck: 1280 }`).
- **IMPLEMENT**: 25–40 lines. One `import { vi } from 'vitest';`, one `import type { BreakpointSize, UseBreakpointResult } from '@/hooks/useBreakpoint';`. Internal helper `flagsFor(size)` returns the 4 boolean flags based on size. Exported `mockBreakpoint(size)` calls `vi.mock(...)` — note that `vi.mock` must be hoisted at module top in practice; the factory form needed here is a helper that _returns a mock implementation_ usable inside `beforeEach`. Prefer a simpler shape: export a `breakpointResult(size)` function returning a `UseBreakpointResult`, and document that test files use `vi.mock('@/hooks/useBreakpoint', () => ({ useBreakpoint: vi.fn(() => breakpointResult('desk')) }))` at module top, then reconfigure via `vi.mocked(useBreakpoint).mockReturnValue(breakpointResult('deck'))` in specific tests. Choose whichever works with the project's hoisting behavior — **decide by reading `LibraryCard.test.tsx` top-of-file `vi.mock` usage** and matching that style.
- **MIRROR**: `TEST_PATTERN_BREAKPOINT_UNIT` from _Patterns to Mirror_. Also mirror the import-alias style (`@/`) from existing tests.
- **IMPORTS**: `vi` from `vitest`; `BreakpointSize` and `UseBreakpointResult` types from `@/hooks/useBreakpoint`.
- **GOTCHA**: `vi.mock` is hoisted by Vitest; factories must be pure. The helper cannot call `vi.mock` at runtime from inside a function body and expect hoisting — instead, export the _return value_ (`breakpointResult(size)`) and document the canonical module-top `vi.mock` one-liner in a JSDoc block so consumers copy-paste it. Put an example usage in a `// Usage:` comment near the export.
- **VALIDATE**:
  - `rg -n 'export function breakpointResult' src/crosshook-native/src/test/breakpoint.ts` — one match.
  - `cd src/crosshook-native && npm run typecheck` — zero errors.
  - Temporary smoke test: a throwaway `__tests__/breakpoint-helper.test.ts` that asserts `breakpointResult('deck').isDeck === true` and `breakpointResult('uw').isUw === true` runs green under `npm test`. Delete the smoke file before the task completes (or keep as a permanent minimal test of the helper itself — author's call; a permanent test is preferred).

### Task 2.1: Create `Inspector.tsx` + `inspectorVariants.ts` + base CSS — Depends on [1.1, 1.2]

- **BATCH**: B2
- **Worktree**: `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-2-1/` (branch: `feat/unified-desktop-phase-4-library-inspector-2-1`)
- **ACTION**: Create three files. (1) `src/crosshook-native/src/components/layout/inspectorVariants.ts` exporting `INSPECTOR_WIDTHS: Record<BreakpointSize, number>` = `{ uw: 360, desk: 320, narrow: 280, deck: 0 }` and `inspectorWidthForBreakpoint(size)`. (2) `src/crosshook-native/src/components/layout/Inspector.tsx` — accepts `{ route: AppRoute; selection?: SelectedGame; width: number }` props, looks up `ROUTE_METADATA[route].inspectorComponent`, renders `<aside class="crosshook-inspector" data-testid="inspector" ...>` with a `<div class="crosshook-inspector__body">` wrapping the inspector component (or an empty-state `null` / `<p role="status">No inspector content for this route</p>` when undefined). Width is styled via `style={{ width }}`. A co-located `InspectorErrorBoundary` (class component) wraps the inspector-body component so a thrown child renders a friendly fallback instead of crashing the shell. (3) Append CSS rules to `src/crosshook-native/src/styles/layout.css`: `.crosshook-inspector` (root), `.crosshook-inspector__body` (scroll-region with `overflow-y: auto; overscroll-behavior: contain;`), `.crosshook-inspector--collapsed` reserved for future pinning. Also create a matching unit test file `src/crosshook-native/src/components/layout/__tests__/Inspector.test.tsx` using the Task 1.3 helper.
- **IMPLEMENT**: `Inspector.tsx` target ≤150 lines. `InspectorErrorBoundary` is a tiny class component — accepts `children`, catches via `componentDidCatch`, renders `<p role="status" className="crosshook-inspector__error">Inspector unavailable.</p>` on error. Error boundary sits **only** around the looked-up `InspectorComponent` call — not around the aside root — so the rail chrome stays painted even if the inspector body throws.
- **MIRROR**: `NAMING_CONVENTION`, `VARIANT_HELPER_MODULE`, `LOADING_ERROR_EMPTY_FALLBACK` from _Patterns to Mirror_. Root aside structure mirrors `Sidebar.tsx:188-197`.
- **IMPORTS**: `import type { ComponentType } from 'react';`, `import React from 'react';` (for the class component), `import { ROUTE_METADATA, type SelectedGame } from './routeMetadata';`, `import type { AppRoute } from './Sidebar';`, `import type { BreakpointSize } from '@/hooks/useBreakpoint';`.
- **GOTCHA**: Do **not** call `useBreakpoint()` inside `Inspector.tsx`. The width is passed in by `AppShell` (derived once there; Task 4.1). Mounting `useBreakpoint` twice would duplicate `ResizeObserver`s. Also — when `width === 0` (deck), `AppShell` skips the Panel entirely; `Inspector.tsx` itself renders the full aside whenever it mounts. Don't add a `width === 0 ? null : ...` short-circuit inside `Inspector.tsx` — that's `AppShell`'s responsibility. For the error boundary, use a **class** component; React hooks cannot implement `componentDidCatch`.
- **VALIDATE**:
  - `rg -n 'export function Inspector' src/crosshook-native/src/components/layout/Inspector.tsx` — one match.
  - `rg -n 'INSPECTOR_WIDTHS' src/crosshook-native/src/components/layout/inspectorVariants.ts` — one match.
  - `rg -n '\.crosshook-inspector__body' src/crosshook-native/src/styles/layout.css` — at least one match.
  - `npm test -- src/components/layout/__tests__/Inspector.test.tsx` — green; asserts `data-testid="inspector"` on root aside, empty-state when `ROUTE_METADATA[route].inspectorComponent` is undefined, and that a thrown child is caught by the boundary (use a `<ThrowingChild />` fixture).

### Task 2.2: Redesign `LibraryCard` chrome + update test — Depends on [1.3]

- **BATCH**: B2
- **Worktree**: `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-2-2/` (branch: `feat/unified-desktop-phase-4-library-inspector-2-2`)
- **ACTION**: Keep the existing `LibraryCardProps` contract 100% backward-compatible — **do not remove or rename any prop**. Add two pieces of chrome: (1) a hover-gradient-reveal layer (`<div class="crosshook-library-card__hover-reveal" aria-hidden="true" />`) that fades in action-affordance hints at hover and stays hidden for `prefers-reduced-motion: reduce`; (2) a favorite heart badge (`<button class="crosshook-library-card__favorite-heart" aria-pressed={isFavorite} aria-label="Favorite / Unfavorite">`) pinned top-right of the card frame — it reuses the same `onToggleFavorite` handler as the existing footer button. Update `LibraryCard.test.tsx` with three new assertions: heart is aria-pressed per `isFavorite`, heart click emits `onToggleFavorite`, and hover-reveal element is present (query by `aria-hidden="true"` child or class).
- **IMPLEMENT**: CSS rules in `src/crosshook-native/src/styles/library.css`: `.crosshook-library-card__hover-reveal` (absolute-positioned overlay, `pointer-events: none`, opacity 0 → 1 on `:hover`/`:focus-within`, `transition: opacity 160ms`), `.crosshook-library-card__favorite-heart` (absolute top-right, visible on hover, always visible when `aria-pressed="true"` so favorites remain discoverable at rest). Add `@media (prefers-reduced-motion: reduce)` block disabling the opacity transition. `LibraryCard.tsx` stays under 500 lines.
- **MIRROR**: `NAMING_CONVENTION` (BEM `crosshook-library-card__favorite-heart`). `CHILD_COMPONENT_MOCK` is not used here — `LibraryCard.test.tsx` is the leaf test; use `renderWithMocks` per `TEST_STRUCTURE`.
- **IMPORTS**: No new prop types. Optional: a single-file SVG import for the heart glyph if not already shared via `@/components/icons/`.
- **GOTCHA**: Existing tests rely on `aria-label="Launch <game>"`, `aria-label="Edit <game>"`, `aria-label={isFavorite ? 'Unfavorite' : 'Favorite'} <game>` for the footer group. **Do not change those labels.** The new heart button uses a **distinct** `aria-label` (suggestion: `"Toggle favorite: <game>"`) so existing RTL selectors still find the footer button. Accessibility: heart is clickable via mouse; gamepad/keyboard users still have the footer button — both paths must call `onToggleFavorite` with identical args. Biome may flag a nested interactive element inside the card-wide `details-hitbox` button — wrap the heart outside the hitbox (render after, as a sibling; both can coexist inside the `<li>`) so there's no `<button>` inside `<button>`.
- **VALIDATE**:
  - `cd src/crosshook-native && npm test -- src/components/library/__tests__/LibraryCard.test.tsx` — green, including the 3 new assertions.
  - `cd src/crosshook-native && npx biome check src/components/library/LibraryCard.tsx src/styles/library.css` — no issues.
  - Manual (optional in worktree): start `./scripts/dev-native.sh --browser`, hover a card at 1920×1080, verify the gradient reveal; set `prefers-reduced-motion` in devtools and verify no animation.
  - `wc -l src/crosshook-native/src/components/library/LibraryCard.tsx` — under 500 lines.

### Task 2.3: Redesign `LibraryToolbar` with chip groups + palette trigger — Depends on [none]

- **BATCH**: B2
- **Worktree**: `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-2-3/` (branch: `feat/unified-desktop-phase-4-library-inspector-2-3`)
- **ACTION**: Extend `LibraryToolbarProps` with **additive** fields: `sortBy: LibrarySortKey`, `onSortChange: (k: LibrarySortKey) => void`, `filter: LibraryFilterKey`, `onFilterChange: (k: LibraryFilterKey) => void`, `onOpenCommandPalette?: () => void`. Define the two new key unions in `src/crosshook-native/src/types/library.ts`: `LibrarySortKey = 'recent' | 'name' | 'lastPlayed' | 'playtime'` and `LibraryFilterKey = 'all' | 'favorites' | 'installed' | 'recentlyLaunched'`. Render two new chip groups using the existing `aria-pressed` + BEM button idiom already used by the view-toggle. Render a trailing palette-trigger button `<button aria-label="Open command palette" class="crosshook-library-toolbar__palette-trigger">⌘K</button>` that calls `onOpenCommandPalette` if provided, else no-op. Create `__tests__/LibraryToolbar.test.tsx`.
- **IMPLEMENT**: `LibraryToolbar.tsx` stays under 500 lines. Use a small local helper (`SORT_OPTIONS: readonly { key: LibrarySortKey; label: string }[]`) + `map` → chip buttons; same for filter. Keep existing `searchQuery` input + `viewMode` toggle unchanged. Palette-trigger button is present regardless of whether `onOpenCommandPalette` is set — dormant button with `onClick` that guards: `onClick={() => onOpenCommandPalette?.()}`. CSS in `src/crosshook-native/src/styles/library.css`: `.crosshook-library-toolbar__chip-group`, `.crosshook-library-toolbar__chip`, `.crosshook-library-toolbar__palette-trigger`. Chips share the toolbar row but wrap at `narrow` breakpoint (use flex-wrap + gap).
- **MIRROR**: View-toggle button pattern at `LibraryToolbar.tsx:21-35`. `TEST_STRUCTURE` for the new test file.
- **IMPORTS**: New type imports (`LibrarySortKey`, `LibraryFilterKey`) from `@/types/library`. Existing icon set under `@/components/icons/` for any chip glyphs; do **not** introduce a new icon dependency.
- **GOTCHA**: Test file uses plain `render` (not `renderWithMocks`) because Toolbar doesn't hit IPC. Still import `userEvent` and use `userEvent.setup()` — fireEvent is not sufficient for `aria-pressed` state changes under happy-dom. Keyboard test: `user.tab()` cycle must reach: search → each sort chip → each filter chip → each view-toggle → palette-trigger, in DOM order.
- **VALIDATE**:
  - `cd src/crosshook-native && npm test -- src/components/library/__tests__/LibraryToolbar.test.tsx` — green.
  - `rg -n "LibrarySortKey|LibraryFilterKey" src/crosshook-native/src/types/library.ts` — two matches.
  - `rg -n 'onOpenCommandPalette' src/crosshook-native/src/components/library/LibraryToolbar.tsx` — used in the props interface and the button click handler.
  - `wc -l src/crosshook-native/src/components/library/LibraryToolbar.tsx` — under 500 lines.

### Task 3.1: Create `GameInspector.tsx` + tests — Depends on [1.1, 1.3, 2.1]

- **BATCH**: B3
- **Worktree**: `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-3-1/` (branch: `feat/unified-desktop-phase-4-library-inspector-3-1`)
- **ACTION**: Create `src/crosshook-native/src/components/library/GameInspector.tsx` — default export `GameInspector`, signature `({ selection }: { selection?: SelectedGame }) => JSX.Element | null`. When `selection` is undefined render an empty-state `<p class="crosshook-game-inspector__empty" role="status">Select a game to see details</p>`. When defined, render five sections in order — each a `<section>` with a short `<h2>` eyebrow: (1) **Hero** — banner/icon art + title + subtitle (from `selection`). (2) **Pills** — engine/status/source badges derived from `selection` fields. (3) **Quick actions** — "Launch", "Edit profile", "Toggle favorite" buttons that defer to `LibraryPage`-owned handlers via props. (4) **Active profile** — pulls the current active profile for this game name from `useProfileContext()` and renders a compact summary (name + prefix + toolchain) or a "No active profile" state. (5) **Health** — calls `useProfileHealthContext()` for the active profile and renders status badge + blocking reasons using the same copy scheme as `OfflineReadinessPanel`. (6) **Recent launches** — ships as an explicit empty-state placeholder (`<p role="status">Recent launches coming soon</p>`) with an inline `// TODO(phase-4/follow-up)` comment referencing the follow-up issue to be filed. Extend the prop contract to `{ selection?: SelectedGame; onLaunch?: (name: string) => void; onEditProfile?: (name: string) => void; onToggleFavorite?: (name: string, next: boolean) => void }` so LibraryPage can wire its existing handlers without GameInspector touching IPC directly. Create co-located test `__tests__/GameInspector.test.tsx`.
- **IMPLEMENT**: Target ≤400 lines. Each section is a small internal component (`HeroSection`, `PillsSection`, `QuickActionsSection`, `ActiveProfileSection`, `HealthSection`, `RecentLaunchesPlaceholder`) defined in the same file for colocation — total file budget still under 500. Use `useProfileContext()` for active profile lookup and `useProfileHealthContext()` for health — mirror how `LibraryPage.tsx` already consumes them (do not re-invoke IPC inside `GameInspector`). All buttons omit disabled states in v1 unless the underlying handler is undefined. Keep markup semantic: one `<h2>` per section, buttons for interactive elements, no divs-as-buttons.
- **MIRROR**: `LOADING_ERROR_EMPTY_FALLBACK`, `ERROR_HANDLING`, `OPTIMISTIC_WITH_REVERT` (for Toggle favorite — delegated to the parent's existing implementation). `TEST_STRUCTURE` for the test file. See `OfflineReadinessPanel.tsx:32-56` as the prime reference for the Health section's status/reasons copy.
- **IMPORTS**: `import { useProfileContext } from '@/context/ProfileContext';`, `import { useProfileHealthContext } from '@/context/ProfileHealthContext';`, `import type { SelectedGame } from '@/components/layout/routeMetadata';`, `import { makeProfileHealthReport } from '@/test/fixtures';` (test only — see Task 3.2).
- **GOTCHA**: Do **not** bind Toggle favorite via a new `callCommand('profile_toggle_favorite', ...)` from inside the inspector. `LibraryPage.handleToggleFavorite` already implements the optimistic-with-revert pattern — `GameInspector` invokes `onToggleFavorite?.(name, next)` and lets the parent handle IPC + state revert. Re-implementing would duplicate the revert logic. Also — `useProfileHealthContext()` may return `{ reports: Map<string, ProfileHealthReport> }`; verify the exact shape by reading the context file **before** writing the Health section. If the hook returns a fetcher you must call (`getReport(name)`), render "Loading…" while pending; do not block render on it.
- **VALIDATE**:
  - `cd src/crosshook-native && npm test -- src/components/library/__tests__/GameInspector.test.tsx` — green; covers empty state, each section's presence when `selection` is defined, and button-click prop plumbing (fires `onLaunch`, `onEditProfile`, `onToggleFavorite`).
  - `rg -n 'export default function GameInspector|export function GameInspector' src/crosshook-native/src/components/library/GameInspector.tsx` — one match.
  - `wc -l src/crosshook-native/src/components/library/GameInspector.tsx` — under 500 lines.
  - `cd src/crosshook-native && npm run typecheck` — zero errors.

### Task 3.2: Add `makeProfileHealthReport` fixture — Depends on [none]

- **BATCH**: B3
- **Worktree**: `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-3-2/` (branch: `feat/unified-desktop-phase-4-library-inspector-3-2`)
- **ACTION**: Append `makeProfileHealthReport(overrides?: Partial<ProfileHealthReport>): ProfileHealthReport` to `src/crosshook-native/src/test/fixtures.ts`. Mirror the style of existing factories (`makeLibraryCardData`, `makeProfileSummary`). Return sensible happy-path defaults (status: `"ready"`, empty `blocking_reasons`, plausible timestamps) merged with `overrides` via shallow spread.
- **IMPLEMENT**: Inspect the real `ProfileHealthReport` type (likely in `@/types/profile.ts` or `@/context/ProfileHealthContext.ts`) before writing the factory to match field names exactly. Add a short JSDoc block documenting which fields are most commonly overridden in tests (`status`, `blocking_reasons`).
- **MIRROR**: Existing factory patterns in `fixtures.ts` — same `import type` conventions, `satisfies` pattern if currently used, `as const` where applicable.
- **IMPORTS**: `import type { ProfileHealthReport } from '<actual path>';` — resolve the path before finalizing.
- **GOTCHA**: If `ProfileHealthReport` is Rust-generated (via `specta`/`ts-rs`), the type file is usually `src/crosshook-native/src/bindings.ts` or similar — do **not** redefine the type in the test file. Always import from source.
- **VALIDATE**:
  - `rg -n 'export function makeProfileHealthReport' src/crosshook-native/src/test/fixtures.ts` — one match.
  - `cd src/crosshook-native && npm run typecheck` — zero errors.
  - `cd src/crosshook-native && npm test -- src/test/__tests__` — green (if a fixtures test file exists; otherwise skip — the real validator is Task 3.1's `GameInspector.test.tsx` using the factory).

### Task 3.3: Register `GameInspector` in `routeMetadata` — Depends on [1.1, 3.1]

- **BATCH**: B3
- **Worktree**: `~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector-3-3/` (branch: `feat/unified-desktop-phase-4-library-inspector-3-3`)
- **ACTION**: Set `ROUTE_METADATA.library.inspectorComponent = GameInspector` in `src/crosshook-native/src/components/layout/routeMetadata.ts`. No other route gets an inspector component in this phase.
- **IMPLEMENT**: Add `import { GameInspector } from '@/components/library/GameInspector';` at the top of the file. Add the single field to the `library` entry. Leave the other 10 entries untouched — the optional field means they still conform to `RouteMetadataEntry`. Do **not** refactor the registry structure; a targeted change keeps the diff reviewable.
- **MIRROR**: `ROUTE_METADATA_REGISTRY` from _Patterns to Mirror_.
- **IMPORTS**: The `GameInspector` symbol — verify the named-vs-default export matches what Task 3.1 defined before committing this line.
- **GOTCHA**: This creates a circular-import risk: `routeMetadata.ts` now imports from `@/components/library/GameInspector`, and if `GameInspector` ever re-imports `routeMetadata` you'll hit a TDZ error at module load. `GameInspector` **must only** import the `SelectedGame` **type** (`import type`) — not `ROUTE_METADATA` itself. Enforce this constraint in Task 3.1's final review.
- **VALIDATE**:
  - `rg -n "inspectorComponent:\s*GameInspector" src/crosshook-native/src/components/layout/routeMetadata.ts` — one match.
  - `cd src/crosshook-native && npm run typecheck` — zero errors.
  - `cd src/crosshook-native && npm run build` — no circular-import warnings (Vite surfaces them as build logs).

### Task 4.1: Wire `Inspector` into `AppShell`, apply sidebar scroll class, add testids — Depends on [2.1, 2.2, 2.3, 3.1, 3.2, 3.3]

- **BATCH**: B4
- **ACTION**: Update `src/crosshook-native/src/components/layout/AppShell.tsx` to compute `const inspectorWidth = inspectorWidthForBreakpoint(breakpoint.size);` right below the existing `sidebarWidth` line. Insert a third `<Panel>` inside the existing `<Group>` **after** the content-area Panel, conditionally: `{inspectorWidth > 0 && ( <Panel defaultSize={inspectorWidth} minSize={inspectorWidth} maxSize={inspectorWidth}><Inspector route={route} width={inspectorWidth} selection={/* provided by LibraryPage via a context or prop bridge — see Task 4.2 */} /></Panel> )}`. For this sub-task, pass `selection={undefined}` — Task 4.2 threads the real selection through. Separately, update `src/crosshook-native/src/components/layout/Sidebar.tsx`: apply `className="crosshook-sidebar__nav crosshook-sidebar__nav--scroll"` to the nav element that scrolls (wherever `SIDEBAR_SECTIONS` renders inside a scrollable `<nav>` or `<ul>`) and add `data-testid="sidebar"` to the root `<aside>`. Update `src/crosshook-native/src/styles/sidebar.css` if needed so the nav has `overflow-y: auto; overscroll-behavior: contain;`.
- **IMPLEMENT**: `AppShell.tsx` grows by ~10 lines. Add imports: `import { Inspector } from './Inspector';`, `import { inspectorWidthForBreakpoint } from './inspectorVariants';`. Read `AppShell.tsx` top-to-bottom before editing — the existing `<Group>` uses a specific prop order; preserve it. `Sidebar.tsx` change is surgical: one class rename/add + one `data-testid` attribute.
- **MIRROR**: `FROZEN_WIDTH_PANEL`, `DERIVED_WIDTH` from _Patterns to Mirror_ exactly.
- **IMPORTS**: New in `AppShell.tsx`: `Inspector` component and `inspectorWidthForBreakpoint`. No new imports in `Sidebar.tsx`.
- **GOTCHA**: **Do not** pass `defaultSize={0}`/`maxSize={0}` to `react-resizable-panels`; behavior is undefined. Use the conditional `{inspectorWidth > 0 && ...}` fence. The `<Group orientation="horizontal">` already has 2 panels; adding a 3rd sibling at the same depth is correct — **do not** introduce a nested `<Group>`. The content-area Panel's `minSize="28%"` still works; the third fixed-width panel reduces the effective space but 28% of remaining width remains enforced. Confirm `AppShell.test.tsx` still passes — existing integration tests may implicitly assert the number of panels; update them in Task 4.3.
- **VALIDATE**:
  - `rg -n 'inspectorWidthForBreakpoint|<Inspector ' src/crosshook-native/src/components/layout/AppShell.tsx` — at least two matches.
  - `rg -n 'data-testid="sidebar"' src/crosshook-native/src/components/layout/Sidebar.tsx` — one match.
  - `rg -n 'crosshook-sidebar__nav--scroll' src/crosshook-native/src/components/layout/Sidebar.tsx src/crosshook-native/src/styles/sidebar.css` — at least two matches (one in markup, one in CSS).
  - `cd src/crosshook-native && npm run typecheck` — zero errors.

### Task 4.2: Wire `LibraryPage` selection through to `Inspector` — Depends on [4.1]

- **BATCH**: B5
- **ACTION**: Introduce a minimal selection bridge so `LibraryPage` can expose its selected game to `AppShell → Inspector`. Two options — choose by reading the existing `ContentArea.tsx` / `LibraryPage.tsx` data-flow first: **Option A (preferred if Content mounts LibraryPage directly):** `LibraryPage` sets a lightweight `useState<string | null>` (`inspectorName`) and computes `const inspectorSelection = summaries.find(s => s.name === inspectorName) ?? null;`. It exposes the selection upward via a new React context `InspectorSelectionContext` (located at `src/crosshook-native/src/context/InspectorSelectionContext.tsx`), with a provider around the `<ContentArea>` call in `AppShell.tsx`. `AppShell` consumes `useContext(InspectorSelectionContext)` and passes `selection` into `<Inspector>`. **Option B (fallback):** thread `inspectorName` upward via props if the `ContentArea` → page hierarchy is shallow enough to not require a context. Default to Option A. Update `LibraryGrid.tsx` to add `onSelect?: (name: string) => void` and `selectedName?: string` props, forward to `LibraryCard` as `isSelected={profile.name === selectedName}`. Update `LibraryGrid.test.tsx` accordingly. In `LibraryPage.tsx`, pass `setInspectorName` as `onSelect` and `inspectorName` as `selectedName` to the grid; also pass `inspectorSelection` to the context provider. Add local `sortBy`/`filter` state on `LibraryPage` wired to the new toolbar props.
- **IMPLEMENT**: Target ~80 lines added across files. New `InspectorSelectionContext` exports `InspectorSelectionProvider` and `useInspectorSelection`. `LibraryPage.tsx` stays under 600 lines. Keep `ProfileContext.selectedProfile` **unchanged** — the inspector's "active profile" section reads it but the **inspector's own selection** is orthogonal.
- **MIRROR**: `CHILD_COMPONENT_MOCK` for grid tests, existing `ProfileContext` structure for the new `InspectorSelectionContext`.
- **IMPORTS**: In `LibraryPage.tsx`: `useInspectorSelection`. In `AppShell.tsx`: `InspectorSelectionProvider`, `useInspectorSelection`. In `LibraryGrid.tsx`: new prop types only.
- **GOTCHA**: `LibraryPage` may unmount when the user navigates away from `library`. The `InspectorSelectionProvider` must be hoisted to `AppShell` (wrap the entire `Group` or `ContentArea`) so the context exists even when `LibraryPage` is unmounted. When `LibraryPage` is unmounted, the selection is `null`, the inspector body renders its empty state, and no crash occurs. Double-check that `inspectorSelection` is a **derived** value — do **not** store the whole `LibraryCardData` in state; store just the `name` and look up the summary on render so stale data can't leak. If the summary list refreshes (IPC poll), the inspector's selection naturally updates.
- **VALIDATE**:
  - `rg -n 'InspectorSelectionContext|useInspectorSelection' src/crosshook-native/src/` — multiple matches across the new context, `AppShell.tsx`, and `LibraryPage.tsx`.
  - `cd src/crosshook-native && npm test -- src/components/library/__tests__/LibraryGrid.test.tsx` — green; asserts `onSelect` fires on card click and `data-selected="true"` reflects `selectedName`.
  - `cd src/crosshook-native && npm test -- src/components/pages/__tests__/LibraryPage.test.tsx` — green (existing file); add at least one new assertion that clicking a `LibraryCard` updates the inspector's visible name.
  - `cd src/crosshook-native && npm run typecheck` — zero errors.

### Task 4.3: Integration tests + acceptance validation — Depends on [4.1, 4.2]

- **BATCH**: B6
- **ACTION**: Update `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx` with two new cases: (1) at `desk` breakpoint, both `sidebar` and `inspector` testids are present in the DOM; (2) at `deck` breakpoint (`setInnerWidth(1280)` + `mockAppShellRect`), `inspector` testid is absent. Update `LibraryPage.test.tsx` (or create if missing) to cover: clicking a card renders the inspector's hero with that game's name; toolbar chip click updates `sortBy` and reorders grid; palette-trigger click calls `onOpenCommandPalette` mock. Run the full validation stack (typecheck, lint, unit tests, smoke tests, dev-native boot) and capture evidence.
- **IMPLEMENT**: Use the existing `setInnerWidth` + `mockAppShellRect` helpers at `AppShell.test.tsx:12-30` — **do not** swap them out for `mockBreakpoint` from Task 1.3. The `AppShell` test is integration-scope; `mockBreakpoint` is for the new unit-scope tests in Tasks 2.1/2.3/3.1. Playwright smoke: update `tests/smoke/shell.spec.ts` (or the closest shell spec) to assert the inspector is visible on a `desk`-sized viewport and absent on a `deck`-sized viewport.
- **MIRROR**: Existing `AppShell.test.tsx` setup and teardown. `TEST_STRUCTURE` for any new test files.
- **IMPORTS**: Existing.
- **GOTCHA**: `resize-target-minimum-size` may confuse happy-dom's layout math; assertions based on `data-testid` presence/absence are reliable, assertions based on pixel widths are not. Prefer the former. For Playwright, resize the browser via `page.setViewportSize({ width: 1280, height: 800 })` before asserting inspector absence at `deck`.
- **VALIDATE**:
  - `cd src/crosshook-native && npm run typecheck` — zero errors.
  - `cd src/crosshook-native && npm run lint` — zero errors (Biome).
  - `cd src/crosshook-native && npm test` — all unit/integration tests green.
  - `cd src/crosshook-native && npm run test:smoke` (or project's Playwright entry — verify in `package.json`) — shell spec green.
  - `./scripts/dev-native.sh --browser` launches, Library renders, cards clickable, inspector updates on click, resize window to <1440px → inspector collapses at deck breakpoint → no console errors.
  - Capture screenshots at `desk` and `deck` for the PR description.
  - `gh issue view 416` → mark deliverables acceptance-criteria satisfied in the PR body.

---

## Testing Strategy

### Test File Matrix

| File                                                        | Level       | New? | Primary assertions                                                                                                                                                            |
| ----------------------------------------------------------- | ----------- | ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/test/breakpoint.ts`                                    | helper      | new  | Helper returns deterministic `UseBreakpointResult` per size — self-tested via throwaway or permanent unit test.                                                               |
| `src/components/layout/__tests__/Inspector.test.tsx`        | unit        | new  | Renders `data-testid="inspector"`; empty body when `ROUTE_METADATA[route].inspectorComponent` unset; error boundary catches a `<ThrowingChild/>`.                             |
| `src/components/library/__tests__/LibraryCard.test.tsx`     | unit        | upd  | Existing Launch/Edit/Unfavorite tests stay green; new: heart `aria-pressed` per `isFavorite`; heart click fires `onToggleFavorite`; hover-reveal element present.             |
| `src/components/library/__tests__/LibraryToolbar.test.tsx`  | unit        | new  | Sort/filter chip click fires `on{Sort,Filter}Change`; chip `aria-pressed` state matches prop; palette-trigger click fires `onOpenCommandPalette`.                             |
| `src/components/library/__tests__/GameInspector.test.tsx`   | unit        | new  | Empty-state when `selection` undefined; each of the 5 sections renders on selection; button clicks fire the correct `on*` props; Health reads from `useProfileHealthContext`. |
| `src/components/library/__tests__/LibraryGrid.test.tsx`     | unit        | upd  | `onSelect` fires on card click; `data-selected` reflects `selectedName`; existing Enter-key + mock child assertions stay green.                                               |
| `src/components/pages/__tests__/LibraryPage.test.tsx`       | integration | upd  | Card click updates inspector hero title; sort chip click reorders grid; palette-trigger click forwards to `onOpenCommandPalette` mock.                                        |
| `src/components/layout/__tests__/AppShell.test.tsx`         | integration | upd  | `sidebar` + `inspector` testids at `desk`; `inspector` absent at `deck`; existing 2-panel resize tests stay green.                                                            |
| `tests/smoke/shell.spec.ts` (or project's shell smoke spec) | e2e         | upd  | Playwright: Library → click card → inspector hero updates; resize to `deck` → inspector hidden; no console errors.                                                            |

### Coverage Targets

No explicit global coverage target. Each new component must have a co-located `__tests__` file. Existing test files must remain green.

### TDD Flow (recommended per task)

Tasks 2.1 / 2.3 / 3.1 are ideal TDD starting points: write the test file first using `mockBreakpoint` + the exported prop contract, run `npm test -- <file>` to watch it fail, then implement the component until green.

---

## Validation Commands

Run from the repo root unless stated otherwise.

```bash
# Typecheck (TS strict)
cd src/crosshook-native && npm run typecheck

# Lint + format (Biome)
cd src/crosshook-native && npm run lint

# Unit + integration tests (Vitest + happy-dom)
cd src/crosshook-native && npm test

# Single test file
cd src/crosshook-native && npm test -- src/components/layout/__tests__/Inspector.test.tsx

# Smoke tests (Playwright)
cd src/crosshook-native && npm run test:smoke
# or: npx playwright test tests/smoke/shell.spec.ts

# Dev boot (WebView2 / Tauri-less browser mode)
./scripts/dev-native.sh --browser

# Full native dev (Tauri)
./scripts/dev-native.sh

# Rust side (sanity only — no changes land in src-tauri)
cd src/crosshook-native/src-tauri && cargo check
```

### Pre-PR gate

All four must pass before opening the PR:

1. `npm run typecheck` → 0 errors
2. `npm run lint` → 0 errors
3. `npm test` → all green
4. `npm run test:smoke` (or equivalent Playwright entry) → shell spec green

---

## Acceptance Criteria

Derived from GitHub issues #443 and #416 + PRD Phase 4:

1. **Inspector rail is persistent** at `uw`/`desk`/`narrow` breakpoints and **collapsed/absent** at `deck` (≤1280 px wide).
2. **Library cards** gain the redesigned chrome: hover-gradient reveal and always-visible favorite-heart badge when favorited.
3. **Library toolbar** exposes sort chips (`recent`/`name`/`lastPlayed`/`playtime`), filter chips (`all`/`favorites`/`installed`/`recentlyLaunched`), and a trailing ⌘K palette-trigger button (dormant placeholder until Phase 6 wires the palette).
4. **`routeMetadata.inspectorComponent`** contract is in place: `library` sets `GameInspector`; every other route omits the field and still compiles.
5. **`GameInspector`** renders: hero → pills → quick actions → active profile (from `useProfileContext`) → recent launches (empty-state placeholder) → health (from `useProfileHealthContext`).
6. **Inspector is keyboard-reachable**: tab order flows main-content → inspector; buttons reachable in DOM order.
7. **`prefers-reduced-motion: reduce`** disables hover-reveal transitions on cards.
8. **Testids**: `data-testid="sidebar"` on Sidebar root, `data-testid="inspector"` on Inspector root (Phase 5 depends on these).
9. **`useScrollEnhance`** registers `.crosshook-sidebar__nav--scroll` and `.crosshook-inspector__body`; both elements have `overscroll-behavior: contain` in CSS.
10. **Scope discipline**: zero Rust changes; zero changes outside `src/crosshook-native/src/{components,hooks,styles,context,test,types}` and `tests/smoke/`.
11. **File-size cap**: no edited/created `.tsx`/`.ts` file exceeds 500 lines (CLAUDE.md contract).
12. **Follow-up issue filed** for `list_launch_history_for_profile` IPC, linked in the PR body.

---

## Completion Checklist

- [ ] Task 1.1: `routeMetadata.ts` extended with `inspectorComponent?` + `SelectedGame` alias.
- [ ] Task 1.2: `SCROLLABLE` in `useScrollEnhance.ts` updated.
- [ ] Task 1.3: `src/test/breakpoint.ts` helper created and self-tested.
- [ ] Task 2.1: `inspectorVariants.ts`, `Inspector.tsx`, `InspectorErrorBoundary`, base CSS, and `Inspector.test.tsx` landed.
- [ ] Task 2.2: `LibraryCard` redesigned (hover-reveal + heart) with updated tests.
- [ ] Task 2.3: `LibraryToolbar` redesigned (chips + palette-trigger) with new test file.
- [ ] Task 3.1: `GameInspector.tsx` + `GameInspector.test.tsx` landed; uses `useProfileContext` + `useProfileHealthContext`.
- [ ] Task 3.2: `makeProfileHealthReport` fixture added to `test/fixtures.ts`.
- [ ] Task 3.3: `ROUTE_METADATA.library.inspectorComponent = GameInspector` set.
- [ ] Task 4.1: `AppShell` renders 3rd Panel conditionally; `Sidebar` has scroll class + `data-testid="sidebar"`.
- [ ] Task 4.2: Selection bridge (`InspectorSelectionContext`) in place; `LibraryGrid.onSelect/selectedName` wired; toolbar state wired.
- [ ] Task 4.3: Integration/e2e tests updated; full validation stack green.
- [ ] Follow-up GitHub issue filed for recent-launches IPC; issue number linked in PR body.
- [ ] Screenshots (desk + deck) attached to PR.
- [ ] `gh pr create` opened with conventional-commit title (suggested: `feat(native): add persistent inspector rail + Library redesign (phase 4)`), body references issues #443 and #416 with `Closes #443` and references #416 deliverables.
- [ ] CI green on PR.

---

## Risks

| Risk                                                                                                  | Impact | Likelihood | Mitigation                                                                                                                                                                                                                            |
| ----------------------------------------------------------------------------------------------------- | ------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Adding a 3rd `react-resizable-panels` Panel breaks existing `AppShell.test.tsx` resize assertions     | Med    | Med        | Task 4.3 explicitly updates those tests; use `data-testid` presence checks (reliable in happy-dom) rather than pixel-width assertions.                                                                                                |
| `vi.mock('@/hooks/useBreakpoint')` hoisting interacts poorly with the `mockBreakpoint` helper         | Med    | Med        | Task 1.3 exports a pure `breakpointResult(size)` return value, not a side-effectful `vi.mock` call. Canonical usage is a module-top `vi.mock(...)` + `vi.mocked(useBreakpoint).mockReturnValue(breakpointResult(size))` inside tests. |
| `GameInspector` reads `useProfileHealthContext` synchronously but context may lazy-fetch              | Med    | Med        | Task 3.1 requires reading the context file before implementing the Health section; render "Loading…" / null while pending, never block render on a pending fetch.                                                                     |
| Circular import between `routeMetadata.ts` and `GameInspector.tsx` when Task 3.3 lands                | High   | Low        | `GameInspector` only `import type { SelectedGame }` — never the runtime `ROUTE_METADATA`. Enforced during Task 3.1 review. Vite surfaces circulars in build logs (validated in Task 3.3).                                             |
| Favorite heart button nested inside card-wide clickable hitbox = invalid `<button>` inside `<button>` | Med    | Med        | Task 2.2 explicitly renders the heart as a **sibling** of the hitbox, not a descendant. Biome will flag nested interactive elements if the mistake is made.                                                                           |
| Breakpoint-driven Panel skipping may trigger `react-resizable-panels` Group re-init and flash         | Low    | Low        | Mounting/unmounting a Panel at a threshold is supported. If flash appears in manual QA, add a transitional CSS `opacity` step (out-of-scope fix → follow-up issue).                                                                   |
| User-pinnable inspector width requested by Phase 7 would conflict with frozen-width Panel pattern     | Low    | Low        | Accepted and explicit — Phase 7 scope. This plan uses frozen widths intentionally. Task 2.1 reserves a `.crosshook-inspector--collapsed` modifier class for future use.                                                               |
| Recent-launches empty-state placeholder may look broken to a first-time user                          | Low    | Low        | Placeholder copy uses `role="status"` + "Coming soon" text; follow-up issue is linked in PR body; copy is intentionally non-alarming.                                                                                                 |
| Tests that stub `ProfileContext`/`ProfileHealthContext` may miss real-world context re-renders        | Low    | Low        | Integration test in Task 4.3 uses `renderWithMocks` to mount the real `AppShell` → `ContentArea` → `LibraryPage` hierarchy with seeded IPC, exercising real context behavior end-to-end.                                              |

---

## Notes

### Worktree lifecycle (inherits from Worktree Setup section)

1. Create the parent worktree first (`git worktree add ~/.claude-worktrees/crosshook-unified-desktop-phase-4-library-inspector/ -b feat/unified-desktop-phase-4-library-inspector`).
2. For each parallel task in a batch, create a child worktree off the parent branch (or off main — either is acceptable; the plan pattern uses off-parent so children share the evolving feature branch state).
3. Implement the task in the child worktree.
4. At the end of each batch, merge all child branches back into the parent feature branch (`git merge --no-ff feat/unified-desktop-phase-4-library-inspector-<task-id>`) inside the parent worktree.
5. Resolve any merge conflicts at batch boundaries (expected in B4 because the B2/B3 tasks touched `LibraryCard.tsx` / `LibraryToolbar.tsx` / `library.css` whose changes both end up being wired by B4).
6. Remove child worktrees after merge (`git worktree remove <path>`).
7. Open the PR from the parent feature branch.

### Follow-up GitHub issues to file (during Phase 4)

1. **Recent launches IPC** — Title: `feat(core): add list_launch_history_for_profile IPC for inspector recent-launches section`. Scope: Rust backend `#[tauri::command]` + TS binding + `GameInspector` wiring. Dependency for Phase 5 hero-detail mode.
2. **User-pinnable inspector width** (deferred from Phase 4) — Title: `feat(native): allow user to resize/pin inspector width with persistence`. Scope: swap frozen-width Panel for a user-resizable Panel + `localStorage` persistence.
3. **Gamepad zones for inspector rail** (deferred from Phase 4) — Title: `feat(native): add gamepad focus-zone for inspector rail`. Scope: extend the gamepad-nav system to treat the inspector rail as a distinct zone.

### Conventional commit scope

Prefer `feat(native)` for component/shell changes, `test(native)` for test-only tasks, `docs(native)` for any doc update. The umbrella PR will be a single `feat(native)` with detailed body.

### Dev-loop guidance for the implementor

- Run `npm test -- --watch` in a dedicated terminal for the component under implementation.
- Run `./scripts/dev-native.sh --browser` in another terminal — fast reload, no Tauri build cost.
- Reserve `./scripts/dev-native.sh` (full Tauri) for the final smoke before opening the PR.
- If a Rust-side change ever seems needed → STOP, re-read the PRD scope, and file a follow-up issue; do not expand Phase 4 scope.
