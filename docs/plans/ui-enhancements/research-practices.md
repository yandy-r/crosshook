# UI Enhancements — Engineering Practices Research

## Executive Summary

The Profiles page is dominated by a single `CollapsibleSection` (titled "Advanced") that contains `ProfileFormSections.tsx` — a 1,144-line monolith that renders every profile field inline. The architecture has a working primitive library (CollapsibleSection, ThemedSelect), a ready Radix UI Tabs dependency, and clean CSS variable infrastructure. However, `ProfileFormSections` is used at three callsites with different layout needs, so the tab layer must live at `ProfilesPage` level only — not inside `ProfileFormSections` itself.

Issue #52 (game metadata/cover art) adds a second external-API feature alongside #53 (ProtonDB). Both share identical infrastructure: `MetadataStore.put_cache_entry` / `get_cache_entry`, `external_cache_entries` table, and the `cache-then-live-then-stale-fallback` lookup pattern. #52 adds one new dimension: filesystem image caching, which has no equivalent in the current codebase. The Figma concept centers on a **cover art card grid** — a browsable library where launch, favorite, and edit actions surface directly on each game card — replacing the current `PinnedProfilesStrip` text chips with a visually driven entry point to the Profiles page.

## Existing Reusable Code

| File                                                                        | Description                                                                                                                                                                                                                                                                        |
| --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/pages/ProfilesPage.tsx`                | Top-level page: Wizard area, profile selector, single "Advanced" CollapsibleSection, Actions bar, Launcher Export section, all modals                                                                                                                                              |
| `src/crosshook-native/src/components/ProfileFormSections.tsx`               | 1,144-line monolith: inline sub-components (FieldRow, ProtonPathField, LauncherMetadataFields, OptionalSection, ProfileSelectorField, TrainerVersionSetField) + main `ProfileFormSections` export; used by ProfilesPage, InstallPage (reviewMode), and imports by OnboardingWizard |
| `src/crosshook-native/src/components/pages/InstallPage.tsx`                 | Uses `ProfileFormSections` with `reviewMode` prop inside a compact `ProfileReviewModal` — a tab-based layout would be wrong UX here                                                                                                                                                |
| `src/crosshook-native/src/components/OnboardingWizard.tsx`                  | Imports only the `ProtonInstallOption` type from `ProfileFormSections`; builds its own step-by-step form from individual components directly                                                                                                                                       |
| `src/crosshook-native/src/components/ui/CollapsibleSection.tsx`             | Controlled/uncontrolled `<details>` wrapper; accepts `meta` slot for inline badges                                                                                                                                                                                                 |
| `src/crosshook-native/src/components/ui/ThemedSelect.tsx`                   | Radix `@radix-ui/react-select` wrapper; supports groups and pinned values                                                                                                                                                                                                          |
| `src/crosshook-native/src/components/layout/ContentArea.tsx`                | Top-level router using `@radix-ui/react-tabs` — the existing tabs pattern for the app shell                                                                                                                                                                                        |
| `src/crosshook-native/src/components/ProfileActions.tsx`                    | Action bar (Save/Duplicate/Rename/Preview/Export/History/Delete buttons)                                                                                                                                                                                                           |
| `src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx` | Already an independent component for env vars; contains security-critical `RESERVED_CUSTOM_ENV_KEYS` constant                                                                                                                                                                      |
| `src/crosshook-native/src/components/GamescopeConfigPanel.tsx`              | Already extracted; uses CollapsibleSection + ThemedSelect                                                                                                                                                                                                                          |
| `src/crosshook-native/src/components/MangoHudConfigPanel.tsx`               | Already extracted; uses CollapsibleSection + ThemedSelect + hook                                                                                                                                                                                                                   |
| `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`          | Already extracted; complex multi-section panel                                                                                                                                                                                                                                     |
| `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx`           | Already extracted; IPC-backed preview panel                                                                                                                                                                                                                                        |
| `src/crosshook-native/src/styles/variables.css`                             | All design tokens; `--crosshook-subtab-min-height` and `--crosshook-subtab-padding-inline` tokens already exist with controller-mode overrides                                                                                                                                     |
| `src/crosshook-native/src/styles/collapsible-section.css`                   | CSS for CollapsibleSection; includes stripping rules for nested panels                                                                                                                                                                                                             |
| `src/crosshook-native/src/styles/theme.css`                                 | Main theme stylesheet                                                                                                                                                                                                                                                              |
| `src/crosshook-native/package.json`                                         | Dependencies: `@radix-ui/react-tabs` v1.1.13 and `@radix-ui/react-select` already installed                                                                                                                                                                                        |

## Cover Art Card Grid — Engineering Analysis

### What the Figma concept describes

The Figma concept shows a library grid of game cards where:

- The cover art image is the primary visual (portrait orientation, Steam `library_600x900.jpg`)
- Each card overlays three quick actions: **Launch**, **Favorite/unfavorite**, and **Edit** (navigate to the profile editor tab)
- Cards surface the profile health status (HealthBadge equivalent) directly on the card
- The grid is the primary entry point for browsing profiles on the Profiles page — it replaces or supplements the `PinnedProfilesStrip` and the `ThemedSelect` dropdown

This is specifically about browsing and quick-acting on profiles with visual context, not a redesign of the profile editor itself.

### Existing grid infrastructure that directly applies

The community browser already implements the exact grid pattern needed:

- **`crosshook-community-browser__profile-grid`** (`theme.css:592–594`): `display: grid; grid-template-columns: repeat(auto-fit, minmax(var(--crosshook-community-profile-grid-min), 1fr))`. The `--crosshook-community-profile-grid-min` CSS variable is already responsive: `280px` default, `340px` in controller mode (`variables.css:52, 99`).
- **`crosshook-community-browser__profile-card`** (`theme.css:596–604`): `display: grid; padding: 18px; border-radius: 16px; background: rgba(8, 14, 26, 0.78); border: 1px solid var(--crosshook-color-border)`. This is the existing card surface — the game card needs the same base but with an image aspect ratio as its primary content area.
- **`.crosshook-card`** (`theme.css:138–144`): The canonical glassmorphism surface — `linear-gradient(180deg, rgba(18, 23, 42, 0.96), rgba(12, 17, 32, 0.96))`, `border: 1px solid var(--crosshook-color-border)`, `border-radius: var(--crosshook-radius-lg)`, `backdrop-filter: blur(18px)`. Use this base for the profile game card.

New CSS variables needed:

- `--crosshook-profile-grid-min` — analogous to `--crosshook-community-profile-grid-min`; proposed default `220px`, controller mode `280px`. Narrower than community cards because portrait cover art benefits from a smaller minimum to fit more cards per row.
- `--crosshook-profile-card-art-aspect` — the cover art aspect ratio, `2/3` for `library_600x900.jpg` portrait format.

### `ProfileGameCard` component design

A new `src/crosshook-native/src/components/ProfileGameCard.tsx` component. It is a **presentation component only** — it receives data as props and fires callbacks. It does not call hooks or access context directly.

```tsx
interface ProfileGameCardProps {
  profileName: string;
  gameName: string | null;
  coverArtSrc: string | null; // local asset:// path or null; from useGameMetadata
  isFavorite: boolean;
  healthStatus: string | null; // 'healthy' | 'stale' | 'broken' | null
  isSelected: boolean; // true when this profile is the active editor selection
  isLaunching: boolean; // true while a launch is in-flight for this profile
  onLaunch: () => void;
  onToggleFavorite: () => void;
  onEdit: () => void; // navigates to Profiles page and selects this profile
}
```

**Slot layout:**

1. Cover art area (`GameCoverArt` component, `aspect-ratio: var(--crosshook-profile-card-art-aspect)`) — fills the card top
2. Card footer: profile name + `gameName` (if different) + health badge
3. Overlay action row (visible on `:focus-within` and `:hover`, always visible in controller mode): Launch button, Favorite toggle, Edit button

**No inline styles.** All layout and theming via `crosshook-profile-card__*` BEM classes and the existing CSS variable system.

### Where `ProfileGameCard` is composed

The grid lives in a new `ProfileLibraryGrid` component (`src/crosshook-native/src/components/ProfileLibraryGrid.tsx`):

```tsx
interface ProfileLibraryGridProps {
  profiles: string[];
  favoriteProfiles: string[];
  selectedProfile: string;
  healthByName: Record<string, { status: string }>;
  coverArtByName: Record<string, string | null>; // keyed by profile name
  launchingProfile: string | null;
  onSelectProfile: (name: string) => void;
  onToggleFavorite: (name: string, favorite: boolean) => Promise<void>;
  onLaunch: (name: string) => void;
}
```

`ProfileLibraryGrid` renders the `repeat(auto-fit, ...)` CSS grid of `ProfileGameCard` components. It holds no state beyond what it receives as props.

### Integration into `ProfilesPage` layout

The grid replaces `PinnedProfilesStrip` and augments (or replaces) the `ThemedSelect` dropdown as the primary profile picker:

```
ProfilesPage
  ├── PageBanner (always visible)
  ├── ProfileLibraryGrid   ← NEW: cover art grid, all profiles (or favorites-first sort)
  │     ProfileGameCard × N  (each card: art + name + health + [Launch, Fav, Edit])
  ├── Panel (editor area — shown when a profile is selected via Edit action on a card)
  │   ├── Profile selector bar (dropdown, still useful for keyboard/text navigation)
  │   └── Tabs.Root (Setup / Runtime / Trainer / Environment / Launcher)
  └── ProfileActions bar (always visible below the panel)
```

The `onEdit` callback on each `ProfileGameCard` calls `selectProfile(name)` from `useProfileContext` and scrolls the editor panel into view. The `onLaunch` callback uses the existing launch IPC path already in `LaunchPage`. The `onToggleFavorite` callback calls `toggleFavorite` from `useProfileContext` — already exists.

**No new context or global state.** `ProfileLibraryGrid` receives its data slice from `ProfilesPage`, which already has `profiles`, `favoriteProfiles`, `selectedProfile`, and `toggleFavorite` from `useProfileContext` and `healthByName` from `useProfileHealthContext`.

### Cover art data flow for the grid

Each `ProfileGameCard` needs a cover art URL. The `ProfilesPage` should batch-load cover art for all visible profiles using `useGameMetadataBatch` (a new hook) rather than mounting N independent `useGameMetadata` hooks:

```ts
// New hook: src/crosshook-native/src/hooks/useGameMetadataBatch.ts
// Takes a map of profileName → steamAppId, returns a map of profileName → coverArtSrc
function useGameMetadataBatch(profileAppIds: Record<string, string | null>): Record<string, string | null>;
```

The steam_app_id for each profile is available from the profile data in `useProfileContext`. The batch hook deduplicates by app_id (multiple profiles may share the same app_id) and issues one `useGameMetadata`-equivalent IPC call per unique app_id.

**Fallback:** If `steam_app_id` is not set for a profile, `coverArtSrc` is `null` and `GameCoverArt` renders the text-initial placeholder.

### List/grid view toggle

A toggle between **grid** (cover art cards) and **list** (the current text-row selector / chip strip) should live in `ProfilesPage` local state — a single `viewMode: 'grid' | 'list'` boolean. Persist in `sessionStorage` using the existing session storage pattern (e.g., `crosshook.profileViewMode`). The toggle control is a two-button row above the grid, following the same pattern as the existing `crosshook-community-toolbar`.

In **list mode**, `ProfileLibraryGrid` renders the existing `PinnedProfilesStrip` + `ThemedSelect` layout (or a new compact list-row variant). `ProfileLibraryGrid` accepts a `viewMode` prop and switches its internal layout — no conditional unmounting, just a CSS class change (`crosshook-profile-library--grid` vs `crosshook-profile-library--list`).

### Controller mode considerations

In controller mode (`data-crosshook-controller-mode='true'`):

- The card overlay actions (Launch/Fav/Edit) must be **always visible**, not hover-dependent — controller users cannot hover
- `--crosshook-touch-target-min: 56px` is already set for controller mode; the action buttons on the card must meet this minimum
- The grid `--crosshook-profile-grid-min` should be wider in controller mode (`280px`) to keep cards large enough for gamepad navigation
- The existing `crosshook-subtab` responsive pattern (flex: 1 1 0 at narrow widths) applies to action buttons on the card too

### `PinnedProfilesStrip` evolution

`PinnedProfilesStrip` (`src/crosshook-native/src/components/PinnedProfilesStrip.tsx`) currently shows text-chip-only favorites. After the grid is introduced:

- In **grid mode**: `PinnedProfilesStrip` is replaced by the `ProfileLibraryGrid` with favorites sorted or highlighted at the top — not two separate surfaces
- In **list mode**: `PinnedProfilesStrip` may remain as-is, or adopt small cover art thumbnails (16:9 inline thumbnail, not full portrait card) as a follow-on
- The component itself does not need to change for Phase 1 of the grid feature; the grid is additive

### Reuse opportunity: CommunityBrowser

`CommunityBrowser` (`CommunityBrowser.tsx`) already uses the `crosshook-community-browser__profile-grid` class and `crosshook-community-browser__profile-card` cards. If cover art is ever shown in the community browser (showing game art alongside community profile entries), `GameCoverArt` is directly reusable — it is a props-only component.

### CSS needed (new additions only)

No existing CSS files need modification. Add new rules to `theme.css` following the existing BEM conventions:

```css
/* Profile library grid */
.crosshook-profile-library {
  display: grid;
  gap: var(--crosshook-grid-gap);
}

.crosshook-profile-library--grid {
  grid-template-columns: repeat(auto-fit, minmax(var(--crosshook-profile-grid-min, 220px), 1fr));
}

.crosshook-profile-library--list {
  /* single column list; inherits from ProfilesPage existing layout */
}

/* Profile game card */
.crosshook-profile-card {
  /* extends .crosshook-card base surface */
  display: grid;
  grid-template-rows: auto 1fr auto;
  position: relative;
  overflow: hidden;
  border-radius: var(--crosshook-radius-lg);
}

.crosshook-profile-card__art {
  aspect-ratio: var(--crosshook-profile-card-art-aspect, 2/3);
  width: 100%;
  object-fit: cover;
}

.crosshook-profile-card__footer {
  padding: 10px 12px;
  display: grid;
  gap: 4px;
}

.crosshook-profile-card__actions {
  /* overlay row: bottom of card, visible on :hover/:focus-within */
  position: absolute;
  inset-inline: 0;
  bottom: 0;
  display: flex;
  gap: 8px;
  padding: 8px;
  background: linear-gradient(to top, rgba(0, 0, 0, 0.82) 0%, transparent 100%);
  opacity: 0;
  transition: opacity var(--crosshook-transition-fast);
}

.crosshook-profile-card:hover .crosshook-profile-card__actions,
.crosshook-profile-card:focus-within .crosshook-profile-card__actions {
  opacity: 1;
}
```

Controller mode override (in the `[data-crosshook-controller-mode='true']` block):

```css
[data-crosshook-controller-mode='true'] .crosshook-profile-card__actions {
  opacity: 1; /* always visible */
  position: static;
  background: none;
}
```

Two new CSS variables to add to `variables.css`:

```css
--crosshook-profile-grid-min: 220px;
--crosshook-profile-card-art-aspect: 2/3;
```

Controller mode override in the existing `:root[data-crosshook-controller-mode='true']` block:

```css
--crosshook-profile-grid-min: 280px;
```

## MetadataStore Reuse Analysis for Issue #52

### Public cache API (already fully general-purpose)

`MetadataStore` exposes three public methods that are already source-agnostic and reusable verbatim for Steam metadata:

```rust
// src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:469–491
pub fn get_cache_entry(&self, cache_key: &str) -> Result<Option<String>, MetadataStoreError>
pub fn put_cache_entry(&self, source_url: &str, cache_key: &str, payload: &str, expires_at: Option<&str>) -> Result<(), MetadataStoreError>
pub fn evict_expired_cache_entries(&self) -> Result<usize, MetadataStoreError>
```

These delegate directly to `cache_store.rs` which performs upsert-on-conflict into `external_cache_entries`. The table accepts any `cache_key` — there is no source-coupling in the schema.

### Namespace convention

ProtonDB uses `PROTONDB_CACHE_NAMESPACE = "protondb"` and `cache_key_for_app_id` in `protondb/models.rs:9–22`:

```rust
pub fn cache_key_for_app_id(app_id: &str) -> String {
    format!("{PROTONDB_CACHE_NAMESPACE}:{}", app_id.trim())
}
```

Issue #52 should follow the exact same pattern. Define `STEAM_METADATA_CACHE_NAMESPACE = "steam"` in a new `steam_metadata/models.rs` and a `cache_key_for_steam_metadata(app_id: &str) -> String` function returning `"steam:{app_id}"`. This keeps namespaces readable in the DB and avoids key collisions.

### Cache-with-stale-fallback pattern

The ProtonDB client (`protondb/client.rs`) implements a four-stage lookup sequence:

1. Check valid (non-expired) cache → serve immediately
2. Fetch live from API → persist + serve
3. On live failure → load stale cache row regardless of expiry → serve with `is_stale: true`
4. If no stale cache → return `Unavailable`

This is exactly the pattern issue #52 should use for Steam metadata JSON. The only new concern is the image binary — that requires a parallel filesystem cache step not present in ProtonDB.

### `with_sqlite_conn` for custom queries

`MetadataStore::with_sqlite_conn` (`mod.rs:133–153`) gives direct `Connection` access without requiring a `Default` return type. ProtonDB uses this in `load_cached_lookup_row` to run custom queries with `allow_expired` branching. The Steam metadata client can use the same escape hatch.

### Payload size guard

`MAX_CACHE_PAYLOAD_BYTES = 524_288` (512 KiB) in `models.rs:152` is enforced by `cache_store::put_cache_entry`. JSON metadata payloads from Steam API will be well under this cap. Image binaries must never go into `payload_json` — they belong in the filesystem cache only.

## Shared Infrastructure: Issue #52 vs. Issue #53

### What is genuinely shared

| Infrastructure                                           | #53 ProtonDB | #52 Steam Metadata  | Notes                                                                     |
| -------------------------------------------------------- | ------------ | ------------------- | ------------------------------------------------------------------------- |
| `MetadataStore` (Tauri `State`)                          | Yes          | Yes                 | Identical usage: `state: State<'_, MetadataStore>`                        |
| `external_cache_entries` table                           | Yes          | Yes                 | Same table, different namespace prefix in `cache_key`                     |
| `put_cache_entry` / `get_cache_entry`                    | Yes          | Yes                 | Call sites are identical; no abstraction needed                           |
| `reqwest::Client` (ad-hoc per request, no shared client) | Yes          | Yes                 | Both construct a client with timeout + user-agent per call                |
| `REQUEST_TIMEOUT_SECS` constant (6 s in ProtonDB)        | Yes          | Yes (needs its own) | Different APIs; each module should own its timeout constant               |
| Cache-then-live-then-stale-fallback sequence             | Yes          | Yes (JSON only)     | Image fetch adds a third concern: filesystem binary cache                 |
| `normalize_app_id` / `cache_key_for_app_id` pattern      | Yes          | Yes (new variant)   | Same shape; not worth sharing a single function (namespace differs)       |
| IPC command signature: `(app_id, force_refresh, store)`  | Yes          | Yes                 | Tauri command shape is identical                                          |
| Frontend hook pattern (`useProtonDbLookup` shape)        | Yes          | Yes (new hook)      | `useGameMetadata` should mirror the returned interface shape              |
| Frontend types file (`src/types/protondb.ts` pattern)    | Yes          | Yes (new file)      | New `src/types/steam-metadata.ts` following the same interface convention |

### Rule-of-three assessment: should a generic `ExternalDataFetcher` trait be extracted?

**Verdict: No.** Two callers do not meet the rule-of-three threshold. The shared logic (cache lookup + live fetch + stale fallback) is ~30 lines in the client; abstracting it into a generic trait would require parameterizing over the result type, TTL, URL construction strategy, and stale-handling — producing a trait with 4–5 associated types and a `fetch_live` async method. The incidental complexity exceeds the duplication cost at two callers. If a third metadata source is added (e.g., IGDB, SteamSpy), revisit.

The correct approach is to keep `protondb/client.rs` and a new `steam_metadata/client.rs` as independent modules that follow the same pattern by convention. Document the pattern in module-level doc comments rather than enforcing it via a trait.

## Image Cache Module Design

### Scope decision: crosshook-core module, not feature-specific

Image caching should be implemented as a module inside `crosshook-core` because:

- It requires filesystem I/O, which belongs in the core layer per the architecture rule (business logic in `crosshook-core`, IPC thin wrappers in `src-tauri`)
- The `MetadataStoreError` type already has an `Io` variant covering path-based I/O failures
- `db.rs` shows the established pattern: `create_dir_all` + `Permissions::from_mode(0o700)` for directory creation; `0o600` for sensitive files — the same permission model applies to the image cache directory
- Other data directories use `BaseDirs::data_local_dir()` (resolves to `~/.local/share/crosshook/`); the image cache fits naturally at `~/.local/share/crosshook/image_cache/`

### Proposed module: `crosshook-core/src/metadata/image_cache.rs`

Locate this as a sub-module of `metadata/` to keep it co-located with the `external_cache_entries` index it mirrors:

```
metadata/
  mod.rs        -- add pub fn get_image_cache_path, store_image, load_image
  image_cache.rs -- new: filesystem image operations
  cache_store.rs -- existing: external_cache_entries SQL (unchanged)
```

**Index in `external_cache_entries`, binary on filesystem.** The design uses two stores:

- `external_cache_entries`: record with `cache_key = "steam_image:{app_id}:{image_type}"`, `payload_json = null` (or a small JSON envelope with `local_path`, `fetched_at`, `content_type`, `file_size`), `expires_at` for TTL eviction
- Filesystem: actual image bytes at `~/.local/share/crosshook/image_cache/{app_id}/{image_type}.{ext}`

This matches how `community/taps.rs` handles Git clones: metadata row in SQLite, actual data in a well-known filesystem path under the crosshook data directory.

### What goes in `image_cache.rs`

```rust
// src/crosshook-native/crates/crosshook-core/src/metadata/image_cache.rs
pub fn image_cache_dir() -> Result<PathBuf, MetadataStoreError>
pub fn image_cache_path(app_id: &str, image_type: &str, ext: &str) -> Result<PathBuf, MetadataStoreError>
pub fn store_image(path: &Path, bytes: &[u8]) -> Result<(), MetadataStoreError>
pub fn load_image(path: &Path) -> Result<Option<Vec<u8>>, MetadataStoreError>
pub fn evict_image(path: &Path) -> Result<(), MetadataStoreError>
```

`store_image` should use `create_dir_all` + `Permissions::from_mode(0o700)` for the directory (matching `db.rs:26`) and write the file with `0o600` permissions (matching `db.rs:40`). No existing `fs::write` abstraction covers both; implement directly.

### Image validation as a shared utility

**No image parsing dependency.** The codebase has no image processing library (`image` crate is absent from `Cargo.toml`). For Phase 1, validation should be limited to:

- Content-Type header check on the HTTP response (accept `image/jpeg`, `image/png`, `image/webp`)
- File size cap (reject > 2 MiB before writing to disk — analogous to `MAX_CACHE_PAYLOAD_BYTES`)
- No magic byte validation needed for Phase 1; the browser's `<img>` tag handles corrupt/invalid images gracefully

A `validate_image_response(content_type: &str, content_length: Option<u64>) -> Result<(), String>` function in `image_cache.rs` is the right boundary. Do not add the `image` crate for decoding — it adds ~15 MB to the binary and is not needed.

### MetadataStore methods to add

```rust
// to add to MetadataStore in mod.rs
pub fn get_image_cache_entry(&self, cache_key: &str) -> Result<Option<ImageCacheEntry>, MetadataStoreError>
pub fn put_image_cache_entry(&self, cache_key: &str, local_path: &str, fetched_at: &str, expires_at: Option<&str>) -> Result<(), MetadataStoreError>
```

`ImageCacheEntry` (a small struct with `local_path`, `fetched_at`, `expires_at`) lives in `image_cache.rs`. The actual binary is read from `local_path` on demand.

## Frontend Component Reuse for Issue #52

### `useGameMetadata` hook (mirrors `useProtonDbLookup`)

`useProtonDbLookup` (`src/crosshook-native/src/hooks/useProtonDbLookup.ts`) is the canonical model. The hook:

- Takes a single `appId: string` parameter
- Manages `loading`, `state` (idle/loading/ready/stale/unavailable), and result state
- Uses a `requestIdRef` race-condition guard for concurrent calls
- Returns a stable `refresh()` callback for force-refresh

`useGameMetadata(appId: string)` should follow the identical shape. Key differences:

- Result type: `GameMetadataResult` with `name`, `cover_url`, `header_url`, `background_url`, `description` fields (from a new `src/types/steam-metadata.ts`)
- The image URLs returned from IPC are local `asset://` paths or base64 data URLs served by the Tauri asset protocol — not remote URLs
- The hook does not need to know about filesystem paths; the backend IPC command resolves the local path and returns a safe asset URL

### `GameCoverArt` component

This is a reusable presentation component, not a data-fetching component. It should accept `src: string | null` and `alt: string` as props and render:

- The image when `src` is present
- A styled placeholder (initials or generic game icon) when `src` is null or on load error
- An `onError` handler that switches to the placeholder without crashing

**Reuse potential beyond Profiles page:** The CommunityBrowser (`CommunityBrowser.tsx`) currently shows game entries as text rows. If cover art is ever shown there, `GameCoverArt` is directly reusable since it is props-driven with no context coupling.

### Shared loading/error/placeholder pattern

The existing `ProtonDbLookupCard` already implements the `state → banner` pattern (`idle`, `loading`, `stale`, `unavailable`) with consistent CSS class naming (`crosshook-protondb-card__banner--loading`, `--stale`, `--unavailable`). The `GameCoverArt` component should follow the same pattern:

- `loading` state: a shimmer placeholder using `--crosshook-panel-bg` token
- `error`/missing state: a fixed-dimension placeholder with a fallback icon
- Do not create a generic `<AsyncImage>` abstraction — `GameCoverArt` is the only image component in scope

## KISS Assessment for Issue #52 (Dual-Source: Steam API + SteamGridDB)

### Complexity breakdown

| Approach                                                      | Complexity                                                                                                                                 | Value                                                                  | Verdict                                                             |
| ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------- | ------------------------------------------------------------------- |
| **Steam CDN direct** (`cdn.akamai.steamstatic.com`)           | Very low — no auth, deterministic URL pattern, no API key needed: `https://cdn.akamai.steamstatic.com/steam/apps/{id}/library_600x900.jpg` | High — covers all Steam-library games; works offline after first fetch | Phase 1 only; sufficient for the Profiles page use case             |
| **Steam Store API** (`store.steampowered.com/api/appdetails`) | Low — single GET, `appid` param, no auth for basic fields; returns `name`, `short_description`, `header_image`, `background`               | High for metadata; header_image URL is reliable                        | Phase 1 metadata source                                             |
| **SteamGridDB**                                               | Medium — requires API key management, different key/value shape, rate limiting, user account on their service                              | Low marginal value over Steam CDN for Phase 1                          | Defer to Phase 2; adds API key storage concern with no clear payoff |
| **Both simultaneously (fan-out)**                             | High — dual HTTP fetches, merge logic, conflict resolution if Steam CDN image differs from SteamGridDB                                     | Unclear benefit for Phase 1                                            | Over-engineered; do not include in Phase 1                          |

**Recommendation: Phase 1 is Steam-only.** The Steam Store API provides game name, description, and a `header_image` URL. The Steam CDN provides the portrait cover (`library_600x900.jpg`) via a deterministic URL with no API call needed. Together these cover the entire Profiles page use case with zero API key infrastructure.

SteamGridDB becomes relevant only if non-Steam game art is needed (games without a `steam_app_id`). That is a different feature scope.

### Phase 1 scope

- Backend: `steam_metadata` module in `crosshook-core` implementing the four-stage cache-then-live-then-stale-fallback pattern for the Steam Store API JSON; separate `image_cache` module for filesystem image storage
- IPC: `steam_metadata_lookup(app_id, force_refresh)` command in `src-tauri/src/commands/steam.rs` (or a new `steam_metadata.rs`); separate `get_game_cover_art(app_id)` command that returns a local asset path or base64
- Frontend: `useGameMetadata(appId)` hook, `GameCoverArt` component
- Storage: `external_cache_entries` for JSON metadata (namespace `steam:`), `image_cache/` filesystem directory for binary images, no new SQLite table needed for Phase 1

### Phase 2 scope

- SteamGridDB as a fallback/supplement source when `steam_app_id` is not available or image quality is preferred over Steam CDN
- API key storage in TOML settings (user-editable) per the persistence policy
- A dedicated `game_image_cache` SQLite table if image metadata needs richer query patterns (e.g., bulk eviction by source, per-game status)

## Architectural Patterns

- **Radix UI as the UI primitive layer**: Both `@radix-ui/react-tabs` (already used in `ContentArea.tsx`) and `@radix-ui/react-select` (wrapped by `ThemedSelect`) are already installed and in use. No new dependency needed.
- **CSS variable token system**: `variables.css` already defines `--crosshook-subtab-min-height` and `--crosshook-subtab-padding-inline` with controller-mode overrides — the design system anticipates sub-tabs and has pre-allocated tokens for them. Also `--crosshook-panel-padding` and `--crosshook-card-padding` cover card/section spacing.
- **CollapsibleSection as the disclosure primitive**: Controlled/uncontrolled, accepts a `meta` slot for badges, already styled. The stripping rules in `collapsible-section.css` let nested panels appear unstyled inside a CollapsibleSection wrapper.
- **BEM-like `crosshook-*` class convention**: All components use `crosshook-<block>__<element>` patterns. For sub-tabs specifically, `crosshook-subtab-row` / `crosshook-subtab` / `crosshook-subtab--active` are already the canonical classes in `theme.css` — use these, do not invent new BEM names.
- **Component extraction pattern**: `GamescopeConfigPanel`, `MangoHudConfigPanel`, `LaunchOptimizationsPanel`, `SteamLaunchOptionsPanel`, and `CustomEnvironmentVariablesSection` demonstrate the target shape: self-contained props interface, internal state only, `CollapsibleSection`/`ThemedSelect` for structure, no context imports.
- **Context at page level**: `useProfileContext`, `useProfileHealthContext`, and `usePreferencesContext` are consumed at page level in `ProfilesPage.tsx`. Extracted sub-panels should receive their slice of data as props — they must not call these hooks directly.
- **`OptionalSection` is a private `<details>` wrapper inside `ProfileFormSections.tsx`**: Not exported; uses hardcoded inline style objects (`optionalSectionStyle`, `optionalSectionSummaryStyle`) instead of CSS classes. Inconsistent with the CSS variable pattern; replace with `CollapsibleSection defaultOpen={false}` when refactoring.

## KISS Assessment (ProfileFormSections restructuring)

| Approach                                            | Complexity                                                                                                         | Value                                                                                         | Verdict                                                                                      |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| **Visual containers / clear boundaries**            | Low — CSS-only, no new components                                                                                  | Medium — reduces visual noise, does not address discoverability                               | Good baseline step; do it as part of any other option                                        |
| **Promote critical sections out of Advanced**       | Low — move JSX up in `ProfilesPage.tsx`, delete or reduce the outer CollapsibleSection                             | High — mandatory fields (Game Path, Runner Method) are always visible                         | Should be done regardless; lowest risk, meaningful gain                                      |
| **Sub-tabs at `ProfilesPage` level**                | Low-Medium — `@radix-ui/react-tabs` already installed; one new CSS block + tab wrappers in `ProfilesPage.tsx` only | High — surfaces all sections as named, accessible tabs without touching `ProfileFormSections` | Correct approach: tabs live in `ProfilesPage`, `ProfileFormSections` stays a linear renderer |
| **Tabs embedded inside `ProfileFormSections`**      | Medium — forces dual rendering paths for `reviewMode` (InstallPage) and normal mode                                | Negative — wrong UX for compact review modal in `InstallPage`                                 | Do not do this; breaks the existing `reviewMode` contract                                    |
| **Full page split (separate routes per section)**   | High — new routes, navigation state, breadcrumbs                                                                   | Low marginal gain over sub-tabs                                                               | Overkill                                                                                     |
| **Contextual/smart settings (game-type detection)** | High — requires metadata that does not exist                                                                       | Low — metadata infrastructure absent                                                          | Scope creep; do not include                                                                  |
| **Drag-and-drop section reordering**                | High — needs DnD library, persistence layer for order preferences                                                  | Low — does not solve clutter                                                                  | Poor effort-to-impact ratio                                                                  |
| **Search/filter for ~15–20 fields**                 | Medium                                                                                                             | Near-zero — search is justified for 50+ settings                                              | Not applicable at this form size                                                             |

## Security Constraints

These patterns must be preserved as shared utilities during any restructuring. Do not inline or duplicate them.

- **`RESERVED_CUSTOM_ENV_KEYS`** (`CustomEnvironmentVariablesSection.tsx:6-10`): Client-side constant that mirrors `RESERVED_CUSTOM_ENV_KEYS` in `crosshook-core/src/launch/request.rs`. This is a **defense-in-depth guard for manually-entered env vars** — the ProtonDB suggestion path is already sanitized at the backend (`aggregation.rs::safe_env_var_suggestions()` applies key regex, value character filtering, and reserved-key stripping before IPC). The frontend constant remains the authoritative guard for user-typed input and a second line of defense for any future code paths. Keep it in `CustomEnvironmentVariablesSection` or extract to `utils/envVars.ts` if the component is split; never inline or remove it.
- **`customEnvKeyFieldError` / `customEnvRowError`** (`CustomEnvironmentVariablesSection.tsx:38–89`): Pure validation functions for env var keys and values. Already well-isolated. If the env var section is split, extract to `utils/envVars.ts` rather than duplicating.
- **`validate_name()` / `profile_path()` gate** (`crosshook-core/src/profile/toml_store.rs:468–521`): All filesystem operations for profile names go through this Rust-side gate. Backend-only; no change needed from UI restructuring. Do not add any new direct `fs::` calls in profile commands that bypass this gate.
- **Path fields are intentionally free-form**: Game path, executable path, and working directory fields have no shared client-side validator — this is deliberate. Do not add one.
- **Tab/navigation state**: Use `sessionStorage` locally in `ProfilesPage.tsx` if persistence is needed. No shared utility required.
- **Image cache filesystem security**: Follow the `db.rs` permission model — directory at `0o700`, files at `0o600`. The `MetadataStoreError::SymlinkDetected` guard in `db.rs:15` should be replicated for the image cache path. Never follow symlinks when writing fetched content to disk.
- **Image content-type validation**: Reject any HTTP response with a `Content-Type` other than `image/jpeg`, `image/png`, or `image/webp` before writing to disk. This boundary check belongs in `image_cache.rs`, not in the IPC command layer.
- **Image URL construction**: Steam CDN URLs must be constructed server-side (in `crosshook-core`) from the `steam_app_id` value, never from frontend-provided strings. The IPC command receives only `app_id` — the backend assembles the URL. This prevents open-redirect / SSRF via crafted app_id values if the `normalize_app_id` gate (which enforces numeric-only) is applied.

## Modularity Design

### Recommended module boundaries for splitting `ProfileFormSections.tsx`

The monolith should be split into section components that `ProfilesPage.tsx` composes inside tab panels, while `ProfileFormSections` continues to render them linearly for `InstallPage`'s `reviewMode`:

1. **`ProfileIdentitySection`** — Profile name, profile selector dropdown, profile load/pin control. Can be hoisted as a permanent-visible header above the tabs in `ProfilesPage`.

2. **`GameSection`** — Game name, game path browse, and (new for #52) `GameCoverArt` thumbnail when `steam_app_id` is set. Purely controlled fields, no internal state.

3. **`RunnerMethodSection`** — Runner method select + helper text. Pure field; switching the method controls what other sections render.

4. **`RuntimeSection`** (method-conditional) — Steam App ID, Prefix Path, Proton Path, Working Directory override, AutoPopulate, ProtonDB lookup.

5. **`TrainerSection`** — Trainer path, trainer type select, trainer loading mode, trainer version display + manual set. `TrainerVersionSetField` and its IPC call stay file-local here.

6. **`EnvVarsSection`** — Already extracted as `CustomEnvironmentVariablesSection`. No further split needed; security-critical validation logic must stay inside this component (see Security Constraints above).

7. **`LauncherMetadataSection`** — Launcher name and icon fields. Consolidate the currently split `LauncherMetadataFields` (private) and method-conditional blocks.

### Shared vs. feature-specific

- **Shared (promote to `ui/`)**: `ProtonPathField` — currently private to `ProfileFormSections.tsx`; check against `ui/ProtonPathField.tsx` before extracting (may be same or diverged). `FieldRow` is resolved: merge into `ui/InstallField.tsx` with `id` prop addition (see Abstraction vs. Repetition).
- **Feature-specific (keep local or delete)**: `OptionalSection` — replace with `CollapsibleSection defaultOpen={false}`.
- **Shared context for sub-tab state**: A single `useState<TabId>` in `ProfilesPage.tsx` is sufficient. Do not create a context for tab selection.

## Abstraction vs. Repetition

- **`FieldRow` vs. `InstallField`**: `FieldRow` (private, 10+ usages in `ProfileFormSections.tsx`) and `ui/InstallField.tsx` (exported) are the same pattern at slightly different API surfaces — `InstallField` has `browseMode`, `browseFilters`, `browseTitle`, and `className` but lacks `id` (uses no `useId`); `FieldRow` has `id` via `useId` but no browse-mode/filter props. Resolution: add `id` support to `InstallField`, migrate all `FieldRow` usages to it, and delete the private copy. No new component needed.
- **`ProtonPathField`** appears twice with near-identical props. A version already exists at `src/crosshook-native/src/components/ui/ProtonPathField.tsx` — verify it is the same component before promoting the private one.
- **`OptionalSection`** — one-off inline `<details>` with hardcoded inline styles. Replace all usages with `CollapsibleSection defaultOpen={false}` and delete.
- **The ProtonDB overwrite confirmation dialog** (lines 549–652 in `ProfileFormSections.tsx`) is 100 lines of inline JSX. Extract to a named `ProtonDbConflictDialog` component.
- **Do not abstract the tab definitions** into a config array — four to five tabs with conditional rendering differences do not warrant a generic tab-registry pattern.
- **Do not abstract `useProtonDbLookup` and `useGameMetadata` into a generic hook** — two callers with different result shapes do not warrant an abstraction layer.

## Interface Design

The correct architecture keeps `ProfileFormSections` as a linear renderer and adds the tab layer only at `ProfilesPage` level:

```
ProfilesPage
  ├── PageBanner (always visible)
  ├── Health/Rename toasts (always visible)
  ├── Panel
  │   ├── Guided Setup header (always visible)
  │   ├── Profile selector bar (always visible when profiles exist)
  │   └── Tabs.Root  ← NEW: replaces the "Advanced" CollapsibleSection
  │       ├── Tabs.List
  │       │   ├── Tabs.Trigger "Setup"       (Profile Identity + Game + Runner Method)
  │       │   ├── Tabs.Trigger "Runtime"     (Proton/Steam paths, AutoPopulate, ProtonDB)
  │       │   ├── Tabs.Trigger "Trainer"     (hidden/disabled when method = 'native')
  │       │   ├── Tabs.Trigger "Environment" (custom env vars)
  │       │   └── Tabs.Trigger "Launcher"    (disabled/empty for native profiles)
  │       ├── Tabs.Content "setup"    → <ProfileIdentitySection> + <GameSection> + <RunnerMethodSection>
  │       │                              GameSection may include <GameCoverArt> when steam_app_id set
  │       ├── Tabs.Content "runtime"  → <RuntimeSection>
  │       ├── Tabs.Content "trainer"  → <TrainerSection>
  │       ├── Tabs.Content "env"      → <CustomEnvironmentVariablesSection>
  │       └── Tabs.Content "launcher" → <LauncherMetadataSection> + <LauncherExport>
  └── ProfileActions bar (always visible, below the panel)

InstallPage (unchanged)
  └── ProfileReviewModal
      └── ProfileFormSections reviewMode={true}  ← linear render, no tabs
```

`ProfileFormSections` stays unchanged as a linear renderer. The extracted section components are composed inside both the tab panels (ProfilesPage) and `ProfileFormSections` (for reviewMode compatibility).

### CSS needed

No new CSS file required. The subtab classes are **fully implemented** in `theme.css:104-135`:

- `.crosshook-subtab-row` — pill-style flex container with border and muted background (`theme.css:104`)
- `.crosshook-subtab` — individual tab button using `--crosshook-subtab-min-height` and `--crosshook-subtab-padding-inline` tokens (`theme.css:115`)
- `.crosshook-subtab--active` — accent gradient + white text for the selected tab (`theme.css:131`)
- Responsive override at narrow widths: `.crosshook-subtab` gets `flex: 1 1 0` so tabs fill the row (`theme.css:3214`)

The Radix `Tabs.Trigger` elements should receive `className="crosshook-subtab"` and the active state applied via `data-state="active"` selector or the `--active` class. No new styles needed.

## Testability Patterns

- **No test framework is currently configured** (noted in `CLAUDE.md`). Extraction produces individually testable components but testing is not a blocker.
- **The section components are pure**: `GameSection`, `TrainerSection`, etc. take controlled props and fire `onUpdateProfile` callbacks — straightforward to test when a framework is added.
- **Tab state is trivially testable**: A single `activeTab` string in `ProfilesPage`; no async logic.
- **IPC-backed components** (`TrainerVersionSetField`, `SteamLaunchOptionsPanel`, `AutoPopulate`) only call `invoke()` — the existing pattern of passing callbacks from the hook layer keeps them boundary-testable.
- **`image_cache.rs` is unit-testable**: `store_image` / `load_image` operate on `&Path` and `&[u8]` — straightforward to test with `tempfile` (already in `crosshook-core`'s dev-dependencies).
- **`validate_image_response` is trivially unit-testable**: Pure function over strings; no filesystem or network I/O.

## Build vs. Depend

| Decision                             | Recommendation                                                                                                                                      |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Tab primitives**                   | `@radix-ui/react-tabs` v1.1.13 — already installed; `orientation="vertical"` available if sidebar-style is ever needed. Do not build custom.        |
| **Collapsible/disclosure animation** | `@radix-ui/react-accordion` is the natural upgrade path if `<details>` animation is needed, but is not required for this refactor.                  |
| **Select component**                 | `ThemedSelect` wrapping `@radix-ui/react-select` — already done, keep it.                                                                           |
| **Collapsible**                      | `CollapsibleSection` (existing) — already done.                                                                                                     |
| **New dependencies**                 | None needed. `@headlessui/react`, `@ark-ui/react`, and shadcn/ui are all redundant or incompatible.                                                 |
| **Animation**                        | Not needed. `--crosshook-transition-fast` and `--crosshook-transition-standard` CSS variables are sufficient.                                       |
| **Image processing (Rust)**          | Do not add the `image` crate. Content-Type validation + size cap at the HTTP boundary is sufficient for Phase 1; the browser renders the raw bytes. |
| **HTTP client (Rust)**               | `reqwest` 0.12 is already in `crosshook-core/Cargo.toml`. No new dependency needed for Steam CDN / Steam Store API calls.                           |
| **Image loading (frontend)**         | Use the native `<img>` element with `onError` fallback. Do not add an image-loading library.                                                        |
| **SteamGridDB client**               | Do not add for Phase 1. Revisit if non-Steam game art becomes a requirement.                                                                        |

## Gotchas and Edge Cases

- **`ProfileFormSections` is used at three callsites with incompatible layout needs**: `ProfilesPage.tsx` (full editing, tab layout appropriate), `InstallPage.tsx` (compact `reviewMode` modal — tabs would be wrong UX here), and `OnboardingWizard.tsx` (type import only; wizard builds its own step form). Embedding tabs inside `ProfileFormSections` would break `InstallPage`. Tabs must live at `ProfilesPage` level only.
- **`OptionalSection` uses hardcoded inline style objects**: `optionalSectionStyle` and `optionalSectionSummaryStyle` in `ProfileFormSections.tsx` lines 60–75. Inconsistent with the CSS variable pattern; replace during extraction.
- **`ProfileFormSections` exports `deriveSteamClientInstallPath` as a re-export** (line 113): `export { deriveSteamClientInstallPath } from '../utils/steam'`. Other files may import this utility via `ProfileFormSections` — check before splitting the file.
- **Method-conditional rendering inside a single component**: `RuntimeSection` renders completely different fields for `steam_applaunch` vs. `proton_run`. The conditional blocks are large and must be preserved exactly when extracting.
- **`ProtonInstallOption` type is imported by `OnboardingWizard`, `InstallGamePanel`, `UpdateGamePanel`, and `ui/ProtonPathField.tsx`** from `ProfileFormSections` directly. If the file is split, this type must be re-exported from a stable location (e.g., `src/types/index.ts` or `ui/ProtonPathField.tsx`) to avoid breaking all four importers.
- **`RESERVED_CUSTOM_ENV_KEYS` must not be duplicated**: The constant in `CustomEnvironmentVariablesSection.tsx` mirrors a Rust-side set in `crosshook-core`. Any refactor that touches env var handling must keep this in a single location — either in the component or extracted to `utils/envVars.ts`. See Security Constraints.
- **`reqwest::Client` is constructed ad-hoc in `protondb/client.rs`**: No shared HTTP client singleton exists. The Steam metadata client must replicate this pattern (build a client per `fetch_live_*` call). Do not attempt to share a single `reqwest::Client` across modules — this would require changing the MetadataStore initialization path.
- **Steam CDN image URLs for library art are not guaranteed stable**: The `library_600x900.jpg` path pattern is not part of a public API contract. The image cache TTL should be relatively long (7–30 days) to reduce re-fetches, and the fallback to a placeholder must be robust.
- **`external_cache_entries` cache key uniqueness constraint**: The table has `UNIQUE(cache_key)` enforced by the upsert pattern. If `steam_metadata:{app_id}` and `steam_image:{app_id}:{image_type}` are both stored, they are separate rows sharing no primary-key space with `protondb:{app_id}` rows — no collision risk, but verify the namespace prefix is consistent across all call sites.
- **`MAX_CACHE_PAYLOAD_BYTES` (512 KiB) must not be applied to image binaries**: Images stored to `payload_json` would hit this limit for any reasonable cover art. The `put_cache_entry` function will silently store `NULL` instead of the payload when the limit is exceeded (see `cache_store.rs:37–47`). Image binaries must go to the filesystem only; use `put_image_cache_entry` (which does not write to `payload_json`).

## Open Questions

1. **Should the active sub-tab be persisted across page navigation?** `sessionStorage` is already used for banner/toast dismissal in `ProfilesPage.tsx` — the same pattern could persist the active tab key.
2. **Does the "Launcher" tab show a disabled state or hide for native profiles?** Currently `supportsLauncherExport` hides `LauncherExport` entirely; an always-visible but conditionally disabled tab may be more discoverable.
3. **`FieldRow` / `InstallField` merger**: Resolved — `InstallField` is the canonical component; add `id` prop support and migrate `FieldRow` usages to it. See Abstraction vs. Repetition.
4. **ProtonDB conflict resolution dialog**: The 100-line inline dialog (lines 549–652) should become a named `ProtonDbConflictDialog` component before or during the `RuntimeSection` extraction.
5. **Phase 1 image cache TTL**: What TTL is appropriate for Steam library art? 7 days is a reasonable starting point; cover art almost never changes for released games. The TTL should be a named constant in the Steam metadata module.
6. **Should `GameCoverArt` be shown in `reviewMode` (InstallPage)?** Cover art would require the `steam_app_id` to already be set on the profile at install time. This is not the common case. Start with no cover art in `reviewMode` — add it as a follow-on if needed.

## Out-of-Scope Follow-ons

These were surfaced during research but are not required for the initial UI cleanup:

- **`UnsavedChangesGuard`**: A hook wrapping `dirty` state (already in `ProfileContext`) to show a confirmation dialog on inter-page navigation. Tab switching within `ProfilesPage` does not trigger data loss — `ContentArea` uses Radix `Tabs.Content forceMount` and `ProfileContext` persists across route switches. The guard would protect against sidebar route changes while dirty. Valid UX improvement; implement as a separate issue after the layout cleanup.
- **SteamGridDB integration**: Defer entirely to Phase 2. No API key storage design or SteamGridDB client should be included in the Phase 1 implementation of issue #52.
- **`game_image_cache` SQLite table**: Not needed for Phase 1. If image metadata needs richer per-row query capabilities (e.g., bulk eviction by source, per-game status queries), add a migration in Phase 2.

## Other Docs

- `docs/plans/ui-enhancements/research-security.md` -- security constraints detail (reserved env key contract, profile name path traversal gate, verified ProtonDB env var sanitization chain in `aggregation.rs`)
- `docs/plans/ui-enhancements/research-external.md` — dependency analysis (Radix UI version inventory, build-vs-depend table)
- `docs/plans/ui-enhancements/research-ux.md` — UX patterns, competitive analysis, proposed reusable components assessment
