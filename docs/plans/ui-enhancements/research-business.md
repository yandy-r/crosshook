# Business Analysis: Profiles Page UI Enhancements

## Executive Summary

The Profiles page is the central configuration hub for CrossHook, but its current layout buries the entire profile editor (identity, game, runner, trainer, environment variables) inside a single collapsed `Advanced` section. The result is that first-time and returning users must expand a non-obvious section to do any actual profile editing. Health status, status badges, and the Refresh button are all pinned to the collapsed header, creating an information hierarchy that rewards power users but confuses newcomers. Separating logically distinct concerns into discrete visual containers (mirroring the existing Profile/Launcher Export split), promoting the editor out of `Advanced`, and grouping form sections by user mental model will dramatically reduce perceived clutter without hiding functionality.

The second pass of this analysis integrates GitHub issue #52 (game metadata and cover art via Steam Store API and SteamGridDB). Issue #52 adds a visual enrichment layer: profile cards and the community browser gain game cover art when `steam_app_id` is available. This enrichment is strictly additive — no profile functionality is blocked if art is unavailable, uncached, or unconfigured. The cover art infrastructure (filesystem image cache + `game_image_cache` SQLite table) follows the same pattern established by ProtonDB lookup (#53), reusing `external_cache_entries` for metadata JSON and introducing a new `game_image_cache` table for filesystem-backed image binaries.

---

## User Stories

**New user creating a first profile**

- As a user setting up my first game + trainer combo, I want to understand what I must fill out versus what is optional, so I can complete setup without guessing.
- As a new user, I do not want to hunt for a collapsed "Advanced" section to enter basic game paths.

**Returning user editing an existing profile**

- As a power user, I want to quickly jump to the section I need (e.g., environment variables or the ProtonDB lookup) without scrolling through unrelated fields.
- As a returning user, I want to see at a glance which profile is active and whether it is healthy, without expanding a collapsible.

**User managing multiple profiles**

- As a user with 10+ profiles, I want the profile selector to remain permanently visible so I can switch between profiles without losing my scroll position.
- As a user managing profiles, I want Save, Duplicate, Rename, Delete in a consistent place — not buried below all form fields.

**User troubleshooting a broken profile**

- As a user whose profile has health issues, I want the health status and issue list to be surfaced without having to expand a collapsed section.
- As a user who just made changes and saved, I want confirmation that the save succeeded without hunting for status indicators.

**User browsing game metadata and cover art (Issue #52)**

- As a user with Steam App IDs in my profiles, I want to see game cover art on profile cards so I can visually identify games at a glance — especially useful on Steam Deck.
- As a user with many profiles, I want cover art to load in the background without blocking me from editing or launching profiles.
- As a user offline or away from a network, I want previously fetched cover art to still display from the local cache.
- As a user who prefers custom artwork, I want to configure a SteamGridDB API key in settings so that higher-quality or community-created art appears instead of the default Steam capsule.
- As a user who has not entered a Steam App ID, I expect the profile card to degrade gracefully to a text-only display — no broken image placeholders.

**User configuring SteamGridDB (Issue #52)**

- As a user, I want to enter a SteamGridDB API key in Settings so that custom artwork is used when available, without requiring it for basic functionality.
- As a user, I want to understand that the SteamGridDB key is optional — its absence degrades art quality but does not disable any profile features.

---

## Current Layout Analysis

The Profiles page is a single-column vertical layout (`display: grid; gap: 24`) with the following top-level containers:

### Top-level containers (in order, always visible)

1. **PageBanner** — Eyebrow "Profiles", title "Profile editor", copy text, illustration art.
2. **Health/rename toast area** — Temporary banners (broken count, rename confirmation).
3. **Single `crosshook-panel` div** — Contains ALL of the following in one visual card:
   - **Guided Setup subsection** — accent-colored top strip with "Profile Setup Wizard" heading and "New Profile" / "Edit in Wizard" buttons.
   - **Active Profile selector** — Only visible when `profiles.length > 0`; select + label.
   - **`CollapsibleSection` titled "Advanced" (defaultOpen=false)** — This collapses all of:
     - Status badges (HealthBadge, OfflineStatusBadge, trainer type chip, version badge)
     - Refresh button
     - Profile health summary / Re-check All
     - Stale data notice
     - **`ProfileFormSections`** — The entire profile editor (see Section Inventory below)
     - **Health Issues nested `CollapsibleSection`** (conditionally rendered, inside Advanced)
   - **ProfileActions footer** — Save, Duplicate, Rename, Preview Profile, Export as Community Profile, Mark as Verified, History, Delete, unsaved-changes indicator.
4. **`CollapsibleSection` "Launcher Export"** — Conditionally visible when `launchMethod === 'steam_applaunch' || 'proton_run'`.
5. **Modal overlays** — Delete confirm, Rename dialog, ProfilePreviewModal, ConfigHistoryPanel, OnboardingWizard.

### Key observation

The `Advanced` section wraps the **entire profile editor form plus health information**. This is the primary clutter source: users must expand "Advanced" to do any editing at all, yet the section is collapsed by default.

---

## Section Inventory

### Inside `ProfileFormSections` (the collapsed Advanced section)

#### 1. Profile Identity

- **Profile Name** — Text input. Read-only when profile exists. Required.

#### 2. Game

- **Game Name** — Text input. Display name for the game.
- **Game Path** — Text input + Browse button. Required (blocks Save).

#### 3. Runner Method

- **Runner Method** — ThemedSelect: `steam_applaunch` / `proton_run` / `native`. Required. Controls which downstream sections appear.

#### 4. Custom Environment Variables

- `CustomEnvironmentVariablesSection` — Editable key/value table of env vars passed at launch. Reserved keys blocked. ProtonDB suggestions flow into this section.

#### 5. Trainer _(only when `launchMethod !== 'native'`)_

- **Trainer Path** — Text input + Browse.
- **Trainer type (offline scoring)** — ThemedSelect from catalog + optional "Offline help" button.
- **Trainer Loading Mode** — ThemedSelect: `source_directory` / `copy_to_prefix`.
- **Trainer Version** (read-only, conditionally rendered when version recorded).
- **Set Trainer Version** (only when profileExists and not reviewMode) — Manual version override field.

#### 6. Steam Runtime _(only when `launchMethod === 'steam_applaunch'`)_

- **Steam App ID** — Text input. Required for ProtonDB lookup.
- **Prefix Path** — Text input + Browse (compatdata_path).
- **Launcher Name** + **Launcher Icon** — LauncherMetadataFields; display name and icon for the exported .desktop entry.
- **Proton Path** — ProtonPathField: ThemedSelect of detected installs + manual text input + Browse.
- **AutoPopulate** component — Automatically fills App ID, compatdata path, proton path from game path.
- **ProtonDbLookupCard** — Fetches ProtonDB recommendations for the App ID. Shows env var suggestion groups with Apply buttons.
- **ProtonDB conflict resolution UI** — Inline conflict per-key resolution when applying ProtonDB env vars.

#### 7. Proton Runtime _(only when `launchMethod === 'proton_run'`)_

- **Prefix Path** — Text input + Browse (runtime.prefix_path).
- **Steam App ID** — Optional, for ProtonDB lookup.
- **Launcher Name** + **Launcher Icon** — LauncherMetadataFields.
- **Working Directory** — Optional override; collapsed in reviewMode when empty.
- **Proton Path** — ProtonPathField.
- **ProtonDbLookupCard** + conflict resolution UI.

#### 8. Native Runtime _(only when `launchMethod === 'native'`)_

- **Working Directory** — Optional override; collapsed in reviewMode when empty.

### Outside `ProfileFormSections` (still inside `Advanced`)

- **Profile health summary chip + Re-check All button** — Shows stale/broken count across all profiles.
- **Stale info notice** — "Last checked N days ago — consider re-checking".
- **Health Issues `CollapsibleSection`** — Per-issue list (field, path, message, remediation), last success time, total launches, failure count, drift warnings, community import note.

### Outside `Advanced`

- **Guided Setup** — Wizard buttons (always visible at top of panel, above Advanced).
- **Active Profile selector** — ThemedSelect + label (always visible when profiles exist).
- **ProfileActions** — All action buttons + save status indicator (always visible at panel bottom).
- **Launcher Export `CollapsibleSection`** — LauncherExport component (conditionally visible, always defaultOpen=false).

---

## Business Rules

The following groupings encode the business logic for how profile settings should be organized, based on user mental models, workflow frequency, and domain relationships.

## Proposed Section Groupings

Based on user mental models and the field inventory, natural groupings emerge:

### Group 1: Profile Management (always visible)

Fields that identify the profile and control profile-level actions.

- Profile Name
- Active Profile selector (switch profile)
- ProfileActions (Save, Duplicate, Rename, Delete, Preview, History, Export, Mark Verified)
- Dirty/save status indicator

### Group 2: Setup Assistance (promote from buried position)

Entry points for assisted setup; should be discoverable but not dominating.

- Profile Setup Wizard (New Profile / Edit in Wizard)

### Group 3: Game Configuration (core, always visible/expanded)

The minimum fields required to configure a launch. Required for Save.

- Game Name
- Game Path
- Runner Method
- Trainer Path (when non-native)

### Group 4: Runtime / Proton Settings (expanded by default for proton users)

Runner-specific paths, conditionally shown based on Runner Method.

- Steam App ID (when steam_applaunch or proton_run)
- Prefix Path
- Proton Path
- AutoPopulate assistance
- Working Directory (proton_run / native, optional)

### Group 5: Launcher Export Settings

Only relevant when building a Steam launcher entry.

- Launcher Name
- Launcher Icon
- LauncherExport component

### Group 6: Environment Variables (toggleable, medium priority)

Power-user configuration. Important for compatibility but rarely touched per-session.

- Custom Environment Variables table
- ProtonDB Lookup + env var suggestion/conflict UI

### Group 7: Optimization & Hardware

Launch flags and hardware tuning (currently entirely on the Launch page).

- Trainer type (offline scoring)
- Trainer Loading Mode
- Trainer Version / Set Trainer Version

### Group 8: Profile Health & Diagnostics (promoted from collapsed Advanced header)

Status that tells users whether their profile is ready to launch.

- HealthBadge, OfflineStatusBadge, version badge, trainer type chip
- Refresh button
- Health summary (stale/broken count, Re-check All)
- Stale info notice
- Health Issues detail list

---

## Business Rules: Cover Art and Game Metadata (Issue #52)

These rules govern the addition of game cover art and Steam Store metadata to profile cards and the community browser.

### Rule 1: Cover art is always an enhancement, never a dependency

Profile functionality (load, save, edit, launch, health check) must work identically regardless of whether cover art is available, cached, or configured. A missing SteamGridDB API key, a failed Steam API request, or an empty cache must not produce any error state in the profile editor or launcher. Cover art is visual decoration — its absence is always a valid state.

### Rule 2: Fallback chain is fixed and non-configurable

When a Steam App ID is available, the art resolution order is:

1. SteamGridDB (requires API key in `settings.toml`; skipped if key is absent)
2. Steam Store capsule image (`https://cdn.akamai.steamstatic.com/steam/apps/{app_id}/header.jpg`)
3. Placeholder graphic (no Steam App ID present, or all fetches failed)
4. Text-only card (final fallback when placeholder asset is unavailable)

No step in this chain may block the profile card from rendering. Each step must fail silently and fall through to the next.

### Rule 3: Image caching is mandatory for offline access

Fetched images must be persisted to the filesystem at `~/.local/share/crosshook/cache/images/{steam_app_id}/` with metadata tracked in the `game_image_cache` SQLite table (path, checksum, source URL, app ID, expiry, preferred source). In-memory image state is ephemeral and must not be the only persistence layer. Images cached on disk survive offline periods until a new fetch succeeds or the user manually clears the cache by deleting the filesystem directory.

### Rule 4: Metadata JSON uses `external_cache_entries`, not filesystem

Steam Store API responses (name, description, genres, release date, tags) are bounded JSON payloads (~3–15 KiB) that fit within `MAX_CACHE_PAYLOAD_BYTES` (512 KiB). These must be cached in `external_cache_entries` using the key `steam:appdetails:v1:{app_id}` with TTL-based expiry. Image binaries (80 KB–2 MB) exceed this cap and must never be stored in `external_cache_entries` — the `NULL payload_json` fallback for oversized entries would silently break offline art access.

### Rule 5: SteamGridDB API key is a user preference, not a secret

The SteamGridDB API key is a user-facing optional setting stored in `settings.toml` as `AppSettingsData.steamgriddb_api_key`. It is not a system secret, not an environment variable, and not stored in the SQLite metadata DB. Users can view and edit it directly by opening `settings.toml`. The Settings page UI should surface this field as a plaintext or masked text input. The field's absence defaults to the Steam-only fallback — no prompting, no warnings.

### Rule 6: Metadata fetch does not block profile operations

Image fetch and metadata lookup must happen asynchronously after profile load. No profile save, load, rename, or launch operation may await or depend on image fetch completion. The fetch should be triggered lazily when a profile card is rendered with a non-empty `steam_app_id`.

### Rule 7: `steam_app_id` remains the single lookup key across all metadata features

Both ProtonDB lookup (#53) and game metadata/cover art (#52) resolve from the same `steam.app_id` field already present in `GameProfile.steam.app_id` for `steam_applaunch` profiles and optionally in `GameProfile.runtime.app_id` (or equivalent) for `proton_run` profiles. Issue #52 must not introduce a duplicate Steam App ID field or a competing lookup mechanism. The canonical identifier is `steam.app_id` as defined in `crosshook-core`.

### Rule 8: Image cache must have an eviction policy

The image cache at `~/.local/share/crosshook/cache/images/` will grow with profile count and must not be allowed to grow without bound. A user with 50 profiles and multiple art sources (Steam header + SteamGridDB grid) could accumulate 100–400 MB of cached images. This is a material concern on Steam Deck with limited eMMC storage.

The `game_image_cache` table must support eviction. The minimum viable eviction policy is TTL-based expiry (entries older than a configurable threshold, e.g., 30 days since last access, are eligible for deletion on next startup or on explicit cache-clear). A stricter policy adds a max total cache size cap (e.g., 200 MB) with LRU eviction when the cap is exceeded. The eviction policy must be enforced by a Rust-side maintenance task — not left to the user to manage manually via the filesystem.

**Minimum requirement**: TTL-based expiry enforced on startup or on cache miss. Images are re-fetched lazily after expiry.
**Recommended addition**: Total cache size cap with LRU eviction, surfaced as a configurable value in settings.

### Rule 9: SteamGridDB integration is separable from the Steam Store capsule path

The Steam Store capsule image path (no API key, same pattern as ProtonDB, landscape art via `https://cdn.akamai.steamstatic.com/steam/apps/{app_id}/header.jpg`) delivers the core library grid visual without any external API key management. SteamGridDB adds higher-quality and portrait-format art but introduces a second external API dependency, user API key management, and additional maintenance surface.

These two paths are independently shippable:

- **Phase 1 (recommended scope)**: `game_image_cache` infrastructure + Steam Store capsule images + library grid UI. No SteamGridDB. No API key. Steam Deck storage impact bounded (landscape-only, single image per game).
- **Phase 2 (separate issue)**: SteamGridDB integration — `AppSettingsData.steamgriddb_api_key`, SteamGridDB P-type portrait art, image type selection in the fetch command. Ships only after Phase 1 is stable.

The fallback chain in Phase 1 simplifies to: Steam Store capsule → placeholder → text-only. The `game_image_cache` table design must accommodate Phase 2 without migration churn (the `preferred_source` column covers this).

---

## Workflows

### Primary Workflow: First-time profile creation

1. User opens Profiles page — sees PageBanner and the main panel.
2. User sees "New Profile" wizard button and the profile name input.
3. User types a profile name.
4. User fills in Game Path.
5. User selects Runner Method.
6. (For non-native) User fills Trainer Path and Proton/prefix paths.
7. User clicks Save.
8. Health check runs automatically; badge appears.

**Current pain point**: Steps 2–7 require the user to expand "Advanced" first. The wizard is the only above-the-fold guided path, which users may overlook if they want direct editing.

### Alternative Workflow: Edit existing profile

1. User selects existing profile from the selector.
2. User expands "Advanced" to see form.
3. User edits specific field(s).
4. User clicks Save.

**Current pain point**: The user must know to expand "Advanced" to see any fields. Health and status badges are in the Advanced header, creating split attention — the badge is partially visible but the content is hidden.

### Alternative Workflow: Diagnose and fix a broken profile

1. User sees health banner ("N profiles have issues").
2. User selects the flagged profile.
3. User expands "Advanced".
4. User scrolls past profile identity, game, runner, trainer, env vars to reach the nested "Health Issues" collapsible.
5. User reads issue, fixes field, saves, re-checks.

**Current pain point**: Health Issues are nested inside a nested collapsible. Finding them requires multi-step expand + scroll.

### Alternative Workflow: Apply ProtonDB recommendations

1. User selects a steam_applaunch or proton_run profile.
2. User expands "Advanced".
3. User scrolls to Steam/Proton Runtime section.
4. User waits for ProtonDB lookup card to load.
5. User clicks "Apply" on a recommendation group.
6. If conflicts: user resolves each key in the inline conflict UI.
7. User scrolls back up to Save.

**Current pain point**: ProtonDB lookup is buried after multiple form sections and requires significant scroll to reach on dense profiles.

### Workflow: Browse profiles in library grid (Issue #52 — new)

1. User opens Profiles page — default view is the library grid (browse mode).
2. Profile cards fill the grid; each card shows cover art (or a text-only placeholder) with game title overlaid via gradient, and three actions: Launch, Heart, Edit.
3. Cards with `steam_app_id` display cover art asynchronously as it loads from cache or fetches. Cards without an App ID display text-only immediately.
4. User clicks Launch on a card — the game launches via the existing launch flow without entering edit mode.
5. User clicks Heart on a card — `profile_set_favorite` toggles the favorite state; the card updates the heart icon.
6. User clicks Edit on a card — the page switches to edit mode, the profile is selected in the editor, and the restructured editor panels appear.
7. User clicks the list toggle in the toolbar — view switches to a compact single-column list with one row per profile; same actions available.
8. User goes offline — previously cached art continues to display; new profiles without cached art fall through to text-only cards.

**Design requirement**: Cover art load must not shift layout. Cards with art and cards without art must have the same dimensions so the grid does not reflow when images load. All card action buttons must meet `--crosshook-touch-target-min: 44px` for controller mode.

### Workflow: Configure SteamGridDB API key (Issue #52 — new)

1. User opens Settings page.
2. User locates the "SteamGridDB API key" field.
3. User enters their API key (obtained from steamgriddb.com).
4. User saves settings.
5. Next time a profile card is rendered, SteamGridDB art is attempted first.
6. If the key is invalid or the API returns an error, the fallback chain proceeds to Steam API art — no error is surfaced to the user in the profile card.

**Edge case**: If the user clears the API key, future image fetches skip SteamGridDB and fall back to Steam API art. Previously cached SteamGridDB images remain valid until their TTL expires.

### Workflow: Offline art access (Issue #52 — new)

1. User launches CrossHook with no network connection.
2. Profile cards with previously cached art display the cached image from `~/.local/share/crosshook/cache/images/`.
3. Profile cards for which no art has been cached display text-only.
4. No error state, no loading spinner — the card renders immediately in whatever state the cache provides.

---

## Persistence and Datum Classification (Issue #52)

Every datum introduced by #52 must be explicitly classified per the CrossHook persistence architecture.

| Datum                                                       | Persistence Layer                                                                                        | Reasoning                                                                              |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `steam_app_id`                                              | TOML profile (existing field `[steam] app_id`)                                                           | Already in profile; no change required                                                 |
| Steam Store metadata JSON (name, description, genres, tags) | SQLite `external_cache_entries` (key: `steam:appdetails:v1:{app_id}`)                                    | Payload ~3–15 KiB; fits 512 KiB cap; TTL-based expiry; mirrors ProtonDB pattern        |
| Cover art / hero image binaries                             | Filesystem `~/.local/share/crosshook/cache/images/{steam_app_id}/` + new `game_image_cache` SQLite table | Images 80 KB–2 MB exceed `MAX_CACHE_PAYLOAD_BYTES`; blobs in SQLite cause WAL pressure |
| SteamGridDB API key                                         | `settings.toml` (`AppSettingsData.steamgriddb_api_key`)                                                  | User-editable preference; not a secret                                                 |
| Image fetch / display state                                 | Runtime-only (in-memory)                                                                                 | Ephemeral UI state; no persistence required                                            |

### Persistence / Usability Summary

- **Migration / backward compatibility**: The new `game_image_cache` table is additive. Users without it (upgrading) have no cover art but retain all profile functionality. The migration must be non-destructive and safe to run on existing installations at schema v14+.
- **Offline behavior**: Metadata JSON available as stale fallback in `external_cache_entries`. Filesystem images survive offline indefinitely until a new fetch succeeds. Profile cards show cached art offline; cards without cached art degrade to text-only with no error indicator.
- **Degraded fallback**: Steam API unavailable → text-only card, no blocked profile operation. SteamGridDB unavailable or unconfigured → fall back to Steam API art. No art available → placeholder or text-only.
- **User visibility / editability**: SteamGridDB API key is visible and editable in `settings.toml` and via the Settings page. Cached images are visible at `~/.local/share/crosshook/cache/images/` and manually deletable to force re-fetch. Users have no UI to invalidate individual cache entries — that is a power-user operation via the filesystem.

---

## Figma Concept Mapping

The Figma concept for this feature is a **library grid system**: a browsable grid of game cover art cards where users can launch, favorite, and edit profiles directly from the card — without opening the full profile editor. The existing CrossHook dark glassmorphism theme, BEM `crosshook-*` CSS classes, CSS variable system, and controller mode support remain unchanged; the Figma concept adds a new card-grid surface inside the existing design system.

### Figma Concept: Core Pattern

The library grid is a **dual-mode layout** for the Profiles page:

- **Browse mode** — a responsive grid of game cards, each showing cover art as the primary visual, with game title, health badge, and three inline actions (Launch, Heart/favorite, Edit)
- **Edit mode** — the existing profile editor panels (the first-pass restructure), reached by clicking Edit on a card or directly from the profile selector

The grid/list view toggle switches between the card grid and a compact list row per profile. Both views coexist within the Profiles page — no new route or separate page required.

### Infrastructure Assessment

| Figma Element                        | CrossHook Infrastructure                                                                                                                    | Verdict                                                                                                          |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Cover art as primary visual on cards | `game_image_cache` table + filesystem cache + `external_cache_entries` for metadata JSON                                                    | In scope — this is the #52 infrastructure core                                                                   |
| Gradient overlay with game title     | CSS `linear-gradient` over `<img>` + absolute-positioned text using existing `--crosshook-*` color tokens                                   | In scope — pure CSS, no new dependencies                                                                         |
| Heart (favorite) action on card      | `profile_set_favorite` / `profile_list_favorites` Tauri commands exist; `onToggleFavorite` prop pattern exists in `PinnedProfilesStrip.tsx` | In scope — wire to existing IPC                                                                                  |
| Launch action on card                | `useLaunchState` hook and `LaunchStateContext` exist; `invoke('launch_profile', ...)` can be triggered from a card button                   | In scope — reuse existing launch entry point                                                                     |
| Edit action on card                  | Clicking Edit selects the profile and switches to edit mode within the same page; no new route needed                                       | In scope — `selectProfile(name)` already exists in `useProfile`                                                  |
| Grid/list view toggle                | `--crosshook-community-profile-grid-min: 280px` CSS token already exists for community browser grid; same pattern applies here              | In scope — CSS grid with `auto-fill / minmax` using existing token                                               |
| Sub-tab navigation within editor     | `crosshook-subtab-row` / `crosshook-subtab` CSS classes defined in `theme.css`; `@radix-ui/react-tabs` installed                            | In scope — part of first-pass restructure                                                                        |
| Portrait aspect ratio (3:4)          | Steam capsule images are 460×215 (landscape); only SteamGridDB P-type grids are portrait 342×482                                            | Nuance: default to landscape (Steam header); SteamGridDB P-type art delivers portrait when API key is configured |

### Patterns Out of Scope

| Figma Element                                                           | Constraint                                                                                                                                                                  |
| ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| Playtime stats on card                                                  | `launch_operations` has `started_at` / `finished_at` columns but no aggregated playtime query is exposed via IPC; computing per-profile totals requires a new backend query | Defer — infrastructure exists but the IPC surface is missing |
| Stat grid per card (last played, total launches displayed on card face) | Same constraint — no summary query IPC path                                                                                                                                 | Defer — add once `launch_operations` aggregation is exposed  |

### Design System Alignment

The Figma grid pattern integrates into the existing CrossHook design system without changes to the theme:

- **Grid layout**: CSS `grid` with `grid-template-columns: repeat(auto-fill, minmax(var(--crosshook-community-profile-grid-min), 1fr))` — the same token already used by the community browser. A profile-specific token (e.g., `--crosshook-profile-grid-min`) can shadow it if a different minimum width is needed.
- **Card surface**: `crosshook-panel` / `crosshook-card` glassmorphism classes already provide dark background, border, and `border-radius`. Cover art sits inside the card as a full-bleed image with `object-fit: cover` and a fixed `aspect-ratio`.
- **Gradient overlay**: `linear-gradient(to top, rgba(0,0,0,0.85) 0%, transparent 60%)` over the image — uses no new tokens; the dark base colors are already in the theme.
- **Card action buttons**: positioned absolutely at the bottom or as a visible action row below the art; styled with `crosshook-btn` classes that already exist. The Heart icon toggles the `is_favorite` state via `profile_set_favorite`.
- **Controller mode**: `--crosshook-community-profile-grid-min` already increases to `340px` in controller mode (`variables.css:99`); cards become larger, touch targets meet the `--crosshook-touch-target-min: 44px` requirement automatically.
- **List view**: a single-column `<table>` or `<ul>` with one row per profile — no new CSS primitives; reuses existing `crosshook-panel` row styling.

### Dual-Mode Page Architecture

The library grid requires a browse/edit mode split within the Profiles page:

```
ProfilesPage
  ├── ProfileLibraryToolbar        ← grid/list toggle, search/filter
  ├── [browse mode]
  │     └── ProfileLibraryGrid    ← auto-fill card grid
  │           └── ProfileGameCard ← cover art + gradient + Launch/Heart/Edit
  └── [edit mode]
        ├── ProfileSelectorBar    ← always visible (first-pass restructure)
        ├── Panel: Core           ← (first-pass restructure panels)
        ├── Panel: Runtime
        ├── Panel: Environment
        ├── Panel: Trainer
        ├── Panel: Diagnostics
        └── ProfileActions
```

Mode state is local to `ProfilesPage` — a `viewMode: 'browse' | 'edit'` flag toggled by the toolbar or by clicking a card's Edit action. No new context or route is required. The profile editor panels (edit mode) are identical to the first-pass restructure output.

### New User Stories (Library Grid)

- As a user with multiple profiles, I want to see all my games as a visual grid with cover art so I can identify and launch them at a glance — especially useful with a gamepad on Steam Deck.
- As a user, I want to launch a game directly from its cover art card without navigating into the full profile editor.
- As a user, I want to favorite or unfavorite a profile from the card without opening the editor.
- As a user, I want to switch between a grid view (for visual browsing) and a list view (for seeing more profiles at once) using a toggle.
- As a user without cover art (no Steam App ID, or art not yet cached), I want the card to display the game name clearly — no broken image state.

---

## Domain Model

### Profile entity (`GameProfile`)

A profile is the complete configuration for launching one game + trainer combination. It is stored as TOML (one file per profile). Key sub-objects:

| Sub-object               | Purpose                                                     | When visible                                                 |
| ------------------------ | ----------------------------------------------------------- | ------------------------------------------------------------ |
| `game`                   | Game name + executable path                                 | Always                                                       |
| `trainer`                | Trainer path, type, loading mode, version                   | When `launchMethod !== 'native'`                             |
| `injection`              | DLL injection paths + flags                                 | (Not in current form; legacy fields)                         |
| `steam`                  | App ID, compatdata path, proton path, launcher display/icon | When `launchMethod === 'steam_applaunch'`                    |
| `runtime`                | Prefix path, proton path, working directory                 | When `launchMethod === 'proton_run'` or working dir override |
| `launch.method`          | The enum Runner Method                                      | Always                                                       |
| `launch.custom_env_vars` | User-set key/value env vars                                 | Always                                                       |
| `launch.optimizations`   | Launch optimization toggle flags                            | LaunchPage only                                              |
| `launch.gamescope`       | Gamescope display config                                    | LaunchPage only                                              |
| `launch.mangohud`        | MangoHud overlay config                                     | LaunchPage only                                              |
| `local_override`         | Per-machine path overrides (not yet in form UI)             | —                                                            |

### Launch methods

Three mutually-exclusive runners — `steam_applaunch`, `proton_run`, `native` — gate which runtime fields appear. This is the primary configuration axis: selecting it should be the first meaningful choice after naming the profile.

### Health system

Profile health is computed asynchronously by `ProfileHealthContext`. A health badge (broken/stale/ok) appears in the Advanced section meta. Health Issues is a nested collapsible that lists per-field validation issues. This is diagnostic information — it does not block saving but informs the user whether the profile will launch.

### Cover art and metadata (Issue #52)

The `game_image_cache` table and `external_cache_entries` table are the two persistence layers for #52. The profile entity itself does not store any reference to cached image paths — the image cache is keyed by `steam_app_id` and resolved at render time. The profile remains pure TOML with no runtime cache pointers embedded in it.

---

## Success Criteria

### First-pass (layout restructure)

1. A user with no prior CrossHook experience can create a working profile without expanding a collapsible section.
2. A user editing an existing profile can see the profile form without any extra interaction (no expand needed).
3. Profile health status is visible at a glance when a profile is selected, without requiring the user to scroll or expand.
4. Users who rarely touch advanced settings (ProtonDB, trainer type, env vars) are not visually overwhelmed by those sections.
5. The page retains all existing functionality — nothing is removed, only reorganized.
6. The new layout is consistent with existing CrossHook design patterns (panel/collapsible composition, `crosshook-*` CSS classes, CSS variable theming).

### Second-pass additions (Issue #52 — library grid and cover art)

7. The Profiles page opens in a library grid view: a responsive grid of game cards with cover art as the primary visual, game title overlay, and Launch/Heart/Edit actions per card.
8. Profile cards display game cover art when a Steam App ID is set and art has been fetched and cached — the first render may be text-only while art loads, but no broken image state is shown.
9. Cover art display does not block, delay, or alter any profile operation (save, load, rename, launch, health check).
10. Images are cached locally at `~/.local/share/crosshook/cache/images/` and display correctly on subsequent opens without re-fetching.
11. Profiles without a Steam App ID render text-only cards with no layout shift compared to cards with art.
12. A user who adds a SteamGridDB API key in Settings sees SteamGridDB art on their next profile card render (assuming the cache TTL has expired or no cached art exists).
13. A user who removes or leaves the SteamGridDB API key empty sees Steam capsule art (or text-only) — no error message, no broken state.
14. A user can switch between grid view and list view using a toolbar toggle; both views show the same profiles with the same action affordances.
15. Clicking Launch on a card launches the game; clicking Heart toggles the favorite; clicking Edit navigates to the profile editor — all without navigating to a different page.

---

## Open Questions

1. **Should the Wizard be a top-level CTA or a secondary option?** With browse mode as the default, new users land on the card grid first. Where should the "New Profile" / wizard entry point live in browse mode?
2. **Where does "Profile Identity" (profile name + selector) live in edit mode?** Currently split between the always-visible "Active Profile" selector and the "Profile Name" field inside Advanced. Should these merge into one always-visible identity card in the restructured editor?
3. **Should Health Issues be promoted to a dedicated panel in edit mode?** Currently nested inside Advanced. A dedicated card beneath the Profile panel (similar to Launcher Export) would eliminate multi-step expand-to-diagnose.
4. **Should environment variables be collapsed by default?** They are rarely edited per-session but can grow large. An opt-in expand with a count badge ("3 env vars set") would reduce visual weight.
5. **Sub-tabs vs. section containers**: Sub-tabs within the edit-mode editor panels (e.g., "Setup | Environment | Health") would reduce scroll but increase navigation complexity and may break the wizard's sequential flow mental model.
6. **Form completeness indicator**: Should a progress or readiness indicator (e.g., 3/5 required fields filled) appear inline on the panel header to guide first-time setup?
7. **Cover art aspect ratio**: Steam capsule images are 460×215 (landscape); SteamGridDB portrait grids are 342×482. Which aspect ratio should library grid cards use? Landscape works without SteamGridDB; portrait requires SteamGridDB P-type art and explicit image type selection in the fetch command.
8. **Library grid default vs. edit mode default**: Should new users land on the grid, or on the editor with an empty state? Users with zero profiles have nothing to browse — the grid is empty. Consider showing the wizard / new-profile CTA when `profiles.length === 0`.
9. **Cache invalidation UI**: Should users have a "Refresh cover art" button per card (or per profile in the editor), or is manual cache deletion via the filesystem sufficient for the first iteration?
10. **`steam_app_id` availability for `proton_run` profiles**: The `proton_run` runner method has an optional `steam.app_id` field. Art should display for `proton_run` profiles when the optional App ID is populated — this must be tested since it is not the primary runner path.

---

## Implementation Constraints

These constraints were identified through cross-team analysis and must govern any structural redesign:

### 1. `ProfileFormSections` is shared across two pages

`ProfileFormSections` is used in:

- `ProfilesPage.tsx` — main profile editor (no `reviewMode`)
- `InstallPage.tsx` — post-install profile review step (`reviewMode={true}`, inside `ProfileReviewModal`)

The `OnboardingWizard` only imports the `ProtonInstallOption` type from this file — it does not render the component. Any restructuring of `ProfileFormSections` (e.g., splitting into sub-components, adding sub-tabs) must be compatible with its `reviewMode` usage in `InstallPage`. Sub-tabs embedded inside `ProfileFormSections` would conflict with the modal context in `InstallPage` where there is no need for tabs — `reviewMode` is a modal review step, not a full editor. **Preferred approach**: keep tabs/panels at the `ProfilesPage` level (the page orchestrator), not inside `ProfileFormSections`.

### 2. Launch method gates section visibility — sub-tabs labeled by runner method would confuse users

The `launchMethod` field controls which sections render:

- `steam_applaunch`: AppID, Prefix Path, Proton Path, Launcher metadata, AutoPopulate, ProtonDB lookup
- `proton_run`: Prefix Path, Proton Path, Launcher metadata, Working Dir, ProtonDB lookup
- `native`: Working Dir only
- Trainer section: all methods except `native`

A sub-tab labeled "Steam Runtime" would be empty/hidden for `native` profiles. A sub-tab labeled "Trainer" would be empty for `native` profiles. Any tab-based approach must either: (a) hide tabs for irrelevant methods, (b) use generic labels ("Runtime Settings", "Trainer & Tools") that work across all methods, or (c) avoid tabs entirely in favor of collapsible panels that naturally collapse to zero height when empty.

### 3. ProtonDB and env vars must stay in the same visual zone

The `ProtonDbLookupCard` applies recommendations directly into `launch.custom_env_vars`, which is managed by `CustomEnvironmentVariablesSection`. If these are separated across tabs or distant panels, users must tab-switch to verify what was applied. They belong in the same panel or in adjacent sections within the same panel.

### 4. Action bar must remain persistent regardless of layout choice

`ProfileActions` (Save, Duplicate, Rename, Preview, Export, History, Delete) must remain visible at all times. Currently it is below the Advanced collapsible in the same card — this means it is visible even when Advanced is collapsed, but it operates on content the user cannot see. In any new layout, the action bar should be either:

- Anchored to the top-level panel (above or below the form content)
- Or sticky/fixed at the bottom of the viewport

It must never be inside a tab panel that becomes hidden.

### 5. Health Issues are diagnostic, not a form section

`CollapsibleSection title="Health Issues"` renders conditionally only when a profile has `broken` or `stale` status. It contains read-only metadata (last success time, launch count, failure count, drift warnings, per-field issue list). It does not contain any editable inputs. It should be treated as a status surface — positioned near the action bar or as a distinct diagnostic panel — not embedded inside the editable form flow.

### 6. The `reviewMode` prop controls launcher metadata visibility

`showLauncherMetadata = supportsTrainerLaunch && !reviewMode`. In `reviewMode`, Launcher Name and Launcher Icon fields are hidden. This is intentional — launcher metadata is not relevant during the install-flow review. Any refactor that splits launcher metadata into its own panel must preserve this conditional: launcher metadata panels should not render during `reviewMode`.

### 7. Cover art must not introduce layout shift (Issue #52)

Profile card dimensions must be fixed and consistent regardless of whether cover art is available. Images must be loaded with `object-fit: cover` and a fixed `aspect-ratio` (or fixed `width`/`height`). The `<img>` element must have explicit `width` and `height` attributes or CSS containment to prevent layout reflow as images load. This is a Core Web Vitals constraint applied to a desktop app context: unpredictable layout shift degrades perceived polish on Steam Deck where screen real estate is limited.

### 8. `game_image_cache` table requires a new migration

The `game_image_cache` table does not exist in the current schema (v13). A new migration must be written in `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`. The migration is additive: it creates the table and does not modify any existing table. The current schema version must increment to v14 or the next available version. Existing installations with no `game_image_cache` table fall back to text-only cards — this is the correct degraded state.

---

## Library & CSS Infrastructure Notes

These findings from API research confirm zero new dependencies are required for either layout approach:

### Radix Tabs already in use at the app level

`@radix-ui/react-tabs` v1.1.13 is installed and actively used in:

- `App.tsx` — `Tabs.Root orientation="vertical"` wraps the entire app shell
- `Sidebar.tsx` — `Tabs.List` + `Tabs.Trigger` drive page navigation
- `ContentArea.tsx` — `Tabs.Content` renders each page

The app's page routing IS the Radix Tabs primitive. Any within-page sub-tabs would be a nested `Tabs.Root` — Radix supports nested tab roots, but care is needed since the outer root uses `orientation="vertical"` and the inner would be `orientation="horizontal"`. The inner root must have a distinct `value`/`onValueChange` scope.

### Sub-tab CSS is already defined

`theme.css` already defines `.crosshook-subtab-row`, `.crosshook-subtab`, and `.crosshook-subtab--active` classes with full styling (pill shape, active gradient, transitions). `variables.css` already defines `--crosshook-subtab-min-height: 40px` and `--crosshook-subtab-padding-inline: 16px`. These classes are currently unused — they were designed in anticipation of within-page sub-tab navigation.

### Implication for layout decision

The existence of pre-built subtab CSS means the sub-tab approach (option 2 from the feature description) has lower implementation cost than previously assumed. However, the constraint from Implementation Constraints §1 still applies: sub-tabs must be composed at the `ProfilesPage` level, not inside `ProfileFormSections`, to avoid breaking the `InstallPage` modal reuse.

A hybrid approach is viable: discrete panels (option 1) for the primary layout restructure, with optional sub-tabs inside a single "Configuration" panel for the runner-specific sections (Steam Runtime / Proton Runtime / Native), using the pre-existing `.crosshook-subtab` classes with Radix `Tabs.Root`.

---

## UX Research Synthesis

Findings from UX research (`docs/plans/ui-enhancements/research-ux.md`) confirmed and integrated:

### Three-level collapse hierarchy violates NN/G two-level limit

The current nesting depth is:

1. `CollapsibleSection "Advanced"` — `defaultOpen=false`, wraps the entire editor (`ProfilesPage.tsx:622`)
2. `OptionalSection "Trainer details"` / `"Working directory override"` — native `<details>` inside `ProfileFormSections.tsx:778,1055,1111`; only collapsed in `reviewMode`
3. `CollapsibleSection "Health Issues"` — nested `CollapsibleSection` inside "Advanced" (`ProfilesPage.tsx:709`), `defaultOpen=true` but only rendered when profile is broken/stale

Nielsen Norman Group's guidance on progressive disclosure limits nesting to two levels before cognitive overhead outweighs the benefit. The current design reaches three levels in the worst case (Advanced > Trainer details > and separately Advanced > Health Issues when both are collapsed). **This is a concrete UX violation, not just a preference.**

### Competitive app patterns confirm task-oriented flat groupings

| App                    | Pattern                                                                | Lesson                                                         |
| ---------------------- | ---------------------------------------------------------------------- | -------------------------------------------------------------- |
| Heroic Games Launcher  | Wine/Proton + Performance + Launch Options + small "Advanced" residual | "Advanced" should be a small true residual, not the whole form |
| Lutris                 | Tabs per runner (Game, Runner, System, Wine)                           | Runner-specific settings deserve a dedicated visual area       |
| Bottles (GTK4)         | Sidebar categories per bottle; new features get prominent cards        | New/important features should not default-hide                 |
| macOS Ventura Settings | Sidebar list + content area grouped by user task                       | Group by task, not by skill level                              |
| VS Code Settings       | Flat list with section headers + search; no collapses within panes     | Flat expandable sections beat nested collapses                 |

### UX-recommended card groupings (task-oriented)

These align closely with the business analysis Proposed Section Groupings but use more task-centric naming:

| Card                 | Contents                                               | Notes                                                |
| -------------------- | ------------------------------------------------------ | ---------------------------------------------------- |
| Profile Identity     | Profile name, selector                                 | Selector already partially visible; consolidate here |
| Game & Runtime       | Game name/path, Runner Method, runtime-specific fields | Core of every profile; must be always-visible        |
| Trainer              | Trainer path, type, loading mode, version              | Keep co-located; only shown for non-native           |
| Environment & Launch | Custom env vars, ProtonDB lookup, working directory    | Power-user panel; collapsible with count badge       |
| Launcher Export      | Already separate — keep as-is                          | Existing separate panel pattern works                |
| Health & Diagnostics | Health issues, stale info, diagnostics                 | Promote from nested collapse to own panel            |

### Health badge disconnect is a known UX anti-pattern

Status badges (HealthBadge, OfflineStatusBadge, version badge) are currently in the `meta` slot of the Advanced `CollapsibleSection` header. They are visible without expanding, but the content they describe (Health Issues) is hidden. This is a "status indicator without context" anti-pattern — users see a red badge but cannot act on it without first expanding a different section, then scrolling, then expanding again. The fix is to co-locate the badge with the actionable content, either in a dedicated Health panel or directly adjacent to the profile selector where the user first looks.

---

## Security Constraints

From security research (`docs/plans/ui-enhancements/research-security.md`):

### Injection fields must remain UI-absent (W3)

`GameProfile.injection` (`dll_paths: string[]`, `inject_on_launch: boolean[]`) exists in the type and default state but is intentionally never rendered in any user-facing form component. These fields are populated exclusively by the install/migration pipeline. `exchange.rs:259` explicitly clears `dll_paths` during community export sanitization — this is active, intentional exclusion.

**Hard constraint**: any component reorganization that iterates over `GameProfile` keys or auto-renders fields from the profile type must explicitly exclude `injection.*`. This rules out any generic "render all profile fields" pattern as a shortcut during refactor.

### Path fields: free-form, backend-enforced, Browse affordances must be preserved

All path fields (`game.executable_path`, `trainer.path`, `steam.compatdata_path`, `steam.proton_path`, `runtime.prefix_path`, `runtime.proton_path`, `runtime.working_directory`, `steam.launcher.icon_path`) are free-form strings with no client-side path validation. This is correct for a launcher. Validation is backend-only. The Browse button pattern (Tauri dialog APIs, not string execution) is the correct UX affordance and must be preserved wherever path fields land in the restructured layout.

### SteamGridDB API key handling (Issue #52)

The SteamGridDB API key stored in `settings.toml` is not a system credential but a user-chosen access token. It must be transmitted only to `api.steamgriddb.com` over HTTPS. It must not be logged, exposed via IPC in cleartext beyond what is required to pass it to the fetch function, or included in community profile exports. The Settings page UI should use a masked input or `type="password"` field as a usability courtesy (not a security control — the value is plaintext in TOML). The Tauri command that reads settings for the frontend must not return the raw API key in the general settings payload if it can be avoided; the backend should make image fetch calls directly rather than passing the key to the frontend.

---

## Technical Design Constraints

From technical design research (`tech-designer`):

### `launch.*` is split across two pages — do not consolidate

`GameProfile.launch` contains fields served by two different pages:

- **ProfilesPage** renders: `launch.method` (Runner Method), `launch.custom_env_vars` (Custom Env Vars)
- **LaunchPage** renders: `launch.optimizations`, `launch.presets`, `launch.gamescope`, `launch.mangohud`

`ProfileFormSections` contains zero references to `gamescope`, `mangohud`, or `optimizations` (confirmed). The restructure must not move these to `ProfilesPage` — the LaunchPage multi-panel pattern (CollapsibleSection per feature: Gamescope, MangoHud, Launch Optimizations, Steam Launch Options) is the right model for those fields and already works well.

### Form state is a single `GameProfile` — tab-switching is safe

`ProfileContext` holds a single `profile: GameProfile` and `updateProfile: (updater) => void` and wraps the entire app. Switching between sub-tabs or panels within `ProfilesPage` carries zero risk of state loss — context stays mounted regardless of which panel is visible. The `onUpdateProfile` updater pattern `(current: GameProfile) => GameProfile` passes cleanly to any sub-component regardless of how sections are reorganized.

### `LauncherMetadataFields` and `TrainerVersionSetField` are local sub-components

Both are defined inside `ProfileFormSections.tsx` and not exported. If launcher metadata or trainer version fields move to a separate panel at the `ProfilesPage` level, these sub-components must either be extracted and exported, or the logic inlined at the call site. Neither is complex — extraction is straightforward.

### LaunchPage multi-panel pattern is the existing precedent for the restructure

`LaunchPage` already demonstrates the target architecture: discrete `CollapsibleSection className="crosshook-panel"` blocks for Gamescope, MangoHud, LaunchOptimizations, and SteamLaunchOptions, each receiving `profile` props and an `onUpdateProfile`-equivalent callback. `ProfilesPage` should adopt the same pattern, replacing the monolithic Advanced section with equivalent discrete panels.

### Cover art fetch is a backend Tauri command, not a frontend fetch (Issue #52)

Image fetching for `game_image_cache` must happen in a `#[tauri::command]` Rust handler that:

1. Checks the `game_image_cache` table for a valid cached entry
2. Falls through the fallback chain (SteamGridDB → Steam API → nil)
3. Writes downloaded bytes to the filesystem at the canonical path
4. Inserts or updates the `game_image_cache` row
5. Returns a filesystem path (or `None`) to the frontend

The frontend renders the image using Tauri's asset protocol (e.g., `convertFileSrc`) to load the local file. It never makes direct HTTP requests for images, and the SteamGridDB API key never crosses the IPC boundary to the frontend.
