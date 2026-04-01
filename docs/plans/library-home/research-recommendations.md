# Library Home — Research Recommendations

## Executive Summary

CrossHook already has all the infrastructure needed to build a Steam-like poster grid home page: `useGameCoverArt` fetches cover art via Tauri IPC with caching, `useImageDominantColor` extracts palette colors, `useProfile` owns `profiles`/`favoriteProfiles`/`toggleFavorite`, and `ProfileContext` is available app-wide. The primary work is a new `home` route, a `LibraryCard` component, and wiring navigation actions to the existing `onNavigate` prop pattern. The app currently defaults to `'profiles'` — adding `'home'` as the default route is a one-line change in `App.tsx`.

**Primary architectural tension** (confirmed from backend analysis): `profiles: string[]` contains only names. Cover art needs `steam.app_id` and `game.custom_cover_art_path` from the full `GameProfile`. There is no lightweight cover art metadata IPC command — only `profile_load` (full profile) exists. Resolving this tension is the first design decision before implementation starts.

---

## Implementation Recommendations

### Technical Approach

The recommended architecture follows the existing page pattern exactly:

1. **New `AppRoute` value**: Add `'home'` to the `AppRoute` union in `Sidebar.tsx:13`.
2. **New `HomePage` page component** at `src/crosshook-native/src/components/pages/HomePage.tsx` — mirrors `ProfilesPage` structure; consumes `useProfileContext` directly.
3. **`LibraryCard` component** at `src/crosshook-native/src/components/ui/LibraryCard.tsx` — self-contained; accepts `profileName`, `steamAppId`, `customCoverArtPath`, `isFavorite`, and callback props. Internally calls `useGameCoverArt` per card.
4. **Sidebar entry** — add a "Home" item as the first entry in `SIDEBAR_SECTIONS` with an appropriate icon.
5. **`ContentArea` switch** — add `case 'home': return <HomePage onNavigate={onNavigate} />;`.

### Resolving the Cover Art Metadata Tension

The `profiles: string[]` list contains names only. Cover art requires `steam.app_id` and `custom_cover_art_path` from each full `GameProfile`. Three options:

**Option A (recommended for Phase 1)**: Lazy-load full profiles on demand. `LibraryCard` receives only `profileName` and calls `invoke<GameProfile>('profile_load', { name })` once on first render (or on intersection), caching the result in a `Map<string, GameProfile>` held in `HomePage` state. This keeps the card component simple and avoids a new IPC command.

**Option B (best long-term)**: Add a new Rust IPC command `profile_list_cover_art_metadata` that returns `Vec<{name, steam_app_id, custom_cover_art_path}>` by reading only those fields from each TOML file. This is a single lightweight batch call instead of N individual `profile_load` calls. Requires a new `crosshook-core` function and Tauri command — medium backend effort, maximum frontend efficiency.

**Option C (simplest short-term)**: Accept that cover art only displays for the currently-selected profile (already loaded in `ProfileContext`). All other cards show the fallback placeholder. Degrade gracefully without any new IPC. Appropriate if Phase 1 is meant as a purely structural PR.

**Recommendation**: Start with Option A for Phase 1 (no new backend work, acceptable performance). Plan Option B as a follow-up optimization once the UI is validated.

### Technology Choices

- **CSS Grid with `repeat(auto-fill, minmax(190px, 1fr))`** — matches the `--crosshook-community-profile-grid-min` pattern already used in `variables.css:51` / `theme.css:997`. No new primitives needed.
- **Radix UI** — no new library needed; existing `@radix-ui/react-tabs` covers tab navigation. For the context menu, `@radix-ui/react-context-menu` would be the correct choice if added (one package, consistent API).
- **Skeleton shimmer** — `crosshook-skeleton` / `crosshook-skeleton-shimmer` keyframe already exists in `theme.css:4738`. Reuse it for card placeholders.
- **No virtual scrolling initially** — the typical CrossHook user manages tens to low hundreds of profiles. Standard CSS grid with `overflow-y: auto` on the scroll container suffices. Add virtualisation (e.g. `@tanstack/react-virtual`) only if performance data shows it's needed.
- **`useImageDominantColor`** — already exists. Can be used for a subtle card border/glow tint on hover (see LaunchSubTabs usage pattern).

### Phasing

**Phase 1 — Skeleton grid (quick win, standalone PR)**

- Add `'home'` route to `AppRoute`, `Sidebar`, `ContentArea`.
- `LibraryCard`: cover art image (3:4, 190px wide), profile name label, three action buttons (Launch, Edit, Heart).
- `HomePage`: responsive `auto-fill` grid, search bar (client-side filter over `profiles` string array), grid/list toggle (boolean state, localStorage-persisted).
- Default route: change `useState<AppRoute>('profiles')` → `'home'` in `App.tsx:43`.
- Cover art: Option A — lazy per-card `profile_load` on intersection, results cached in `HomePage` local state.
- Empty state: when `profiles.length === 0`, show a CTA that links to `'profiles'` or triggers `OnboardingWizard`.

**Phase 2 — Polish & UX (follow-up PR)**

- Skeleton loading states per card while cover art fetches.
- Dominant-color glow on card hover.
- Card size slider (CSS variable `--crosshook-library-card-width`, range input persisted to localStorage).
- Backend Option B: `profile_list_cover_art_metadata` batch IPC command.

**Phase 3 — Power features (deferred)**

- Right-click context menu (`@radix-ui/react-context-menu`).
- Keyboard shortcuts.
- Recently-played section (requires new `last_launched_at` field in SQLite metadata DB).

### Quick Wins

- The search filter is pure client-side — filter `profiles: string[]` by substring match. Zero new IPC.
- Grid/list toggle is a single boolean in `useState` with `localStorage` persistence. Trivially fast.
- The Heart action already exists: `useProfileContext().toggleFavorite(name, !isFavorite)`.
- Launch action: `await selectProfile(name)` then `onNavigate('launch')`. The `await` is mandatory — see navigation risk below.
- Edit action: `await selectProfile(name)` then `onNavigate('profiles')`.

### CSS Approach

Follow the existing BEM-like `crosshook-*` class convention throughout:

```css
/* New CSS variables to add to variables.css */
--crosshook-library-card-width: 190px;
--crosshook-library-card-aspect: 3 / 4;
--crosshook-library-grid-gap: var(--crosshook-grid-gap);
--crosshook-library-grid-min: var(--crosshook-library-card-width);
```

CSS classes follow the pattern: `.crosshook-library-grid`, `.crosshook-library-card`, `.crosshook-library-card__art`, `.crosshook-library-card__label`, `.crosshook-library-card__actions`, `.crosshook-library-card--favorite`.

---

## Improvement Ideas

### Related Features & Enhancements

- **Card size slider**: A `<input type="range">` that adjusts `--crosshook-library-card-width` as a CSS custom property on the grid container. Identical to Steam's grid size slider. Persist to `localStorage`. Cost: trivial.

- **Recently played section**: A horizontal strip at the top (like `PinnedProfilesStrip`) showing the last 3–5 launched profiles. Requires a `last_launched_at` timestamp in SQLite metadata DB (new column, single migration). On the frontend, a new IPC `profile_list_recently_launched` command returns `[{name, launched_at}]`. Medium effort, high UX value.

- **Right-click context menu**: `@radix-ui/react-context-menu` (consistent with Radix UI already used). Menu items: Launch, Edit, Duplicate, Rename, Delete, Toggle Favorite. Avoids always-visible action buttons that clutter the poster art. Consider replacing the three overlay buttons with a single "primary action" on hover and a context menu for the rest.

- **Empty state / onboarding**: When `profiles.length === 0`, render a centered hero card with a "Create your first profile" CTA that navigates to `'profiles'`. The `OnboardingWizard` already handles the full wizard flow — just link into it from the empty state. Zero backend work.

- **Keyboard shortcuts for power users**: `Enter` on a focused card → Launch; `E` → Edit; `F` → Toggle Favorite; `/` → focus the search bar. Use `onKeyDown` on the card element with `tabIndex={0}`. No library needed.

- **Sort options**: Sort by name (alphabetical, default), by favorite (pinned first), by recently-played (requires Phase 3 metadata). A `<ThemedSelect>` dropdown (already exists) in the toolbar.

- **Drag-to-reorder**: High effort, low priority. Requires a custom order array persisted to SQLite (`profile_order` table or `user_sort_order` column). Deferred.

### Integrations

- **ProtonDB badge on card**: `useGameMetadata` already fetches `SteamAppDetails`. A small colored dot (ProtonDB rating color) on the card corner would surface compatibility status at a glance. Requires associating ProtonDB rating with the profile name (already in `CommunityBrowser`).

- **Health badge**: `useProfileHealthContext` is already global. A small `HealthBadge` on each card could show profile readiness at a glance — the component already exists at `HealthBadge.tsx`.

### Explicitly Out of Scope

- **Playtime tracking**: No backend support exists. Do not show zero or fake playtime values — this would confuse users. Placeholder text ("—") is acceptable only if the feature is explicitly planned with a timeline. Otherwise omit the field entirely from the card design.

---

## Risk Assessment

### Technical Risks

| Risk                                                                             | Severity | Likelihood             | Mitigation                                                                                                                                                                                                                                   |
| -------------------------------------------------------------------------------- | -------- | ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Cover art needs full profile data, but only names are in `profiles: string[]`    | High     | Certain                | Use Option A (lazy per-card `profile_load` on intersection) for Phase 1. Plan Option B (batch metadata IPC) for Phase 2.                                                                                                                     |
| Cover art fetch storm: N parallel IPC calls on mount for a 50+ profile library   | Medium   | High                   | Use `IntersectionObserver` — fire `profile_load` + `fetch_game_cover_art` only when card enters viewport. `useGameCoverArt` already handles race conditions with `requestIdRef`.                                                             |
| Navigation race: `onNavigate` fires before `selectProfile` resolves              | High     | Certain unless guarded | Always `await selectProfile(name)` before calling `onNavigate`. LaunchPage and ProfilesPage both render with `ProfileContext` state — if profile isn't loaded yet they'll briefly show the previous profile or empty state.                  |
| Route state loss: navigating Home → Launch → Home resets grid scroll position    | Low      | Medium                 | `ContentArea` already handles `scrollTop = 0` on route change (intentional). Not a regression.                                                                                                                                               |
| `toggleFavorite` optimistic update absent: heart button feels laggy              | Low      | Medium                 | Implement optimistic UI: flip `isFavorite` immediately, revert on error. Same pattern as other toggle actions.                                                                                                                               |
| CSS variable cascade: `--crosshook-library-card-width` slider breaks at extremes | Low      | Low                    | Clamp the CSS variable: `clamp(130px, var(--crosshook-library-card-width), 300px)`.                                                                                                                                                          |
| No `home` route in `VALID_APP_ROUTES` exhaustiveness check                       | High     | Certain                | The `isAppRoute` guard in `App.tsx:29` uses a `Record<AppRoute, true>` — adding `'home'` to `AppRoute` without adding it to `VALID_APP_ROUTES` will cause a TypeScript compile error. This is a compile-time safety net, not a runtime risk. |
| forceMount on all Tabs.Content: all pages are always mounted                     | Info     | Certain                | `ContentArea` uses `forceMount: true as const` (line 31). HomePage will be mounted even when inactive. Avoid effects that fire continuously; gate them on `route === 'home'` if needed.                                                      |

### Integration Challenges

- **Navigation state for Launch action**: `selectProfile` is async (~50–200ms IPC round trip). The flow must be: `await selectProfile(name)` → `onNavigate('launch')`. If `onNavigate` fires before the profile is loaded, `LaunchPage` will render with the previous profile. Optionally add a brief loading indicator on the card during this gap.
- **`onNavigate` prop threading**: `HomePage` needs `onNavigate` but `ContentArea` passes it to pages (`InstallPage`, `HealthDashboardPage` already receive it). The same pattern applies — add `onNavigate?: (route: AppRoute) => void` to `HomePageProps`.
- **Favorites terminology**: `toggleFavorite` / `favoriteProfiles` in IPC use "favorite" semantics, but `PinnedProfilesStrip` uses "pinned" language. The library card spec says "Heart" action. These are currently the same underlying concept — clarify whether the home page heart button should map to the same "pinned" set or a separate future concept before implementation.
- **Default route and onboarding**: `OnboardingWizard` fires from a Tauri `onboarding-check` event on `AppShell` mount, regardless of route. This is unaffected by the route change. However, when `home` is the default route and `profiles.length === 0`, the wizard flow starts on the home page (empty grid) rather than the profiles editor. Ensure the empty state on home links naturally into the wizard.

### Performance

- **With 100+ profiles (Steam Deck risk)**: Each card calls `profile_load` (TOML file read) then `fetch_game_cover_art` (file system + optional network). Without `IntersectionObserver`, this fires 100+ sequential IPC calls. On slower hardware this is the primary performance bottleneck.
- **Dominant color per card**: `useImageDominantColor` uses an offscreen canvas per card. At 100 cards this is 100 canvas operations, staggered naturally by the cover art fetch queue. Low risk.
- **Search filter**: Client-side substring match on a `string[]` of profile names. O(n) per keypress. Negligible even at 1000 profiles.

### Security

- **`convertFileSrc` for custom cover art paths**: Already used in `useGameCoverArt` — Tauri's asset protocol handles local file access securely. No new attack surface.
- **`fetch_game_cover_art` `image_type` parameter**: The backend validates this with a match expression defaulting to `Cover` for unknown strings. Frontend hardcodes `'cover'` — no injection risk.
- **User-controlled cover art path**: Already validated in `MediaSection`; no change needed.

---

## Alternative Approaches

### A: CSS Grid `auto-fill` (Recommended)

```css
.crosshook-library-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--crosshook-library-card-width), 1fr));
  gap: var(--crosshook-library-grid-gap);
}
```

**Pros**: Matches existing `community-profile-grid` pattern exactly. Automatically responsive. No JS layout calculation.
**Cons**: Cards grow wider than 190px when few profiles exist. Acceptable for this use case.
**Effort**: Minimal.

### B: CSS Grid Fixed Columns (e.g. 4 columns)

Fixed column count; cards always exactly 190px wide with `justify-content: start`.

**Pros**: Matches Steam's fixed 4-column layout exactly.
**Cons**: Does not adapt to window size. Wasted space on wide displays.
**Effort**: Minimal but less flexible than A.

### C: Virtual Scrolling (`@tanstack/react-virtual`)

Renders only visible rows; essential for 500+ profiles.

**Pros**: Near-zero render cost regardless of profile count. Naturally pairs with `IntersectionObserver`-style lazy loading.
**Cons**: Adds a dependency. Significantly more complex — 2D grid virtualisation is harder than list virtualisation. Not needed at typical library sizes.
**Effort**: High.

**Recommendation**: Start with A (auto-fill grid). Add virtual scrolling in a future phase if profiling reveals real performance problems.

### D: Flexbox Wrap

`display: flex; flex-wrap: wrap; gap: N` with fixed card widths.

**Pros**: Simple to understand.
**Cons**: Last row of cards may be uneven widths. CSS Grid is strictly better here.
**Effort**: Same as A.

---

## Task Breakdown Preview

### Phase 1 — Route & Grid Foundation (~medium complexity)

| Task Group              | Description                                                                                                                                                                                                                                             |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Route wiring            | Add `'home'` to `AppRoute`, `VALID_APP_ROUTES`, `Sidebar`, `ContentArea` switch, `PageBanner`. Change default route in `App.tsx`.                                                                                                                       |
| `LibraryCard` component | Cover art (3:4 aspect, 190px base width), name label, Launch / Edit / Heart buttons, skeleton state, favorite visual indicator. Accepts `steamAppId` + `customCoverArtPath` props (populated by parent from lazy-loaded `GameProfile`).                 |
| `HomePage` component    | `useProfileContext` consumption, grid layout, search bar (client-side filter), grid/list toggle (localStorage). Manages `Map<string, GameProfile>` cache for cover art metadata. Uses `IntersectionObserver` to trigger `profile_load` per card lazily. |
| CSS tokens              | Add `--crosshook-library-card-width`, `--crosshook-library-card-aspect`, `--crosshook-library-grid-min` to `variables.css`. Add grid + card CSS classes to `theme.css`.                                                                                 |
| Navigation actions      | `await selectProfile(name)` + `onNavigate('launch')` for Launch; `await selectProfile(name)` + `onNavigate('profiles')` for Edit; `toggleFavorite` for Heart.                                                                                           |
| Empty state             | When `profiles.length === 0`: centered CTA linking to `'profiles'` or triggering `OnboardingWizard`.                                                                                                                                                    |

### Phase 2 — Polish (~low complexity)

| Task Group            | Description                                                                                                                                                                                              |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Skeleton states       | Use `crosshook-skeleton` + `crosshook-skeleton-shimmer` while `loading` is true in `useGameCoverArt`.                                                                                                    |
| Card size slider      | `<input type="range">` in toolbar; sets CSS custom property on grid container; persisted to `localStorage`.                                                                                              |
| Dominant-color glow   | On card `:hover`, apply `box-shadow` using RGB from `useImageDominantColor`.                                                                                                                             |
| Backend batch command | Add `profile_list_cover_art_metadata` IPC command to `crosshook-core` — returns `Vec<ProfileCoverArtMeta>` with `name`, `steam_app_id`, `custom_cover_art_path`. Replaces per-card `profile_load` calls. |

### Phase 3 — Power Features (~medium–high complexity)

| Task Group               | Description                                                                                                                                                                                                        |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Right-click context menu | Add `@radix-ui/react-context-menu`. Expose Duplicate, Rename, Delete actions (already wired in `useProfile`).                                                                                                      |
| Keyboard navigation      | `tabIndex` on cards, `onKeyDown` handlers for Enter/E/F/`/`.                                                                                                                                                       |
| Recently-played section  | New `last_launched_at` column in SQLite metadata DB (migration); new `profile_list_recently_launched` IPC command; horizontal strip above grid. **Storage classification**: operational/history metadata → SQLite. |
| Sort controls            | `ThemedSelect` dropdown; sort by name / favorite / recent.                                                                                                                                                         |

---

## Key Decisions Needed

1. **Cover art data strategy**: Use Option A (lazy per-card `profile_load`), Option B (new batch IPC), or Option C (cover art only for active profile)? This must be decided before `LibraryCard` API is finalized.
2. **Default route change**: Should `'home'` replace `'profiles'` as the startup route, or should the user be able to configure this in Settings? Simplest: make `'home'` the default unconditionally.
3. **Card action UX model**: Three always-visible buttons vs. hover-reveal overlay vs. primary CTA + right-click menu. The always-visible model is straightforward; the hover-reveal is cleaner for dense grids.
4. **Favorites vs. "pinned" terminology**: The heart button maps to the existing `toggleFavorite` / `profile_set_favorite` IPC. This is the same set currently called "pinned" in `PinnedProfilesStrip`. Decide whether to unify the language to "favorites" or keep "pinned" for the strip and "favorite" for the grid.
5. **Playtime data**: Explicitly declare it out of scope for Phase 1 and Phase 2, or add a `last_launched_at` tracking stub in Phase 1 (backend only, no UI) to unblock Phase 3.
6. **List view design**: Should the list view be a compact table (name + health badge + last-played + action buttons) or a tall list of smaller cards? A simple table aligns with the existing `HealthDashboardPage` table patterns.

---

## Open Questions

- Is there a Figma design node ID available for the poster art cards? The `--crosshook-profile-cover-art-aspect` variable in `variables.css:90` uses `460 / 215` (landscape, ~2.14:1), which is Steam's header format. The spec says 3:4 (portrait) — confirm which format the cover art IPC (`fetch_game_cover_art` with `image_type = "cover"`) actually caches. The backend routes `"cover"` to `GameImageType::Cover`, `"capsule"` to `GameImageType::Capsule`, and `"hero"` to `GameImageType::Hero` — check which type produces portrait (3:4) art.
- Will the `'home'` route replace `'profiles'` in the sidebar navigation, or will both coexist? If both exist, the sidebar needs a clear label distinction ("Library" vs. "Profiles").
- What is the fallback card art design for profiles with no `steam.app_id` and no `custom_cover_art_path`? A dark placeholder with the first letter of the profile name (like avatar initials) is a common pattern and requires zero IPC.
- Should `PinnedProfilesStrip` be surfaced on the home page (favorites section above the full grid) or removed from `ProfilesPage` once the home page exists?
