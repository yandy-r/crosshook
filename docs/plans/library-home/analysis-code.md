# Library Home — Code Analysis

This document maps every file that must be created or modified to implement library-home, extracts exact integration points (file + line number), and documents the conventions and gotchas discovered by reading source code directly.

---

## Executive Summary

The library-home feature slots into a well-established three-part route wiring pattern (Sidebar → App → ContentArea), consumes the `ProfileContext` that already exposes all required profile state, and reuses the `useGameCoverArt` hook with a one-parameter extension. The only non-trivial Rust work is adding a `Portrait` variant to `GameImageType` (3 files, ~5 arms), adding `profile_list_summaries` to `profile.rs` (~25 lines), and registering both in `lib.rs`. TypeScript exhaustion checking on `AppRoute` enforces completeness — the compiler will refuse to compile until every affected switch/record is updated.

---

## Existing Code Structure

### Route Infrastructure

| File                                                         | Key Line | Content                                                         |
| ------------------------------------------------------------ | -------- | --------------------------------------------------------------- |
| `src/crosshook-native/src/components/layout/Sidebar.tsx`     | 13       | `AppRoute` union type — all valid routes                        |
| `src/crosshook-native/src/components/layout/Sidebar.tsx`     | 33       | `SIDEBAR_SECTIONS` — drives sidebar rendering                   |
| `src/crosshook-native/src/components/layout/Sidebar.tsx`     | 58       | `ROUTE_LABELS` — display strings per route                      |
| `src/crosshook-native/src/App.tsx`                           | 19       | `VALID_APP_ROUTES: Record<AppRoute, true>` — runtime validation |
| `src/crosshook-native/src/App.tsx`                           | 43       | `useState<AppRoute>('profiles')` — default route                |
| `src/crosshook-native/src/components/layout/ContentArea.tsx` | 34       | `renderPage()` switch over `route`                              |
| `src/crosshook-native/src/components/layout/ContentArea.tsx` | 50–54    | `never` exhaustive guard — compile-time completeness check      |

Current `AppRoute`:

```ts
// Sidebar.tsx:13
export type AppRoute = 'profiles' | 'launch' | 'install' | 'community' | 'compatibility' | 'settings' | 'health';
```

### Profile State

| File                                                  | Purpose                                                                    |
| ----------------------------------------------------- | -------------------------------------------------------------------------- |
| `src/crosshook-native/src/context/ProfileContext.tsx` | `ProfileContextValue extends UseProfileResult` — exposes all profile state |
| `src/crosshook-native/src/hooks/useProfile.ts`        | Full state machine — profiles, favorites, selectProfile, toggleFavorite    |
| `src/crosshook-native/src/types/profile.ts:92`        | `GameProfile` TypeScript interface                                         |

Key fields available from `useProfileContext()`:

```ts
profiles: string[]                                  // all profile names
favoriteProfiles: string[]                          // names of favorited profiles
selectProfile: (name: string) => Promise<void>      // loads profile via profile_load IPC
toggleFavorite: (name: string, favorite: boolean) => Promise<void>  // profile_set_favorite IPC
refreshProfiles: () => Promise<void>
```

`GameProfile` relevant fields:

```ts
// profile.ts:92
GameProfile.steam.app_id: string       // Steam App ID for cover art fetch
GameProfile.game.name: string          // display name
GameProfile.game.custom_cover_art_path?: string  // user-set art; bypasses CDN fetch
```

### Cover Art Hook

**`src/crosshook-native/src/hooks/useGameCoverArt.ts`**

```ts
export function useGameCoverArt(
  steamAppId: string | undefined,
  customCoverArtPath?: string,    // custom path bypasses IPC — returned directly
): UseGameCoverArtResult { ... }  // { coverArtUrl: string | null, loading: boolean }
```

Critical line **:42** — hardcoded `imageType: 'cover'` in the invoke call:

```ts
const path = await invoke<string | null>('fetch_game_cover_art', {
  appId: normalizedAppId,
  imageType: 'cover', // ← MUST become imageType ?? 'cover' after adding parameter
});
```

Race-condition guard: `requestIdRef.current` increments before each fetch; result is discarded if `requestId !== requestIdRef.current` at completion. This is essential for the library grid where many cards load simultaneously.

Custom path short-circuit (`:70–77`): when `customUrl` is truthy, the effect increments `requestIdRef` (cancelling any in-flight IPC) and returns `{ coverArtUrl: customUrl, loading: false }` without invoking the IPC command. The library card must pass `customCoverArtPath` correctly to get this behaviour for free.

### IPC Commands (Rust)

**`src/crosshook-native/src-tauri/src/commands/profile.rs`**

```rust
// line 222 — profile list
#[tauri::command]
pub fn profile_list(store: State<'_, ProfileStore>) -> Result<Vec<String>, String> {
    store.list().map_err(map_error)
}

// line 227 — individual load
#[tauri::command]
pub fn profile_load(name: String, store: State<'_, ProfileStore>) -> Result<GameProfile, String> {
    store.load(&name).map_err(map_error)
}

// line 626 — toggle favorite (writes + emits event)
#[tauri::command]
pub fn profile_set_favorite(
    name: String, favorite: bool, app: AppHandle, metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> { ... emit_profiles_changed(&app, "favorite-updated"); ... }

// line 640 — read favorites
#[tauri::command]
pub fn profile_list_favorites(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<String>, String> { ... }
```

Conventions: sync (`pub fn`, not `pub async fn`) for local file/DB I/O; `State<'_, T>` for managed deps; always `map_err(|e| e.to_string())` at IPC boundary; `emit_profiles_changed()` after mutations.

**`src/crosshook-native/src-tauri/src/commands/game_metadata.rs`**

```rust
// line 17 — image fetch IPC (async — network)
#[tauri::command]
pub async fn fetch_game_cover_art(
    app_id: String,
    image_type: Option<String>,
    metadata_store: State<'_, MetadataStore>,
    settings_store: State<'_, SettingsStore>,
) -> Result<Option<String>, String> {
    let image_type = match image_type.as_deref().unwrap_or("cover") {
        "hero"    => GameImageType::Hero,
        "capsule" => GameImageType::Capsule,
        _         => GameImageType::Cover,   // ← "portrait" currently falls here
    };
    ...
}
```

### Rust Image Pipeline

**`src/crosshook-native/crates/crosshook-core/src/game_images/models.rs`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameImageType {
    Cover,     // display: "cover"
    Hero,      // display: "hero"
    Capsule,   // display: "capsule"
    // Portrait variant goes here
}
```

`Display` impl outputs lowercase variant name (used as cache key string and filename prefix). Adding `Portrait` produces `"portrait"` — no DB migration needed since `game_image_cache.image_type` is free-form TEXT.

**`src/crosshook-native/crates/crosshook-core/src/game_images/client.rs`**

```rust
// line 334 — Steam CDN URL builder
fn build_download_url(app_id: &str, image_type: GameImageType) -> String {
    match image_type {
        GameImageType::Cover    => format!(".../{app_id}/header.jpg"),
        GameImageType::Hero     => format!(".../{app_id}/library_hero.jpg"),
        GameImageType::Capsule  => format!(".../{app_id}/capsule_616x353.jpg"),
        // Portrait: library_600x900_2x.jpg (see gotcha below)
    }
}

// line 354 — filename builder (used for disk cache path)
fn filename_for(image_type: GameImageType, source: GameImageSource, extension: &str) -> String {
    let type_prefix = match image_type {
        GameImageType::Cover    => "cover",
        GameImageType::Hero     => "hero",
        GameImageType::Capsule  => "capsule",
        // Portrait: "portrait"
    };
    format!("{type_prefix}_{source_suffix}.{extension}")
}

// line 146 — main entry point (async)
pub async fn download_and_cache_image(
    store: &MetadataStore,
    app_id: &str,
    image_type: GameImageType,
    api_key: Option<&str>,
) -> Result<Option<String>, String> { ... }
```

**`src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs`**

```rust
// line 100 — endpoint builder
fn build_endpoint(app_id: &str, image_type: &GameImageType) -> String {
    let (path_segment, dimensions) = match image_type {
        GameImageType::Cover    => ("grids", Some("460x215,920x430")),
        GameImageType::Hero     => ("heroes", None),
        GameImageType::Capsule  => ("grids", Some("342x482,600x900")),
        // Portrait: ("grids", Some("342x482,600x900")) — same as Capsule dims per spec
    };
    ...
}
```

### Registration

**`src/crosshook-native/src-tauri/src/lib.rs:189`** — `invoke_handler` registers all commands:

```rust
.invoke_handler(tauri::generate_handler![
    ...
    commands::profile::profile_set_favorite,     // line 251
    commands::profile::profile_list_favorites,   // line 252
    // profile_list_summaries goes here (new)
    ...
])
```

### CSS Infrastructure

**`src/crosshook-native/src/main.tsx`** — CSS imports (lines 4–10):

```ts
import './styles/theme.css';
import './styles/focus.css';
import './styles/layout.css';
import './styles/sidebar.css';
import './styles/console-drawer.css';
import './styles/themed-select.css';
import './styles/collapsible-section.css';
// library.css goes here (new)
```

**`src/crosshook-native/src/components/layout/PageBanner.tsx`** — per-route SVG art components:

- `ProfilesArt`, `LaunchArt`, `InstallArt`, `CommunityArt`, `CompatibilityArt`, `HealthDashboardArt`
- Pattern: inline SVG in exported named function, using shared `SVG_DEFAULTS` object
- `LibraryArt` goes here

---

## Implementation Patterns

### Pattern 1 — Route Registration (Three-File Wiring)

Every new route must touch Sidebar.tsx (type + sections + labels), App.tsx (valid routes + default), and ContentArea.tsx (switch case + import).

```ts
// 1. Sidebar.tsx — extend type at line 13:
export type AppRoute = 'profiles' | 'launch' | ... | 'health' | 'library';

// 2. Sidebar.tsx — add to SIDEBAR_SECTIONS at line 33:
{ label: 'Game', items: [
  { route: 'library', label: 'Library', icon: LibraryIcon },
  { route: 'profiles', label: 'Profiles', icon: ProfilesIcon },
  ...
]}

// 3. Sidebar.tsx — add to ROUTE_LABELS at line 58:
const ROUTE_LABELS: Record<AppRoute, string> = {
  ...
  library: 'Library',
};

// 4. App.tsx — add to VALID_APP_ROUTES at line 19:
const VALID_APP_ROUTES: Record<AppRoute, true> = {
  ...
  library: true,
};

// 5. App.tsx — change default at line 43:
const [route, setRoute] = useState<AppRoute>('library');

// 6. ContentArea.tsx — add import + case before line 50:
import LibraryPage from '../pages/LibraryPage';
...
case 'library':
  return <LibraryPage onNavigate={onNavigate} />;
```

TypeScript's `Record<AppRoute, true>` and `Record<AppRoute, string>` force exhaustive coverage — omitting `library` from either record is a compile error.

### Pattern 2 — Page Component Structure

Reference: `HealthDashboardPage.tsx:826`

```tsx
// Page receives onNavigate and destructures from context
export function HealthDashboardPage({ onNavigate }: { onNavigate?: (route: AppRoute) => void }) {
  const { selectProfile } = useProfileContext();
  ...
  // Navigation always awaits selectProfile before calling onNavigate
  async function handleFixNavigation(profileName: string) {
    await selectProfile(profileName);
    onNavigate?.('profiles');
  }
}
```

`LibraryPage` follows identical shape:

```tsx
export function LibraryPage({ onNavigate }: { onNavigate?: (route: AppRoute) => void }) {
  const { profiles, favoriteProfiles, selectProfile, toggleFavorite } = useProfileContext();
  ...
  async function handleLaunch(profileName: string) {
    await selectProfile(profileName);
    onNavigate?.('launch');
  }
}
```

### Pattern 3 — Cover Art Hook Extension

`useGameCoverArt` needs an optional `imageType` parameter that defaults to `'cover'`:

```ts
// Current signature (line 13):
export function useGameCoverArt(
  steamAppId: string | undefined,
  customCoverArtPath?: string,
): UseGameCoverArtResult

// New signature:
export function useGameCoverArt(
  steamAppId: string | undefined,
  customCoverArtPath?: string,
  imageType?: string,            // ← add this
): UseGameCoverArtResult

// Change line 42 from:
imageType: 'cover',
// to:
imageType: imageType ?? 'cover',
```

All existing callers are unaffected (third parameter defaults to `undefined` → `'cover'`).

### Pattern 4 — Rust IPC Command (Sync Local I/O)

`profile_list_summaries` reads all profiles from TOML and returns lightweight summaries:

```rust
// Return type — new DTO struct (add to profile.rs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub name: String,
    pub game_name: String,
    pub steam_app_id: String,
    pub custom_cover_art_path: String,
}

// Command (add to profile.rs, before or after profile_list):
#[tauri::command]
pub fn profile_list_summaries(
    store: State<'_, ProfileStore>,
) -> Result<Vec<ProfileSummary>, String> {
    let names = store.list().map_err(map_error)?;
    let mut summaries = Vec::with_capacity(names.len());
    for name in names {
        let profile = store.load(&name).map_err(map_error)?;
        summaries.push(ProfileSummary {
            name,
            game_name: profile.game.name,
            steam_app_id: profile.steam.app_id,
            custom_cover_art_path: profile.game.custom_cover_art_path,
        });
    }
    Ok(summaries)
}
```

Note: sync (`pub fn`, not async) — all I/O is local filesystem reads, consistent with `profile_list` and `profile_load`.

### Pattern 5 — GameImageType Rust Variant

Three files need `Portrait` arms added:

```rust
// models.rs — enum variant:
pub enum GameImageType { Cover, Hero, Capsule, Portrait }
// Display impl arm:
Self::Portrait => write!(f, "portrait"),

// client.rs:334 — CDN URL (primary attempt; fallback chain handled separately):
GameImageType::Portrait => format!(".../{app_id}/library_600x900_2x.jpg"),
// client.rs:354 — filename:
GameImageType::Portrait => "portrait",

// steamgriddb.rs:100 — endpoint (same dimensions as portrait):
GameImageType::Portrait => ("grids", Some("342x482,600x900")),

// game_metadata.rs:25 — string match arm (before _ catch-all):
"portrait" => GameImageType::Portrait,
```

---

## Integration Points

### Files to Modify

| File                                                                        | Lines          | Change                                                      |
| --------------------------------------------------------------------------- | -------------- | ----------------------------------------------------------- |
| `src/crosshook-native/src/components/layout/Sidebar.tsx`                    | 13             | Add `\| 'library'` to `AppRoute` union                      |
| `src/crosshook-native/src/components/layout/Sidebar.tsx`                    | 33–55          | Add `library` item to a section in `SIDEBAR_SECTIONS`       |
| `src/crosshook-native/src/components/layout/Sidebar.tsx`                    | 58–66          | Add `library: 'Library'` to `ROUTE_LABELS`                  |
| `src/crosshook-native/src/App.tsx`                                          | 19–27          | Add `library: true` to `VALID_APP_ROUTES`                   |
| `src/crosshook-native/src/App.tsx`                                          | 43             | Change default from `'profiles'` to `'library'`             |
| `src/crosshook-native/src/components/layout/ContentArea.tsx`                | 4–11           | Import `LibraryPage`                                        |
| `src/crosshook-native/src/components/layout/ContentArea.tsx`                | 49             | Add `case 'library':` before `case 'health':`               |
| `src/crosshook-native/src/components/layout/PageBanner.tsx`                 | end            | Add `LibraryArt` SVG component                              |
| `src/crosshook-native/src/hooks/useGameCoverArt.ts`                         | 13             | Add `imageType?: string` param                              |
| `src/crosshook-native/src/hooks/useGameCoverArt.ts`                         | 42             | Replace `'cover'` with `imageType ?? 'cover'`               |
| `src/crosshook-native/src/main.tsx`                                         | 10             | Add `import './styles/library.css'`                         |
| `src/crosshook-native/crates/crosshook-core/src/game_images/models.rs`      | 8–12           | Add `Portrait` to enum + Display arm                        |
| `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs`      | 334–352        | Add `Portrait` arm to `build_download_url`                  |
| `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs`      | 354–365        | Add `Portrait` arm to `filename_for`                        |
| `src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs` | 100–105        | Add `Portrait` arm to `build_endpoint`                      |
| `src/crosshook-native/src-tauri/src/commands/game_metadata.rs`              | 25–28          | Add `"portrait" => GameImageType::Portrait,` before `_`     |
| `src/crosshook-native/src-tauri/src/commands/profile.rs`                    | after line 224 | Add `ProfileSummary` DTO + `profile_list_summaries` command |
| `src/crosshook-native/src-tauri/src/lib.rs`                                 | after 252      | Register `commands::profile::profile_list_summaries`        |

### Files to Create

| File                                                        | Purpose                                           |
| ----------------------------------------------------------- | ------------------------------------------------- |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx` | Page orchestrator — owns all state, calls context |
| `src/crosshook-native/src/components/LibraryCard.tsx`       | Pure props card — no context access               |
| `src/crosshook-native/src/components/LibraryGrid.tsx`       | Stateless grid layout wrapper                     |
| `src/crosshook-native/src/styles/library.css`               | BEM `crosshook-library-*` styles                  |

---

## Code Conventions

### TypeScript / React

- **Component naming**: `PascalCase` — `LibraryPage`, `LibraryCard`, `LibraryGrid`
- **CSS classes**: BEM with `crosshook-library-*` prefix — e.g. `crosshook-library__grid`, `crosshook-library-card`, `crosshook-library-card__cover`
- **Context access**: only in the page component (`LibraryPage`); child components (`LibraryCard`, `LibraryGrid`) receive data as props
- **Async navigation**: always `await selectProfile(name)` before `onNavigate?.(route)` — never call concurrently
- **Hook invocation**: `invoke<ReturnType>('command_name', { param1, param2 })` — camelCase param keys, snake_case command name
- **CSS variables**: define new tokens in `styles/variables.css`; use `var(--crosshook-library-*)` in `library.css`

### Rust

- **IPC naming**: `snake_case` command function names matching the string passed to `invoke()` on the frontend
- **Sync vs async**: local file/DB I/O → `pub fn`; network I/O → `pub async fn`
- **Error boundary**: always `Result<T, String>` at IPC boundary (`.map_err(|e| e.to_string())`)
- **State access**: `State<'_, ProfileStore>` and/or `State<'_, MetadataStore>` as parameters
- **Serde**: all IPC return types must derive `Serialize, Deserialize`; field names match TypeScript interfaces via serde default (snake_case)
- **Mutations emit event**: calls that change profile data call `emit_profiles_changed(&app, "reason")`

---

## Gotchas and Warnings

### 1. TypeScript Exhaustive Guard Enforces Completeness

`ContentArea.tsx:50–54` uses a `never` guard:

```ts
default: {
  const _exhaustive: never = route;
  return _exhaustive;
}
```

Adding `'library'` to `AppRoute` without adding its `case` in `renderPage()` is a **compile error**. Similarly, `Record<AppRoute, true>` and `Record<AppRoute, string>` force updates to `VALID_APP_ROUTES` and `ROUTE_LABELS`. **The compiler enforces the three-file update atomically.**

### 2. Portrait CDN Fallback Chain

`build_download_url` currently returns a single URL per type. Portrait requires trying multiple Steam CDN URLs: `library_600x900_2x.jpg → library_600x900.jpg → header.jpg`. The simplest approach is to attempt `library_600x900_2x.jpg` first as the primary URL and let the HTTP 404 fall through to the stale-cache path, then the caller can detect `Ok(None)` and retry with `header.jpg`. Alternatively, implement a portrait-specific helper that loops through candidate URLs before giving up. Do not model Portrait exactly like Cover/Hero/Capsule — a single URL will silently return a cover-format image if the portrait URL 404s.

### 3. `fetch_game_cover_art` String Fallthrough

`game_metadata.rs:28`: the `_` catch-all maps any unrecognized string to `GameImageType::Cover`. If you add `Portrait` to the enum but forget to add `"portrait"` before the `_`, all portrait requests silently fetch cover-format art. Add the explicit arm **before** the wildcard.

### 4. `useGameCoverArt` Custom Path Short-Circuit

When `customCoverArtPath` is set, the hook immediately returns `{ coverArtUrl: convertFileSrc(path), loading: false }` without any IPC call. For `LibraryCard`, passing `customCoverArtPath={profile.game.custom_cover_art_path}` correctly handles this case — no extra logic needed.

### 5. `profile_list_summaries` is Sync, Not Async

Following the pattern of `profile_list` and `profile_load`, this command should be `pub fn` (blocking). It performs N sequential TOML file reads. For users with many profiles this is still fast (local SSD, small files), but it must not be `pub async fn` unless using `tokio::task::spawn_blocking`.

### 6. Portrait Image Cache Key

`download_and_cache_image` uses `image_type.to_string()` as the `image_type` column value in `game_image_cache`. Adding the `Display` arm `Self::Portrait => write!(f, "portrait")` produces `"portrait"` — no schema migration needed (the column is free-form TEXT at schema v14).

### 7. `requestIdRef` Race Guard in Library Grid

Each `LibraryCard` that calls `useGameCoverArt` has its own independent `requestIdRef`. Cards do not share state, so there is no cross-card cancellation concern. The guard prevents stale results only within the same card instance across re-renders.

### 8. Profile TOML `list()` Returns Alphabetically Sorted Names

`ProfileStore.list()` in `toml_store.rs` sorts names before returning. `profile_list_summaries` inherits this order. The grid should render in the order returned (favorites floated to top by the frontend, not the IPC command).

### 9. `profile_list_summaries` vs Calling `profile_list` + `profile_load` N Times from Frontend

The new IPC command is justified: calling `profile_list` then N calls to `profile_load` from the frontend would require N Tauri IPC round-trips on mount, each with serialization overhead. A single batched command amortizes this to one round-trip.

### 10. Icon Required for Sidebar

`SIDEBAR_SECTIONS` items require a `ComponentType<SVGProps<SVGSVGElement>>`. A `LibraryIcon` must be added to `src/crosshook-native/src/components/icons/SidebarIcons.tsx` (or the same file pattern) and imported in `Sidebar.tsx`. Omitting this is a TypeScript type error.

---

## Task-Specific Guidance

### Rust Changes (models.rs, client.rs, steamgriddb.rs, game_metadata.rs)

- These four files form one atomic Rust unit — all `match` arms must be exhaustive. After adding `Portrait` to the enum, `cargo build` will surface every unhandled arm as an error. Resolve all before marking Rust work done.
- Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` after changes. `client.rs` has extensive tests including `filename_for_uses_inferred_extension` — add a Portrait test case.
- The portrait CDN URL (`library_600x900_2x.jpg`) returns HTTP 404 for many old games. The stale-cache fallback path in `download_and_cache_image` handles this gracefully (`Ok(None)` propagates to the frontend as `null`, which the hook renders as skeleton/fallback). **Do not treat 404 as an error.**

### IPC Command (profile.rs + lib.rs)

- `ProfileSummary` DTO goes in `profile.rs` (not a new file). It derives `Serialize, Deserialize`.
- Register in `lib.rs` in the same block as `profile_list_favorites` (line 252) — keep related commands grouped.
- Frontend TypeScript type for the return value: `{ name: string; game_name: string; steam_app_id: string; custom_cover_art_path: string }[]` — add to `src/crosshook-native/src/types/profile.ts`.

### useGameCoverArt (hooks)

- The change is surgical: one new parameter, one changed string literal. All existing callers (`GameCoverArt.tsx`, `PinnedProfilesStrip.tsx`) are backward-compatible.
- `LibraryCard` calls: `useGameCoverArt(profile.steamAppId, profile.customCoverArtPath, 'portrait')`

### Route Wiring (Sidebar, App, ContentArea)

- Change the default route (`App.tsx:43`) from `'profiles'` to `'library'` as specified in `shared.md`. This means users land on the library grid on app launch.
- The icon for the Library route (`LibraryIcon`) should be added to `SidebarIcons.tsx` first, then imported in `Sidebar.tsx` — otherwise the SIDEBAR_SECTIONS entry cannot be added without a type error.

### LibraryPage Component Decomposition

Per the Component Decomposition Rule in `shared.md`:

- `LibraryPage` — owns all state, calls `useProfileContext()` and `useProfileSummaries()` hook wrapping `profile_list_summaries`
- `LibraryGrid` — accepts `children` or an array prop; pure layout with CSS Grid
- `LibraryCard` — accepts `name`, `gameName`, `steamAppId`, `customCoverArtPath`, `isFavorite`, `onLaunch`, `onToggleFavorite` as plain props; calls `useGameCoverArt` internally for its own loading state; no context access

### CSS

- All new classes use `crosshook-library-*` BEM prefix
- CSS variables (`--crosshook-library-card-width: 190px`, `--crosshook-library-card-aspect: 3 / 4`) go in `styles/variables.css`
- Grid rule mirrors `crosshook-community-browser__profile-grid` from `theme.css:~997`: `repeat(auto-fill, minmax(var(--crosshook-library-card-width), 1fr))`
- Skeleton loading: apply `crosshook-skeleton` class to cover placeholder div while `loading === true`; class + keyframe already defined in `theme.css:~4738`
