# Spec: Breadcrumb navigation (Library → game → edit/launch)

- **Date**: 2026-06-03
- **Status**: approved (brainstormed + design approved in session)
- **Related**: `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md` (Phases 8–10 delete the Profiles/Launch routes this spec interim-wires; Phase 9 can reuse the intent mechanism introduced here)

## Problem Statement

Clicking **Edit profile** or **Launch** in a game's Hero Detail deliberately exits the detail view and navigates to the standalone Profiles / Launch routes (`game-details-actions.ts:6-22` → `LibraryPage.tsx` `handleEdit`/`handleLaunch`). Those pages have no back affordance — users must re-click **Library** in the sidebar and re-open the game to get back. There is no breadcrumb or contextual trail anywhere in the app; the only back affordance is the Hero Detail "Back" button.

## Resolved Decisions

1. **Scope**: durable breadcrumb system — a reusable component designed for the post-consolidation world (`Library › {game} › {tab}`), wired to today's Profiles/Launch pages as a cheap, explicitly-marked interim so the pain is fixed now without throwaway architecture.
2. **Placement**: the `RouteBanner` eyebrow slot. The interactive trail replaces the static eyebrow text where a trail exists; pages without a trail are unchanged. Hero Detail's hardcoded `Library` eyebrow (`HeroDetailHeader.tsx`) becomes a trail.
3. **Back button**: Hero Detail keeps its existing Back button alongside the breadcrumb (redundant but harmless; fast muscle-memory exit).
4. **Trail semantics**: breadcrumbs reflect **hierarchy, not history**. Direct sidebar visits to Profiles/Launch show no game crumb. No library-filter segments (e.g. "Favorites") in v1 — trail is at most `Library › {game} › {page}`.
5. **Architecture**: derived trail + one runtime-only origin field (Approach A). Rejected: navigation-history stack (breadcrumbs-as-history anti-pattern, more state to keep correct); URL-router migration (massive scope, collides with the in-flight consolidation PRD).

## Requirements

- R1: On the Profiles and Launch pages, when reached via a game's Edit profile / Launch buttons, the banner shows a clickable trail `Library › {game display name} › Edit profile` (or `› Launch`).
- R2: Clicking the **game** crumb returns to that game's Hero Detail view (reopened in Library). Clicking **Library** returns to the library grid.
- R3: When Profiles/Launch are visited directly from the sidebar, the banner shows the existing static eyebrow (no stale game crumb).
- R4: Hero Detail's header eyebrow becomes a trail: `Library` (clickable, fires existing `onBack`) `› {game}` (current page, non-clickable).
- R5: The breadcrumb is accessible: `<nav aria-label="Breadcrumb">`, ordered list, `aria-current="page"` on the terminal segment, separators `aria-hidden`.
- R6: If the origin game's profile no longer exists when the game crumb is clicked, the navigation degrades silently to the library grid.
- R7: All other route pages are visually and behaviorally unchanged.

## Technical Approach

### New component — `components/layout/Breadcrumb.tsx`

Pure presentational; no state or data fetching.

```tsx
export interface BreadcrumbSegment {
  label: string;
  /** Absent = current page: rendered as plain text with aria-current="page". */
  onNavigate?: () => void;
}

export function Breadcrumb({ segments }: { segments: BreadcrumbSegment[] }): JSX.Element;
```

- Markup: `<nav aria-label="Breadcrumb"><ol><li>…</li></ol></nav>`; clickable segments are ghost buttons; `›` separators are `aria-hidden` elements between items.
- Styling: BEM `crosshook-breadcrumb__*` classes, tokens from `styles/variables.css`, sized to match `crosshook-heading-eyebrow` so it sits in the eyebrow slot without layout shift.

### `RouteBanner` extension

`RouteBannerProps` gains optional `trail?: BreadcrumbSegment[]`. When provided, render `<Breadcrumb segments={trail} />` in place of the static eyebrow `<p>`; otherwise unchanged. All existing call sites compile untouched.

### `HeroDetailHeader` trail (durable)

Replace the hardcoded `<p className="crosshook-hero-detail__eyebrow">Library</p>` with a `Breadcrumb`: `Library` → `onBack`; `{displayName}` current. Back button retained.

### Origin tracking (interim — deleted by consolidation Phase 10)

Mirrors the existing `libraryFilter` / `LibraryFilterIntent` patterns:

- `types/navigation.ts`: `AppNavigateOptions` gains
  `gameDetailOrigin?: { profileName: string; displayName: string }`.
- `LibraryPage.handleEdit` / `handleLaunch`: pass `gameDetailOrigin` in their `onNavigate` calls (display name from the card summary).
- `AppShell.handleNavigate`: store in `useState<GameDetailOrigin | null>`; **set when the option is provided, cleared on any navigation without it** (guarantees R3).
- `AppShell` threads the origin (plus a navigate callback) to `ProfilesPage` / `LaunchPage`, which build the `trail` prop for their `RouteBanner`.
- All interim code is annotated `// NOTE(hero-detail-consolidation): delete with Phase 10 route removal.`

### Reopen-detail intent (durable — Phase 9 reuses it)

Clicking the game crumb must reopen `LibraryPage`-local detail state. Copy the proven `LibraryFilterIntent` token pattern:

- `types/navigation.ts`: `OpenGameDetailIntent { profileName: string; token: number }`, and `AppNavigateOptions` gains `openGameDetail?: string` (profile name).
- The game crumb's `onNavigate` is simply `onNavigate('library', { openGameDetail: origin.profileName })`; `AppShell.handleNavigate` converts the option into the token intent exactly as it does `libraryFilter` → `LibraryFilterIntent`.
- `LibraryPage`: a `useEffect` consumes the intent once summaries are loaded and calls the existing `handleOpenGameDetail(profileName)`; if no summary matches, the intent is dropped (R6).

### Data flow

```
HeroDetail Edit/Launch click
  → LibraryPage.handleEdit/handleLaunch (adds gameDetailOrigin)
  → AppShell.handleNavigate (stores origin, sets route)
  → ProfilesPage/LaunchPage (origin → trail → RouteBanner → Breadcrumb)

game-crumb click
  → AppShell (openGameDetailIntent + navigate('library'))
  → LibraryPage effect (summaries ready → handleOpenGameDetail)
  → Hero Detail reopened
```

## Storage Boundary

| Datum                                   | Classification            | Notes                                       |
| --------------------------------------- | ------------------------- | ------------------------------------------- |
| `gameDetailOrigin` (AppShell state)     | **Runtime-only (memory)** | Set/cleared per navigation; never persisted |
| `openGameDetailIntent` (AppShell state) | **Runtime-only (memory)** | Token-consumed by LibraryPage               |
| TOML settings                           | **None**                  | —                                           |
| SQLite metadata                         | **None**                  | Schema stays v23                            |

## Persistence & Usability

- **Migration / backward compatibility**: none required — no persisted data changes.
- **Offline expectations**: fully offline; pure UI state.
- **Degraded fallback**: origin lost on app restart or non-origin navigation → banner falls back to the static eyebrow (R3); missing profile on crumb click → library grid (R6). A trail never blocks rendering.
- **User visibility / editability**: breadcrumb is read-only navigation UI; nothing user-editable, nothing persisted to view elsewhere.

## Integration Points

- `components/layout/RouteBanner.tsx` — `trail` prop
- `components/layout/AppShell.tsx` — origin state, intent state, crumb navigation callbacks
- `components/library/HeroDetailHeader.tsx` — eyebrow → trail
- `components/pages/LibraryPage.tsx` — origin on edit/launch, intent consumption
- `components/pages/ProfilesPage.tsx`, `components/pages/LaunchPage.tsx` — trail wiring (interim)
- `types/navigation.ts` — `gameDetailOrigin`, `OpenGameDetailIntent`
- `hooks/useScrollEnhance.ts` — **not** affected (no new scroll container)

## Lifecycle vs. Hero Detail consolidation PRD

| Piece                                                          | Fate                                                                                                                                           |
| -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `Breadcrumb` component, `RouteBanner.trail`, Hero Detail trail | **Carried forward** — exactly what the post-consolidation `Library › {game} › {tab}` world needs                                               |
| `openGameDetailIntent`                                         | **Carried forward** — Phase 9's nav rewire (~12 callers opening Hero Detail from other routes) needs this mechanism                            |
| `gameDetailOrigin` option/state, Profiles/Launch page trails   | **Deleted with Phase 10** (~15–20 lines, all in files Phase 10 already touches; `NOTE(hero-detail-consolidation)` markers make them greppable) |

## Testing Strategy

- **RTL (Vitest, happy-dom)**:
  - `Breadcrumb`: renders segments in order, terminal segment has `aria-current="page"` and is not a button, separators `aria-hidden`, click fires `onNavigate`.
  - `RouteBanner`: with `trail` renders breadcrumb (no static eyebrow); without, unchanged snapshot of current behavior.
  - `HeroDetailHeader`: Library crumb fires `onBack`; game name is the current segment; Back button still present.
  - `LibraryPage`: edit/launch pass `gameDetailOrigin`; `openGameDetailIntent` consumption opens detail once summaries load; unknown profile drops intent.
  - `AppShell`: origin set on origin-navigations, cleared on plain navigations.
- **Playwright smoke**: one flow — open game → Edit profile → assert trail `Library › {game} › Edit profile` visible → click game crumb → assert Hero Detail reopened. Added to the existing profiles/launch smoke block (Phase 11 rewrites that block; the durable Hero Detail assertions survive).

## Risks

| Risk                                         | Mitigation                                                                  |
| -------------------------------------------- | --------------------------------------------------------------------------- |
| Interim wiring outlives the routes it serves | `NOTE(hero-detail-consolidation)` markers; Phase 10 touches the same files  |
| Stale game crumb after unrelated navigation  | Origin cleared on every navigation that doesn't set it (R3)                 |
| Crumb click for deleted profile              | Intent dropped when no summary matches; lands on grid (R6)                  |
| Eyebrow slot layout shift                    | Breadcrumb sized to `crosshook-heading-eyebrow` metrics; RTL snapshot guard |

## NOT Building

- No library-filter crumbs (`Favorites`, `Currently Playing`) in the trail.
- No navigation-history stack or back/forward buttons.
- No URL-based routing.
- No breadcrumbs on routes without hierarchy (Install, Settings, etc.) — they keep static eyebrows until a real hierarchy exists.
- No persisted navigation state.
