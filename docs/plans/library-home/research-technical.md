# Technical Research: Library-Home Feature

## Executive Summary

CrossHook already has all necessary backend infrastructure for a library-home poster grid: `profile_list`, `profile_list_favorites`, `profile_set_favorite`, and `fetch_game_cover_art` IPC commands exist and are wired to the SQLite metadata DB (schema v13). The `useGameCoverArt` hook handles async cover-art fetching with cache-busting and cancellation. The main work is a new `library` route + page, three new components (`LibraryCard`, `LibraryGrid`, `LibraryToolbar`), and a new CSS file.

**One confirmed backend change is required**: `GameImageType::Cover` maps to `header.jpg` (460×215 landscape) in both the Steam CDN and SteamGridDB paths. The library grid needs portrait art. A new `GameImageType::Portrait` variant must be added to `game_images/models.rs` and `client.rs` — changing `Cover` would break the existing profile editor's landscape banner. The optional `profile_list_summaries` command is a performance optimization, not a blocker.

Playtime display is deferred — it does not exist in any current surface.

---

## Architecture Design

### Route integration

Routes are a flat union in `Sidebar.tsx` (`AppRoute` type) and dispatched in `ContentArea.tsx`'s `renderPage()` switch. Adding `'library'` requires:

1. Extend `AppRoute` in `src/crosshook-native/src/components/layout/Sidebar.tsx:13`
2. Add `library: true` to `VALID_APP_ROUTES` in `src/crosshook-native/src/App.tsx:19`
3. Add `case 'library': return <LibraryPage onNavigate={onNavigate} />;` in `ContentArea.tsx:35`'s `renderPage()`
4. Add a sidebar item (icon + label) to the **Game** section in `SIDEBAR_SECTIONS` in `Sidebar.tsx:33`

The `onNavigate` prop is already threaded from `App` → `ContentArea` → page components, enabling cross-page navigation from card actions.

### Component tree

```
LibraryPage                            (new page component)
├── LibraryToolbar                     (search input + grid/list toggle)
└── LibraryGrid                        (grid layout)
    └── LibraryCard (×N)               (one per profile)
        ├── cover art <img> / skeleton
        ├── gradient overlay
        ├── title
        └── action buttons: Launch / Edit / Heart
```

All components live under `src/crosshook-native/src/components/pages/` (LibraryPage) and a new `src/crosshook-native/src/components/library/` subdirectory.

### Data flow

1. `LibraryPage` reads `profiles`, `favoriteProfiles`, `selectProfile`, and `toggleFavorite` from `useProfileContext()`. `refreshProfiles()` is called on mount to ensure freshness.
2. A `useLibraryProfiles` hook filters `profiles[]` (strings) by the search query and sorts favorites-first, then alphabetically.
3. Each `LibraryCard` receives `LibraryCardData` props (name, gameName, steamAppId, customCoverArtPath, isFavorite) and fires callbacks (`onLaunch`, `onEdit`, `onToggleFavorite`).
4. Cover art is fetched **per card** via `useGameCoverArt(steamAppId, customCoverArtPath)`, passing `imageType: 'portrait'` (new variant — see API Design). To populate `steamAppId` and `gameName`, the page needs profile data beyond the names array. Two approaches:
   - **Option A (recommended)**: New `profile_list_summaries` Rust command returns `{name, game_name, steam_app_id, custom_cover_art_path}[]` in a single IPC call. Reads TOML files server-side — authoritative, always fresh.
   - **Option B (MVP shortcut)**: Each `LibraryCard` calls `profile_load` on mount. Zero new backend code beyond the image type change; causes N IPC calls on first render. Acceptable for libraries under ~50 profiles.
5. `onLaunch(name)`: calls `selectProfile(name)` then `onNavigate('launch')`. The Launch page handles all IPC — the library card never triggers a launch directly.
6. `onEdit(name)`: calls `selectProfile(name)` then `onNavigate('profiles')`.
7. `onToggleFavorite(name, current)`: calls `toggleFavorite(name, !current)`.

### Navigation contract

`LibraryPage` receives `onNavigate?: (route: AppRoute) => void` as a prop — the same pattern used by `InstallPage` (`ContentArea.tsx:41`) and `HealthDashboardPage` (`ContentArea.tsx:49`). The handler is already plumbed from `App.tsx`. No new prop threading is needed.

### Profile activation model

`selectProfile(name)` (alias for `loadProfile` in `useProfile.ts`) is the single entry point for activating a profile. It invokes `profile_load` IPC, sets `selectedProfile` in `ProfileContext`, and syncs `last_used_profile` in app settings. Both Launch and Edit actions must call this before navigating. No bespoke load logic belongs in library components.

---

## Data Models

### Existing — no schema changes needed for MVP

The `profiles` table in the SQLite metadata DB already has:

| Column             | Type      | Notes                     |
| ------------------ | --------- | ------------------------- |
| `profile_id`       | TEXT PK   | UUID                      |
| `current_filename` | TEXT      | profile name (no `.toml`) |
| `game_name`        | TEXT NULL | denormalized from TOML    |
| `is_favorite`      | INTEGER   | 0/1                       |
| `updated_at`       | TEXT      | ISO-8601                  |

The `launch_operations` table has `started_at`, `finished_at`, `profile_name`, and `status`. A playtime aggregate is possible but not required for MVP — no playtime surface exists anywhere in the app today.

### Frontend card data shape

```typescript
// New type — lives in src/crosshook-native/src/types/library.ts
export type LibraryViewMode = 'grid' | 'list';

export interface LibraryCardData {
  /** Profile filename (no extension), used as the stable React key. */
  name: string;
  /** From profile.game.name — may be empty string if not set. */
  gameName: string;
  /** From profile.steam.app_id — drives useGameCoverArt. */
  steamAppId: string;
  /** From profile.game.custom_cover_art_path — takes priority in useGameCoverArt. */
  customCoverArtPath?: string;
  /** Derived from ProfileContext.favoriteProfiles[]. */
  isFavorite: boolean;
  /** Optional — populated only once playtime tracking is implemented. */
  playtimeSeconds?: number;
}
```

### Search state

Local component state (`useState<string>('')`) inside `LibraryPage`. Ephemeral — resets on route change. Filter is case-insensitive substring match on `name` and `gameName`.

### View mode state

Local state `useState<LibraryViewMode>('grid')`. Ephemeral for MVP. Can be persisted to `localStorage` or `PreferencesContext` in a follow-up without interface changes.

### Favorites storage

Favorites live in `profiles.is_favorite` (SQLite metadata DB). `ProfileContext.favoriteProfiles` is the live frontend view. `toggleFavorite(name, bool)` writes via `profile_set_favorite` IPC and reloads `favoriteProfiles` — no local state management needed in library components.

### Playtime (deferred — no placeholder needed)

Omit the playtime field entirely from the initial implementation rather than showing "0 hrs" — there is no launch tracking surface in the current UI to set user expectations. Add `playtimeSeconds?` as an optional typed field so the component interface requires no breaking change when the feature lands.

---

## API Design

### Existing commands — used as-is, no changes

| IPC name                 | Signature                                     | Notes                                                                      |
| ------------------------ | --------------------------------------------- | -------------------------------------------------------------------------- |
| `profile_list`           | `() → Vec<String>`                            | Called via `refreshProfiles()` on mount                                    |
| `profile_list_favorites` | `() → Vec<String>`                            | Read from `ProfileContext.favoriteProfiles`                                |
| `profile_set_favorite`   | `{ name, favorite } → ()`                     | Called via `toggleFavorite()`                                              |
| `profile_load`           | `{ name } → GameProfile`                      | Used for navigation activation via `selectProfile()`                       |
| `fetch_game_cover_art`   | `{ appId, imageType? } → Option<String>` path | Used by existing `useGameCoverArt` hook — **pass `imageType: 'portrait'`** |

### Required backend change — new `GameImageType::Portrait` variant

**This is a required change, not optional.** Confirmed by reading `client.rs:336–340` and `steamgriddb.rs:102–104`:

- `GameImageType::Cover` → Steam CDN `header.jpg` (460×215 landscape) + SteamGridDB `dimensions=460x215,920x430` (landscape)
- `GameImageType::Capsule` → Steam CDN `capsule_616x353.jpg` (landscape) + SteamGridDB `dimensions=342x482,600x900` (portrait)

Neither existing variant provides portrait art from both sources. Modifying `Cover` would break the profile editor's landscape banner display. The correct approach is a new **`Portrait`** variant:

```rust
// game_images/models.rs — add to GameImageType enum
GameImageType::Portrait  // new variant, 3:4 poster art

// game_images/client.rs — build_download_url addition
GameImageType::Portrait => {
    // Try 2x first (600×900); fall through to 1x (300×450) via 404 handling
    format!(
        "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900_2x.jpg"
    )
}

// filename_for addition
GameImageType::Portrait => "portrait",

// game_images/steamgriddb.rs — build_endpoint addition
GameImageType::Portrait => ("grids", Some("342x482,600x900")),
```

The Steam CDN fallback chain for `Portrait`: `library_600x900_2x.jpg` → `library_600x900.jpg` → `None` (404 means the game has no portrait art; `object-fit: cover` on a landscape `header.jpg` substitute is an acceptable degraded fallback when SteamGridDB is also unavailable).

The `fetch_game_cover_art` Tauri command already accepts `imageType` as `Option<String>` and maps it to `GameImageType` — adding `"portrait"` to that match branch is the only command-layer change needed.

Frontend: `useGameCoverArt` passes `imageType: 'cover'` to `fetch_game_cover_art` today. Library cards must pass `imageType: 'portrait'`. The hook signature already accepts the image type parameter via the IPC call at `useGameCoverArt.ts:42`; however, the hook currently hardcodes `imageType: 'cover'`. The hook needs a new optional parameter or the call site must override it. **Recommended**: add an optional `imageType?: string` parameter to `useGameCoverArt` defaulting to `'cover'`, so existing callers are unaffected.

### New command — `profile_list_summaries` (Option A, recommended for performance)

**`profile_list_summaries`** — returns lightweight profile metadata for all profiles in a single server-side I/O pass.

**Request**: no parameters  
**Response**: `Vec<ProfileSummary>`  
**Error**: `Err(String)` on store failure (matches all other profile commands)

```rust
// In src-tauri/src/commands/profile.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub name: String,
    pub game_name: String,             // empty string when not set
    pub steam_app_id: String,          // empty string when not set
    pub custom_cover_art_path: String, // empty string when not set
}

#[tauri::command]
pub fn profile_list_summaries(
    store: State<'_, ProfileStore>,
) -> Result<Vec<ProfileSummary>, String> {
    // Read profile list, then load each TOML; map to slim DTO.
    // Skip unparseable entries with a tracing::warn — do not fail the whole call.
    // All I/O server-side — one IPC round-trip from the frontend.
}
```

Implementation reads TOML directly (not the metadata DB) to guarantee freshness. Newly created profiles that haven't been observed by the metadata sync are still returned correctly. Unparseable profiles are skipped with a warning (resilience pattern matching `profile_list`).

### New command — playtime (deferred)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaytimeSummary {
    pub profile_name: String,
    pub total_seconds: i64,
    pub last_played_at: Option<String>, // ISO-8601
}

#[tauri::command]
pub fn profile_get_playtime_summaries(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<PlaytimeSummary>, String>
```

Query: `SELECT profile_name, SUM(strftime('%s', finished_at) - strftime('%s', started_at)) AS total_seconds, MAX(finished_at) AS last_played_at FROM launch_operations WHERE status = 'succeeded' AND finished_at IS NOT NULL GROUP BY profile_name`

---

## System Constraints

### Performance

- **Cover art concurrency**: `useGameCoverArt` uses a request-id pattern to cancel stale responses. N cards rendered simultaneously will fire N parallel `fetch_game_cover_art` Tauri async calls — the runtime handles these in parallel. No throttling needed for typical library sizes (<100 profiles).
- **Virtual scrolling**: Not needed for MVP. The api-researcher identified `@tanstack/react-virtual` v3 (10–15 KB) as the preferred library if virtualization becomes necessary. Add only if paint profiling shows jank at 200+ profiles.
- **Image lazy loading**: Cards should use `loading="lazy"` on the `<img>` element. Cover art is a local file URL via `convertFileSrc` — no CORS or network latency concern. Virtualization already prevents off-screen renders if it is added later.
- **`refreshProfiles()` on mount**: `profile_list` is a fast filesystem scan. Calling it on `LibraryPage` mount ensures the list is fresh without coupling to global state timing.

### CSS layout

The responsive poster grid uses CSS Grid `auto-fill` with `minmax`:

```css
/* Additions to variables.css */
--crosshook-library-card-width: 190px;
--crosshook-library-card-aspect: 3 / 4;
--crosshook-library-grid-gap: 16px;

/* library.css */
.crosshook-library-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--crosshook-library-card-width), 1fr));
  gap: var(--crosshook-library-grid-gap);
  align-content: start;
}

.crosshook-library-card {
  position: relative;
  border-radius: var(--crosshook-radius-md);
  overflow: hidden;
  background: var(--crosshook-color-surface);
}

.crosshook-library-card__art {
  aspect-ratio: var(--crosshook-library-card-aspect);
  width: 100%;
  object-fit: cover;
  display: block;
}

.crosshook-library-card__overlay {
  position: absolute;
  inset: 0;
  background: linear-gradient(to top, rgba(0, 0, 0, 0.82) 0%, rgba(0, 0, 0, 0) 52%);
  pointer-events: none;
}

.crosshook-library-card__footer {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 10px 10px 8px;
}

.crosshook-library-card__actions {
  display: flex;
  gap: 6px;
  opacity: 0;
  transition: opacity var(--crosshook-transition-fast) ease;
}

.crosshook-library-card:hover .crosshook-library-card__actions,
.crosshook-library-card:focus-within .crosshook-library-card__actions {
  opacity: 1;
}
```

### Cover art aspect ratio — confirmed analysis

**The existing `--crosshook-profile-cover-art-aspect` is `460 / 215` (landscape banner). Do not reuse it.**

The new `--crosshook-library-card-aspect: 3 / 4` variable drives the card image container. The `object-fit: cover` rule handles the case where the downloaded image does not exactly match the 3:4 ratio (e.g., when Steam CDN falls back to a landscape `header.jpg` because no portrait art exists). The image will be cropped to fill the container rather than letterboxed — acceptable degraded behavior.

**Per-source aspect ratios confirmed from source code:**

| Source      | `Cover` variant                          | `Portrait` variant (new)                    |
| ----------- | ---------------------------------------- | ------------------------------------------- |
| Steam CDN   | `header.jpg` — 460×215 landscape         | `library_600x900_2x.jpg` — 600×900 portrait |
| SteamGridDB | `dimensions=460x215,920x430` — landscape | `dimensions=342x482,600x900` — portrait     |

### Controller / accessibility

- The grid container should receive `data-crosshook-focus-zone="content"` (already on the outer scroll body via `ContentArea`).
- Each card's three action buttons need explicit `aria-label` values: `"Launch {gameName}"`, `"Edit {gameName}"`, `"Add {gameName} to favorites"` / `"Remove {gameName} from favorites"`.
- Button `min-height` must respect `--crosshook-touch-target-min` (48px default, 56px in controller mode).
- Card focus ring should follow the existing `.crosshook-focus-scope` pattern from `focus.css`.

### Offline behavior

`useGameCoverArt` falls back to `null` on failure — cards render with the `--crosshook-color-surface` background. `profile_list` reads TOML files on disk — no network dependency. The library grid renders fully offline; only cover art is degraded.

---

## Codebase Changes

### Files to create

| File                                                             | Purpose                                                                                                             |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx`      | Page root; owns search state, view-mode toggle; calls `refreshProfiles` on mount                                    |
| `src/crosshook-native/src/components/library/LibraryGrid.tsx`    | Grid layout; maps `LibraryCardData[]` to `LibraryCard` instances                                                    |
| `src/crosshook-native/src/components/library/LibraryCard.tsx`    | Poster card; uses `useGameCoverArt` (with `imageType='portrait'`) + `useImageDominantColor`; fires action callbacks |
| `src/crosshook-native/src/components/library/LibraryToolbar.tsx` | Search `<input>` + grid/list toggle; controlled by `LibraryPage` state                                              |
| `src/crosshook-native/src/hooks/useLibraryProfiles.ts`           | Pure filter/sort transform over `profiles[]` and `favoriteProfiles[]`; no IPC                                       |
| `src/crosshook-native/src/styles/library.css`                    | All library-specific CSS                                                                                            |
| `src/crosshook-native/src/types/library.ts`                      | `LibraryCardData`, `LibraryViewMode`                                                                                |

### Files to modify

| File                                                                        | Change                                                                                                  |
| --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/layout/Sidebar.tsx:13`                 | Add `'library'` to `AppRoute` union; add sidebar item to Game section                                   |
| `src/crosshook-native/src/App.tsx:19`                                       | Add `library: true` to `VALID_APP_ROUTES`                                                               |
| `src/crosshook-native/src/components/layout/ContentArea.tsx:35`             | Add `case 'library': return <LibraryPage onNavigate={onNavigate} />;`                                   |
| `src/crosshook-native/src/components/layout/PageBanner.tsx`                 | Add `LibraryArt` SVG illustration (grid of rectangles motif)                                            |
| `src/crosshook-native/src/styles/variables.css`                             | Add `--crosshook-library-card-width`, `--crosshook-library-card-aspect`, `--crosshook-library-grid-gap` |
| `src/crosshook-native/src/main.tsx`                                         | Import `./styles/library.css`                                                                           |
| `src/crosshook-native/src/hooks/useGameCoverArt.ts`                         | Add optional `imageType?: string` parameter (defaults to `'cover'`); existing callers unaffected        |
| `src/crosshook-native/crates/crosshook-core/src/game_images/models.rs`      | Add `Portrait` to `GameImageType` enum                                                                  |
| `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs`      | Add `Portrait` arm to `build_download_url` and `filename_for`                                           |
| `src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs` | Add `Portrait` arm to `build_endpoint` with `dimensions=342x482,600x900`                                |
| `src/crosshook-native/src-tauri/src/commands/game_metadata.rs`              | Add `"portrait"` → `GameImageType::Portrait` mapping in `fetch_game_cover_art`                          |
| `src/crosshook-native/src-tauri/src/commands/profile.rs`                    | Add `profile_list_summaries` command (Option A, if implemented)                                         |
| `src/crosshook-native/src-tauri/src/commands/mod.rs`                        | Register new commands in the Tauri builder's `invoke_handler`                                           |

---

## Technical Decisions

### Decision 1: Data loading strategy (resolved — Option A recommended)

| Option                                 | Approach                                                             | Trade-off                                                            |
| -------------------------------------- | -------------------------------------------------------------------- | -------------------------------------------------------------------- |
| A — `profile_list_summaries`           | New Rust command reads all TOMLs server-side; returns slim DTO array | One IPC round-trip; ~20 lines of new Rust; always fresh              |
| B — Individual `profile_load` per card | Each card invokes `profile_load` on mount                            | Zero new backend (beyond image type); N IPC calls; staggered loading |
| C — Extend `ProfileContext`            | Eagerly load summaries into context on app start                     | Zero per-page overhead; wasteful when library route not visited      |

**Recommendation: Option A** for the shipped feature. Option B is acceptable as a development shortcut. Option C is rejected — unconditional loading for an optional route is wasteful.

### Decision 2: Profile activation before navigation (resolved)

`selectProfile(name)` from `useProfileContext()` is the single activation entry point. Both Launch and Edit card actions call `selectProfile(name)` then `onNavigate(route)`. No alternative load path should be invented in library components.

### Decision 3: Playtime for MVP (resolved — omit entirely)

No playtime surface exists in the current UI. Showing a placeholder ("0 hrs") would mislead users. The `LibraryCardData.playtimeSeconds` field is typed as optional so the component interface is stable when real data arrives.

### Decision 4: Search state persistence (resolved — ephemeral)

Component `useState` only. Search resets on route leave. No `sessionStorage` or context persistence for MVP.

### Decision 5: Default sort order (resolved)

Favorites-first, then alphabetical by `gameName` (falling back to `name` when `gameName` is empty).

### Decision 6: List view scope (resolved — deferred)

Grid-only for MVP. `LibraryViewMode` type and toolbar toggle are scaffolded but the list view renders nothing until a follow-on task implements it.

### Decision 7: Image type for portrait art (resolved — new `Portrait` variant)

Confirmed from source:

- `GameImageType::Cover` → `header.jpg` / `460x215` SteamGridDB (landscape)
- `GameImageType::Capsule` → `capsule_616x353.jpg` / `342x482,600x900` SteamGridDB (CDN fallback is landscape; SteamGridDB is portrait)

Neither variant is correct for the library grid. A new `GameImageType::Portrait` variant is added, using `library_600x900_2x.jpg` on Steam CDN and `342x482,600x900` on SteamGridDB. This preserves all existing behavior of `Cover` and `Capsule`. The `game_image_cache` DB stores entries keyed by `(steam_app_id, image_type)` string — `"portrait"` is a new distinct cache key, so existing cached landscape images are not affected.

---

## Open Questions

1. **Cover art fallback when no Steam app ID**: When `steamAppId` is empty and `customCoverArtPath` is absent, the card background is `--crosshook-color-surface`. Confirm with UX whether a placeholder icon or text initial (first letter of profile name) is preferred over a blank dark surface.

2. **`profile_list_summaries` partial failure handling**: If one TOML fails to parse during the server-side loop, skip the bad entry with a `tracing::warn` and continue (resilience pattern matching `profile_list`). This is the recommended default — document it explicitly in the Rust implementation.
