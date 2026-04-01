# Library Home — Analysis Context

Synthesized from `shared.md` + `feature-spec.md`. Skim research files only for implementation-specific details.

---

## Executive Summary

Library Home adds a Steam-style poster grid as CrossHook's **default landing page** (`'library'` replaces `'profiles'` in `App.tsx`). Each profile renders as a 190×253px (3:4) card showing cover art, game title, and three actions: Launch, Heart, Edit. Phase 1 delivers a fully functional grid with **zero new npm dependencies** and one new Rust IPC command (`profile_list_summaries`). Architecture is entirely additive — new route, new components, one new Rust variant — with no changes to existing IPC contracts or data models.

---

## Architecture Context

```
LibraryPage (new default route)
├── LibraryToolbar     — search input + grid/list toggle (client-state only)
└── LibraryGrid        — CSS Grid auto-fill, stateless layout
    └── LibraryCard ×N — pure props; useGameCoverArt + useImageDominantColor (Phase 2)

Data flow:
  ProfileContext.profiles (string[])
      ↓  profile_list_summaries IPC (one round-trip, reads TOMLs server-side)
  LibraryCardData[]  {name, gameName, steamAppId, customCoverArtPath, isFavorite}
      ↓  per card
  useGameCoverArt → fetch_game_cover_art → disk cache → asset:// URL
```

**Critical architectural fact**: `profiles: string[]` in ProfileContext contains **names only**. Cover art requires `steam.app_id` and `custom_cover_art_path` from TOML — `profile_list_summaries` IPC solves this in a single batch call. Without it, N `profile_load` calls would fire on mount.

**Component decomposition rule (enforced)**:

- `LibraryCard` — pure props, no context
- `LibraryGrid` — stateless layout only
- `LibraryPage` — all state, passes down via props

---

## Critical Files Reference

### Files to Create (Phase 1)

| File                                                             | Role                                                                      |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx`      | Page root; owns search state, view mode; calls `refreshProfiles` on mount |
| `src/crosshook-native/src/components/library/LibraryCard.tsx`    | Poster card; pure props; calls `useGameCoverArt`; fires action callbacks  |
| `src/crosshook-native/src/components/library/LibraryGrid.tsx`    | CSS Grid layout; maps `LibraryCardData[]` to cards                        |
| `src/crosshook-native/src/components/library/LibraryToolbar.tsx` | Search `<input>` + view toggle                                            |
| `src/crosshook-native/src/hooks/useLibraryProfiles.ts`           | Pure filter/sort over profiles + favorites; no IPC                        |
| `src/crosshook-native/src/styles/library.css`                    | All BEM `crosshook-library-*` styles                                      |
| `src/crosshook-native/src/types/library.ts`                      | `LibraryCardData`, `LibraryViewMode` types                                |

### Files to Modify (Phase 1)

| File                                                                       | Change                                                                                                                                               |
| -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/layout/Sidebar.tsx:13`                | Add `'library'` to `AppRoute` union + `SIDEBAR_SECTIONS` + `ROUTE_LABELS`                                                                            |
| `src/crosshook-native/src/App.tsx:19,43`                                   | Add `library: true` to `VALID_APP_ROUTES`; change default `useState<AppRoute>` to `'library'`                                                        |
| `src/crosshook-native/src/components/layout/ContentArea.tsx:35`            | Add `case 'library': return <LibraryPage onNavigate={onNavigate} />`                                                                                 |
| `src/crosshook-native/src/components/layout/PageBanner.tsx`                | Add `LibraryArt` SVG component                                                                                                                       |
| `src/crosshook-native/src/styles/variables.css`                            | Add `--crosshook-library-card-width: 190px`, `--crosshook-library-card-aspect: 3 / 4`, `--crosshook-library-grid-gap`                                |
| `src/crosshook-native/src/main.tsx`                                        | Add `import './styles/library.css'`                                                                                                                  |
| `src/crosshook-native/src-tauri/src/commands/profile.rs`                   | Add `profile_list_summaries` command                                                                                                                 |
| `src/crosshook-native/src-tauri/src/lib.rs:189`                            | Register `profile_list_summaries` in `invoke_handler`                                                                                                |
| `src/crosshook-native/crates/crosshook-core/src/game_images/models.rs`     | Add `Portrait` to `GameImageType` enum                                                                                                               |
| `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs:334` | Add `Portrait` arms to `filename_for`; add new portrait CDN helper (see Cross-Cutting Concerns #5); **do not change `build_download_url` signature** |

### Key Reference Files (read-only)

| File                                                                    | Why                                                                                           |
| ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`     | **Best page pattern reference** — `onNavigate` prop, `await selectProfile` before navigate    |
| `src/crosshook-native/src/components/PinnedProfilesStrip.tsx`           | Optimistic heart toggle reference (`toggleFavorite(name, false)`)                             |
| `src/crosshook-native/src/hooks/useGameCoverArt.ts:42`                  | `imageType` param to extend to `'portrait'`; `requestIdRef` race guard pattern                |
| `src/crosshook-native/src/styles/theme.css:997,4738`                    | Grid auto-fill pattern + skeleton shimmer keyframe                                            |
| `src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx` | Skeleton/fallback render pattern using `useGameCoverArt`                                      |
| `src/crosshook-native/src/context/ProfileContext.tsx`                   | Provides `profiles`, `favoriteProfiles`, `selectProfile`, `toggleFavorite`, `refreshProfiles` |

---

## Patterns to Follow

- **Page pattern**: Thin orchestrator consuming context hooks. Receives `onNavigate?: (route: AppRoute) => void`. Always `await selectProfile(name)` before `onNavigate()`. See `HealthDashboardPage.tsx`.

- **Route wiring**: Three-file change — `Sidebar.tsx` (type union + nav item), `App.tsx` (`VALID_APP_ROUTES` + default state), `ContentArea.tsx` (switch case + import). TypeScript `never` guard enforces all cases handled.

- **CSS Grid**: `repeat(auto-fill, minmax(var(--crosshook-library-card-width), 1fr))` with CSS variable tokens. See `crosshook-community-browser__profile-grid` in `theme.css:997`.

- **Cover art hook**: `useGameCoverArt(steamAppId, customCoverArtPath)` returns `{ coverArtUrl, loading }`. Currently hardcodes `imageType: 'cover'` at line 42 — needs optional `imageType?` parameter for `'portrait'`.

- **Skeleton loading**: Apply `crosshook-skeleton` class to placeholder `<div>` while `loading` is true. Keyframe `crosshook-skeleton-shimmer` already in `theme.css:4738`.

- **IPC command**: `Result<T, String>`, `State<'_, Store>`, `snake_case` name, register in `lib.rs invoke_handler`. Frontend: `invoke<T>(name, params)`.

- **Dominant color** (Phase 2): `useImageDominantColor(coverArtUrl)` → `[r,g,b] | null` → set as `--crosshook-game-color-r/g/b` inline style vars. See `ProfileSubTabs.tsx:114`.

- **BEM naming**: All new classes use `crosshook-library-*` prefix.

---

## Cross-Cutting Concerns

1. **Navigation race (critical)**: `onNavigate` must fire only after `selectProfile` resolves. Pattern: `await selectProfile(name)` → `onNavigate(route)`. Missing the `await` causes the target page to render with stale/null profile.

2. **Default route change**: `App.tsx:43` changes `useState<AppRoute>('profiles')` → `'library'`. OnboardingWizard is event-driven (Tauri event), unaffected by route.

3. **forceMount**: `ContentArea` uses `forceMount` on all `Tabs.Content` — `LibraryPage` stays mounted when inactive. Gate effects on `route === 'library'` if interval/polling is ever added.

4. **Favorites terminology mismatch**: IPC/SQLite = `is_favorite`; `PinnedProfilesStrip` = "pinned/star"; LibraryHome = "heart". All three write the same field. No data model changes needed — visual difference only.

5. **Portrait cover art Rust — higher complexity than a simple match arm**: `build_download_url` (line 334) returns a single `String` and is called at **3 sites** in `client.rs` (lines 213, 230, 289 for CDN fallback, direct CDN, and DB metadata storage). Changing its return type breaks all three callers. Recommended approach: add a private `try_download_portrait_from_cdn(app_id) -> Result<(Vec<u8>, String), GameImageError>` helper that iterates candidate URLs (`library_600x900_2x.jpg` → `library_600x900.jpg` → `header.jpg`) and returns `(bytes, used_url)` on first success. In `download_and_cache_image`, branch on `Portrait` to call this helper; use the returned `used_url` as the `download_url` stored in DB step (g). `build_download_url` stays untouched — Portrait never hits it. Existing `Cover` type unchanged.

6. **Security: `custom_cover_art_path`**: Path passed to `convertFileSrc` without scope enforcement. Validate resolves inside known safe dirs or broker via IPC. Asset protocol scope is currently limited to cache dir.

7. **`game_image_cache` table**: Keyed `(steam_app_id, image_type, source)` with `image_type` as free-form text. Adding `'portrait'` works without any schema migration.

---

## Parallelization Opportunities

Phase 1 can be split into **four parallel streams** with one sequential dependency gate:

| Stream                   | Tasks                                                                                                       | Gate                                                                      |
| ------------------------ | ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| **A — Route Wiring**     | Sidebar.tsx + App.tsx + ContentArea.tsx + PageBanner.tsx                                                    | Independent; start immediately                                            |
| **B — Types + CSS**      | `types/library.ts` + `variables.css` additions + `library.css` scaffold                                     | Independent; start immediately                                            |
| **C — Rust**             | `GameImageType::Portrait` variant + `client.rs` arms + `profile_list_summaries` IPC + `lib.rs` registration | Independent; start immediately                                            |
| **D — React Components** | LibraryPage + LibraryCard + LibraryGrid + LibraryToolbar + useLibraryProfiles                               | **Blocked by B** (needs types); can start after `types/library.ts` exists |

Stream D can further parallelize internally:

- `LibraryCard` + `LibraryToolbar` + `useLibraryProfiles` — all depend on types, no mutual deps
- `LibraryGrid` — needs `LibraryCard` interface
- `LibraryPage` — needs all other components

---

## Implementation Constraints

- **Zero new npm deps** for Phase 1 (all infrastructure exists)
- **TypeScript strict mode** — no `any`; define `LibraryCardData` before writing components
- **CSS**: All tokens in `variables.css`; all rules in `library.css`; BEM `crosshook-library-*`
- **Rust**: `snake_case` IPC names; `Result<T, String>`; Serde on all IPC boundary types
- **`profile_list_summaries`**: Returns `Vec<ProfileSummary>` — DTO with `name`, `game_name`, `steam_app_id`, `custom_cover_art_path`. Async (TOML file I/O). Does NOT replace `profile_load` — just aggregates cover art metadata cheaply.
- **Playtime**: Omit entirely — no backend tracking, no placeholder "0h" values
- **View mode persistence**: `localStorage` only (not SQLite) for MVP
- **Search**: Pure client-side; no IPC; O(n) substring match on profile name
- **Favorites sort**: Mixed alphabetical for Phase 1 — no pinning to top

---

## Key Recommendations

1. **Start with types (`library.ts`) first** — unblocks all component work across the team.
2. **Route wiring + Rust can run in parallel with types** — no inter-dependency.
3. **`profile_list_summaries` is ~20 lines of Rust** — mirrors `profile_list` but calls `load()` per entry and maps to slim DTO.
4. **Reuse `crosshook-skeleton-shimmer` verbatim** — no new CSS animation needed; just apply the class.
5. **`useGameCoverArt` param extension is minimal** — add `imageType?: string = 'cover'` and pass through to `fetch_game_cover_art` IPC. No internal logic change.
6. **`IntersectionObserver` in LibraryCard** defers `useGameCoverArt` invocation until card enters viewport — prevents N concurrent fetches on large libraries. Not required for Phase 1 correctness but strongly recommended.
7. **`await selectProfile` before every `onNavigate`** — non-negotiable; see `HealthDashboardPage` for exact pattern.
