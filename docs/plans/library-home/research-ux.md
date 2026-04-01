# UX Research: Library-Home — Steam-Like Game Grid

**Confidence**: High (multiple authoritative sources, industry consensus, real implementations)  
**Date**: 2026-04-01  
**Topic velocity**: Moderate — game launcher UI patterns are relatively stable, CSS techniques evolve faster

---

## Executive Summary

CrossHook's library-home page should follow proven game launcher conventions: a poster-art grid as the primary view with a list-view alternative, instant client-side search, contextual filter controls, and a three-action card (Launch / Favorite / Edit). The most critical UX decisions are:

1. **Cards must display actions persistently** (not only on hover) because hover-only patterns are inaccessible, undiscoverable, and fail touch/gamepad users. The app already uses `crosshook-focus-scope` and `useGamepadNav` — button visibility must support this controller navigation model.
2. **Skeleton screens** are preferred over spinners for grid loads; they prevent layout shift and set spatial expectations. Navigation buttons (Launch/Edit) must be available even while cover art is still loading.
3. **Gradient scrims** for text-on-art require a minimum 4.5:1 WCAG AA contrast ratio — a bottom-anchored dark scrim from `rgba(0,0,0,0.85)` to transparent consistently achieves this. Cards display **title only** (no playtime — playtime data does not exist in the backend).
4. **The empty-library state** is a critical first-run onboarding moment; it must guide the user to create a profile (Profiles page wizard) or install a game (Install page), not just display a blank grid.
5. **Virtual scrolling** is a should-have for libraries > 80 profiles; CSS Grid with `auto-fill` + `minmax` handles responsive layout without media queries for moderate library sizes.

> **Data corrections (from business-analyzer + tech-designer)**:
>
> - No playtime or last-played data exists. Cards show title only.
> - Cover art is **local disk** (cached by Rust backend, served via Tauri asset URL using `convertFileSrc`). No external HTTP from the browser — blur-up LQIP is unnecessary; native `loading="lazy"` is sufficient.
> - `useGameCoverArt(steamAppId, customCoverArtPath)` returns `{ coverArtUrl: string|null, loading: boolean }` — `loading: true` drives the skeleton state directly.
> - `.crosshook-skeleton` CSS class already provides shimmer animation — reuse for card art placeholder.
> - `useImageDominantColor(imageUrl)` can extract a dominant color from cover art to generate a dynamic gradient fallback background.
> - Profile names are available immediately from `profiles[]`; art loads per-card asynchronously. Cards must render name + skeleton art immediately, not wait for art.
> - Existing aspect ratio CSS variable `--crosshook-profile-cover-art-aspect` is landscape (`460/215`). A new variable `--crosshook-library-card-aspect: 3 / 4` must be added — do not reuse the existing one.
> - Favorites are stored in SQLite via `favoriteProfiles: string[]`.

---

## User Workflows

### 1.1 Primary Flows

#### A. Browse & Launch (most common path)

```
Home (grid) → scan cover art → hover/focus card → click Launch button
                                                  → sets ProfileContext.selectedProfile
                                                  → navigates to LaunchPage
                                                  → user clicks "Launch Game" (step 2)
                                                  → user clicks "Launch Trainer" (step 3)
```

**Key requirements**:

- Cover art must be recognizable at 190 px width; title is a fallback identifier, not the primary
- Launch is the primary CTA — it must be the most visually prominent button
- The card Launch button only navigates; it does NOT directly launch the game
- **2-step launch clarity**: for `proton_run`/`steam_applaunch` profiles, consider a subtle "2-step" hint or badge on the card so users coming from Steam's single-click model understand the flow
- Navigating to Launch with the profile pre-activated via `ProfileContext` avoids a redundant selection step

#### B. Edit a Profile

```
Home (grid) → locate game → click Edit button on card
                          → sets ProfileContext.selectedProfile
                          → navigates to ProfilesPage editor
```

Edit should NOT be the primary action — it is a secondary maintenance action. Button should be visually de-emphasized (glass morphism / icon-only is appropriate here). No separate edit mode exists on the home page; it defers entirely to the existing ProfilesPage editor.

#### C. Toggle Favorite

```
Home (grid) → click Heart on card → optimistic UI update (heart fills immediately)
                                  → badge appears in card top-right corner
```

Favoriting must feel instant. The heart state should flip optimistically — do not wait for backend confirmation before updating the visual state.

#### D. Active Profile Indicator

When the user navigates away from home (e.g., to LaunchPage) and returns, `ProfileContext.selectedProfile` persists. The previously activated card should display a subtle visual indicator (e.g., brand-color border glow or accent highlight) so users have spatial context for which profile is "active".

#### E. Search & Filter

```
Header search bar → user types → results filter in-place within 200 ms (debounced)
Filter button     → filter panel opens → user selects criteria → grid re-renders
```

Search is **client-side only**, case-insensitive substring match on `profile.game.name`. No backend call is made. Applied filters should be visible as removable chips. The result count ("12 games") should update live.

#### F. Switch Views (Grid ↔ List)

```
Grid/List toggle in header → view transitions; all other state (filters, search) preserved
```

View preference should persist across sessions (localStorage or settings).

#### G. First-Run / Empty Library

```
Home (empty) → branded illustration + headline "No game profiles yet"
              → two CTAs: "Create a Profile" → ProfilesPage wizard
                          "Install a Game"   → Install page
```

Do not show a blank grid. The empty state is onboarding, not a dead end. The onboarding wizard is triggered by a Tauri event separately — this CTA is a direct navigation, not a wizard launch.

### 1.2 Alternative Flows

| Flow                      | Trigger                  | Resolution                                                          |
| ------------------------- | ------------------------ | ------------------------------------------------------------------- |
| Missing cover art         | Profile has no image URL | Fallback tile: dark gradient with game initials centered            |
| Failed profile load       | Backend error            | Card shows error icon + "Reload" action; does not block other cards |
| Search returns no results | Query matches nothing    | Inline "No games match '[query]'" with "Clear search" link          |
| Filtered to zero results  | Over-filtered            | "No games match these filters" + "Reset filters" CTA                |

### 1.3 Default Landing State

**Recommended**: Alphabetical sort by `profile.game.name` as the default, since no playtime or last-played data exists in the backend. If/when playtime tracking is added, recency-first would be the preferred default (matching Steam, GOG Galaxy, Heroic conventions).

**Confidence**: High for alphabetical as interim default. Recency-first is the industry standard but requires backend playtime tracking first.

---

## 2. UI/UX Best Practices

### 2.1 Card Design

#### Sizing & Proportions

- **190 px width, 3:4 aspect ratio** (253 px tall) — matches the Figma concept; also aligns with Steam's cover art portrait format (600×900, i.e., 2:3, very close to 3:4)
- Steam capsule art uses 600×900 portrait; displaying at 190×253 renders at ~0.32× scale — sufficient fidelity for standard cover art
- Cards below ~150 px wide lose legibility for text overlays; 190 px is within the safe range

**Confidence**: High — Steam Steamworks documentation, community standards, and Heroic Launcher source confirm 600×900 (2:3) as the dominant portrait format.

#### Text Overlay (Gradient Scrim)

- Apply a `linear-gradient(to top, rgba(0,0,0,0.85) 0%, rgba(0,0,0,0) 50%)` scrim anchored to the card bottom
- This reliably achieves 4.5:1+ contrast ratio for white text (#FFFFFF) against `rgba(0,0,0,0.85)` (effectively ~#252525 at mid-point)
- WCAG 2.1 SC 1.4.3 requires 4.5:1 for normal text, 3:1 for large text (18pt+ / 14pt+ bold)
- Show: game title (1 line, clamp with ellipsis) — **playtime is not available in the backend; omit it**

**Confidence**: High — Smashing Magazine accessibility guide (2023), WCAG 2.1 spec, and gradient accessibility analysis confirm these values.

#### Action Buttons Row

- **Launch** (filled, blue `#2563eb` or brand blue): left-aligned, widest button — primary CTA
- **Heart** (glass morphism icon button): right cluster
- **Edit** (glass morphism icon button): right cluster, beside heart
- Action row must be **always visible** (not hover-only). Hover/focus can intensify the scrim opacity to draw attention, but buttons must not disappear without hover.

**Why always-visible buttons?** Hover-only action patterns:

- Fail touch/gamepad users entirely
- Reduce discoverability — many users never discover hidden hover actions
- Create accessibility issues (SC 2.5.8 Target Size, 1.4.1 Use of Color)
- Are deprecated in modern UX guidance (NN/Group, CSS-Tricks "Hover is over")

**Confidence**: Medium-High — strong UX community consensus, NN/Group guidance; specific game launchers (Steam) still use hover-show patterns in some contexts, but this is increasingly viewed as a known accessibility debt.

#### Favorite Indicator Badge

- Small filled heart icon in top-right corner of card
- Badge should appear at all times when favorited (not just on hover)
- Use `aria-label="Favorited"` on the badge for screen reader context

### 2.2 Grid Layout

#### Responsive CSS Grid

```css
.crosshook-library-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: 16px;
}
```

This eliminates the need for breakpoint media queries. At 1280 px wide (typical desktop) with 16 px gap and sidebar (~240 px), the main content area is ~1024 px → fits ~5 cards per row. At 800 px → ~4 cards.

For fixed-width card behavior (no stretching beyond 200 px):

```css
grid-template-columns: repeat(auto-fill, 190px);
justify-content: start; /* or center */
```

**Confidence**: High — CSS-Tricks, LogRocket, and Mastery Games document this as the standard responsive grid pattern.

#### Breakpoint Guidance (as fallback or override)

| Container width | Cards per row |
| --------------- | ------------- |
| < 640 px        | 2–3           |
| 640–1024 px     | 3–4           |
| 1024–1440 px    | 4–6           |
| > 1440 px       | 6–8           |

### 2.3 List View

- Horizontal rows with: small poster thumbnail (48×64 px or 60×80 px), game title, favorite indicator, platform badge, and same action buttons (Launch / Heart / Edit) inline
- Column headers: Title, Actions — **playtime and last-played columns omitted (no backend data)**
- Sortable columns: title alphabetical only (for now); add playtime/recency columns when backend tracks them
- Preserve the same favorite badge as a row-level colored heart icon in the row

### 2.4 Header Controls

```
[ Search bar (flex-grow)        ] [ Filters ▼ ] [ Grid | List ]
```

- **Search**: instant, debounced at 200 ms, client-side string match on game title
- **Filters**: dropdown or slide-out panel; filter by: favorites only, platform tag, or custom tags
- Applied filters: show as removable chip pills below the search bar
- **Result count**: live counter, e.g., "Showing 12 of 47 games"
- **View toggle**: icon buttons (grid icon / list icon), filled icon = active state

### 2.5 Glass Morphism Buttons

The Figma concept specifies glass morphism for Heart and Edit buttons. Recommended CSS:

```css
.crosshook-card__btn--glass {
  background: rgba(255, 255, 255, 0.12);
  backdrop-filter: blur(8px);
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 6px;
}
```

`backdrop-filter` has 97%+ browser support as of December 2024. For Tauri v2's WebView (Chromium-based), support is guaranteed.

**Performance caveat**: `backdrop-filter: blur()` is GPU-intensive. Limit to small icon buttons, not large panels. Reduce blur radius (≤12 px) and avoid stacking blurred elements.

---

## 3. Error Handling UX

### 3.1 Error States Table

| State                     | Trigger                             | Visual Treatment                                                                            | Action Available            |
| ------------------------- | ----------------------------------- | ------------------------------------------------------------------------------------------- | --------------------------- |
| Missing cover art         | No image URL or image load failure  | Dark gradient tile with game initials (2 chars, centered, large font) in brand color accent | None required               |
| Profile load error        | Backend error on individual profile | Card shows warning icon overlay + muted opacity, tooltip: "Failed to load"                  | Retry button on hover/focus |
| Full library load failure | Cannot fetch profiles list          | Full-page error state: icon + "Could not load library" + Retry button                       | Retry                       |
| Search no results         | Query matches nothing               | Inline message within grid area: "No games match '[query]'"                                 | "Clear search" link         |
| Filter no results         | Over-filtered set is empty          | Same inline message + "Reset filters" CTA                                                   | Reset filters               |
| Empty library (first run) | Zero profiles exist                 | Illustrated empty state (see Section 1.1.F)                                                 | "Add your first game" CTA   |

### 3.2 Missing Cover Art Fallback

Recommended implementation for the fallback tile:

- Background: `linear-gradient(135deg, #20243d 0%, #1a1a2e 100%)` (app brand dark colors)
- Center: game initials in 2–3 characters, `font-size: 2rem`, `color: #3b82f6` (brand accent blue)
- This avoids placeholder images (network requests) and maintains visual consistency

Lutris uses "dynamically generated gradients as fallbacks for missing art" — this pattern is validated by a major Linux game launcher.

### 3.3 Error Message Security Note

Do not expose internal error messages (backend stack traces, file paths, internal IDs) in card tooltips or error UI. Use user-friendly strings. This applies to: failed image URLs, failed IPC calls, failed profile reads.

---

## Performance UX

### 4.1 Skeleton Loading

Use skeleton screens (not spinners) as the loading state for the grid:

- Reuse the existing `.crosshook-skeleton` CSS class (shimmer animation already implemented)
- Render N placeholder card-shaped rectangles at `190px × (190 * 4/3)px` = `190×253 px`
- Profile names are available immediately from `profiles[]` — render the name in the skeleton card right away; only the art area needs the shimmer
- `useGameCoverArt` returns `{ loading: boolean }` — show shimmer while `loading === true`, swap to real art or fallback tile when `loading === false`
- N = cards visible in current viewport; a safe default is 12–20 for the initial render before the profile list arrives

**Why skeletons over spinners?**

- Reduce perceived loading time significantly
- Prevent layout shift — grid is already the correct size when images load
- Industry-standard: Steam (initial library load), Netflix, Spotify

**Confidence**: High — LogRocket UX article, Mobbin skeleton UI guidelines, IBM Carbon Design System all confirm skeleton screens as best practice for grid/card layouts.

### 4.2 Image Loading Strategy

**Local disk — no blur-up needed**:
Cover art is cached on disk by the Rust backend and served as a Tauri asset URL via `convertFileSrc`. There is no external HTTP fetch from the browser. Disk reads are fast enough that LQIP/blur-up adds complexity without meaningful perceived-performance benefit.

Recommended approach:

1. Card art area shows `.crosshook-skeleton` shimmer while `useGameCoverArt` `loading === true`
2. When `loading === false` and `coverArtUrl` is a string: swap in `<img src={coverArtUrl} loading="lazy">` with a `opacity: 0 → 1` fade (150–200 ms)
3. When `loading === false` and `coverArtUrl` is null: render the gradient fallback tile using `useImageDominantColor` (or static brand gradient if dominant color is unavailable)

**`useImageDominantColor` for fallback gradient**:
When cover art is absent, use the dominant color extracted from any available art (or default brand colors `#1a1a2e` / `#20243d`) to generate a per-card gradient background. This is more visually coherent than a uniform static fallback and is already supported by the existing hook.

**Native lazy loading** (`loading="lazy"`) for off-screen cards:

- Supported natively in Tauri v2's Chromium WebView
- Defers image decode for cards below the fold — reduces initial render cost for large libraries

### 4.3 Virtual Scrolling

For libraries with > 100 game profiles, CSS Grid alone will cause performance degradation:

- 100+ cards in DOM = significant render time + scroll jank
- Recommendation: implement virtual scrolling when card count exceeds ~80

**TanStack Virtual** (2024–2025 recommended library):

- Framework-agnostic, excellent TypeScript support
- Supports 2D grid virtualization
- ~10–15 KB gzipped
- 60 FPS performance guarantee
- Tauri v2 (React frontend) is fully compatible

For initial implementation with < 100 profiles, CSS Grid without virtualization is acceptable. Add virtual scrolling as a follow-up when scale demands it.

**Confidence**: High — web.dev, bvaughn/react-window docs, TanStack Virtual docs confirm this threshold and approach.

### 4.4 Optimistic UI for Favorites

- Toggle heart → update local state immediately
- Fire IPC command in background
- If IPC fails: revert heart state + show subtle toast ("Failed to update favorite")
- Do NOT show a loading spinner during the toggle — favorites should feel instant

---

## 5. Competitive Analysis

### 5.1 Steam Library

**What works**:

- Poster-art grid as default view is universally recognized
- Quick-access shelves ("Recently Played", "Updates") on the home page surface timely content without hunting
- Custom cover art upload via drag-and-drop (community ecosystem around SteamGridDB)
- Platform filter (macOS/Linux) is contextually useful for multi-platform libraries
- Right-click context menus for secondary actions

**What doesn't work**:

- Grid view with very large libraries becomes sluggish without windowing
- Cover art for older games is inconsistent in quality (too small, wrong aspect ratio)
- Hover-only play button on small grid tiles is a known discoverability pain point — users submitted requests to make it permanent
- Users reported "new library layout is really bad" for large game lists in 2019 beta — primarily due to performance and hidden navigation

**Confidence**: High — Steam community discussions, Steam library update page, user feedback threads.

### 5.2 GOG Galaxy

**What works**:

- Unified library across multiple storefronts is a model CrossHook can reference for multi-source profiles
- Clean, large cover art grid — prioritizes visual browsing
- Recent activity shelf prominent on home page

**What doesn't work**:

- Galaxy 2.0 integration plugins are community-maintained and can break; this is a maintenance model concern, not a UI concern
- Can feel slow on large libraries compared to native alternatives

**Confidence**: Medium — limited direct source data; based on community comparisons and general Linux gaming community feedback.

### 5.3 Heroic Games Launcher

**What works**:

- Scale animation on GameCard hover (`transform: scale(1.05)`) creates tactile responsiveness without being distracting
- Fast library switching (< 0.5 s) between Store and Library
- First game in "Recently Played" uses a wide landscape art (2 slots wide) — visual hierarchy within the grid
- Clean, low-friction install/play flow
- Sidebar can collapse — maximizes grid space

**What doesn't work**:

- Relies on Epic/GOG/Amazon APIs — sideloaded games have worse cover art integration (open GitHub issue #4821 for "Sideloaded games cover art improvements")
- Some users find the interface slow to start when Epic Games authentication is required

**Confidence**: Medium-High — based on Heroic GitHub releases, community reviews, and direct inspection.

### 5.4 Lutris

**What works**:

- Left panel "Sources" for platform/source filtering — very effective organization for mixed libraries
- Grid of banners ↔ detailed list toggle — both views are first-class
- Dynamically generated gradient fallback tiles for missing art — strong pattern CrossHook should adopt
- Game cards show cover art (Lutris tracks playtime/last played — CrossHook does not yet)
- "Recently Played" shelf included

**What doesn't work**:

- UI can feel dated / less polished compared to Heroic
- Configuration complexity is a barrier for new users (strength for power users, weakness for casual users)
- Search/filter UX is functional but not refined

**Confidence**: Medium — based on Lutris FAQ, LinuxForDevices review, and community comparisons.

### 5.5 Playnite

**What works**:

- Three view modes: Details View, Grid View, List View — most flexible of all launchers
- Grid View with community themes (JG00SE/GridViewCards, Minimal, etc.) — confirms community desire for visual customization
- Details panel alongside grid — allows browsing and reading metadata simultaneously
- Can display game names below cover in grid (configurable)

**What doesn't work**:

- Windows-first; Linux support exists but is secondary
- Theme engine is powerful but complex; default theme is not as polished as Heroic
- Grid details panel can cause layout confusion (positioning bugs cited in GitHub issues)

**Confidence**: Medium — based on Playnite GitHub issues, documentation, and community themes.

### 5.6 Competitive Summary

| Feature                    | Steam   | GOG       | Heroic  | Lutris    | Playnite |
| -------------------------- | ------- | --------- | ------- | --------- | -------- |
| Poster grid (primary)      | Yes     | Yes       | Yes     | Yes       | Yes      |
| List view                  | Yes     | Yes       | Yes     | Yes       | Yes      |
| Details panel              | Yes     | Yes       | No      | No        | Yes      |
| Hover scale animation      | No      | Subtle    | Yes     | No        | No       |
| Missing art fallback       | Minimal | Generated | Limited | Generated | Custom   |
| Persistent action buttons  | Partial | Yes       | Partial | Yes       | Yes      |
| Virtual scroll             | Yes     | Yes       | Partial | Partial   | Yes      |
| Recently played shelf      | Yes     | Yes       | Yes     | Yes       | No       |
| Client-side instant search | Yes     | Yes       | Yes     | Yes       | Yes      |

---

## 6. Recommendations

### Must Have (MVP)

1. **Poster grid with `auto-fill minmax(190px, 1fr)`** — no breakpoint media queries needed
2. **Always-visible action row** (Launch / Heart / Edit) on every card — not hover-only
3. **Bottom gradient scrim** `rgba(0,0,0,0.85) → transparent` covering the bottom 40–50% of card; title + playtime text overlay
4. **Gradient fallback tile** for missing cover art — dark brand colors + game initials (2 chars)
5. **Skeleton loading** for initial grid render — 12–20 placeholder cards with shimmer
6. **Empty-library onboarding state** — illustrated zero-state with single "Add your first game" CTA
7. **Instant search** (200 ms debounce, client-side, by game title)
8. **Favorite badge** (top-right corner, persistent when favorited)
9. **List view toggle** — rows with thumbnail, title, playtime, last played, action buttons
10. **Optimistic favorite toggle** — instant visual feedback; revert on IPC error

### Should Have (Post-MVP V1)

11. **Active profile indicator** — subtle border/glow on the card whose profile is currently set in `ProfileContext.selectedProfile`
12. **Filter panel** — filter by: favorites only; applied filter chips with individual remove buttons; result count indicator
13. **Sort options** — alphabetical (default for now); add recency/playtime when backend tracks them
14. **2-step launch hint** — subtle badge or tooltip on cards using `proton_run`/`steam_applaunch` to set expectations
15. **Dominant-color gradient fallback** — `useImageDominantColor` to generate per-card gradient background when art is absent, instead of a static brand-color tile
16. **Hover scale animation** on cards — subtle `scale(1.02–1.05)` with 150 ms ease
17. **ARIA roles** on cards — `role="article"`, `aria-label="[game title]"`, `aria-pressed` on heart button

### Nice to Have (V2+)

18. **Virtual scrolling** (TanStack Virtual) for libraries > 80 profiles
19. **Configurable card size** — small/medium/large grid density slider (Playnite model)
20. **Context menu on right-click** — quick access to Launch / Edit / Remove / View Details
21. **Keyboard navigation** — arrow keys between cards, Enter to launch, Space to toggle favorite
22. **Drag-to-reorder** within collections
23. **Recently launched shelf** collapse/expand toggle

---

## 7. Open Questions

| Question                                                         | Status                                                                                                 | Why it matters                                            | Owner     |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | --------------------------------------------------------- | --------- |
| Does CrossHook track playtime natively?                          | **Resolved: No** — omit from cards                                                                     | Cards show title only                                     | —         |
| Should search match title only?                                  | **Resolved: Yes** — client-side substring on `profile.game.name`                                       | Confirmed scope                                           | —         |
| Is cover art stored locally or fetched remotely?                 | **Resolved: Local disk** — Tauri asset URL via `convertFileSrc`; blur-up LQIP not needed               | No external HTTP from browser                             | —         |
| What is the IPC contract for listing profiles?                   | **Resolved: `profile_list` returns names array immediately; per-card art loads via `useGameCoverArt`** | Skeleton renders name-first, art fills in async           | —         |
| What is the expected library size (p50, p99)?                    | Open                                                                                                   | Determines if virtual scrolling is MVP or V2              | team-lead |
| Is keyboard/gamepad navigation a V1 requirement?                 | Open — `useGamepadNav` exists; V1 scope unclear                                                        | Affects focus management and button sizing                | team-lead |
| Favorites: sorted to top of grid, or mixed in?                   | Open                                                                                                   | Drives default sort behavior                              | team-lead |
| Empty state: one CTA or two ("Create Profile" + "Install Game")? | Open                                                                                                   | Two CTAs may cause decision paralysis for first-run users | team-lead |

---

## Sources

- [Steam Graphical Assets — Steamworks Documentation](https://partner.steamgames.com/doc/store/assets)
- [The New Steam Library — Steam Store](https://store.steampowered.com/libraryupdate)
- [Heroic Games Launcher — GitHub](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher)
- [Heroic v2.4.0 Beta — New Design + Unified Library](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/releases/tag/v2.4.0-beta)
- [Playnite — video game library manager](https://playnite.link/)
- [Lutris — Open Gaming Platform](https://lutris.net/)
- [8 Best Practices for UI Card Design — UX Collective](https://uxdesign.cc/8-best-practices-for-ui-card-design-898f45bb60cc)
- [UX Pattern Analysis: Loading Feedback — Pencil & Paper](https://www.pencilandpaper.io/articles/ux-pattern-analysis-loading-feedback)
- [Skeleton Loading Screen Design — LogRocket](https://blog.logrocket.com/ux-design/skeleton-loading-screen-design/)
- [Designing Accessible Text Over Images (Part 1) — Smashing Magazine](https://www.smashingmagazine.com/2023/08/designing-accessible-text-over-images-part1/)
- [Designing Accessible Text Over Images (Part 2) — Smashing Magazine](https://www.smashingmagazine.com/2023/08/designing-accessible-text-over-images-part2/)
- [Gradients: Accessible Colour Contrasts — ACHECKS](https://www.achecks.org/gradients-accessible-colour-contrasts-with-gradient-backgrounds/)
- [Empty State UX Examples — Eleken](https://www.eleken.co/blog-posts/empty-state-ux)
- [Designing Empty States in Complex Applications — NN/Group](https://www.nngroup.com/articles/empty-state-interface-design/)
- [Getting Filters Right: UX/UI Design Patterns — LogRocket](https://blog.logrocket.com/ux-design/filtering-ux-ui-design-patterns-best-practices/)
- [Auto-Sizing Columns in CSS Grid — CSS-Tricks](https://css-tricks.com/auto-sizing-columns-css-grid-auto-fill-vs-auto-fit/)
- [Virtualize Large Lists with react-window — web.dev](https://web.dev/articles/virtualize-long-lists-react-window)
- [Blur-Up Technique for Loading Background Images — CSS-Tricks](https://css-tricks.com/the-blur-up-technique-for-loading-background-images/)
- [Next-level Frosted Glass with backdrop-filter — Josh W. Comeau](https://www.joshwcomeau.com/css/backdrop-filter/)
- [Dark Glassmorphism Trends 2026 — Medium](https://medium.com/@developer_89726/dark-glassmorphism-the-aesthetic-that-will-define-ui-in-2026-93aa4153088f)
- [Animation Duration — NN/Group](https://www.nngroup.com/articles/animation-duration/)
- [Hover is Over — SSG's](https://ssg.dev/hover-is-over-5ca728a01cde/)
- [Ensure High Contrast for Text Over Images — NN/Group](https://www.nngroup.com/articles/text-over-images/)
- [TanStack Virtual](https://tanstack.com/virtual/latest)
- [react-window — GitHub](https://github.com/bvaughn/react-window)
