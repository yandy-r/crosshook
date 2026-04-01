# Feature Spec: UI Enhancements — Profiles Page Restructuring + Game Metadata & Cover Art

## Executive Summary

The Profiles page hides its entire editing surface behind a single collapsed "Advanced" `<details>` section (`defaultOpen=false`), forcing users to click-to-reveal before doing any work. Simultaneously, profile cards are text-only — users with 10+ profiles have no visual anchor to quickly identify games. This spec unifies two scopes: (1) restructuring the Profiles page into visually distinct section cards with sub-tab navigation, and (2) integrating GitHub issue #52's game metadata and cover art via Steam Store API + optional SteamGridDB, drawing visual inspiration from a Figma concept of a **library grid with game cover art cards** where launch/favorite/edit actions are accessible directly from the card. The implementation uses zero new frontend dependencies (`@radix-ui/react-tabs` and `crosshook-subtab-*` CSS already exist), reuses the ProtonDB lookup's cache-first pattern via `MetadataStore` for Steam metadata JSON, and adds filesystem image caching at `~/.local/share/crosshook/cache/images/` tracked by a new `game_image_cache` SQLite table. SteamGridDB is deferred to Phase 3 — Phase 2 ships with Steam Store API only (no API key friction). Overall risk is LOW-MEDIUM; the primary concerns are SVG rejection for downloaded images (WARNING), path traversal in cache construction (WARNING), and preserving `CustomEnvironmentVariablesSection` local state during tab switches (WARNING — mitigated by CSS show/hide).

## External Dependencies

### APIs and Services

#### Steam Store API

- **Documentation**: <https://wiki.teamfortress.com/wiki/User:RJackson/StorefrontAPI>
- **Authentication**: None (public endpoint, no API key)
- **Key Endpoints**:
  - `GET https://store.steampowered.com/api/appdetails?appids={id}`: Game metadata (name, description, genres, categories, header_image URL)
- **Rate Limits**: Undocumented; community reports ~200 requests/minute before throttling
- **Pricing**: Free, no key required
- **Image URLs from response**:
  - `header_image`: 460x215 JPEG (primary target for Phase 2)
  - `capsule_image`: 231x87 JPEG
  - Library art: `https://cdn.cloudflare.steamstatic.com/steam/apps/{id}/library_600x900.jpg` (portrait, 2:3)
  - Hero art: `https://cdn.cloudflare.steamstatic.com/steam/apps/{id}/library_hero.jpg` (1920x620)

#### SteamGridDB API (Phase 3 — optional)

- **Documentation**: <https://www.steamgriddb.com/api/v2>
- **Authentication**: Bearer token (user-provided API key)
- **Key Endpoints**:
  - `GET /grids/steam/{id}`: Grid art (600x900 portrait)
  - `GET /heroes/steam/{id}`: Hero art (1920x620)
  - `GET /logos/steam/{id}`: Logo overlay art
- **Rate Limits**: Not officially published
- **Pricing**: Free with API key (rate-limited)

### Libraries and SDKs

| Library                  | Version  | Status                    | Purpose                                                            |
| ------------------------ | -------- | ------------------------- | ------------------------------------------------------------------ |
| `@radix-ui/react-tabs`   | ^1.1.13  | **Already installed**     | Sub-tab primitives (WAI-ARIA compliant, keyboard nav built-in)     |
| `@radix-ui/react-select` | ^2.2.6   | **Already installed**     | Used by `ThemedSelect` for dropdowns                               |
| `reqwest`                | existing | **Already in Cargo.toml** | HTTP client (reuse for Steam API, already used by ProtonDB)        |
| `rusqlite`               | existing | **Already in Cargo.toml** | SQLite driver for MetadataStore + game_image_cache                 |
| `infer`                  | ~0.16    | **New (Rust only)**       | Magic-byte MIME type detection for image validation (security: I1) |

**Zero new frontend dependencies** for any phase.

### External Documentation

- [Radix UI Tabs](https://www.radix-ui.com/primitives/docs/components/tabs): Tab primitive API reference
- [W3C WAI ARIA Tab Pattern](https://www.w3.org/WAI/ARIA/apg/): Accessibility requirements
- [Tauri v2 Asset Protocol](https://v2.tauri.app/security/csp/): CSP + `convertFileSrc` for local image rendering
- [Steam Store API Wiki](https://wiki.teamfortress.com/wiki/User:RJackson/StorefrontAPI): Endpoint reference
- [SteamGridDB API v2](https://www.steamgriddb.com/api/v2): Grid/hero/logo endpoints

## Business Requirements

### User Stories

**New user creating a first profile**

- As a new user, I want to see the profile editor fields immediately — without hunting for a collapsed "Advanced" section — so I can complete setup without guessing.
- As a user, I want to understand which fields are required vs optional so I can fill only what's needed.

**Returning user editing an existing profile**

- As a power user, I want to jump directly to the section I need (env vars, ProtonDB, trainer) without scrolling through unrelated fields.
- As a returning user, I want to see at a glance which profile is active and whether it is healthy.

**User browsing profiles with cover art (NEW — #52)**

- As a user with 10+ profiles, I want to see game cover art on profile cards so I can visually identify games at a glance.
- As a user, I want cover art to load automatically when a Steam App ID is set, without manual configuration.
- As a user on Steam Deck (limited connectivity), I want cached cover art to display offline.

**User configuring SteamGridDB (NEW — #52, Phase 3)**

- As a user who wants custom artwork, I want to enter my SteamGridDB API key in Settings and have higher-quality art automatically used.

### Business Rules

1. **Profile Identity and Game Path are always visible**: Required fields must not live behind any collapsed section.
2. **Runner Method gates section visibility**: `steam_applaunch` shows Steam fields, `proton_run` shows Proton fields, `native` shows only Working Directory.
3. **ProtonDB and Environment Variables stay co-located**: ProtonDB's "Apply" action writes to `custom_env_vars`. Separating them across tabs forces unnecessary tab switching.
4. **Action bar is always visible**: Save, Delete, Duplicate, Rename must never be inside a tab panel.
5. **Cover art is enhancement-only (NEW)**: Missing art must never block profile load, edit, save, or launch. Art slot is hidden when no art is available — no empty placeholders.
6. **Image fetch is non-blocking (NEW)**: Profile form renders immediately; cover art loads asynchronously. Matches ProtonDB lookup's loading/stale/unavailable states.
7. **Fallback chain (NEW)**: SteamGridDB (if API key configured) → Steam Store header_image → stale cached image → hidden art slot (text-only).
8. **`injection.*` fields must not be surfaced**: Present in `GameProfile` but intentionally absent from all form components. Must not be exposed during restructuring.
9. **`ProfileFormSections` is shared**: Used by `ProfilesPage` (full editor) and `InstallPage` (`reviewMode` modal). Tabs must live at `ProfilesPage` level only.
10. **Disclosure capped at one level**: No nested collapsibles within cards.

### Edge Cases

| Scenario                                    | Expected Behavior                                              | Notes                                            |
| ------------------------------------------- | -------------------------------------------------------------- | ------------------------------------------------ |
| Native launch method selected               | Trainer card hidden, Runtime shows only Working Directory      | Conditional rendering already exists             |
| New profile (no data)                       | All cards default open; cover art slot hidden (no app_id yet)  | Quick setup flow                                 |
| Profile with steam_app_id but no cached art | Cover art loads asynchronously; shimmer skeleton during load   | Non-blocking                                     |
| Steam Store API unavailable                 | Stale cache fallback; text-only if no cache                    | No error surfaced                                |
| Sub-tab switch with dirty state             | No confirmation needed — state persists in context             | Tab switching is purely visual                   |
| `CustomEnvironmentVariablesSection` unmount | Must use CSS show/hide, not conditional rendering              | Local `rows` state would be lost on unmount (W1) |
| App ID is non-numeric string                | Rejected at `GameImageStore` boundary; no filesystem operation | Path traversal prevention (W6)                   |
| Downloaded image is SVG                     | Rejected by magic-byte validation before write to disk         | XSS prevention (I1)                              |

### Success Criteria

- [ ] A new user can create a working profile without expanding a collapsible section
- [ ] Profile health status is visible at a glance when a profile is selected
- [ ] Profile cards display game cover art when Steam App ID is available and art is cached
- [ ] Images are cached locally; cached art displays when offline
- [ ] Metadata fetch does not block profile functionality (load, edit, save, launch)
- [ ] All existing functionality preserved — nothing removed, only reorganized
- [ ] Layout consistent with existing CrossHook design patterns (`crosshook-panel`, `crosshook-*` CSS classes)
- [ ] Keyboard and controller navigation preserved (F2 rename, focus zones, gamepad D-pad)

## Technical Specifications

### Architecture Overview

```text
Current:
  ProfilesPage
    └── CollapsibleSection("Advanced", defaultOpen=false)  ← EVERYTHING hidden
          ├── ProfileFormSections (1,144 lines, ALL fields)
          ├── HealthIssues (nested collapsible)
          └── ProfileActions

Proposed (Phase 1 — Cards with cover art slots):
  ProfilesPage
    ├── ProfileSelectorBar (always visible)
    │     ├── ThemedSelect (profile dropdown)
    │     ├── HealthBadge, OfflineStatusBadge, VersionBadge
    │     └── Refresh button
    ├── Panel: Core (always open)
    │     ├── GameCoverArt (conditional: when steam_app_id set + art cached)
    │     ├── ProfileIdentity + Game + RunnerMethod
    │     └── GameMetadataBar (genres, description — conditional)
    ├── Panel: Runtime (collapsible, default open)
    │     └── Steam/Proton/Native fields + AutoPopulate + ProtonDB
    ├── Panel: Environment (collapsible, default open)
    │     └── CustomEnvVars + ProtonDB lookup
    ├── Panel: Trainer (collapsible, default open, conditional)
    │     └── Trainer path/type/loading mode/version
    ├── Panel: Diagnostics (conditional, when issues exist)
    │     └── Health Issues + stale info
    ├── ProfileActionsBar (always visible)
    └── Panel: Launcher Export (existing separate section)

Backend (NEW — #52):
  crosshook-core/src/
    ├── steam_metadata/
    │     ├── client.rs      ← Steam Store API client (mirrors protondb/client.rs)
    │     └── models.rs      ← SteamAppDetails, SteamGenre, lookup result types
    ├── game_images/
    │     ├── client.rs      ← Image download from Steam CDN + SteamGridDB
    │     ├── cache.rs       ← Filesystem cache manager with validation
    │     └── models.rs      ← ImageType, ImageSource enums
    └── metadata/
          └── game_image_store.rs  ← SQLite CRUD for game_image_cache table
```

### Data Models

#### game_image_cache (NEW — Migration v14)

```sql
CREATE TABLE IF NOT EXISTS game_image_cache (
    cache_id         TEXT PRIMARY KEY,
    steam_app_id     TEXT NOT NULL,
    image_type       TEXT NOT NULL DEFAULT 'cover',
    source           TEXT NOT NULL DEFAULT 'steam_cdn',
    file_path        TEXT NOT NULL,
    file_size         INTEGER NOT NULL DEFAULT 0,
    content_hash     TEXT NOT NULL DEFAULT '',
    mime_type        TEXT NOT NULL DEFAULT 'image/jpeg',
    width            INTEGER,
    height           INTEGER,
    source_url       TEXT NOT NULL DEFAULT '',
    preferred_source TEXT NOT NULL DEFAULT 'auto',
    expires_at       TEXT,
    fetched_at       TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_game_image_cache_app_type_source
    ON game_image_cache(steam_app_id, image_type, source);
CREATE INDEX IF NOT EXISTS idx_game_image_cache_expires
    ON game_image_cache(expires_at);
```

**Relationship to `external_cache_entries`**: Steam metadata JSON goes into `external_cache_entries` (cache key `steam:appdetails:v1:{app_id}`, within 512 KiB cap). Image binaries go to filesystem + `game_image_cache` (exceed cap). Both share `steam_app_id` as join dimension — no FK, independent caches with independent TTLs.

**Filesystem layout**: `~/.local/share/crosshook/cache/images/{steam_app_id}/cover_steam_cdn.jpg`

#### State Management (unchanged)

```text
ProfileContext (app root, persists across ALL tabs)
  ├── profile: GameProfile          ← single state object
  ├── updateProfile(updater)        ← immutable updater: (current) => GameProfile
  ├── dirty: boolean                ← tracks unsaved changes
  └── saveProfile() / selectProfile()
```

### API Design

#### `#[tauri::command] fetch_game_metadata`

**Purpose**: Fetch Steam Store metadata for a game by app ID (cache-first with stale fallback)

**Signature**: `async fn fetch_game_metadata(app_id: String, metadata_store: State<'_, MetadataStore>) -> Result<SteamMetadataLookupResult, String>`

**Returns**: `{ app_id, state, details: { name, short_description, genres, header_image, ... }, from_cache, is_stale }`

#### `#[tauri::command] fetch_game_cover_art`

**Purpose**: Download and cache cover art, returning local filesystem path

**Signature**: `async fn fetch_game_cover_art(app_id: String, image_type: String, metadata_store: State<'_, MetadataStore>) -> Result<Option<String>, String>`

**Returns**: Absolute path to cached image file (e.g., `/home/user/.local/share/crosshook/cache/images/1245620/cover_steam_cdn.jpg`) or `None` if unavailable.

### System Integration

#### Files to Create

**Backend (Rust)**:

- `crosshook-core/src/steam_metadata/mod.rs`: Module exports
- `crosshook-core/src/steam_metadata/client.rs`: Steam Store API client (mirrors protondb/client.rs)
- `crosshook-core/src/steam_metadata/models.rs`: `SteamAppDetails`, `SteamGenre`, `SteamMetadataLookupResult`
- `crosshook-core/src/game_images/mod.rs`: Module exports
- `crosshook-core/src/game_images/client.rs`: Image download + magic-byte validation
- `crosshook-core/src/game_images/cache.rs`: Filesystem cache manager with path traversal protection
- `crosshook-core/src/metadata/game_image_store.rs`: SQLite CRUD for game_image_cache

**Frontend (TypeScript/React)**:

- `components/profile-sections/GameCoverArt.tsx`: Cover art display (loading/error/placeholder states)
- `components/profile-sections/GameMetadataBar.tsx`: Genre chips, description
- `hooks/useGameMetadata.ts`: Steam metadata + cover art fetching hook
- `hooks/useGameCoverArt.ts`: Cover art path + loading state hook
- `types/game-metadata.ts`: TypeScript types for Steam metadata
- `components/profile-sections/ProfileIdentitySection.tsx`: Extracted from ProfileFormSections
- `components/profile-sections/GameSection.tsx`: Extracted from ProfileFormSections
- `components/profile-sections/RunnerMethodSection.tsx`: Extracted from ProfileFormSections
- `components/profile-sections/TrainerSection.tsx`: Extracted from ProfileFormSections
- `components/profile-sections/RuntimeSection.tsx`: Extracted from ProfileFormSections
- `components/ProfileSubTabs.tsx`: Sub-tab row + content routing (Phase 3)

#### Files to Modify

- `ProfilesPage.tsx`: Remove Advanced wrapper, add card layout, add cover art integration
- `ProfileFormSections.tsx`: Reduce to thin composition of extracted sections; keep for InstallPage compatibility
- `ui/InstallField.tsx`: Add `id` prop to replace private `FieldRow`
- `settings/mod.rs`: Add `steamgriddb_api_key: Option<String>` to `AppSettingsData`
- `metadata/migrations.rs`: Add v14 migration for `game_image_cache` table
- `src-tauri/src/commands/`: New `steam.rs` command module
- `src-tauri/capabilities/default.json`: Enable `assetProtocol` with scope `$LOCALDATA/cache/images/**`
- `tauri.conf.json`: Add `img-src 'self' asset: http://asset.localhost` to CSP
- `styles/variables.css`: Cover art CSS variables (aspect-ratio, skeleton animation)
- `styles/theme.css`: `crosshook-profile-cover-art`, `crosshook-skeleton` classes

#### Prerequisite: Circular Dependency Fix

`ui/ProtonPathField.tsx` imports `formatProtonInstallLabel` from `ProfileFormSections.tsx`. Extract to `utils/proton.ts` before splitting.

## UX Considerations

### User Workflows

#### Primary Workflow: Create a New Profile

1. **Open Profiles page** — user sees all section cards (no collapse needed)
2. **Enter profile name** — in Core card (always visible)
3. **Set Game Path** — browse button in Core card
4. **Select Runner Method** — dropdown in Core card; Runtime card content updates
5. **Fill runtime fields** — Steam App ID triggers async cover art fetch
6. **Cover art appears** — shimmer skeleton → loaded image in Core card (non-blocking)
7. **Click Save** — in always-visible Actions bar

#### Primary Workflow: Browse Profiles with Cover Art

1. **Open Profiles page** — profiles listed in dropdown or grid view (Phase 4)
2. **Visual identification** — cover art on profile cards helps identify games
3. **Select profile** — art + metadata load for selected profile
4. **Edit or Launch** — actions always accessible

### UI Patterns

| Component          | Pattern                                           | Notes                                      |
| ------------------ | ------------------------------------------------- | ------------------------------------------ |
| Section cards      | `CollapsibleSection` + `crosshook-panel`          | Matches existing LaunchPage pattern        |
| Cover art          | `GameCoverArt` with aspect-ratio: 460/215         | Landscape Steam header for Phase 2         |
| Cover art loading  | Shimmer skeleton (`crosshook-skeleton` keyframe)  | 1.8s cycle, intersectionObserver lazy load |
| Sub-tabs (Phase 3) | `@radix-ui/react-tabs` + `crosshook-subtab-*` CSS | Existing unused infrastructure             |
| Genre chips        | `crosshook-status-chip` badges                    | Reuse existing chip styling                |
| Actions bar        | Fixed footer below tab content                    | Always visible, sticky optional            |

### Accessibility Requirements

- **Tab pattern**: `role="tablist"`, `role="tab"` with `aria-selected`, `role="tabpanel"` with `aria-labelledby`
- **Keyboard**: Arrow keys navigate tabs, Tab moves into panel, Home/End jump to first/last tab
- **Cover art alt text**: `"{game_name} cover art"` (informational context); empty `alt=""` in decorative contexts
- **Touch targets**: 44x44px minimum (controller mode: 48px via existing CSS variables)
- **Image loading**: Skeleton placeholder with `role="status"` and `aria-label="Loading cover art"`

### Performance UX

- **Loading States**: Cover art uses shimmer skeleton during fetch; all other fields render instantly from local state
- **Lazy Loading**: Cover art uses IntersectionObserver with `rootMargin: "200px"` — only fetches when near viewport
- **Tab Switching**: Instantaneous — CSS `display: none` for inactive panels, not conditional rendering
- **Image Caching**: 24-hour TTL with stale fallback; cached images serve immediately from filesystem

## Recommendations

### Implementation Approach

**Recommended Strategy**: Hybrid Promote + Cards (Phase 1) with cover art slots designed from day 1, Steam-only art integration (Phase 2), then Sub-Tabs + optional SteamGridDB (Phase 3). SteamGridDB deferred to reduce initial scope.

**Rationale**:

1. Addresses root cause immediately — everything hidden behind single collapsed toggle
2. Cover art slots in Phase 1 prevent layout rework when art is wired in Phase 2
3. Steam-only-first eliminates API key friction; Steam header images (460x215) are adequate
4. `game_image_cache` table has `source` column from Phase 0 — SteamGridDB slots in without migration
5. Zero new frontend dependency cost; backend reuses existing `reqwest` + `MetadataStore`

### Technology Decisions

| Decision               | Recommendation                                                   | Rationale                                        |
| ---------------------- | ---------------------------------------------------------------- | ------------------------------------------------ |
| Tab library            | `@radix-ui/react-tabs` (already installed)                       | Zero cost, WAI-ARIA, matches codebase            |
| Tab rendering          | CSS `display: none` for inactive panels                          | Preserves component local state (W1)             |
| Image source (Phase 2) | Steam Store API only                                             | No API key friction; adequate art quality        |
| Image source (Phase 3) | Add SteamGridDB as optional                                      | Higher-res art for users who want it             |
| Image storage          | Filesystem + SQLite metadata tracking                            | Images exceed external_cache_entries 512 KiB cap |
| Image rendering        | Tauri `asset://` via `convertFileSrc`                            | Documented Tauri v2 pattern; scoped CSP          |
| Cover art aspect       | Landscape 460x215 (Phase 2)                                      | Fits horizontal card layout naturally            |
| Image validation       | `infer` crate magic-byte detection                               | SVG rejection, MIME allowlist (I1)               |
| Metadata cache         | `external_cache_entries` with `steam:appdetails:v1:{app_id}` key | Reuses ProtonDB pattern exactly                  |

### Quick Wins

- **Remove the Advanced wrapper**: Single biggest impact change — promotes all content to always-visible
- **Move ProfileActions outside any collapsible**: Save/Delete always accessible
- **Promote health badges to profile selector bar**: Health status visible at a glance
- **Define cover art CSS class early**: `crosshook-profile-cover-art` with aspect-ratio rules — prevents layout rework

### Future Enhancements

- **Profile grid view**: Card layout with cover art as primary visual; grid/list toggle (Phase 4)
- **SteamGridDB gallery picker**: Multiple art options per game for user selection
- **Profile templates**: Pre-fill common configurations (extends `BundledOptimizationPreset` pattern)
- **Profile comparison view**: Side-by-side diff (reuses existing `ConfigHistoryPanel` TOML diff rendering)
- **`@radix-ui/react-accordion` upgrade**: Animated expand/collapse for section cards

### Creative Ideas

1. **Smart defaults on card collapse**: One-line summary in header when collapsed (e.g., "Trainer: Aurora v1.2 (copy mode)") using existing `CollapsibleSection` `meta` prop
2. **Cover art as visual anchor**: With 10+ profiles, art helps users find the right one faster than text-only names
3. **Genre chips as at-a-glance context**: RPG, Action, etc. as `crosshook-status-chip` badges below cover art
4. **Launch method badges**: Visual indicator in card headers ("Steam", "Proton", "Native") using existing chip CSS

## Risk Assessment

### Technical Risks

| Risk                                               | Likelihood | Impact | Mitigation                                                                               |
| -------------------------------------------------- | ---------- | ------ | ---------------------------------------------------------------------------------------- |
| `ProfileFormSections` reuse breakage (InstallPage) | Medium     | High   | Phase 1 changes only ProfilesPage; Phase 3 keeps ProfileFormSections as thin composition |
| Component state loss on tab switch (W1)            | Medium     | Medium | CSS `display: none` instead of conditional rendering                                     |
| Circular dependency (`formatProtonInstallLabel`)   | High       | Low    | Extract to shared utility in Phase 0                                                     |
| Steam Store API rate limiting                      | Medium     | Low    | 24-hour cache TTL; stale fallback; fetch only when app_id is set                         |
| Image cache disk bloat                             | Medium     | Medium | SQLite tracking; planned eviction (Phase 4); manual cache clear in Settings              |
| Path traversal via malicious app_id (W6)           | Low        | High   | Numeric-only validation at GameImageStore boundary                                       |
| SVG XSS in downloaded images (I1)                  | Low        | High   | Magic-byte rejection via `infer` crate before write to disk                              |

### Integration Challenges

- **ProtonDB apply-to-env-vars cross-section flow**: Must keep ProtonDB and env vars co-located (same card/tab)
- **Cover art loading in profile switch**: Previous profile's art must not flash during transition
- **Asset protocol scope**: `tauri.conf.json` must enable `assetProtocol` with narrow scope `$LOCALDATA/cache/images/**`

### Security Considerations

#### Critical -- Hard Stops

| Finding         | Risk | Required Mitigation |
| --------------- | ---- | ------------------- |
| None identified | --   | --                  |

#### Warnings -- Must Address

| Finding                                                  | Risk                                     | Mitigation                                                        | Alternatives                                 |
| -------------------------------------------------------- | ---------------------------------------- | ----------------------------------------------------------------- | -------------------------------------------- |
| W1: `CustomEnvironmentVariablesSection` local state loss | In-progress edits silently discarded     | CSS `display: none` for tab panels                                | `useEffect` cleanup to flush rows on unmount |
| W3: `injection.*` fields must not be surfaced            | Exposes removed DLL injection capability | Explicitly exclude from all new sections                          | --                                           |
| I1: SVG downloads must be rejected at Rust layer         | SVG XSS in Tauri webview                 | `infer` crate magic-byte validation; allowlist JPEG/PNG/WEBP      | --                                           |
| I2: Path traversal in image cache construction           | Write outside cache directory            | `canonicalize` + prefix assertion; numeric-only app_id validation | --                                           |
| K1: SteamGridDB API key in plaintext settings.toml       | Key exposure via dotfiles sync           | UX warning in Settings UI; mask input; don't log key              | Future: OS keyring via `keyring` crate       |

#### Advisories -- Best Practices

- **A1**: Image file size cap (5 MB) before write to disk (deferral: advisory, not blocking)
- **A2**: Cache size limit (default 500 MB) with LRU eviction (deferral: implement if disk usage becomes reported issue)
- **A3**: CSP `img-src` expansion to `'self' asset: http://asset.localhost` — required for `convertFileSrc` rendering
- **A4**: URL domain validation for image downloads — Steam CDN and SteamGridDB CDN allowlists

## Task Breakdown Preview

### Phase 0: Component Cleanup + Image Cache Infrastructure (~2 days)

**Focus**: Deduplicate components AND build #52 backend infrastructure in parallel

**UI Cleanup Tasks**:

- Deduplicate `FieldRow` → `InstallField` (add `id` prop, migrate 10+ usages)
- Consolidate `ProtonPathField` implementations (make `ui/` version canonical)
- Extract `formatProtonInstallLabel` to `utils/proton.ts` (fix circular import)
- Replace `OptionalSection` with `CollapsibleSection defaultOpen={false}`

**#52 Backend Tasks**:

- SQLite migration v14: add `game_image_cache` table
- Create filesystem cache directory `~/.local/share/crosshook/cache/images/`
- Implement `GameImageStore` module (put/get/evict for game_image_cache)
- Add `steamgriddb_api_key: Option<String>` to `AppSettingsData`

**Parallelization**: UI cleanup and backend infrastructure touch different files — full parallel

### Phase 1: Promote + Cards with Cover Art Slots (~3-4 days)

**Focus**: Remove Advanced wrapper, create section cards with cover art CSS ready from day 1
**Dependencies**: Phase 0 complete

**Tasks**:

- Remove `CollapsibleSection("Advanced")` wrapper in ProfilesPage
- Wrap each logical group in `CollapsibleSection` + `crosshook-panel`
- Add `crosshook-profile-cover-art` CSS class (conditional render — only when art available)
- Move ProfileActions to dedicated bottom area (outside any card)
- Promote health badges to profile selector bar
- Move Health Issues to dedicated diagnostic card
- Test OnboardingWizard reviewMode, keyboard nav, controller mode

### Phase 2: Steam API Integration + Art Display + Polish (~3-4 days)

**Focus**: Wire Steam Store API, display cover art, add first-pass polish
**Dependencies**: Phase 1 complete (card layout provides art slots)

**Tasks**:

- Implement Rust Steam Store client (`steam_metadata/client.rs`, mirrors protondb pattern)
- Implement image downloader with magic-byte validation (`game_images/client.rs`)
- Add Tauri IPC commands (`fetch_game_metadata`, `fetch_game_cover_art`)
- Enable asset protocol in `tauri.conf.json` with scoped CSP
- Implement `useGameMetadata` and `useGameCoverArt` frontend hooks
- Display cover art in Core card + genre chips + description
- Add shimmer skeleton loading animation
- Polish: sticky action footer, card header summaries, launch method badges

**Parallelization**: Rust backend and frontend hooks can develop in parallel against mock data

### Phase 3: Sub-Tabs + SteamGridDB (~4-5 days)

**Focus**: Split ProfileFormSections into composable sections, add tab navigation, integrate SteamGridDB
**Dependencies**: Phase 1 complete (Phase 2 optional)

**Tasks**:

- Extract 6 section components from ProfileFormSections
- Add `ProfileSubTabs` using `@radix-ui/react-tabs` + `crosshook-subtab-*` CSS
- CSS `display: none` for inactive tab panels (W1 mitigation)
- Persist active tab in sessionStorage (`crosshook.profilesActiveTab`)
- Implement SteamGridDB Rust client (optional, requires API key)
- Add SteamGridDB API key field to Settings panel
- Extend `fetch_game_cover_art` with `preferred_source` parameter

**Parallelization**: Section extraction can be parallelized across 3-4 agents

### Phase 4: Visual Polish + Figma Concept Elements (~1-2 days)

**Focus**: Aspirational visual refinements from the Figma concept
**Dependencies**: Phase 2 complete

**Tasks**:

- Gradient overlays on cover art for text readability
- Portrait card layout option (2:3 aspect ratio)
- Grid/list view toggle for profile browsing
- Stat grid metadata display (playtime, last launched, health, ProtonDB rating)

## Decisions Needed

1. **Sticky action footer vs. inline actions**
   - Options: Fixed footer (always visible) vs. bottom of scrollable area
   - Recommendation: Sticky footer — Save/Delete discoverability is more important

2. **Default collapse state for cards**
   - Options: All open vs. smart defaults (open for new, collapsed for existing)
   - Recommendation: All default open — the whole point is removing hidden content

3. **Steam-only vs. dual-source for initial launch**
   - Options: Steam Store API only (Phase 2) vs. Steam + SteamGridDB together
   - Recommendation: Steam-only-first; SteamGridDB adds in Phase 3 without migration

4. **Cover art aspect ratio**
   - Options: Landscape 460x215 (Steam header) vs. Portrait 600x900 (library art)
   - Recommendation: Landscape for Phase 2 (fits horizontal cards); portrait option in Phase 4

5. **Sub-tabs timeline**
   - Options: Immediately after Phase 2 vs. wait for user feedback on cards
   - Recommendation: Plan Phase 3 but gate on user feedback after Phase 1 ships

6. **Tab panel rendering strategy**
   - Options: CSS `display: none` vs. conditional rendering
   - Recommendation: CSS `display: none` — required by W1 to prevent data loss

7. **Cover art in InstallPage review modal**
   - Options: Show art in review modal vs. gate with `!reviewMode`
   - Recommendation: No — review modal is compact confirmation; gate with `!reviewMode`

8. **Image cache eviction policy**
   - Options: Automatic LRU vs. manual-only clear
   - Recommendation: Deferred; track sizes in SQLite; add "Clear cache" in Settings (Phase 3)

## Persistence & Usability

### Datum Classification

| Datum                     | Layer                                                                                 | Reasoning                                 |
| ------------------------- | ------------------------------------------------------------------------------------- | ----------------------------------------- |
| `steam_app_id`            | TOML profile (existing `[steam] app_id`)                                              | Already exists; no change                 |
| Steam metadata JSON       | SQLite `external_cache_entries` (key: `steam:appdetails:v1:{app_id}`)                 | 3-15 KiB; within 512 KiB cap; 24h TTL     |
| Cover art binaries        | Filesystem `~/.local/share/crosshook/cache/images/` + `game_image_cache` SQLite table | 80 KB-2 MB; exceed cache cap              |
| SteamGridDB API key       | `settings.toml` (`AppSettingsData`)                                                   | User-editable preference                  |
| Image fetch/display state | Runtime-only (memory)                                                                 | Ephemeral UI state                        |
| Sub-tab active state      | Runtime-only (`useState`)                                                             | Optional sessionStorage persistence       |
| Card collapse state       | Runtime-only                                                                          | Default open on page load; no persistence |

### Migration/Backward Compatibility

- **Phase 0 migration (v14)**: Adds `game_image_cache` table. Additive — existing functionality unaffected.
- **`AppSettingsData` extension**: New field with `#[serde(default)]`. Existing settings deserialize cleanly.
- **Profile data**: No changes to TOML storage. `steam.app_id` already exists.

### Offline Behavior

- **Metadata JSON**: Stale fallback in `external_cache_entries` (matching ProtonDB pattern)
- **Cached images**: Persist on filesystem indefinitely until successful refresh
- **Without cache**: Cards degrade to text-only. No broken image icons. No blocked profile load/launch.

### Degraded Fallback Chain

```text
SteamGridDB art (if API key configured)
  → Steam Store API header_image
    → Stale cached image (from previous successful fetch)
      → Hidden art slot (text-only card)
```

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Steam Store API, SteamGridDB API, Radix Tabs, image handling
- [research-business.md](./research-business.md): User stories, business rules, section inventory, datum classification
- [research-technical.md](./research-technical.md): Component hierarchy, game_image_cache schema, Rust modules, IPC commands
- [research-ux.md](./research-ux.md): Game art card patterns, image loading UX, grid/list views, Figma concept analysis
- [research-security.md](./research-security.md): Severity-leveled findings (SVG rejection, path traversal, API key management, asset protocol)
- [research-practices.md](./research-practices.md): MetadataStore reuse, KISS assessment, build-vs-depend, modularity design
- [research-recommendations.md](./research-recommendations.md): Unified phasing, Figma integration, risk assessment, decisions needed
