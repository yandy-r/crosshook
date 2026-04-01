# Feature Spec: Library Home

## Executive Summary

The library-home feature adds a Steam-like poster art grid as CrossHook's default landing page, enabling users to browse, launch, edit, and favorite game profiles at a glance. Each profile renders as a 190px-wide, 3:4 aspect-ratio card showing Steam cover art (fetched via the existing `useGameCoverArt` hook and cached on disk by the Rust backend), with three direct actions: Launch (navigates to LaunchPage with the profile activated), Edit (navigates to ProfilesPage), and Heart (toggles favorite via the existing `profile_set_favorite` IPC). The implementation builds entirely on existing infrastructure — `ProfileContext`, `useGameCoverArt`, `useImageDominantColor`, CSS Grid `auto-fill`, and the `crosshook-skeleton` shimmer — requiring zero new npm dependencies and one optional new Rust IPC command (`profile_list_summaries`) for optimal cover art metadata loading. The primary architectural challenge is that `profiles: string[]` contains names only; cover art requires `steam.app_id` and `custom_cover_art_path` from each profile's TOML, necessitating either lazy per-card loading or a batch metadata IPC.

## External Dependencies

### APIs and Services

#### Steam CDN (Cloudflare Edge)

- **Documentation**: [Steamworks Standard Assets](https://partner.steamgames.com/doc/store/assets/standard)
- **Authentication**: None (unauthenticated CDN)
- **Key Endpoints**:
  - `GET /steam/apps/{APP_ID}/library_600x900_2x.jpg`: Portrait cover art (600x900, preferred for poster grid)
  - `GET /steam/apps/{APP_ID}/library_600x900.jpg`: Portrait cover art (300x450, fallback)
  - `GET /steam/apps/{APP_ID}/header.jpg`: Landscape capsule (460x215, last-resort fallback)
- **Rate Limits**: Undocumented for CDN image requests (CDN-cached; practically unlimited). JSON API at `store.steampowered.com` is limited to 200 req/5min — not used for image loading.
- **Pricing**: Free
- **Note**: Not all games have `library_600x900` assets; HTTP 404 is expected. The Rust `download_and_cache_image` fallback chain handles this.

#### SteamGridDB (Already Integrated)

- **Documentation**: [SteamGridDB API v2](https://www.steamgriddb.com/api/v1)
- **Authentication**: API key (90-day rotation, 2FA enforced)
- **Purpose**: Community fallback for portrait art when Steam CDN lacks the format
- **Status**: Already integrated in `steamgriddb.rs`; no new integration needed

### Libraries and SDKs

No new npm dependencies required for Phase 1.

| Library                        | Version | Purpose                                        | When to Add                       |
| ------------------------------ | ------- | ---------------------------------------------- | --------------------------------- |
| `@tanstack/react-virtual`      | v3      | Grid virtualization for 200+ profile libraries | Phase 3 (if profiling shows jank) |
| `@radix-ui/react-context-menu` | latest  | Right-click context menu on cards              | Phase 3                           |

### External Documentation

- [Steam Library 600x900 Format](https://steamcommunity.com/discussions/forum/1/4202490864582293420/): Community thread confirming portrait URL patterns
- [TanStack Virtual](https://tanstack.com/virtual/latest): Virtual scrolling docs (deferred)

## Business Requirements

### User Stories

**Primary User: Game launcher power user with 5+ profiles**

- As a user with multiple profiles, I want to see all my games as visual poster art so I can identify and launch games without navigating dropdowns
- As a user, I want to click Launch on a card to go directly to the LaunchPage with that profile activated, avoiding a separate profile-selection step
- As a user, I want to click Edit on a card to jump directly to the ProfilesPage editor for that profile
- As a user, I want to mark profiles as favorites with a heart button so I can build a personal shortlist for a future favorites filter

**Secondary User: Steam Deck / gamepad user**

- As a Steam Deck user, I want large poster art cards with visible action buttons (not hover-only) so I can navigate with a gamepad

**Tertiary User: New user with no profiles**

- As a first-time user, I want a clear empty-state prompt directing me to create a profile so the blank grid is not a dead end

### Business Rules

1. **R1 — Profile-to-card mapping (1:1)**: Each saved profile (persisted to TOML disk) maps to exactly one card. Unsaved/draft profiles are not shown. Card title derives from `profile.game.name`; falls back to the executable filename stripped of extension.

2. **R2 — Cover art resolution order**: (a) `custom_cover_art_path` if non-empty → local file via `convertFileSrc`, (b) Steam CDN portrait art via `fetch_game_cover_art` when `steam.app_id` is set, (c) graceful fallback placeholder (dark gradient + game initials).

3. **R3 — Launch action (two-step model)**: Click Launch → `await selectProfile(name)` → `onNavigate('launch')`. The card does not launch the game directly. For dual-mode (`proton_run`/`steam_applaunch`), the user still clicks "Launch Game" then "Launch Trainer" on the LaunchPage.

4. **R4 — Edit action**: Click Edit → `await selectProfile(name)` → `onNavigate('profiles')`. ProfilesPage displays the editor for the now-active profile.

5. **R5 — Favorite toggle**: Click Heart → `toggleFavorite(name, !isFavorite)` from `ProfileContext`. Persisted in SQLite via `profile_set_favorite`. Optimistic UI: heart flips immediately, reverts on error.

6. **R6 — Search/filter**: Client-side case-insensitive substring match on profile name. No backend call. Filter is display-only — does not affect active profile or favorites.

7. **R7 — View mode toggle**: Grid (default) vs. list. UI-only preference state. Not persisted to backend for MVP (localStorage acceptable).

8. **R8 — Playtime is out of scope**: No playtime tracking exists. Omit the playtime field entirely — do not show "0h" or placeholder values that imply tracking.

9. **R9 — Profile must be saved to appear**: Only profiles from `profile_list` IPC (persisted TOML files) are displayed.

10. **R10 — Favorites terminology**: The heart button maps to the same `is_favorite` field used by `PinnedProfilesStrip` (which uses star/pin language). Same data store, different visual. No data model change needed.

### Edge Cases

| Scenario                                                  | Expected Behavior                                                                               | Notes                                       |
| --------------------------------------------------------- | ----------------------------------------------------------------------------------------------- | ------------------------------------------- |
| Empty profiles list                                       | Show empty-state with CTA to create a profile                                                   | First-run onboarding moment                 |
| Profile TOML deleted between list load and card click     | `selectProfile` fails; error surfaces on target page via `ProfileContext.error`                 | Library home does not handle this           |
| Cover art fetch fails (Steam CDN 404, no SteamGridDB key) | Card shows dark gradient + game initials (2 chars)                                              | `useGameCoverArt` returns `null` gracefully |
| Long profile name                                         | Truncate with CSS `text-overflow: ellipsis` at 1 line                                           | BEM class handles this                      |
| Search with no results                                    | Inline message: "No games match '[query]'" with "Clear search" link                             | Within grid area                            |
| Profile has no `executable_path`                          | Launch button still navigates to LaunchPage; LaunchPage handles the "no executable" error state | No pre-validation needed                    |

### Success Criteria

- [ ] All saved profiles display as cards in a responsive grid on the home page
- [ ] Launch button activates profile and navigates to Launch page
- [ ] Edit button activates profile and navigates to Profiles page
- [ ] Heart button toggles favorite with optimistic UI feedback
- [ ] Search bar filters cards by profile name (case-insensitive)
- [ ] Grid/list toggle switches layout; both views support all actions
- [ ] Profiles without cover art display a gradient fallback with game initials
- [ ] Empty library shows a CTA directing to profile creation
- [ ] `refreshProfiles()` on mount ensures the list is current

## Technical Specifications

### Architecture Overview

```text
LibraryPage                           (new page, src/components/pages/)
├── LibraryToolbar                    (search + view toggle, src/components/library/)
└── LibraryGrid                       (CSS Grid auto-fill, src/components/library/)
    └── LibraryCard (×N)              (src/components/library/)
        ├── useGameCoverArt(steamAppId, customCoverArtPath)
        ├── useImageDominantColor(coverArtUrl)   [Phase 2]
        ├── Cover art <img> / skeleton / fallback
        ├── Gradient overlay
        ├── Title label
        └── Action row: Launch | Heart | Edit

Data flow:
  ProfileContext.profiles (string[])
      ↓  profile_list_summaries IPC (Option A) or per-card profile_load
  LibraryCardData[] (name, gameName, steamAppId, customCoverArtPath, isFavorite)
      ↓  per card
  useGameCoverArt → fetch_game_cover_art IPC → disk cache → asset:// URL
```

### Data Models

#### LibraryCardData (Frontend Type)

```typescript
// src/crosshook-native/src/types/library.ts
export type LibraryViewMode = 'grid' | 'list';

export interface LibraryCardData {
  name: string; // Profile filename (no extension) — React key
  gameName: string; // profile.game.name — may be empty
  steamAppId: string; // profile.steam.app_id — drives useGameCoverArt
  customCoverArtPath?: string; // profile.game.custom_cover_art_path
  isFavorite: boolean; // derived from ProfileContext.favoriteProfiles
}
```

#### ProfileSummary (Backend DTO — Option A)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub name: String,
    pub game_name: String,
    pub steam_app_id: String,
    pub custom_cover_art_path: String,
}
```

#### Existing SQLite Schema (No Changes for MVP)

The `profiles` table stores `current_filename`, `game_name`, `is_favorite`, `launch_method`, `content_hash`. The `game_image_cache` table stores cached images keyed on `(steam_app_id, image_type, source)`. Neither table stores `steam_app_id` per profile — this lives only in TOML.

### API Design

#### Existing IPC Commands (No Changes)

| Command                  | Signature                                | Used By                           |
| ------------------------ | ---------------------------------------- | --------------------------------- |
| `profile_list`           | `() → Vec<String>`                       | `refreshProfiles()` on mount      |
| `profile_list_favorites` | `() → Vec<String>`                       | `ProfileContext.favoriteProfiles` |
| `profile_set_favorite`   | `{ name, favorite } → ()`                | `toggleFavorite()`                |
| `profile_load`           | `{ name } → GameProfile`                 | `selectProfile()` for navigation  |
| `fetch_game_cover_art`   | `{ appId, imageType? } → Option<String>` | `useGameCoverArt` hook            |

#### New IPC Command (Recommended — Option A)

**`profile_list_summaries`** — Returns lightweight cover art metadata for all profiles in a single server-side pass.

**Request**: No parameters
**Response**: `Vec<ProfileSummary>` — reads TOML files server-side; returns slim DTO
**Error**: `Err(String)` on store failure
**Benefit**: One IPC round-trip instead of N `profile_load` calls; always fresh from TOML

### System Integration

#### Files to Create

| File                                                             | Purpose                                                                     |
| ---------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx`      | Page root; owns search state, view toggle; calls `refreshProfiles` on mount |
| `src/crosshook-native/src/components/library/LibraryGrid.tsx`    | Grid/list layout; maps `LibraryCardData[]` to `LibraryCard` instances       |
| `src/crosshook-native/src/components/library/LibraryCard.tsx`    | Poster card; uses `useGameCoverArt`; fires action callbacks                 |
| `src/crosshook-native/src/components/library/LibraryToolbar.tsx` | Search `<input>` + grid/list toggle                                         |
| `src/crosshook-native/src/hooks/useLibraryProfiles.ts`           | Pure filter/sort transform over profiles + favorites; no IPC                |
| `src/crosshook-native/src/styles/library.css`                    | All library-specific CSS (BEM `crosshook-library-*`)                        |
| `src/crosshook-native/src/types/library.ts`                      | `LibraryCardData`, `LibraryViewMode`                                        |

#### Files to Modify

| File                                | Change                                                                                                  |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `Sidebar.tsx:13`                    | Add `'library'` to `AppRoute` union; add sidebar item (icon + "Library" label)                          |
| `App.tsx:19`                        | Add `library: true` to `VALID_APP_ROUTES`; change default route to `'library'`                          |
| `ContentArea.tsx:35`                | Add `case 'library': return <LibraryPage onNavigate={onNavigate} />;`                                   |
| `PageBanner.tsx`                    | Add library page banner illustration                                                                    |
| `variables.css`                     | Add `--crosshook-library-card-width`, `--crosshook-library-card-aspect`, `--crosshook-library-grid-gap` |
| `main.tsx`                          | Import `./styles/library.css`                                                                           |
| `src-tauri/src/commands/profile.rs` | Add `profile_list_summaries` command (Option A)                                                         |
| `src-tauri/src/commands/mod.rs`     | Register `profile_list_summaries` in invoke handler                                                     |

#### CSS Variables

```css
--crosshook-library-card-width: 190px;
--crosshook-library-card-aspect: 3 / 4;
--crosshook-library-grid-gap: var(--crosshook-grid-gap);
```

## UX Considerations

### User Workflows

#### Primary: Launch a Game from Home

1. User opens CrossHook → lands on library home (default route)
2. `refreshProfiles()` on mount; grid renders with skeleton cards
3. Cover art loads progressively per card
4. User clicks **Launch** button on a card
5. `await selectProfile(name)` activates the profile
6. `onNavigate('launch')` switches to LaunchPage
7. User clicks "Launch Game" on LaunchPage (two-step flow continues normally)

#### Primary: Edit a Profile from Home

1. User clicks **Edit** on a card
2. `await selectProfile(name)` → `onNavigate('profiles')`
3. ProfilesPage editor opens with the selected profile

#### Primary: Toggle Favorite

1. User clicks **Heart** on a card
2. Heart fills immediately (optimistic update)
3. `toggleFavorite(name, !isFavorite)` fires in background
4. On failure: heart reverts; error surfaced via toast or inline message

### UI Patterns

| Component              | Pattern                                                                 | Notes                                                             |
| ---------------------- | ----------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Card actions           | Hover/focus-reveal row                                                  | Buttons appear on `:hover` and `:focus-within`; scrim intensifies |
| Gradient scrim         | `linear-gradient(to top, rgba(0,0,0,0.85) 0%, transparent 50%)`         | WCAG AA 4.5:1 contrast for white text                             |
| Missing cover art      | Dark gradient + 2-char game initials in brand accent color              | Matches Lutris pattern                                            |
| Skeleton loading       | `crosshook-skeleton` + `crosshook-skeleton-shimmer` keyframe (existing) | 12-20 placeholder cards on mount                                  |
| Empty state            | Centered illustration + "No game profiles yet" + CTA to create          | First-run onboarding                                              |
| Favorite badge         | Filled heart icon in card top-right corner (persistent when favorited)  | Always visible, not hover-only                                    |
| Glass morphism buttons | `backdrop-filter: blur(8px)` on Heart/Edit buttons                      | Supported in WebKitGTK                                            |

### Accessibility Requirements

- Action buttons: explicit `aria-label` values (`"Launch {gameName}"`, `"Edit {gameName}"`, `"Add/Remove {gameName} from favorites"`)
- Button `min-height` respects `--crosshook-touch-target-min` (48px default, 56px in controller mode)
- Card focus ring follows existing `.crosshook-focus-scope` pattern
- Heart button: `aria-pressed` for toggle state
- Grid role: `role="list"` on container, `role="listitem"` on cards

### Performance UX

- **Loading States**: Skeleton card placeholders with shimmer animation; cover art loads progressively
- **Optimistic Updates**: Favorite toggle (instant heart flip, revert on error)
- **Lazy Loading**: `loading="lazy"` on `<img>` elements; virtual scrolling deferred to Phase 3
- **Search**: Client-side substring match — O(n) per keystroke, negligible at any realistic library size

## Recommendations

### Implementation Approach

**Recommended Strategy**: Build in three phases. Phase 1 delivers a fully functional grid with no new dependencies and one optional backend command. Phases 2-3 add polish and power features.

**Phasing:**

1. **Phase 1 — Route & Grid Foundation**: Route wiring, `LibraryCard`, `LibraryPage`, search, grid/list toggle, empty state, `profile_list_summaries` backend command
2. **Phase 2 — Polish**: Skeleton states, card size slider, dominant-color glow, hover scale animation
3. **Phase 3 — Power Features**: Right-click context menu, keyboard shortcuts, recently-played section (SQLite migration), virtual scrolling

### Technology Decisions

| Decision             | Recommendation                              | Rationale                                                                          |
| -------------------- | ------------------------------------------- | ---------------------------------------------------------------------------------- |
| Grid layout          | CSS Grid `auto-fill minmax(190px, 1fr)`     | Matches existing `community-profile-grid` pattern; automatically responsive        |
| Cover art metadata   | New `profile_list_summaries` IPC (Option A) | One round-trip vs. N `profile_load` calls; ~20 lines of Rust                       |
| Virtual scrolling    | Defer (CSS Grid only for MVP)               | Typical library < 100 profiles; `content-visibility: auto` as interim optimization |
| New npm dependencies | None for Phase 1                            | All infrastructure exists: hooks, skeleton CSS, grid patterns                      |
| Playtime display     | Omit entirely                               | No backend tracking exists; avoid misleading zero values                           |

### Quick Wins

- Search: pure client-side filter over `profiles: string[]` — zero IPC
- Heart: `toggleFavorite` already wired in `ProfileContext`
- Navigation: existing `onNavigate` prop pattern from `ContentArea`
- Skeleton shimmer: `crosshook-skeleton-shimmer` keyframe already in `theme.css`

### Future Enhancements

- **Card size slider**: CSS variable `--crosshook-library-card-width` + range input → instant grid resize
- **Recently played section**: Requires `last_launched_at` SQLite column + new IPC
- **Right-click context menu**: `@radix-ui/react-context-menu` for Launch/Edit/Duplicate/Delete
- **ProtonDB badge**: Colored dot on card corner from existing `useGameMetadata`
- **Health badge**: `useProfileHealthContext` already global; `HealthBadge` component exists

## Risk Assessment

### Technical Risks

| Risk                                                                      | Likelihood             | Impact | Mitigation                                                                             |
| ------------------------------------------------------------------------- | ---------------------- | ------ | -------------------------------------------------------------------------------------- |
| Cover art needs full profile data but only names in `profiles[]`          | Certain                | High   | `profile_list_summaries` batch IPC (Option A)                                          |
| N parallel `fetch_game_cover_art` calls on mount for large libraries      | High                   | Medium | `IntersectionObserver` or virtual scrolling limits concurrent fetches to visible cards |
| Navigation race: `onNavigate` fires before `selectProfile` resolves       | Certain unless guarded | High   | Always `await selectProfile(name)` before `onNavigate`                                 |
| `forceMount` on ContentArea means LibraryPage stays mounted when inactive | Certain                | Low    | Gate effects on `route === 'library'` if needed                                        |
| CSS `backdrop-filter: blur()` perf on many cards simultaneously           | Low                    | Low    | Limit blur to small icon buttons; reduce blur radius ≤ 12px                            |

### Integration Challenges

- **Navigation state**: `selectProfile` is async (~50-200ms IPC). Flow must be `await selectProfile(name)` → `onNavigate(route)`. Brief loading indicator on card during the gap.
- **Favorites terminology**: IPC uses "favorite"; `PinnedProfilesStrip` uses "pinned"; library uses "heart". All map to the same `is_favorite` field. Keep visual differences, same data.
- **Default route change**: Changing `useState<AppRoute>('profiles')` → `'library'` in App.tsx. OnboardingWizard fires from a Tauri event regardless of route — unaffected.
- **Portrait cover art format**: `GameImageType::Cover` currently maps to `header.jpg` (landscape). A Rust change to `build_download_url` is needed to try `library_600x900_2x.jpg` first with `header.jpg` fallback.

### Security Considerations

#### Critical — Hard Stops

| Finding         | Risk | Required Mitigation |
| --------------- | ---- | ------------------- |
| None identified | —    | —                   |

#### Warnings — Must Address

| Finding                                                                         | Risk                         | Mitigation                                                    | Alternatives                                         |
| ------------------------------------------------------------------------------- | ---------------------------- | ------------------------------------------------------------- | ---------------------------------------------------- |
| `custom_cover_art_path` passed to `convertFileSrc` without scope enforcement    | Path traversal               | Validate path resolves inside known safe dirs; broker via IPC | Add `fs:allow-read-file` scope for user image dirs   |
| Asset protocol scope limited to cache dir — custom paths outside scope will 403 | Broken images for custom art | Either validate scope or broker reads via IPC                 | Accept current behavior (custom art from cache only) |

#### Advisories — Best Practices

- Client-side search uses React text rendering (no `dangerouslySetInnerHTML`) — no XSS risk (deferral: safe indefinitely)
- Profile names from TOML rendered safely by React default escaping (deferral: safe indefinitely)
- All SQLite queries use `rusqlite params![]` macros — no injection risk (deferral: N/A)
- Prefer `loading="lazy"` over npm image libraries to minimize attack surface (deferral: safe indefinitely)
- Error messages must not expose file paths or internal IDs (deferral: address during implementation)

## Task Breakdown Preview

### Phase 1: Route & Grid Foundation

**Focus**: Functional poster grid with search, navigation actions, and favorites
**Tasks**:

- Add `'library'` route to `AppRoute`, `VALID_APP_ROUTES`, `Sidebar`, `ContentArea`
- Change default route from `'profiles'` to `'library'` in `App.tsx`
- Create `LibraryCardData` and `LibraryViewMode` types
- Add `profile_list_summaries` Rust IPC command
- Create `LibraryCard` component (cover art, title, Launch/Edit/Heart buttons)
- Create `LibraryToolbar` component (search input, grid/list toggle)
- Create `LibraryGrid` component (CSS Grid `auto-fill`)
- Create `LibraryPage` page component (wires context, toolbar, grid)
- Create `useLibraryProfiles` hook (filter/sort)
- Add CSS variables and `library.css` stylesheet
- Implement empty state for zero profiles
- Add portrait cover art URL to Rust `build_download_url` fallback chain

**Parallelization**: Route wiring, types, and CSS can run in parallel with the Rust IPC command. Components depend on types being defined first.

### Phase 2: Polish

**Focus**: Visual refinement and performance optimization
**Dependencies**: Phase 1 complete
**Tasks**:

- Skeleton loading states per card (reuse `crosshook-skeleton-shimmer`)
- Dominant-color glow on card hover (via `useImageDominantColor`)
- Card size slider (CSS variable + range input + localStorage)
- Hover scale animation (`transform: scale(1.03)`)
- Active profile indicator (accent border glow on currently-selected card)

### Phase 3: Power Features

**Focus**: Advanced interactions and data enrichment
**Dependencies**: Phase 2 polish provides visual foundation
**Tasks**:

- Right-click context menu (`@radix-ui/react-context-menu`)
- Keyboard shortcuts (Enter=Launch, E=Edit, F=Favorite, /=Search)
- Recently-played section (SQLite `last_launched_at` migration + IPC + horizontal strip)
- Virtual scrolling (`@tanstack/react-virtual`) if profiling shows need
- Sort controls (`ThemedSelect` dropdown)

## Decisions (Resolved)

1. **Cover art data strategy** — **Resolved: Option A (batch IPC)**
   - New `profile_list_summaries` Rust command reads all TOMLs server-side, returns slim DTOs in one IPC round-trip. No rate limit or ban risk — this reads local filesystem only; Steam CDN image fetches happen separately per-card with 24-hour disk cache.

2. **Default route change** — **Resolved: Yes**
   - `'library'` replaces `'profiles'` as the default startup route in `App.tsx`.

3. **Card action visibility** — **Resolved: Hover-reveal**
   - Action buttons (Launch, Edit, Heart) appear on hover/focus, keeping the poster art clean. The gradient scrim intensifies on hover to ensure button contrast. Cards still receive focus via keyboard/gamepad — buttons appear on `:focus-within` as well.

4. **Portrait cover art URL** — **Resolved: New `Portrait` variant from Steam**
   - Add `GameImageType::Portrait` that tries `library_600x900_2x.jpg` → `library_600x900.jpg` → `header.jpg`. Existing `Cover` type unchanged. Custom cover art path continues to work as an override (same `custom_cover_art_path` field, same resolution priority).

5. **Favorites ordering** — **Resolved: Mixed alphabetical**
   - Favorites are mixed into the alphabetical grid for now. A separate favorites filter/tab will be added later. No special sort pinning.

## Persistence & Usability

### Storage Boundary

| Datum                     | Classification                              | Rationale                                            |
| ------------------------- | ------------------------------------------- | ---------------------------------------------------- |
| Favorite toggle           | SQLite metadata DB (`profiles.is_favorite`) | Already implemented; operational metadata            |
| View mode (grid/list)     | Runtime-only (localStorage)                 | UI preference; no backend persistence needed for MVP |
| Search query              | Runtime-only (React state)                  | Ephemeral; resets on route change                    |
| Card size preference      | Runtime-only (localStorage)                 | Phase 2; UI preference                               |
| Recently-played timestamp | SQLite metadata DB (Phase 3 migration)      | Operational history                                  |

### Migration & Backward Compatibility

- **No SQLite migration for Phase 1** — all existing tables are sufficient
- **Phase 3**: Adding `last_launched_at` column requires a schema migration (v14); standard `ALTER TABLE ADD COLUMN` with nullable default
- **TOML profiles**: Read-only access for cover art metadata; no TOML format changes

### Offline Expectations

- `profile_list` reads TOML files from disk — fully offline
- Cover art from cache is served as `asset://` URLs — no network dependency after first fetch
- Cover art for profiles never fetched before degrades to gradient fallback — grid remains functional

### User Visibility & Editability

- Favorites: toggled directly via heart button on cards (and existing star on Profiles/Launch pages)
- View mode: toggled via toolbar; implicit persistence via localStorage
- All profile data remains editable via the Profile editor (Edit button navigates there)

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Steam CDN patterns, image libraries, integration patterns
- [research-business.md](./research-business.md): User stories, business rules, workflows, domain model
- [research-technical.md](./research-technical.md): Architecture, data models, API design, CSS layout
- [research-ux.md](./research-ux.md): Competitive analysis, card design, accessibility, loading patterns
- [research-security.md](./research-security.md): Image loading security, input validation, Tauri model
- [research-practices.md](./research-practices.md): Reusable code inventory, modularity, KISS assessment
- [research-recommendations.md](./research-recommendations.md): Phasing, alternatives, risk assessment
