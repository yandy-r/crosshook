# Library Home

CrossHook's UI is a single-page Tauri v2 app with a Radix UI tab-based shell where routes are a flat `AppRoute` union in `Sidebar.tsx:13`, dispatched via a `renderPage()` switch in `ContentArea.tsx` with `forceMount` on all `Tabs.Content` panels. Adding the library-home poster grid requires extending `AppRoute` with `'library'`, wiring it through `VALID_APP_ROUTES` and `ContentArea`, and creating a new `LibraryPage` component that consumes `useProfileContext()` (for `profiles`, `favoriteProfiles`, `selectProfile`, `toggleFavorite`) and a new `profile_list_summaries` IPC command to batch-load cover art metadata (`steam_app_id`, `game_name`, `custom_cover_art_path`) from TOML files. Cover art flows through a new `GameImageType::Portrait` Rust variant that tries `library_600x900_2x.jpg` → `library_600x900.jpg` → `header.jpg` on Steam CDN, cached in the existing `game_image_cache` SQLite table (schema v14, free-form `image_type` text column). The grid uses CSS Grid `auto-fill minmax(190px, 1fr)` following the existing `crosshook-community-browser__profile-grid` pattern, BEM `crosshook-library-*` classes, and the `crosshook-skeleton` shimmer for loading states.

## Relevant Files

- src/crosshook-native/src/components/layout/Sidebar.tsx: Defines `AppRoute` union type (line 13), `SIDEBAR_SECTIONS` array, `ROUTE_LABELS` record — add `'library'` to all three
- src/crosshook-native/src/App.tsx: `VALID_APP_ROUTES` record (line 19), default route `useState<AppRoute>('profiles')` (line 43) — add `library: true` and change default to `'library'`
- src/crosshook-native/src/components/layout/ContentArea.tsx: `renderPage()` switch with `never` exhaustive guard — add `case 'library': return <LibraryPage onNavigate={onNavigate} />`
- src/crosshook-native/src/components/layout/PageBanner.tsx: Per-route SVG art components — add `LibraryArt`
- src/crosshook-native/src/main.tsx: CSS import list — add `import './styles/library.css'`
- src/crosshook-native/src/context/ProfileContext.tsx: Provides `profiles`, `favoriteProfiles`, `selectProfile`, `toggleFavorite`, `refreshProfiles` — consumed directly by LibraryPage
- src/crosshook-native/src/hooks/useProfile.ts: Full profile state machine; `toggleFavorite` calls IPC then re-fetches favorites; `selectProfile` loads profile via `profile_load` IPC
- src/crosshook-native/src/hooks/useGameCoverArt.ts: Takes `steamAppId`, `customCoverArtPath`; returns `{ coverArtUrl, loading }`; hardcodes `imageType: 'cover'` at line 42 — add optional `imageType?` parameter for `'portrait'`
- src/crosshook-native/src/hooks/useImageDominantColor.ts: Canvas-based color extraction; returns `[r,g,b] | null` — used in Phase 2 for card glow
- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx: Best page pattern reference — uses `onNavigate?: (route: AppRoute) => void` prop and `await selectProfile(name)` before navigation
- src/crosshook-native/src/components/pages/InstallPage.tsx: Second `onNavigate` prop pattern reference
- src/crosshook-native/src/components/PinnedProfilesStrip.tsx: Existing favorites UI; consumes `favoriteProfiles` and `toggleFavorite(name, false)` — reference for optimistic heart toggle
- src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx: Existing consumer of `useGameCoverArt`; shows skeleton/fallback rendering pattern
- src/crosshook-native/src/styles/variables.css: All CSS custom properties; add `--crosshook-library-card-width: 190px`, `--crosshook-library-card-aspect: 3 / 4`, `--crosshook-library-grid-gap: var(--crosshook-grid-gap)`
- src/crosshook-native/src/styles/theme.css: `crosshook-skeleton` class + `crosshook-skeleton-shimmer` keyframe (~line 4738); `crosshook-community-browser__profile-grid` auto-fit grid pattern (~line 997)
- src/crosshook-native/src/types/profile.ts: `GameProfile` TypeScript interface — `steam.app_id`, `game.name`, `game.custom_cover_art_path`
- src/crosshook-native/crates/crosshook-core/src/game_images/models.rs: `GameImageType` enum (Cover, Hero, Capsule) — add `Portrait` variant
- src/crosshook-native/crates/crosshook-core/src/game_images/client.rs: `build_download_url()` maps `GameImageType` to CDN URL (line 334); `download_and_cache_image()` cache pipeline; `filename_for()` — add `Portrait` arms
- src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs: SteamGridDB client; `build_endpoint()` maps image type to API params — add `Portrait` arm with `dimensions=342x482,600x900`
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: `ProfileStore` struct with `list()` and `load()` methods — `profile_list_summaries` iterates `list()` then `load()` for each
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile` Rust struct (TOML source of truth)
- src/crosshook-native/src-tauri/src/commands/profile.rs: All `profile_*` IPC commands — add `profile_list_summaries` here
- src/crosshook-native/src-tauri/src/commands/game_metadata.rs: `fetch_game_cover_art` command; maps string to `GameImageType` — add `"portrait"` match arm
- src/crosshook-native/src-tauri/src/lib.rs: `invoke_handler` registration (line 189) — register `profile_list_summaries`

## Relevant Tables

- profiles: `current_filename` (profile name), `is_favorite` (0/1), `game_name` (denormalized, nullable); does NOT store `steam_app_id` — must read from TOML
- game_image_cache: Keyed on `(steam_app_id, image_type, source)`; `image_type` is free-form text — `'portrait'` works without migration; stores `file_path`, `mime_type`, `expires_at` (24h TTL)

## Relevant Patterns

**Page Component Pattern**: Pages are thin orchestrators consuming contexts via hooks. `LibraryPage` follows `HealthDashboardPage` — receives `onNavigate?: (route: AppRoute) => void`, destructures from `useProfileContext()`, and always calls `await selectProfile(name)` before `onNavigate?.()`. See [src/crosshook-native/src/components/pages/HealthDashboardPage.tsx](src/crosshook-native/src/components/pages/HealthDashboardPage.tsx).

**Route Registration Pattern**: Three-file wiring — `Sidebar.tsx` (type union + sections), `App.tsx` (`VALID_APP_ROUTES` + default state), `ContentArea.tsx` (switch case + import). TypeScript `never` exhaustive guard enforces completeness. See [src/crosshook-native/src/components/layout/ContentArea.tsx](src/crosshook-native/src/components/layout/ContentArea.tsx).

**CSS Grid Auto-Fill Pattern**: Responsive grids use `repeat(auto-fill, minmax(var(--token), 1fr))` with CSS variables and BEM classes. See [src/crosshook-native/src/styles/theme.css](src/crosshook-native/src/styles/theme.css) at the `crosshook-community-browser__profile-grid` rule (~line 997).

**Cover Art Hook Pattern**: `useGameCoverArt` wraps `invoke('fetch_game_cover_art')` with `requestIdRef` race-condition guard, returns `{ coverArtUrl, loading }`, handles custom path priority. See [src/crosshook-native/src/hooks/useGameCoverArt.ts](src/crosshook-native/src/hooks/useGameCoverArt.ts).

**Dominant Color CSS Variable Pattern**: `useImageDominantColor` returns `[r,g,b]`; set as `--crosshook-game-color-r/g/b` CSS custom properties via inline style. CSS reads them with `rgba(var(--crosshook-game-color-r), ...)`. Used in ProfileSubTabs, LaunchSubTabs, UpdateGamePanel, InstallGamePanel. See [src/crosshook-native/src/components/ProfileSubTabs.tsx:114](src/crosshook-native/src/components/ProfileSubTabs.tsx).

**Skeleton Loading Pattern**: `crosshook-skeleton` class + `crosshook-skeleton-shimmer` keyframe in theme.css. Apply class directly to placeholder `<div>` while `loading` is true. See [src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx](src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx).

**IPC Command Pattern**: Rust commands return `Result<T, String>`, use `State<'_, Store>` for managed deps, register in `lib.rs` `invoke_handler`. Synchronous for local I/O; async only for network. Frontend uses `invoke<ReturnType>(name, params)`. See [src/crosshook-native/src-tauri/src/commands/profile.rs](src/crosshook-native/src-tauri/src/commands/profile.rs).

**Component Decomposition Rule**: `LibraryCard` must be pure-props-driven (no context access); `LibraryGrid` must be stateless layout-only; `LibraryPage` owns all state and passes down through props.

## Relevant Docs

**docs/plans/library-home/feature-spec.md**: You _must_ read this before any implementation. Contains all resolved decisions, component tree, data models, IPC contracts, CSS variables, phasing, persistence classification, and security considerations.

**docs/plans/library-home/research-technical.md**: You _must_ read this when implementing Rust changes (Portrait variant, profile_list_summaries). Contains exact file locations, line numbers, Rust code samples, and confirmed architectural decisions.

**docs/plans/library-home/research-patterns.md**: You _must_ read this when creating React components. Contains page component patterns, CSS conventions, hook architecture, IPC invocation patterns, and skeleton loading examples.

**docs/plans/library-home/research-ux.md**: You _must_ read this when implementing the card UI. Contains card design spec (gradient scrim WCAG values, glass morphism CSS, hover-reveal pattern), skeleton loading approach, empty-state design, and competitive analysis.

**docs/plans/library-home/research-integration.md**: You _must_ read this when working on IPC commands or database queries. Contains exact Rust signatures, SQLite schema (v14 confirmed), cover art pipeline data flow, and portrait URL fallback chain.

**AGENTS.md**: Reference for stack overview, directory map, and naming conventions when deciding where to place new files.

**CLAUDE.md**: Reference for IPC naming (`snake_case`), Serde requirements, and commit conventions.
