# CrossHook Unified Desktop Redesign

## Problem Statement

CrossHook today is a **desktop app that feels like a fixed-width website**. On any monitor, the shell centers at `1440px` (`src/styles/variables.css:87`, enforced twice at `src/styles/layout.css:2,50`) and leaves the rest of the screen dark. On ultrawide monitors (3440×1440) the user stares at a letterboxed frame; on a Steam Deck (1280×800) the same frame crops the sidebar. Steam, Lutris, and Heroic all scale their shells to fill the display — CrossHook does not. The cost: the app reads as a toy or an Electron wrapper rather than a first-class Linux desktop tool, and the most valuable secondary-pane affordances (always-on inspector, ⌘K command palette, ultrawide context rail) cannot exist at all.

## Evidence

- User-reported observation (design-bundle chat, `2026-04-22`): _"It's built like a 'web page' so the horizontal space is fixed. No matter how wide a monitor is, it's only ever presented as you see in the screenshot. While Steam, Lutris and others take advantage of the entire desktop space."_
- Codebase confirms: `--crosshook-content-width: 1440px` applied twice in `layout.css`. No `useBreakpoint`, no `useMediaQuery`, no viewport-aware rendering path anywhere in `src/crosshook-native/src/`.
- Design assistant delivered a full Unified Design bundle (`/tmp/crosshook-unified-design/extracted/crosshook/project/hifi/*`) with three-pane shell, Library ↔ Hero Detail modes, ⌘K palette, toned-down steel-blue palette, and responsive breakpoints `uw≥2200 · desk≥1440 · narrow≥1100 · deck<1100`. The user validated it end-to-end across 4 viewport sweeps.
- No ⌘K, cmdk, kbar, or command-palette pattern exists anywhere in the current codebase (grep-confirmed). The design introduces a category that doesn't exist today.
- The current detail path is a blocking **modal** (`GameDetailsModal.tsx`, 479 lines). Design specifies an in-shell **hero takeover** so sidebar + inspector remain mounted and the palette can open over it.

## Proposed Solution

Adopt the Unified Design across the entire frontend as a single coherent design system rework, split into ordered phases so each phase is independently shippable and verifiable. The chosen shape is a **responsive three-pane shell** (sidebar · main · inspector) with an **optional ultrawide context rail**, driven by a single `useBreakpoint` hook (`uw≥2200 · desk≥1440 · narrow≥1100 · deck`). **Library** and **Hero Detail** are _modes_ of the same shell (not routes) so the sidebar/inspector never unmount. A **⌘K command palette** overlays any mode, any viewport. The existing routes (Profiles, Proton Manager, Host Tools, Community, Settings, Install, Launch, Health, Compatibility, Discover) are reworked to fit the shell's visual language — panels, pills, kv-rows, tokens. The **steel-blue palette** (`#4a7db5 / #6ba3d9`) replaces the current `#0078d4 / #2da3ff` tokens in place; no theme switcher, no alternate. Why this approach: the design was authored specifically against this codebase, the user has already reviewed and approved it at 4 breakpoints, and an alternate-theme path would double visual QA for a calm-desktop goal the user explicitly wants to be _the_ look.

## Key Hypothesis

We believe a **responsive three-pane shell with in-shell mode transitions and a toned-down calm palette** will **make CrossHook feel like a native Linux desktop app that fills any screen (Steam Deck → ultrawide) instead of a fixed-width web page** for **Linux gamers running CrossHook as their primary game launcher**. We'll know we're right when **the shell scales from 1280×800 to 3440×1440 with zero letterboxing, Library and Hero Detail transition without unmounting the sidebar/inspector, and the palette loads without a single reference to `#0078d4` or `#1a1a2e` in any stylesheet**.

## What We're NOT Building

- **Alternate themes / theme switcher** — user chose token replacement; `Classic` mode is out of scope. Existing user prefs that touch color (high-contrast override in `useAccessibilityEnhancements.ts`) remain untouched.
- **Persisted layout preferences beyond what exists** — the `react-resizable-panels` Panel sizes already persist via browser storage. Adding per-mode panel-size persistence, inspector-collapsed memory, or cmdk recency ranking is a separate PRD.
- **A new router** — state-driven `Tabs.Root` stays. No URL routes, no back/forward history, no deep-linking. Library↔Detail are state toggles inside the Library route, not separate URLs.
- **A new icon library** — we use the existing icon system under `src/components/icons/`. The design's inline SVGs are a reference, not a replacement.
- **Replacing `react-resizable-panels`** — we extend the existing Panel Group from 2 columns to 3, and optionally 4 on uw. No swap to a different panel library.
- **Community/marketplace changes**, profile-editor feature changes, any backend or `crosshook-core` logic changes. This PRD is frontend-only. `src-tauri` IPC surfaces and `#[tauri::command]` signatures remain unchanged.
- **`gamepad-nav` focus-zone overhaul to 4+ zones** as a prerequisite — we'll treat palette + inspector as overlay/modal-like zones that reuse existing zone primitives for v1. A proper n-zone refactor is called out as a follow-up.

## Success Metrics

| Metric                                | Target                                                                            | How Measured                                                           |
| ------------------------------------- | --------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Shell fills the viewport              | 0 letterboxed pixels at `1280×800`, `1920×1080`, `2560×1440`, `3440×1440`         | Playwright smoke screenshots at each breakpoint                        |
| Library → Hero Detail without unmount | Sidebar/inspector DOM nodes persist across mode toggle                            | Vitest + React Testing Library                                         |
| Zero legacy palette leakage           | 0 matches for `#0078d4`, `#2da3ff`, `#1a1a2e`, `#20243d`, `rgba(0, 120, 212, …)`  | grep in `src/styles/**` and component styles post-migration            |
| ⌘K palette MTTO (mean time to open)   | ≤120ms from keypress to first frame rendered                                      | Playwright perf trace                                                  |
| Deck viewport usability               | Sidebar collapses to rail, inspector hides, status bar replaces drawer at `<1100` | Visual smoke at `1280×800`, manual Deck pass                           |
| No scroll jank (nested scroll)        | Every new overflow-y container registered in `useScrollEnhance` SCROLLABLE        | grep audit + manual scroll test on WebKitGTK                           |
| Route regression rate                 | 0 broken routes in `ROUTE_ORDER` smoke sweep                                      | `npm run test:smoke` (plus newly added `host-tools`, `proton-manager`) |

## Resolved Decisions

- [x] **Inspector content per-route** — non-Library routes get an opt-in inspector rail via `routeMetadata.ts` (`inspectorComponent` or `null`). Resolved in GitHub via `#434`.
- [x] **Console drawer default** — the redesign uses an explicit toggle only; it does not auto-open on first log line. Resolved in GitHub via `#435`.
- [x] **Hero-detail tabs backfill** — omit `Media` for v1 and keep the follow-up tracked in GitHub via `#433`. `Compatibility` remains opt-in behind `showCompatibility`. Resolved in GitHub via `#436`.
- [x] **Scope of `ProfilesPage` rework** — Phase 11 is a full redesign, not a chrome-only re-skin. File splits still happen first, but the editor information architecture may change as needed to match the new shell. Resolved in GitHub via `#437`.
- [x] **Gamepad focus on the palette** — v1 uses focus-trap modal behavior via `useFocusTrap`; the broader n-zone gamepad-nav follow-up stays tracked in GitHub via `#432`. Resolved in GitHub via `#438`.
- [x] **Sidebar group ordering** — formalize `Collections` as a first-class section by merging `CollectionsSidebar` output into declared `SIDEBAR_SECTIONS`. Resolved in GitHub via `#439`.

---

## Users & Context

**Primary User**

- **Who**: Linux gamers who use CrossHook as a primary game launcher across a range of displays — desktop monitors (1080p/1440p/ultrawide) and Steam Deck handhelds. They launch Windows games via Proton/Wine, manage profiles, and frequently jump between library/profile-editor/proton-manager surfaces.
- **Current behavior**: Launch CrossHook → get a centered 1440px column on a 3440px ultrawide → switch between routes via the left sidebar → open a blocking modal to view game details → dismiss the modal to go back.
- **Trigger**: Launching on a new display (ultrawide desktop, Deck, 1080p laptop) and noticing the app doesn't adapt; trying to compare two games or a game and its pipeline side-by-side and realizing there is no second pane.
- **Success state**: The app fills the monitor, secondary panes carry useful content (game metadata, health, launch activity), and common actions (launch, edit profile, switch proton) are keystroke-accessible without a route change.

**Job to Be Done**
When **I open CrossHook on any display from a Steam Deck to a 3440px ultrawide**, I want to **see my library with a secondary detail pane that fills the screen and jump to any action with a keyboard shortcut**, so I can **treat CrossHook like a native desktop tool, not a cramped web app**.

**Non-Users**

- Not for people running CrossHook **only** on 1280×800 Steam Decks — they already get a reasonable experience; the rework doesn't target them exclusively, it just doesn't break them.
- Not for browser-only users — `./scripts/dev-native.sh --browser` is a dev-time mock layer, not a product surface. The redesign optimizes for the native Tauri window first.
- Not for power users who want a tiling-WM-style keyboard-only app — we add ⌘K but don't go full modal-editor. Mouse/gamepad/touchpad remain first-class.

---

## Solution Detail

### Core Capabilities (MoSCoW)

| Priority | Capability                                                                                          | Rationale                                                                                 |
| -------- | --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Must     | `useBreakpoint` hook with `uw / desk / narrow / deck` outputs                                       | Everything responsive routes through one source of truth; no duplicated media queries     |
| Must     | Drop `--crosshook-content-width` cap on `.crosshook-app-layout`                                     | Without this, nothing else looks like it fills the screen                                 |
| Must     | Three-pane shell (sidebar · main · inspector) via `react-resizable-panels`                          | The whole hypothesis is "three-pane fills the screen"                                     |
| Must     | Sidebar variants: `full 240` · `mid 68` · `rail 56`, driven by breakpoint                           | Design specifies this explicitly; CSS already exists at `sidebar.css:204-244` but unwired |
| Must     | Token swap to steel-blue palette in `variables.css` + literal-fallback sweep across all stylesheets | User decision: replace tokens, no alternate theme                                         |
| Must     | Library mode: hover-gradient-reveal cards, inspector rail (right), library topbar + palette trigger | Core Library surface rework                                                               |
| Must     | Hero Detail mode: full-bleed hero, cover thumbnail, tabs, panel grid (4/3/2/1 cols by breakpoint)   | Replaces blocking `GameDetailsModal`; keeps sidebar/inspector mounted                     |
| Must     | ⌘K command palette overlay with focus-trap, list scroll, keyboard nav                               | Third design pillar; greenfield feature                                                   |
| Must     | Console drawer → status bar swap at `narrow` and `deck`                                             | Existing drawer takes 60% of deck height — intolerable                                    |
| Must     | All existing routes re-skinned in the new visual language                                           | User chose "Adopt shell + redesign all routes"                                            |
| Must     | Context rail (ultrawide only): host readiness, pinned profiles, 7-day activity chart, most played   | Only exists when `useBreakpoint() === 'uw'`                                               |
| Must     | Register new scroll containers in `useScrollEnhance` SCROLLABLE selector                            | CLAUDE.md explicitly requires this; otherwise WebKitGTK dual-scroll jank                  |
| Must     | Visual smoke test coverage at each breakpoint (`1280`, `1920`, `2560`, `3440`)                      | PRD metric; prevents regressions                                                          |
| Should   | Sidebar "Collections" formalized as a declared section (not runtime-injected)                       | Aligns code with design; simpler sidebar model                                            |
| Should   | Topbar (per-route breadcrumb + title + search + view/sort/filter chips)                             | Improves wayfinding; design specifies it                                                  |
| Should   | Per-route inspector content contract via `routeMetadata.ts`                                         | Lets Profiles/Proton Manager/etc. ship empty inspector without blocking the shell landing |
| Should   | Titlebar traffic-light dots + size/mode indicator as a dev/debug affordance                         | Design shows it; cheap to add; helpful for visual QA                                      |
| Could    | Command palette result ranking (recency, fuzzy scoring)                                             | Nice but not required for MVP; a hand-coded static command list is enough for v1          |
| Could    | Inspector-collapsed state persistence per route                                                     | Users may want a default collapsed inspector on some routes                               |
| Could    | ⌘K palette history ("recent commands")                                                              | Power-user nicety                                                                         |
| Won't    | URL routing                                                                                         | Not in scope; explicit                                                                    |
| Won't    | `Media` tab on Hero Detail                                                                          | No data source; deferred                                                                  |
| Won't    | Alternate `Classic` palette theme                                                                   | User rejected it                                                                          |
| Won't    | n-zone gamepad-nav refactor                                                                         | Treated as a follow-up; v1 reuses focus-trap                                              |
| Won't    | Persisted cmdk recency / fuzzy scoring                                                              | MVP uses static hand-authored command list                                                |

### MVP Scope

**Minimum to validate the hypothesis**: Phases 1–4 (shell skeleton + tokens + sidebar variants + Library redesign with inspector rail). After those four phases land, the shell already fills any monitor, the Library already has an inspector rail, and the palette already replaces the old accent. That's enough to answer "does this feel like a desktop app now?" — the rest (Hero Detail, ⌘K, routes rework, context rail) are the amplifier, not the proof.

### User Flow

**Critical path — shortest journey to value on each display**:

- **Ultrawide (3440×1440)**: app opens → full sidebar + library grid + inspector rail + context rail all visible → click card → inspector populates → double-click or press `Enter` → Hero Detail takeover with sidebar/inspector persistent → ⌘K launches palette over any state.
- **1080p (1920×1080)**: app opens → full sidebar + library grid + inspector rail → context rail hidden → same double-click → Hero Detail with 3-column panel body.
- **Laptop (1440×900)**: full sidebar + grid + narrower inspector (280px); 2-column hero panel body.
- **Steam Deck (1280×800)**: icon rail sidebar + grid only (no inspector) + status bar (no drawer); Hero Detail stacks panels 1-column with compact hero.

### Responsive contract

The single source of truth is a `useBreakpoint()` hook that returns `'uw' | 'desk' | 'narrow' | 'deck'` and is consumed by the shell, sidebar, inspector, and console-drawer components. CSS media queries for visual-only breakpoints are allowed but must match the hook thresholds to keep behavior and visual state aligned.

---

## Technical Approach

**Feasibility**: MEDIUM overall. The shell skeleton, token swap, and Library redesign are MEDIUM. Hero Detail is MEDIUM-HIGH. ⌘K is HIGH (greenfield). Routes rework is HIGH (6 pages, 3 past the 500-line cap).

**Architecture Notes**

- **Shell rewrite** in `src/App.tsx` (+ extract into `src/components/layout/AppShell.tsx` to keep both files under the 500-line cap). Replace the current 2-column `Group` with a 3-column `Group` (sidebar · content · inspector) and optionally add a 4th panel (context rail) when `useBreakpoint() === 'uw'`. Keep `react-resizable-panels` — it already handles persistence and collapsibility.
- **Drop the width cap** at `src/styles/layout.css:2,50` — switch `width: min(100%, var(--crosshook-content-width))` to `width: 100%`. Delete `--crosshook-content-width` from `variables.css:87` so no future CSS can reintroduce the cap. Update `.crosshook-content-viewport` likewise.
- **Token swap** in `src/styles/variables.css`:
  - `--crosshook-color-accent: #0078d4` → `#4a7db5`
  - `--crosshook-color-accent-strong: #2da3ff` → `#6ba3d9`
  - `--crosshook-color-accent-soft: rgba(0, 120, 212, 0.18)` → `rgba(74, 125, 181, 0.16)`
  - `--crosshook-color-bg: #1a1a2e` → `#181a24`
  - `--crosshook-color-bg-elevated: #20243d` → `#1f2233`
  - `--crosshook-color-surface: #12172a` → `#14162099`
  - Add new sibling tokens where the design introduces them: `--crosshook-color-sidebar: #10121c`, `--crosshook-color-titlebar: #0c0e16`, `--crosshook-color-surface-1/2/3` for rows/raised/hover, `--crosshook-color-scrim: rgba(8, 10, 18, 0.78)`, `--crosshook-color-accent-glow: rgba(107, 163, 217, 0.22)`, desaturated status colors (`success #5fb880`, `warning #d4a94a`, `danger #d77a8a`).
- **Literal sweep** — after token update, grep `rgba(0, 120, 212,` (~61 spots in `theme.css`, `sidebar.css:26,89,111,131,146,149,183`, `library.css`, `layout.css`, `themed-select.css`), `rgba(45, 163, 255,` (~7 spots), `#1a1a2e` (2 spots at `theme.css:4547,4590`), `#0078d4`, `#2da3ff`, `#20243d`, `#12172a`, `#0c1120` — replace with the new literals using the design's `theme.css` as the reference. Document the rule in `docs/internal/design-tokens.md`: _literal accent/background colors are banned — always use the token_.
- **`useBreakpoint`** at `src/hooks/useBreakpoint.ts` (new). Uses `window.matchMedia` + `ResizeObserver` on the shell root for SSR-safe init. Returns `{ size, width, height, isDeck, isNarrow, isDesk, isUw }`. Consumed by `AppShell`, `Sidebar`, `Inspector`, `ConsoleDrawer`/`StatusBar`.
- **Sidebar variants** — wire the existing CSS at `sidebar.css:204-244`. `Sidebar.tsx` reads breakpoint, sets `data-collapsed` + `.crosshook-sidebar--rail|mid|full` classes. Item labels/badges hide under rail/mid. Formalize `Collections` as a declared `SIDEBAR_SECTIONS` entry instead of runtime injection.
- **Inspector rail** — new `src/components/layout/Inspector.tsx`, width `360 (uw) / 320 (desk) / 280 (narrow) / 0 (deck)`. Per-route inspector content declared in `routeMetadata.ts` as `inspectorComponent?: ComponentType<{ selection?: … }>`. Library supplies a Game inspector; other routes ship `null` for v1 and fill in over the routes-rework phase.
- **Hero Detail mode** — new `src/components/library/GameDetail.tsx` (hero + tabs + panels). Mode toggle lives in `LibraryPage`'s state (`mode: 'library' | 'detail'`, `selected: gameId`). When `mode === 'detail'`, `LibraryPage` renders `GameDetail` in the main slot; the shell sidebar + inspector stay mounted. Deprecates `GameDetailsModal.tsx` (removed in its phase). Tabs: `Overview · Profiles · Launch options · Trainer · History · Compatibility`. `Media` tab skipped for v1.
- **⌘K palette** — `src/components/palette/CommandPalette.tsx` (new) + `src/hooks/useCommandPalette.ts` + `src/lib/commands.ts` (static command list: launch game, edit profile, open proton manager, open host tools, settings, nav). Trigger: `Cmd/Ctrl+K` registered in `AppShell`. Focus-trap via existing `useFocusTrap`. Backdrop uses existing `useScrollEnhance` contract (new `.crosshook-palette__list` registered). No fuzzy/ranking for v1 — substring match over command title.
- **Console drawer → status bar** — `ConsoleDrawer.tsx` gains a `mode: 'drawer' | 'status'` prop driven by `useBreakpoint`. On `deck|narrow` it renders as a 32px status bar showing readiness chips + tip "⌘K commands". On wider, the drawer remains available behind an explicit toggle and does not auto-open on first log line.
- **Context rail** — `src/components/layout/ContextRail.tsx` (new). Only mounted when `useBreakpoint() === 'uw'` AND mode is Library. Contents match the design: host-readiness pills (wraps `useHostReadiness`), pinned profiles (reads from existing profiles store), 7-day launch-activity bar chart (reads `metadata.db` via existing launch-history IPC), most-played list. All data sources exist; this phase is composition, not new data.
- **Routes rework** — each route keeps its functional scope but adopts the new panel/pill/kv-row/field-readonly idioms. Profiles gets a full redesign in Phase 11 rather than a chrome-only pass; Launch is redesigned alongside it with behavior parity preserved. Any page currently >500 lines is split _before_ the rework (e.g. `LaunchPage.tsx` 591, `OnboardingWizard.tsx` 606, `ProfileFormSections.tsx` 582, `CommunityBrowser.tsx` 561, `ProtonDbLookupCard.tsx` 519, `LaunchSubTabs.tsx` 508) — split first, redesign second.
- **Scroll containers** — append to `src/hooks/useScrollEnhance.ts` SCROLLABLE selector: `.crosshook-sidebar__nav--scroll, .crosshook-inspector__body, .crosshook-context-rail__body, .crosshook-palette__list, .crosshook-hero-detail__body`.
- **Gamepad** — palette registers as a focus-trap modal (reuses `useFocusTrap`); don't extend zones for v1. File a follow-up issue for n-zone gamepad-nav as a post-v1 item.
- **Testing** — Vitest for shell/inspector/palette/hero-detail unit + interaction tests; Playwright smoke expanded to cover all 4 breakpoints and include `host-tools` and `proton-manager` (both currently skipped per `tests/smoke.spec.ts:33-45`).

**Dependencies / Integration points**

- `react-resizable-panels` (already used, extended)
- `@radix-ui/react-tabs` (already used; Hero Detail tabs reuse the pattern)
- `useFocusTrap`, `useScrollEnhance`, `useAccessibilityEnhancements`, `useGamepadNav` (all existing)
- Host readiness via `useHostReadiness` (existing)
- Existing Icon component set under `src/components/icons/`
- No new npm dependencies required for v1 (palette is hand-rolled — avoids `cmdk` or `kbar`)

### Persistence & usability

Per CLAUDE.md, classify each datum introduced by this feature:

- **TOML settings (user preferences)**: `sidebar_variant_override?` (optional — auto by breakpoint unless user pins), `inspector_collapsed_override?`, `console_drawer_mode?` (auto by breakpoint unless user pins). All optional with `auto` defaults.
- **SQLite metadata**: none directly — context-rail consumes existing tables (`host_readiness_snapshots`, launch-history).
- **Runtime-only**: `mode: 'library' | 'detail'` per session, selected game id (ephemeral), palette query text, breakpoint state.

**Migration/backward compatibility**: token swap is atomic — deployed with the phase that ships tokens. Any third-party screenshots/demos showing the old indigo palette will visually drift; this is documented in `CHANGELOG.md`. No data migration required. On first load after upgrade, users see the new palette immediately; no prompt. Preferences added in this PRD are optional and absent-by-default.

**Offline behavior**: fully offline. No network dependency.

**Degraded fallback**: if `useBreakpoint` fails to init (no `window.matchMedia`), default to `desk`. If an inspector component throws, the shell renders without it (React error boundary already wraps content in `App.tsx`).

**User visibility/editability**: sidebar variant, inspector width, and console mode all adjust automatically by breakpoint. Power users can pin variants via `Settings` (future; v1 leaves it auto-only).

**Technical Risks**

| Risk                                                                                 | Likelihood | Mitigation                                                                                                                                                                          |
| ------------------------------------------------------------------------------------ | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Literal palette leakage — `rgba(0,120,212,…)` and `#1a1a2e` in ~60 stylesheet spots  | H          | Dedicated Phase 2 sweep + grep audit metric (`0 matches`). Add a `./scripts/check-legacy-palette.sh` sentinel run in CI to prevent regressions.                                     |
| Gamepad-nav zones only support sidebar/content; overlay palette breaks LB/RB mapping | M          | V1 treats palette as a focus-trap modal (captures DPad locally). File a follow-up issue to extend zones to `overlay`/`inspector`. `tests/smoke` adds a DPad-navigates-palette case. |
| `theme.css` at 137KB — monolithic, risky to refactor                                 | M          | Token swap is surgical: change `variables.css` + sweep literals. No structural refactor of `theme.css` in this PRD. A separate `chore(theme-split)` PRD can chunk the file later.   |
| `--crosshook-content-width` is referenced elsewhere (e.g. banners)                   | M          | Grep all usages before deletion. If non-layout uses exist (e.g. a banner max-width), preserve those as local constants; don't keep the global token.                                |
| Route-rework phases individually violate the 500-line soft cap                       | M          | Split oversized pages before redesigning them. Any new file drafted during this PRD aims for <400 lines to leave edit headroom.                                                     |
| `GameDetailsModal` removal breaks linkers (other components `import` it)             | L          | Grep imports; route all calls to the new `GameDetail` mode toggle. Retain the file with a deprecation warning for one phase, then delete.                                           |
| Playwright smoke only covers 9 of 11 routes                                          | L          | Add `host-tools` and `proton-manager` to `ROUTE_ORDER` in `tests/smoke.spec.ts` as part of the testing phase.                                                                       |
| WebKitGTK scroll jank in 5 new overflow containers                                   | M          | Every new scroll container must be appended to `SCROLLABLE` in `useScrollEnhance.ts` in the same commit. CI lint or a review checklist enforces this.                               |

---

## Implementation Phases

<!--
  STATUS: pending | in-progress | complete
  PARALLEL: phases that can run concurrently
  DEPENDS: phases that must complete first
  PRP: link to generated plan file once created
-->

| #   | Phase                                                                         | Description                                                                                                                                       | Status      | Parallel  | Depends | PRP Plan                                                                                                                                                       |
| --- | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- | --------- | ------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Foundation: `useBreakpoint` + layout unlock                                   | Add the breakpoint hook, drop the `--crosshook-content-width` cap, extract `AppShell` from `App.tsx`, land a shell skeleton with 2-col fallback   | pending     | -         | -       | -                                                                                                                                                              |
| 2   | Token swap + legacy-palette sweep                                             | Update `variables.css`, add new sibling tokens, sweep literal hex/rgba across all stylesheets, ship CI sentinel `scripts/check-legacy-palette.sh` | pending     | with 3    | 1       | -                                                                                                                                                              |
| 3   | Sidebar variants + formalized sections                                        | Wire `.crosshook-sidebar--full/mid/rail`, driven by breakpoint; formalize Collections as a declared section                                       | pending     | with 2    | 1       | -                                                                                                                                                              |
| 4   | Library redesign: cards + inspector rail                                      | Rework `LibraryCard`/`LibraryGrid`/`LibraryToolbar`, add `Inspector.tsx`, register inspector-component contract in `routeMetadata.ts`             | in-progress | -         | 2, 3    | [unified-desktop-phase-4-library-inspector.plan.md](../plans/unified-desktop-phase-4-library-inspector.plan.md)                                                |
| 5   | Hero Detail mode                                                              | New `GameDetail.tsx` (hero + tabs + panel grid); Library ↔ Detail mode toggle; deprecate `GameDetailsModal.tsx`                                   | complete    | with 6    | 4       | [plan](../plans/completed/unified-desktop-phase-5-hero-detail-mode.plan.md), [report](../reports/unified-desktop-phase-5-hero-detail-mode-report.md)           |
| 6   | ⌘K command palette                                                            | `CommandPalette.tsx`, `useCommandPalette`, static command list, focus-trap, keyboard shortcut; smoke test                                         | pending     | with 5    | 4       | -                                                                                                                                                              |
| 7   | Context rail (ultrawide only)                                                 | `ContextRail.tsx` with host readiness + pinned profiles + 7-day activity chart + most played                                                      | pending     | with 5, 6 | 4       | -                                                                                                                                                              |
| 8   | Console drawer → status bar swap                                              | `ConsoleDrawer` gains `mode` prop driven by breakpoint; deck/narrow renders 32px status bar                                                       | pending     | with 5–7  | 1       | -                                                                                                                                                              |
| 9   | Route rework — Dashboards (Health, Host Tools, Proton Manager, Compatibility) | Re-skin dashboards in the new panel/pill/kv-row idioms; closest fit to the design — smallest delta                                                | complete    | with 10   | 2, 3    | [plan](../plans/completed/github-issues-421-448-route-rework-dashboards.plan.md), [report](../reports/github-issues-421-448-route-rework-dashboards-report.md) |
| 10  | Route rework — Install + Settings + Community + Discover                      | Re-skin non-editor routes; Community/Discover thin, Install wizard heavier                                                                        | pending     | with 9    | 2, 3    | -                                                                                                                                                              |
| 11  | Route rework — Profiles + Launch (editor routes)                              | Split `ProfileFormSections` (582 → <500), `LaunchPage` (591 → <500), `LaunchSubTabs` (508 → <500) first, then redesign                            | in-progress | -         | 2, 3    | [github-issues-423-450-profiles-launch-rework.plan.md](../plans/github-issues-423-450-profiles-launch-rework.plan.md)                                          |
| 12  | Responsive-sweep tests + smoke-spec expansion                                 | Playwright screenshots at 1280/1920/2560/3440; add `host-tools` and `proton-manager` to `ROUTE_ORDER`; palette smoke; mode-toggle smoke           | complete    | -         | 4–11    | [plan](../plans/completed/unified-desktop-phase-12-responsive-smoke-tests.plan.md)                                                                             |
| 13  | Polish + accessibility + docs                                                 | Focus-ring audit, reduced-motion passes, `docs/internal/design-tokens.md`, changelog-worthy release notes, Steam Deck manual-QA pass              | pending     | -         | 4–12    | [unified-desktop-phase-13-polish-a11y-docs.plan.md](../plans/unified-desktop-phase-13-polish-a11y-docs.plan.md)                                                |

### Phase Details

**Phase 1: Foundation — `useBreakpoint` + layout unlock**

- **Goal**: Shell can grow past 1440px, and the rest of the feature has a breakpoint source of truth.
- **Scope**: Add `src/hooks/useBreakpoint.ts` + test. Remove `--crosshook-content-width: 1440px` from `variables.css:87` and strip its two uses in `layout.css:2,50`. Extract `AppShell` from `App.tsx` into `src/components/layout/AppShell.tsx` (keeping each file <500 lines). `AppShell` renders the existing 2-column Group as-is (no inspector yet) — this phase is purely plumbing.
- **Success signal**: At 3440×1440 the existing app stretches to fill the viewport without letterboxing; all existing smoke tests still pass; `useBreakpoint()` returns `uw` at 3440 and `deck` at 1280.

**Phase 2: Token swap + legacy-palette sweep**

- **Goal**: The entire app renders in the toned-down steel-blue palette with zero legacy-color leakage.
- **Scope**: Update the ~8 accent/bg tokens in `variables.css`; add new sibling tokens for the design's sidebar/titlebar/surface-1/2/3/scrim/accent-glow concepts and desaturated status colors. Mechanical grep-replace sweep across every stylesheet under `src/styles/**` (and any `*.module.css`/CSS-in-TSX) for `#0078d4`, `#2da3ff`, `#1a1a2e`, `#20243d`, `#12172a`, `rgba(0,120,212,…)`, `rgba(45,163,255,…)` literals. Add `scripts/check-legacy-palette.sh` + wire it into `scripts/lint.sh`. Document the "no literal accent colors" rule in `docs/internal/design-tokens.md`.
- **Success signal**: `grep -rnE '(#0078d4|#2da3ff|#1a1a2e|#20243d|rgba\(\s*0\s*,\s*120\s*,\s*212)' src/ === 0 matches`. `scripts/lint.sh` passes. Visual screenshot diff shows new palette across all routes at 1920×1080.

**Phase 3: Sidebar variants + formalized sections**

- **Goal**: Sidebar auto-collapses to `mid` at `narrow` and `rail` at `deck`; Collections is a declared section not a runtime injection.
- **Scope**: Wire `Sidebar.tsx` to consume `useBreakpoint()` and set `data-collapsed`/variant classes. Merge `CollectionsSidebar` contents into `SIDEBAR_SECTIONS` as a proper section. Respect the existing `sidebar.css:204-244` collapsed-state rules.
- **Success signal**: At 1280×800, sidebar shows icon rail only (56px); at 1920×1080 full 240px; Collections items appear in the declared section order.

**Phase 4: Library redesign — cards + inspector rail**

- **Goal**: Library page has the new hover-gradient-reveal cards and a persistent right inspector (on non-deck).
- **Scope**: Rework `LibraryCard.tsx` chrome (badge · heart · hover-gradient · actions), `LibraryGrid.tsx` spacing, `LibraryToolbar.tsx` (add sort + filter + view chips + ⌘K trigger). New `src/components/layout/Inspector.tsx` with per-route content contract in `routeMetadata.ts`. Library declares its `inspectorComponent` (`GameInspector.tsx`) with hero image · pills · quick actions · active profile · recent launches · health. Extend `useScrollEnhance` SCROLLABLE. Update `LibraryCard.test.tsx` and `LibraryGrid.test.tsx` for the new DOM.
- **Success signal**: Hover a card → actions fade in; click card → inspector populates with selected game; keyboard nav works; library test suite green.

**Phase 5: Hero Detail mode**

- **Goal**: Clicking/Enter on a library card swaps the main slot to a hero-takeover detail view; sidebar and inspector remain mounted.
- **Scope**: New `src/components/library/GameDetail.tsx` (hero + tabs + panel grid) + supporting `HeroDetailHeader.tsx`/`HeroDetailTabs.tsx` if needed to stay under 500 lines. `LibraryPage` gains `mode` state. Tabs: `Overview · Profiles · Launch options · Trainer · History · Compatibility` (Compatibility opt-in). Launch command preview panel renders the existing pipeline-preview data. Deprecate `GameDetailsModal.tsx` (remove after callsites updated). Register `.crosshook-hero-detail__body` in `useScrollEnhance`.
- **Success signal**: Double-click a card → hero takeover renders with sidebar/inspector still in the DOM (assertion: `queryByTestId('sidebar')` and `queryByTestId('inspector')` still resolve after mode toggle). Back button returns to Library without re-fetching data.

**Phase 6: ⌘K command palette**

- **Goal**: `Cmd/Ctrl+K` from anywhere opens a focus-trapped palette over the current mode.
- **Scope**: New `src/components/palette/CommandPalette.tsx`, `src/hooks/useCommandPalette.ts`, `src/lib/commands.ts` (static command list). Register a global keyboard listener in `AppShell`. Palette list items have icon · label · optional kbd hint. Substring match only (no fuzzy). Register `.crosshook-palette__list` in `useScrollEnhance`. Smoke test: open/close, navigate with arrow keys, execute command.
- **Success signal**: `Cmd+K` at any viewport opens the overlay; Arrow↓/↑ moves selection; Enter executes; Esc closes. Palette honors focus-trap (focus returns to trigger on close).

**Phase 7: Context rail (ultrawide only)**

- **Goal**: Ultrawide users see a fourth pane with operational context.
- **Scope**: New `src/components/layout/ContextRail.tsx` mounted only when `useBreakpoint() === 'uw'` and Library mode. Renders host-readiness chip list (`useHostReadiness`), pinned profiles list, 7-day launch-activity mini-chart (reads launch-history IPC), and most-played list. Register `.crosshook-context-rail__body` in `useScrollEnhance`.
- **Success signal**: At 3440×1440 the rail appears with all four sections populated; at 2560×1440 it is absent.

**Phase 8: Console drawer → status bar swap**

- **Goal**: Deck users get a compact status bar; wider displays keep the existing drawer.
- **Scope**: `ConsoleDrawer.tsx` accepts a `mode: 'drawer' | 'status'` prop driven by `useBreakpoint`. Status mode renders a 32px bar with readiness chips + `⌘K commands` tip. Drawer mode stays available on wider displays, but opening it becomes an explicit user action instead of an auto-open-on-first-log behavior.
- **Success signal**: At 1280×800 the bottom of the shell is a single 32px bar, never expanding past it. At 1920×1080 the drawer is present, but only opens via explicit user action.

**Phase 9: Route rework — Dashboards**

- **Goal**: Health, Host Tools, Proton Manager, Compatibility visually cohere with the new shell.
- **Scope**: Rewrap content in `panel`/`kv-row`/`pill` idioms. Host Tools has the least delta; Proton Manager and Health are moderate. Keep all existing functionality.
- **Success signal**: Visual screenshot diff vs. the design reference panels. Smoke test green.

**Phase 10: Route rework — Install + Settings + Community + Discover**

- **Goal**: Install wizard, Settings panel, Community/Discover browsers align with the new visual language.
- **Scope**: Install wizard stays a wizard — just re-skin. Settings preserves sub-tab structure. Community/Discover redo their result cards in the new card idiom.
- **Success signal**: Visual parity; flows work end-to-end; smoke test green.

**Phase 11: Route rework — Profiles + Launch (editor routes)**

- **Goal**: The two densest routes are fully redesigned into the new shell language without regressing behavior.
- **Scope**: Split `ProfileFormSections` (582 → <500), `LaunchPage` (591 → <500), `LaunchSubTabs` (508 → <500) _first_. Then fully redesign the Profiles editor and Launch surfaces: restructure sections and navigation as needed, panel-wrap logical groups, use `kv-row`/`field-readonly` for read-only values, and convert command preview to the mono/panel style shown in the design's detail mode. Behavior parity remains mandatory even where layout or information architecture changes.
- **Success signal**: Each split file stays under the soft cap. Smoke test green. Profile edit and launch configuration flows both work end-to-end after the redesign.

**Phase 12: Responsive-sweep tests + smoke-spec expansion**

- **Goal**: Guard every breakpoint against future regression.
- **Scope**: Extend `tests/smoke.spec.ts` to iterate `1280×800, 1920×1080, 2560×1440, 3440×1440`. Add `host-tools` and `proton-manager` to `ROUTE_ORDER`. Add smoke cases for: ⌘K open/close, Library → Detail → Library, inspector-present-on-non-deck, context-rail-present-only-on-uw, status-bar-only-on-deck.
- **Success signal**: `npm run test:smoke` green with 4× route sweep.

**Phase 13: Polish + accessibility + docs**

- **Goal**: Ship-ready.
- **Scope**: Focus-ring audit (every interactive element visible at 2px ring). `prefers-reduced-motion` check on hover-reveal animations. `docs/internal/design-tokens.md` documents the new tokens and the "no literal accent" rule. Changelog-ready release notes. Manual Steam Deck pass (gamepad + touchscreen).
- **Success signal**: axe-core unit test passes on every page. Reduced-motion test passes. Deck manual pass finds no blocking issues.

### Parallelism Notes

- Phases **2 and 3** run in parallel — different files (variables/stylesheets vs. `Sidebar.tsx`), same depends-on Phase 1.
- Phases **5, 6, 7, 8** run in parallel — different surfaces (detail mode, palette, context rail, console). All depend on Phase 4 having shipped the inspector-and-shell contract.
- Phases **9 and 10** run in parallel — disjoint route sets.
- Phase **11** runs alone — the editor routes are the most fragile and benefit from focused attention.
- Phases **12 and 13** are sequential (tests first, polish last).

---

## Decisions Log

| Decision                     | Choice                                                              | Alternatives                                 | Rationale                                                                                               |
| ---------------------------- | ------------------------------------------------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Palette strategy             | Replace tokens in place                                             | Alternate "Classic" theme + switcher         | User decision; the "calm desktop" goal is the whole point. Doubles visual QA otherwise.                 |
| Scope                        | Full redesign — shell + library + hero detail + ⌘K + routes rework  | Shell-only / Shell+Library                   | User decision.                                                                                          |
| Routing model                | State-driven `Tabs.Root` + state toggle for Library↔Detail          | React Router / URL routes                    | Existing architecture; routes can all stay mounted via `forceMount`; adding URLs is a separate concern. |
| Panel library                | Keep `react-resizable-panels`                                       | Swap to custom / `allotment` / other         | Already integrated; extending from 2 to 3–4 Panels is trivial.                                          |
| Command-palette library      | Hand-rolled                                                         | `cmdk` npm package / `kbar`                  | Zero new deps; static command list is enough for v1; matches CLAUDE.md dependency-hygiene guidance.     |
| Inspector content contract   | Per-route, declared via `routeMetadata.ts` (opt-in, `null` allowed) | Inspector is Library-only                    | Usable on ultrawide for non-Library routes later without a new shell contract.                          |
| Gamepad-nav for palette      | Focus-trap modal (reuse `useFocusTrap`)                             | Add a third `overlay` focus zone             | Scope discipline — n-zone refactor is a follow-up PRD.                                                  |
| Sidebar sections             | Formalize `Collections` as declared section                         | Keep runtime injection                       | Code/design parity; simpler mental model.                                                               |
| Media tab on Hero Detail     | Omit for v1                                                         | Stub with "coming soon" / hide conditionally | No data source; matches the design's pragmatism; avoids dead UI.                                        |
| Console drawer default       | Explicit toggle only                                                | Auto-open on first log                       | Prevents surprise expansion and aligns the redesigned shell with intentional bottom-chrome behavior.    |
| Profiles editor rework depth | Full redesign of the editor                                         | Re-skin only; preserve sub-tab structure     | Chosen explicitly so Phase 11 can fix the densest surface rather than only repainting it.               |

---

## Research Summary

**Market Context** (skipped per user guidance — design-bundle chat explicitly states _"I'll design something original inspired by general desktop app patterns (not recreating Steam/Lutris/etc.'s specific branded UI)"_. The Unified Design bundle IS the distilled market-grounded recommendation from that session.)

**Technical Context** — grounded in `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/`:

- Shell today is a 2-panel `react-resizable-panels` Group (sidebar + vertical content/drawer stack) at `App.tsx:202-242`, capped at 1440px by `layout.css:2,50` + `variables.css:87`. Three-pane with inspector + optional context rail is greenfield composition, reusing the existing Panel library.
- Routes: 11 `*Page.tsx` files with widely varying complexity (`CommunityPage` 28 lines → `LaunchPage` 591). 6 files past the 500-line soft cap; phase 11 splits them first.
- Sidebar has full CSS for a collapsed/rail variant (`sidebar.css:204-244`) but no TSX driver — wiring is cheap.
- Palette migration risk: 68 `var(--crosshook-color-accent*)` sites clean, but ~61 `rgba(0, 120, 212, …)` literal fallbacks + ~7 `rgba(45, 163, 255, …)` + 2 hard-coded `#1a1a2e` (`theme.css:4547,4590`) need a sweep.
- `theme.css` is 6049 lines / 137KB — monolithic but variable-driven; no structural refactor needed for this PRD.
- Command palette is greenfield — no cmdk/kbar/CmdK pattern anywhere.
- No `useBreakpoint` / `useMediaQuery` hook — greenfield.
- `GameDetailsModal.tsx` (479 lines, `createPortal`) is today's blocking detail surface; the Hero Detail mode replaces it in-shell.
- Gamepad-nav zones limited to `'sidebar' | 'content'` (`gamepad-nav/types.ts:3`); palette reuses focus-trap for v1, n-zone refactor deferred.
- `useScrollEnhance` SCROLLABLE selector must grow by 5 classes (sidebar scroll, inspector, context rail, palette list, hero-detail body).
- Playwright smoke skips `host-tools` and `proton-manager` (`tests/smoke.spec.ts:33-45`) — expand in Phase 12.
- Persistence classification per CLAUDE.md done above (TOML for optional overrides only; no new SQLite tables; runtime-only for mode/selection/palette query).

---

_Generated: 2026-04-22_
_Status: DRAFT — needs validation_
