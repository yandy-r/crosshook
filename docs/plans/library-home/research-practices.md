# Engineering Practices Research: library-home

## Executive Summary

The CrossHook codebase already provides all the core primitives needed for the library-home feature: cover art loading, dominant-color extraction, favorites toggling, grid layout patterns, and a clean page-hosting contract. The key open question is the cover art data-loading strategy: `profiles[]` from context is a `string[]` of names only — full TOML (containing `steam.app_id`) must not be pre-loaded for all cards. A new `profile_list_summaries` IPC is the recommended path (see Open Questions #0).

## Existing Reusable Code

| Module                                | Location                                                                               | Purpose                                                                                                                                                   | How to Reuse                                                                                                                 |
| ------------------------------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ----- |
| `useGameCoverArt`                     | `src/hooks/useGameCoverArt.ts`                                                         | Loads Steam cover art by app ID (with custom path override); race-condition safe via request-id ref                                                       | Call per card once `steamAppId` is available; requires a summary IPC to avoid N TOML loads at render — see Open Questions #0 |
| `useImageDominantColor`               | `src/hooks/useImageDominantColor.ts`                                                   | Extracts dominant RGB from an image URL via offscreen canvas; top-third weighted for banner tinting                                                       | Feed it `coverArtUrl` from `useGameCoverArt`; outputs `[r, g, b]                                                             | null` |
| `GameCoverArt`                        | `src/components/profile-sections/GameCoverArt.tsx`                                     | Thin presentational wrapper around `useGameCoverArt`; shows skeleton → image → null                                                                       | Reuse directly for the card image area if no overlay is needed; extend if an overlay is required                             |
| `useProfile` / `useProfileContext`    | `src/hooks/useProfile.ts` / `src/context/ProfileContext.tsx`                           | Owns all profile state: list of names, `favoriteProfiles`, `selectedProfile`, `toggleFavorite`, `selectProfile`                                           | Consume `useProfileContext()` in the page; the hook already exposes everything the grid needs                                |
| `toggleFavorite`                      | `src/hooks/useProfile.ts:519`                                                          | Calls `profile_set_favorite` IPC and refreshes the list; typed `(name: string, favorite: boolean) => Promise<void>`                                       | Call from card's heart-button `onClick`; already used on LaunchPage and ProfilesPage                                         |
| `favoriteProfiles`                    | `src/hooks/useProfile.ts:42`                                                           | Array of profile names from `profile_list_favorites` IPC; already in context                                                                              | Compare against card `profileName` to derive initial heart state                                                             |
| `GameMetadataBar` / `useGameMetadata` | `src/components/profile-sections/GameMetadataBar.tsx` / `src/hooks/useGameMetadata.ts` | Loads genre tags and display name from SQLite metadata DB; debounced 400ms; shows "Cached" badge for stale data                                           | Use `useGameMetadata` directly inside the card for richer labelling; or embed `GameMetadataBar` as a sub-element             |
| `ThemedSelect`                        | `src/components/ui/ThemedSelect.tsx`                                                   | Radix Select with pin/unpin and option badges; already used for profile selection on LaunchPage and ProfilesPage                                          | Not needed for the grid itself but could drive a "sort by" or "filter by" dropdown in the toolbar                            |
| `CollapsibleSection`                  | `src/components/ui/CollapsibleSection.tsx`                                             | Controlled/uncontrolled `<details>` wrapper; supports `meta` slot                                                                                         | Not needed for the initial grid but a good primitive for a future filter panel                                               |
| CSS variables                         | `src/styles/variables.css`                                                             | Tokens: `--crosshook-color-bg`, `--crosshook-radius-lg`, `--crosshook-grid-gap`, `--crosshook-profile-cover-art-aspect` (460/215), skeleton keyframe vars | Use these tokens for card sizing and aspect ratio; do **not** hard-code hex values                                           |
| Community grid pattern                | `src/styles/theme.css:996`                                                             | `grid-template-columns: repeat(auto-fit, minmax(var(--crosshook-community-profile-grid-min), 1fr))`                                                       | Clone this pattern for the library grid with a new `--crosshook-library-card-min` variable (token, not hard-coded)           |
| Game-color theming                    | `src/components/ProfileSubTabs.tsx:112` / `src/components/LaunchSubTabs.tsx:150`       | CSS custom properties `--crosshook-game-color-r/g/b` set inline from `useImageDominantColor`; `crosshook-subtab-row--themed` class activates the tint     | Adopt the same inline-style pattern on the card element to derive a subtle border/glow from the cover art                    |
| `PanelRouteDecor`                     | `src/components/layout/PanelRouteDecor.tsx`                                            | Absolute decorative art layer inside a panel with `--with-route-decor` modifier                                                                           | Use on the page hero/header panel; a new `LibraryArt` SVG follows the existing pattern in `PageBanner.tsx`                   |
| `crosshook-button` variants           | `src/styles/theme.css:795`                                                             | Primary, `--secondary`, `--ghost`, `--ghost--small`                                                                                                       | Card action buttons should use `--ghost--small` for compactness; Launch action uses the primary style                        |
| Route scroll contract                 | `src/styles/layout.css`                                                                | `.crosshook-page-scroll-shell--fill` + `.crosshook-route-stack` + `.crosshook-route-stack__body--scroll`                                                  | Wrap the library page in these classes exactly as ProfilesPage and LaunchPage do                                             |
| `ContentArea` render switch           | `src/components/layout/ContentArea.tsx:34`                                             | `switch(route)` maps `AppRoute` to page components                                                                                                        | Add `'library'` to `AppRoute` in `Sidebar.tsx` and a case here                                                               |
| `AppRoute` type                       | `src/components/layout/Sidebar.tsx:13`                                                 | Union literal type + `VALID_APP_ROUTES` record                                                                                                            | Extend both with `'library'`                                                                                                 |
| `useScrollEnhance`                    | `src/hooks/useScrollEnhance.ts`                                                        | Already active globally via `App.tsx`; targets `.crosshook-page-scroll-body`                                                                              | Grid scroll is handled automatically if the standard scroll class is used                                                    |

## Modularity Design

**Recommended decomposition:**

```
LibraryHomePage          ← page shell (route-level); owns search state, view-mode state
  LibraryGrid            ← stateless layout container; receives cards array + grid config
    LibraryCard          ← standalone component; owns cover art and dominant-color logic
      GameCoverArt       ← existing component reused (or inlined if overlay is needed)
      LibraryCardOverlay ← hover/focus action surface (Launch, Edit, Favorite)
```

- **`LibraryCard`** should be standalone. It encapsulates `useGameCoverArt` + `useImageDominantColor` per-card, exactly matching how `ProfileSubTabs` and `LaunchSubTabs` do it. This keeps each card self-contained and makes the grid stateless.
- **`LibraryGrid`** should be generic (accepts `ReactNode[]` or a typed `cards` prop). Keeping it layout-only means it can be reused for any future poster-style list (e.g. a "Recent" strip).
- **Search state lives in `LibraryHomePage`**, not in `LibraryGrid`. Search is page-level UI; the grid only receives a filtered/sorted array. This matches how `CommunityBrowser` handles its search state locally.
- **View-mode toggle state** (grid vs. list) lives in `LibraryHomePage`; can be persisted to `sessionStorage` on the same pattern used by the health-banner and rename-toast dismissed flags in `ProfilesPage`.
- **Navigation** uses the existing `onNavigate` prop passed from `ContentArea` to pages; `LibraryHomePage` receives `onNavigate: (route: AppRoute) => void` and calls it with `'profiles'` for Edit, `'launch'` for Launch.

## KISS Assessment

**Downloads and Statistics tabs are OUT of scope.** No extension points should be added speculatively. The decision rule is:

- If the tab content would require new IPC commands, new hooks, or new data structures, do not add a stub/placeholder tab — that is speculative scaffolding that will need to be undone or maintained.
- Adding an empty `<Tabs.Trigger>` or a disabled tab is harmless UI chrome but still creates a false contract with the user; leave it out entirely until the feature exists.

The only structural choice that is low-cost and reversible is using `@radix-ui/react-tabs` for the page header (already a project dependency) so that Downloads/Statistics tabs can be added later without structural surgery. But the current scope should render a flat, tab-free page — the grid is the entire content.

## Abstraction vs. Repetition

**The cover art loading pattern already exists and should be reused directly.**

`ProfileSubTabs` and `LaunchSubTabs` share a near-identical pattern: call `useGameCoverArt`, call `useImageDominantColor`, apply `--crosshook-game-color-r/g/b` as inline styles. This pattern is repeated rather than extracted because each caller uses the result slightly differently (backdrop image vs. tab-bar tint). For `LibraryCard`, the same hooks apply again — but the result drives a card-level border/glow and a small skeleton placeholder. The repetition is acceptable: the hooks themselves are the reusable layer, and the rendering context differs enough to justify per-component composition rather than a shared wrapper.

**Do not create a `useCoverArtWithDominantColor` composite hook.** Three callsites using two hooks is not a DRY violation that warrants a new abstraction. The hooks are already tiny; a composite hook would just move complexity without reducing it.

## Interface Design

Proposed `LibraryCardProps`:

```typescript
export interface LibraryCardProps {
  /** Profile name (key and display label). */
  profileName: string;
  /** From GameProfile.steam.app_id — drives cover art + metadata. */
  steamAppId: string;
  /** From GameProfile.game.custom_cover_art_path — overrides Steam art. */
  customCoverArtPath?: string;
  /** Whether this profile is in the favorites list. */
  isFavorite: boolean;
  /** Whether this is the currently selected profile. */
  isSelected: boolean;
  /** Called when the Launch action is clicked. */
  onLaunch: (profileName: string) => void;
  /** Called when the Edit action is clicked. */
  onEdit: (profileName: string) => void;
  /** Called when the favorite toggle is clicked. */
  onToggleFavorite: (profileName: string, nextFavorite: boolean) => void;
}
```

Notes:

- `steamAppId` should be typed `string | undefined` in the final interface — the card degrades gracefully to a name-only tile when the app ID is not yet available.
- No `profile: GameProfile` prop — pass only what the card needs. Avoids coupling the card to the full type and makes future grid virtualization simpler (smaller closure per card).
- `onLaunch` and `onEdit` receive the profile name so the grid can pass a stable callback reference.
- If health-badge display is desired, add `healthStatus?: ProfileHealthStatus` as an optional prop; default to not rendering it.

## Testability Patterns

The project has no configured frontend test framework (`package.json` has no test runner). Testability guidance therefore focuses on structural isolation:

- `LibraryCard` should be pure-props-driven with no direct context consumption. This makes it renderable in isolation with `@testing-library/react` when a test framework is added.
- `LibraryGrid` should be a stateless layout component; no hooks, only props. Snapshot/visual testing requires nothing more than an array of mock props.
- `LibraryHomePage` can be integration-tested once a test framework exists by wrapping it in `ProfileProvider` with a mocked `invoke` from `@tauri-apps/api/core`.
- Search logic (the `matchesQuery` function pattern from `CommunityBrowser`) should be a pure exported utility function, not inlined in the render, so it is unit-testable independently.

## Build vs. Depend

**Virtual scrolling: do not add a dependency.**

The existing profile list in production is expected to stay under a few hundred entries. The community grid renders without virtualization. The app already compensates for WebKitGTK's sluggish scroll via `useScrollEnhance` (`src/hooks/useScrollEnhance.ts`). CSS `content-visibility: auto` on each card can provide native browser-level deferred rendering without a JS library.

`react-window` and `@tanstack/virtual` are viable if the list reaches 500+ items, but that threshold is not realistic for a personal game library. Adding either dependency now would be speculative and would complicate the card implementation (fixed-height cells, ref forwarding). The right path: **ship without virtualization, add `content-visibility: auto` to the card CSS, revisit if user reports performance issues**.

The existing `package.json` has only six production dependencies (`@radix-ui/react-select`, `@radix-ui/react-tabs`, `@tauri-apps/api`, `@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-fs`, `react-resizable-panels`). New dependencies must clear a high bar; no new dependency is needed for this feature.

## Open Questions

0. **[CRITICAL] Cover art data-loading strategy**: `useProfileContext().profiles` is a `string[]` of names only. Full `GameProfile` TOML (containing `steam.app_id` and `custom_cover_art_path`) must NOT be pre-loaded for all cards — that is N `invoke('load_profile')` calls at mount. Three options:
   - **Option A (recommended):** New `profile_list_summaries` IPC command returning `{ name, steam_app_id, custom_cover_art_path }[]`. One call, all cards render with art immediately. Requires a new Rust command in `crosshook-core`.
   - **Option B:** Lazy load on hover/focus — cards start with name only; `selectProfile(name)` is called on hover to fetch art. Cover art not visible in initial render.
   - **Option C:** Query the SQLite metadata DB for `steam_app_id` per profile name via a dedicated IPC. Fast if cached; may not include `custom_cover_art_path`.
     This decision must be made before implementation begins.
1. **Search scope**: Should search match on profile name only, or also on `game.name` from the metadata DB? The latter requires either loading metadata for all profiles on mount (N IPC calls) or a new `profile_search` backend command. This decision belongs with the feature spec, but the implementation should be designed to support either.
2. **Selected-profile sync**: Clicking "Launch" from the library should call `selectProfile(name)` before navigating to `'launch'`. Should the library card call `selectProfile` directly, or should `LibraryHomePage` coordinate this? Recommend: `onLaunch` callback in the page calls `selectProfile(name).then(() => onNavigate('launch'))`.
3. **Empty state**: When `profiles.length === 0`, show an empty state that links to profile creation. What CTA label and where does it navigate? (`'profiles'` page or the onboarding wizard?)
4. **Cover art for profiles with no Steam app ID**: `useGameCoverArt` returns `null` gracefully; the card should show a placeholder illustration using the existing skeleton/empty pattern from `GameCoverArt.tsx`.
5. **Route name**: Adding `'library'` to `AppRoute` is a breaking change to the discriminated union. Confirm the desired route key before implementation to avoid a second refactor.

## Other Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/library-home/research-business.md` — business requirements and data model analysis (produced by business-analyzer agent)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useGameCoverArt.ts` — cover art hook
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useImageDominantColor.ts` — dominant color hook
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/context/ProfileContext.tsx` — profile state context
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/variables.css` — design tokens
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css:996` — community grid pattern (reference for library grid CSS)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileSubTabs.tsx:112` — dominant-color inline-style pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/pages/LaunchPage.tsx` — example of full page using `useProfileContext`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/ContentArea.tsx` — route rendering switch to extend
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/Sidebar.tsx` — `AppRoute` type to extend
