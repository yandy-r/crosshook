# Architecture Research: Library Home

## System Overview

CrossHook's UI is a single-page Tauri v2 app built on `@radix-ui/react-tabs`. Navigation state (`route`) lives in `AppShell` (`App.tsx:43`) as `useState<AppRoute>('profiles')`. The `AppRoute` union type is the single source of truth for all valid routes, defined in `Sidebar.tsx:13`. Adding `'library'` requires changes to exactly four files: `Sidebar.tsx`, `App.tsx`, `ContentArea.tsx`, and `main.tsx` (CSS import). The default route at `App.tsx:43` must be changed from `'profiles'` to `'library'`. All page-level context (`ProfileContext`) is provided at the `App` level, so `LibraryPage` can call `useProfileContext()` directly with no prop drilling.

---

## Relevant Files

### Frontend — Routing & Layout

- `src/crosshook-native/src/App.tsx` — owns `route` state (`useState<AppRoute>('profiles')`), `VALID_APP_ROUTES` record, `AppShell` component tree; wraps `ProfileProvider` → `ProfileHealthProvider` → `AppShell`
- `src/crosshook-native/src/components/layout/Sidebar.tsx` — defines `AppRoute` union type (line 13), `SIDEBAR_SECTIONS` data array, exports `AppRoute` for all other consumers
- `src/crosshook-native/src/components/layout/ContentArea.tsx` — `renderPage()` switch statement with `never` exhaustiveness check; `Tabs.Content` uses `forceMount: true` (all pages stay mounted)
- `src/crosshook-native/src/components/layout/PageBanner.tsx` — exports one SVG art component per route (`ProfilesArt`, `LaunchArt`, `InstallArt`, etc.)
- `src/crosshook-native/src/components/layout/PanelRouteDecor.tsx` — wrapper for per-route decorative illustration (used inside page root components)
- `src/crosshook-native/src/main.tsx` — CSS entry point; imports all `./styles/*.css` files

### Frontend — Context & Hooks

- `src/crosshook-native/src/context/ProfileContext.tsx` — thin context wrapper over `useProfile`; adds `launchMethod`, `steamClientInstallPath`, `targetHomePath` derived values; listens for `auto-load-profile` Tauri event
- `src/crosshook-native/src/hooks/useProfile.ts` — full state machine for profile CRUD; exports `profiles: string[]`, `favoriteProfiles: string[]`, `selectedProfile`, `profile: GameProfile`, `selectProfile()`, `toggleFavorite()`, `refreshProfiles()`
- `src/crosshook-native/src/hooks/useGameCoverArt.ts` — takes `steamAppId?: string`, `customCoverArtPath?: string`; returns `{ coverArtUrl: string | null, loading: boolean }`; priority: custom path → IPC fetch
- `src/crosshook-native/src/hooks/useImageDominantColor.ts` — Phase 2 hook for card glow; already exists

### Frontend — Existing Pages (Pattern Reference)

- `src/crosshook-native/src/components/pages/LaunchPage.tsx` — page that calls `useProfileContext()` directly; no `onNavigate` needed; wraps `crosshook-page-scroll-shell` CSS
- `src/crosshook-native/src/components/pages/InstallPage.tsx` — pattern example of `onNavigate?: (route: AppRoute) => void` prop
- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx` — another `onNavigate` prop example
- `src/crosshook-native/src/components/PinnedProfilesStrip.tsx` — existing favorites UI; consumes `favoriteProfiles: string[]` and `toggleFavorite(name, false)`

### Frontend — CSS

- `src/crosshook-native/src/styles/variables.css` — all CSS custom properties; `--crosshook-grid-gap` (responsive: 20/16/14/12px), `--crosshook-touch-target-min` (48px; 56px in controller mode), `--crosshook-community-profile-grid-min` (280px)
- `src/crosshook-native/src/styles/theme.css` — `crosshook-skeleton` class + `crosshook-skeleton-shimmer` keyframe at line ~4736; `crosshook-community-browser__profile-grid` auto-fit grid pattern at line ~996

### Rust — IPC Commands

- `src/crosshook-native/src-tauri/src/lib.rs` — `invoke_handler` macro; where `profile_list_summaries` must be registered
- `src/crosshook-native/src-tauri/src/commands/profile.rs` — `profile_list`, `profile_load`, `profile_set_favorite`, `profile_list_favorites`; add `profile_list_summaries` here
- `src/crosshook-native/src-tauri/src/commands/game_metadata.rs` — `fetch_game_cover_art` command; maps string → `GameImageType`; needs `"portrait"` match arm

### Rust — Core Library

- `src/crosshook-native/crates/crosshook-core/src/game_images/models.rs` — `GameImageType` enum: `Cover`, `Hero`, `Capsule`; add `Portrait` here
- `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs` — `build_download_url()` fn (line 334): maps `GameImageType` to CDN URL; `Cover` → `header.jpg` (landscape); add `Portrait` arm with `library_600x900_2x.jpg` → `library_600x900.jpg` → `header.jpg` fallback chain
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — `GameProfile` struct (source of truth for TOML schema; `steam.app_id`, `game.name`, `game.custom_cover_art_path` are the fields `ProfileSummary` needs)

---

## Data Flow

### Navigation Flow

```
AppShell (App.tsx)
  useState<AppRoute>('profiles')  ← change to 'library'
    │
    ├─ Sidebar: Tabs.Trigger onClick → setRoute(route)
    └─ ContentArea: renderPage() switch → Tabs.Content forceMount=true
         case 'library': return <LibraryPage onNavigate={setRoute} />
```

`onNavigate` is `setRoute` — a plain `Dispatch<SetStateAction<AppRoute>>`. Pages that only navigate away (never receive navigation) don't need this prop (e.g., `ProfilesPage`, `LaunchPage` today). `LibraryPage` needs it to fire `onNavigate('launch')` and `onNavigate('profiles')` after `await selectProfile(name)`.

### Profile Data Flow

```
ProfileProvider (App.tsx)
  └─ useProfile() → ProfileContext
       profiles: string[]          ← from profile_list IPC
       favoriteProfiles: string[]  ← from profile_list_favorites IPC
       selectProfile(name)         ← profile_load IPC + setState
       toggleFavorite(name, bool)  ← profile_set_favorite IPC
       refreshProfiles()           ← profile_list IPC re-fetch
```

`LibraryPage` needs `profiles`, `favoriteProfiles`, `selectProfile`, `toggleFavorite`, `refreshProfiles` — all available from `useProfileContext()`.

**Critical gap**: `profiles: string[]` contains names only. Cover art metadata (`steam_app_id`, `game_name`, `custom_cover_art_path`) requires a TOML read. Solution: new `profile_list_summaries` IPC returns `Vec<ProfileSummary>` in one batch call.

### Cover Art Pipeline

```
LibraryCard
  useGameCoverArt(steamAppId, customCoverArtPath)
    │
    ├─ customCoverArtPath non-empty?
    │    └─ convertFileSrc(path) → asset:// URL  (no IPC)
    │
    └─ steamAppId non-empty?
         invoke('fetch_game_cover_art', { appId, imageType: 'portrait' })
           └─ commands/game_metadata.rs → download_and_cache_image(store, appId, GameImageType::Portrait, api_key)
                └─ Check MetadataStore cache → SteamGridDB → Steam CDN (library_600x900_2x.jpg)
                     → Steam CDN (library_600x900.jpg) → header.jpg fallback
                Returns absolute file path → convertFileSrc → asset:// URL
```

**Important**: `useGameCoverArt` currently hardcodes `imageType: 'cover'`. For portrait art, this must be changed to `'portrait'` (after the new `Portrait` variant and `"portrait"` match arm are added in Rust).

---

## Integration Points

### Adding `'library'` to the Route System

| File | Change | Location |
|------|--------|----------|
| `Sidebar.tsx` | Add `'library'` to `AppRoute` union; add item to `SIDEBAR_SECTIONS` (new "Game" section entry or before "Profiles"); import `LibraryIcon` from `SidebarIcons.tsx` | Line 13 (union), line 33 (sections array) |
| `App.tsx` | Add `library: true` to `VALID_APP_ROUTES`; change default `useState<AppRoute>('profiles')` to `'library'` | Lines 19–27, line 43 |
| `ContentArea.tsx` | Add `case 'library': return <LibraryPage onNavigate={onNavigate} />;` in `renderPage()` | Lines 34–54 |
| `main.tsx` | Add `import './styles/library.css';` | After existing style imports |
| `PageBanner.tsx` | Export `LibraryArt` SVG component | New export function |
| `variables.css` | Add `--crosshook-library-card-width: 190px`, `--crosshook-library-card-aspect: 3 / 4`, `--crosshook-library-grid-gap: var(--crosshook-grid-gap)` | After existing vars |

### Adding `GameImageType::Portrait` in Rust

Four files require changes — the enum definition, two URL builders (CDN and SteamGridDB), and the IPC dispatch:

| File | Change |
|------|--------|
| `crates/crosshook-core/src/game_images/models.rs` | Add `Portrait` variant to `GameImageType` enum; update `Display` impl (`"portrait"` string) |
| `crates/crosshook-core/src/game_images/client.rs` | Add `Portrait` arm in `build_download_url()`: try `library_600x900_2x.jpg` → `library_600x900.jpg` → `header.jpg`; update `filename_for()` |
| `crates/crosshook-core/src/game_images/steamgriddb.rs` | Add `Portrait` arm in `build_endpoint()` (line 101): `("grids", Some("600x900"))` — requests 600×900 portrait grids from SteamGridDB v2 API |
| `src-tauri/src/commands/game_metadata.rs` | Add `"portrait" => GameImageType::Portrait` in match arm (currently handles `"hero"`, `"capsule"`, default `Cover`) |

**SteamGridDB `build_endpoint` detail**: The existing `Capsule` variant already uses `"grids"` with `dimensions=342x482,600x900`. `Portrait` should use `("grids", Some("600x900"))` to request portrait-format grids specifically. The `None` dimensions branch omits the query param (used only by `Hero`).

### SQLite Schema Note

Current schema is **v14** (feature-spec.md states v13 — outdated). The `game_image_cache` table's `image_type` column is free-form text. The `portrait` string value requires no migration — it just works once the Rust `GameImageType::Portrait` variant serializes to `"portrait"`.

### Adding `profile_list_summaries` in Rust

| File | Change |
|------|--------|
| `src-tauri/src/commands/profile.rs` | Add `ProfileSummary` struct (Serialize + Deserialize) and `profile_list_summaries` command function |
| `src-tauri/src/lib.rs` | Register `commands::profile::profile_list_summaries` in `invoke_handler` |

Pattern from existing `profile_list`:
```rust
#[tauri::command]
pub fn profile_list(store: State<'_, ProfileStore>) -> Result<Vec<String>, String> {
    store.list().map_err(map_error)
}
```

`profile_list_summaries` will call `store.list()` → for each name, `store.load(name)` → extract `ProfileSummary` fields → return `Vec<ProfileSummary>`.

---

## Key Dependencies

### Context Hierarchy (App.tsx)

```
<App>                           (gamepad nav, scroll enhance)
  <ProfileProvider>             (profile CRUD, selection, favorites)
    <ProfileHealthProvider>     (health snapshots)
      <AppShell>                (route state, onboarding)
        <PreferencesProvider>   (settings, recent files)
          <LaunchStateProvider>
            <Sidebar />
            <ContentArea />     ← LibraryPage rendered here
```

`LibraryPage` is inside `ProfileProvider` and `ProfileHealthProvider` — can call `useProfileContext()` and `useProfileHealthContext()` directly.

### Type Dependencies

- `AppRoute` — exported from `Sidebar.tsx`; imported in `App.tsx`, `ContentArea.tsx`, `InstallPage.tsx`, `HealthDashboardPage.tsx`
- `GameProfile` — defined in `types/profile.ts`; `game.custom_cover_art_path` is `string | undefined`; `steam.app_id` is `string` (empty string when not a Steam game)
- `UseProfileResult` — exported from `hooks/useProfile.ts`; `ProfileContextValue extends UseProfileResult`

### Skeleton CSS Variables (already defined in theme.css)

- `--crosshook-skeleton-color-from` / `--crosshook-skeleton-color-to` — referenced in `.crosshook-skeleton`
- `crosshook-skeleton-shimmer` keyframe — available for card placeholder animation
- No new keyframes or colors needed for Phase 1 skeleton states

### CSS Grid Precedent (`theme.css:996`)

```css
.crosshook-community-browser__profile-grid {
  grid-template-columns: repeat(auto-fit, minmax(var(--crosshook-community-profile-grid-min), 1fr));
}
```

Library grid uses the same pattern with `auto-fill` instead of `auto-fit` and `--crosshook-library-card-width` (190px) instead of community's 280px.

### `forceMount` Behavior

`ContentArea` renders all `Tabs.Content` elements with `forceMount: true` (line 31). `LibraryPage` stays mounted while on other routes. Gate any `route === 'library'`-specific effects if needed to avoid background IPC calls.
