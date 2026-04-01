# Business Analysis: Library Home Page

## Executive Summary

The library-home feature introduces a Steam-style poster art grid as a new primary entry point for CrossHook. It surfaces all user game profiles as visual cards using Steam cover art (portrait 600×900), each with three direct actions: launch, edit, and favorite toggle. The home page is additive — it does not replace the existing Profiles or Launch pages, but provides a faster path to the most common user actions (launch a game, edit a profile, mark favorites) without requiring the user to navigate the full profile editor. Playtime metadata is displayed on cards but is not yet tracked in the backend; it is a display-only placeholder for a future tracking feature.

---

## User Stories

**User with multiple game profiles wanting to quickly launch a game**

- As a user with 5+ profiles, I want to see all my games at a glance as visual poster art so I can immediately identify and launch the game I want without selecting from a text dropdown.

**User on Steam Deck in couch/gamepad mode**

- As a Steam Deck user, I want large poster art cards that are easy to click or navigate with a gamepad so that finding and launching games is as immediate as the Steam library experience.

**User managing favorites for faster access**

- As a frequent user of a small set of games, I want to mark profiles as favorites so they surface prominently and I can build a personal shortlist without changing the full profiles list.

**User who wants to quickly jump to editing a specific profile**

- As a returning user who wants to change trainer settings, I want an "Edit" button on each card so I can jump directly to that profile's editor without first navigating to the Profiles page and then finding the profile in a dropdown.

**User searching for a specific profile by name**

- As a user with many profiles, I want a search bar on the library home so I can type a partial name and instantly filter the grid to the matching profile.

**User who prefers a list view over a grid**

- As a user on a narrow screen or who prefers compact display, I want a toggle between grid and list view so the library adapts to my preference.

**User without Steam App IDs configured**

- As a user who hasn't configured Steam App IDs on all profiles, I want profiles without cover art to display gracefully with a text-based fallback so the grid is still usable.

---

## Business Rules

### Core Rules

**R1 — Profile-to-card mapping (1:1)**
Each saved profile maps to exactly one card. Unsaved (new/draft) profiles that exist only in memory are not shown. The card's game title is derived from `profile.game.name` if set; otherwise it falls back to the filename component of `profile.game.executable_path` (stripping the extension), using the same `deriveGameName` logic from `useProfile.ts:164`.

**R2 — Cover art resolution order**
Cover art for a card is resolved in this priority order:

1. `profile.game.custom_cover_art_path` if non-empty (local file, converted via `convertFileSrc`)
2. Steam cover art fetched via the `fetch_game_cover_art` IPC command when `profile.steam.app_id` is a non-empty numeric string
3. Fallback placeholder (no broken image; graceful text-only display)

This mirrors the logic in `useGameCoverArt.ts` (the existing hook that handles both custom and Steam-sourced art).

**Backend constraint (confirmed):** `GameImageType::Cover` in `game_images/client.rs:336` currently maps to `header.jpg` (460×215 landscape), not the portrait 600×900 `library_600x900` format. A Rust-layer change to `build_download_url` is required to support portrait art for library-home. The portrait URL format is `https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900.jpg`. Not all games have this asset; a fallback to `header.jpg` is needed when the portrait image returns 404.

**R3 — Launch button behavior (two-step launch model)**
Clicking "Launch" from a library card must:

1. Call `selectProfile(profileName)` on `ProfileContext` to make that profile the active one
2. Navigate to the `launch` route

The `LaunchStateContext` re-derives its `LaunchRequest` from `ProfileContext` on every render, so simply activating the profile and switching route is sufficient — no bespoke launch logic is needed in the library-home page. The user still interacts with the `LaunchPanel` to actually start the process.

For two-step (dual-mode) launches (`proton_run` / `steam_applaunch`), the trainer is launched separately. The library-home "Launch" button puts the user at the Launch page; they then click "Launch Game" and subsequently "Launch Trainer" per the existing `LaunchPhase` state machine: `Idle → GameLaunching → WaitingForTrainer → TrainerLaunching → SessionActive`.

**R4 — Edit button behavior**
Clicking "Edit" on a card must:

1. Call `selectProfile(profileName)` on `ProfileContext`
2. Navigate to the `profiles` route

No additional state is required — `ProfilesPage` reads `selectedProfile` from `ProfileContext` and displays the editor for whatever profile is currently active.

**R5 — Favorite toggle**
Clicking the heart button calls `toggleFavorite(name, !isFavorite)` from `ProfileContext`, which wraps the `profile_set_favorite` IPC command. Favorites are persisted in the SQLite metadata DB via `MetadataStore.set_profile_favorite`. The heart reflects the current `favoriteProfiles` list from `ProfileContext`. A profile is a favorite if its name is in `favoriteProfiles: string[]`.

Toggling favorite on the library home must be optimistic if possible (immediate UI feedback), falling back to re-read from context on completion. If the IPC call fails, the toggle should revert and surface an error.

**Terminology note (confirmed):** The existing UI uses "pin" and ★/☆ (star icons) as the visible label for this action (`ProfileFormSections.tsx:295-300`, `ThemedSelect.tsx:57-73`, `PinnedProfilesStrip`). The IPC layer uses "favorite" (`profile_set_favorite`, `profile_list_favorites`, `favoriteProfiles`). These are the same concept — one data store, one toggle. Library-home introduces a heart (♥/♡) icon per the Figma spec, which is a third label for the same action. The implementation decision is: use heart on library-home cards (new visual), leave existing star on Profiles/Launch pages unchanged. No data model change is needed; the `is_favorite` field and IPC commands remain as-is.

**R6 — Search/filter**
Search is client-side filtering of the already-loaded `profiles: string[]` list from `ProfileContext`. Matching is case-insensitive substring against the profile name. No backend call is required. The filter applies to the display only — it does not change the active profile or affect `favoriteProfiles`.

**R7 — View mode toggle (grid vs. list)**
The toggle is UI-only preference state, not persisted to the backend (no TOML or SQLite entry needed for v1). It can be stored in React state or `localStorage`. Grid is the default.

**R8 — Profile must be saved to appear**
Only profiles returned by `profile_list` (i.e., persisted to disk) are shown. This is the same `profiles: string[]` array already available in `ProfileContext` via `refreshProfiles → invoke('profile_list')`.

**R9 — No launch from home if profile has no executable**
If `profile.game.executable_path` is empty, `buildProfileLaunchRequest` returns `null`, making the launch button disabled/inert on the LaunchPage. Library home should not try to validate this pre-emptively; it navigates to Launch, and the Launch page handles the "no executable" state. The card's Launch button should still navigate to Launch.

**R10 — Playtime metadata is display-only placeholder**
No playtime tracking exists in the backend today. Playtime on cards must be either omitted or displayed as a static placeholder (e.g., "0h played"). It must NOT show a number that implies tracked data.

### Edge Cases

- **Empty profiles list**: If `profiles.length === 0`, show an empty-state prompt directing the user to create a profile via the Profiles page or the onboarding wizard.
- **Profile load error on navigation**: If `selectProfile(name)` fails (e.g., file deleted between list refresh and click), the error surfaces via `ProfileContext.error` on the target page (LaunchPage or ProfilesPage). Library home does not need to handle this itself.
- **Stale `profiles` list**: The `refreshProfiles` call from `ProfileContext` is triggered on mount; the library home should trigger `refreshProfiles` on first mount to ensure the list is current, since the user may have added/deleted profiles in another session.
- **Cover art loading during navigation**: Cover art fetches are non-blocking. A card can display while its art is loading (skeleton/spinner). Navigation is not blocked by art loading.
- **Long profile names**: Names can be arbitrary length. Cards must truncate with ellipsis at a sensible line count.

---

## Workflows

### Primary: Launch a Game from Home

1. User opens CrossHook → lands on library-home (new default route, or navigates via sidebar)
2. Library home calls `refreshProfiles()` on mount; displays a spinner while loading
3. Grid of profile cards renders with cover art, game title, and three action buttons
4. User clicks the **Launch** button on a card
5. `selectProfile(name)` is called → `ProfileContext` loads the profile via `profile_load` IPC
6. `setRoute('launch')` navigates to the Launch page
7. LaunchPage displays the `LaunchPanel` with the now-active profile pre-selected
8. User clicks "Launch Game" → the two-step launch sequence begins

### Primary: Edit a Profile from Home

1. User sees a card for a profile they want to edit
2. User clicks the **Edit** button
3. `selectProfile(name)` loads the profile into `ProfileContext`
4. `setRoute('profiles')` navigates to the Profiles page
5. ProfilesPage displays the profile editor populated with the loaded profile

### Primary: Toggle Favorite from Home

1. User sees a card and clicks the **heart** button
2. Heart icon reflects current favorite state (filled = favorite, outline = not)
3. `toggleFavorite(name, !isFavorite)` is called
4. IPC `profile_set_favorite` persists the change
5. `loadFavorites()` re-fetches the favorites list; heart updates to new state
6. On failure: heart reverts to previous state; error is surfaced (toast or inline)

### Primary: Search / Filter

1. User types in the search bar
2. Client-side filter reduces the displayed cards to those whose names contain the search string (case-insensitive)
3. Clearing the search restores the full grid
4. Search state is not persisted across sessions

### Primary: Switch View Mode

1. User clicks grid/list toggle button
2. Layout reflows between card grid and compact list view
3. All card actions (Launch, Edit, Favorite) remain available in both views

### Error Recovery: Profile Load Failure after Card Click

1. User clicks Launch or Edit
2. `selectProfile` invokes `profile_load` IPC which throws (e.g., file missing)
3. `ProfileContext.error` is set to the error message
4. User lands on LaunchPage/ProfilesPage which surfaces the error from `ProfileContext`
5. No special handling needed in library-home

---

## Domain Model

### Key Entities

**GameProfile** — The core configuration unit. Persisted as a TOML file under the user's data directory. Identified by a string `name` (the filename without extension). Contains: `game` (name, executable path, optional custom cover art path), `trainer` (path, type, loading mode), `steam` (enabled, app_id, compatdata_path, etc.), `launch` (method, optimizations, presets, env vars, gamescope, mangohud).

**Profile Name (string)** — The stable identity for a profile. It is both the display label and the key used to load/save/delete. Names are not globally unique beyond the local TOML store; they are filesystem filenames.

**Favorite** — A boolean tag stored in SQLite `MetadataStore` (not in the profile TOML). The `profile_list_favorites` command returns `Vec<String>` of favorited profile names. This is purely metadata — a profile is valid and launchable regardless of its favorite state.

**Cover Art** — Either a local file path (`custom_cover_art_path`) or a filesystem-cached image fetched from Steam Store / SteamGridDB, cached at `game_image_cache` table location via `download_and_cache_image`. The `useGameCoverArt` hook abstracts this and returns a `coverArtUrl: string | null`.

**Active Profile** — The profile currently loaded into `ProfileContext` (`selectedProfile: string`). Switching the active profile triggers an IPC call to load its data. At any given time, only one profile is "active" globally across the application.

**Resolved Launch Method** — One of `steam_applaunch | proton_run | native`. Derived by `resolveLaunchMethod(profile)` from the profile's `launch.method` field with fallback rules. Determines whether the launch is two-step (game then trainer separately) or single-step (native).

### State Transitions

**Active Profile State Machine** (within `ProfileContext`):

- `no profile selected` → `loading` (on `selectProfile`)
- `loading` → `loaded/dirty=false` (success) | `loaded with error` (failure)
- `loaded` → `dirty=true` (on `updateProfile`)
- `dirty=true` → `loaded/dirty=false` (on `saveProfile`)

**Launch Phase State Machine** (within `LaunchStateContext`):

- `Idle` → `GameLaunching` (user clicks Launch Game)
- `GameLaunching` → `WaitingForTrainer` (game started; two-step mode) | `SessionActive` (native/single-step mode)
- `WaitingForTrainer` → `TrainerLaunching` (user clicks Launch Trainer)
- `TrainerLaunching` → `SessionActive` (trainer started)
- Any active phase → `Idle` (reset)

---

## Existing Codebase Integration

### Profile Loading

`ProfileContext` (`src/crosshook-native/src/context/ProfileContext.tsx`) wraps `useProfile` and exposes `profiles`, `favoriteProfiles`, `selectedProfile`, `selectProfile`, `toggleFavorite`, and `refreshProfiles`. Library home must be rendered inside `ProfileProvider` (already wraps the entire app).

Calling `selectProfile(name)` is the canonical way to activate a profile before navigating to Launch or Profiles. It invokes `profile_load` IPC, sets `selectedProfile`, and syncs `last_used_profile` in app settings.

### Navigation

Navigation is managed by `App.tsx` via `route` state and `setRoute`. The `ContentArea` component renders pages in a `Tabs.Content` switch. Library-home will need to be added as a new `AppRoute` (e.g., `'home'`), added to `VALID_APP_ROUTES`, `SIDEBAR_SECTIONS`, and the `ContentArea` switch. The `onNavigate` callback passed to `ContentArea` is available for child-to-parent navigation requests.

### Favorite Toggle IPC

`toggleFavorite(name: string, favorite: boolean)` in `useProfile.ts:519` calls `invoke('profile_set_favorite', { name, favorite })` then calls `loadFavorites()` to refresh the list. This is already exposed via `ProfileContext`.

### Cover Art Hook

`useGameCoverArt(steamAppId, customCoverArtPath)` (`src/crosshook-native/src/hooks/useGameCoverArt.ts`) handles both custom and Steam-sourced art. It calls `fetch_game_cover_art` IPC with `imageType: 'cover'`. Returns `{ coverArtUrl: string | null, loading: boolean }`.

### Game Metadata (Title / Description)

`useGameMetadata(steamAppId)` (`src/crosshook-native/src/hooks/useGameMetadata.ts`) fetches from the Steam Store API via cache. Returns `appDetails: SteamAppDetails | null` containing `name`, `short_description`, `header_image`, and `genres`. The card title can optionally use `appDetails.name` as a display override over `profile.game.name`.

### Existing Profile List Access and Cover Art Data Gap

`profileState.profiles` is a `string[]` loaded on mount via `refreshProfiles → profile_list` IPC. The list contains profile names only. The SQLite `profiles` table stores `current_filename`, `game_name` (denormalized from TOML), `is_favorite`, `launch_method`, and `content_hash` — but does **not** store `steam_app_id` or `custom_cover_art_path`.

This creates a data gap for cover art display: `useGameCoverArt` requires both `steamAppId` and `customCoverArtPath`, which live only in the full profile TOML (`GameProfile.steam.app_id`, `GameProfile.game.custom_cover_art_path`). To show cover art on cards without loading every full profile at page render, one of two approaches is required:

1. **New `profile_list_summaries` IPC command** — returns lightweight per-profile records (`name`, `game_name`, `steam_app_id`, `custom_cover_art_path`) by reading TOML files server-side in a single batch call. This is the preferred approach for performance.
2. **N individual `profile_load` calls** — acceptable for very small libraries but does not scale.

The SQLite `profiles` table already contains `game_name` (denormalized), so search by game name within the metadata DB is feasible without TOML reads — but `steam_app_id` and `custom_cover_art_path` are not stored there today.

**Confirmed schema columns** (from `metadata/migrations.rs` and `metadata/models.rs:ProfileRow`):

- `profile_id`, `current_filename`, `current_path`, `game_name` (nullable), `launch_method` (nullable), `content_hash` (nullable), `is_favorite` (integer, default 0), `source`, `deleted_at`, `created_at`, `updated_at`

The `game_image_cache` table stores images keyed on `(steam_app_id, image_type, source)`, not on profile name. Art can be served from cache without knowing which profile maps to which app ID, once the app ID is known.

---

## Success Criteria

1. All saved profiles are displayed as cards in a responsive grid on the home page.
2. Clicking "Launch" on a card activates that profile and navigates to the Launch page with the profile pre-selected.
3. Clicking "Edit" on a card activates that profile and navigates to the Profiles page with the editor pre-populated.
4. Clicking the heart button on a card toggles the favorite state and the icon updates to reflect the new state.
5. The search bar filters the visible cards by profile name (case-insensitive substring); clearing search restores all cards.
6. Grid/list toggle switches the layout; both layouts show all three card actions.
7. Profiles without a Steam App ID or custom cover art display a text-based fallback with no broken image placeholders.
8. The `refreshProfiles()` call on mount ensures the list is up-to-date when the home page is opened.
9. Playtime is either omitted or clearly presented as a non-tracked placeholder.
10. Navigation state transitions do not reset the `LaunchStateContext` (the provider wraps the entire app shell, so route changes do not tear down the context).

---

## Open Questions

1. **Is library-home the default startup route?** If so, the `useState<AppRoute>('profiles')` in `AppShell` needs to change to `'home'`. This affects first-time user onboarding flow — the `OnboardingWizard` checks `has_profiles` and currently assumes the Profiles page is visible.

2. **Playtime tracking scope**: The feature description shows "playtime metadata" on cards. Is this in scope for the initial implementation, or strictly a future feature? If deferred, what placeholder copy is acceptable?

3. **Favorites-first ordering**: Should the grid default to showing favorites at the top, or should sort order be strictly alphabetical/by recent use?

4. **Card "Launch" button when profile has no executable path**: Navigate to Launch anyway (current plan), or disable the button with a tooltip? The latter gives the user immediate feedback but adds complexity.

5. **Profile load latency on card click**: If the user clicks "Launch" and `selectProfile` takes >300ms, is there a visual loading indicator on the card? Or does the user just see the Launch page in a loading state?

6. **Grid card count per row**: The spec says 190px-wide cards at 3:4. Should the grid column count be fixed (e.g., `auto-fill` CSS grid) or configurable?

7. **Library home sidebar label**: What is the label and icon for the new route in the sidebar? ("Home", "Library", or something else?)

8. **Cover art data access strategy**: The `profiles: string[]` list contains names only; `steam_app_id` and `custom_cover_art_path` are not stored in SQLite. Will a new `profile_list_summaries` IPC command be added to return lightweight per-profile cover art metadata, or will cards fall back to profile-name-only display until a profile is loaded? This is the primary unresolved architectural question confirmed by schema analysis.

9. **Navigation action sequencing correctness**: `selectProfile(name)` is async (IPC call, ~50–200ms). The Launch and Edit card actions must `await selectProfile` before calling `onNavigate`. If navigation fires before profile load completes, `LaunchPage`/`ProfilesPage` will briefly show the previously active (or empty) profile. The implementation must explicitly await the selection and handle loading state on the card — this is a correctness requirement, not just a UX polish concern.

10. **Favorites icon terminology alignment**: The existing Profiles/Launch pages show ★/☆ (star) as the "pin" action. Library-home Figma spec shows ♥/♡ (heart). Both map to the same `is_favorite` backend field. Should the icon be unified to heart across the whole app, or remain star on existing pages and heart only on library-home? This is a design decision with no data model impact.

11. **Empty-state / onboarding integration**: If library-home is the default route and `profiles.length === 0`, the page must surface a CTA ("Create your first profile" or similar). The `OnboardingWizard` fires via Tauri `onboarding-check` event on `AppShell` mount regardless of route — it will still appear. But the grid empty-state must not be a blank screen. Define what the zero-profile empty state looks like and whether it links to the wizard or to the Profiles page.

12. **`last_launched_at` / recently-played (Phase 3 scope)**: Storing per-profile last-launch timestamps requires a new SQLite column or table (`launch_operations` already has `started_at`/`finished_at` per operation but no aggregate view is exposed via IPC). If recently-played ordering is desired, this is a storage boundary decision: operational/history metadata → SQLite, correct classification. Must be deferred to Phase 3 and not assumed in Phase 1 business rules.
