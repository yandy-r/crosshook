# Business Analysis: Proton App ID & Tri-Art System

## Executive Summary

Proton-run profiles currently cannot display cover or portrait art in the Library page because the art resolution system exclusively reads `steam.app_id`, which is only populated for `steam_applaunch` profiles. The fix is a targeted, additive change: add an optional `runtime.steam_app_id` field to `RuntimeSection` in the TOML profile model, extend the art resolution chain to fall back to it, and surface a corresponding input field in the profile editor. A companion scope item extends the single `custom_cover_art_path` on `GameSection` to cover three art types (cover, portrait, background) with per-type custom override slots. The net result is that any proton-run game with a known Steam App ID gets the same automatic art experience as Steam-managed games — without any change to launch behavior.

---

## User Stories

### US-1: Proton Gamer — Auto Art for Known Steam Games

> As a gamer who runs titles via umu-run, Heroic, or Lutris with a manual Proton prefix, I want to set my game's Steam App ID once so the Library page fills in cover and portrait art automatically — without having to find and upload image files myself.

**Acceptance criteria:**

- A "Steam App ID" field appears in the profile editor when `launchMethod === 'proton_run'`.
- Saving the profile triggers art download on next Library page load without a page refresh.
- The field does not appear or influence anything when `launchMethod === 'steam_applaunch'` (that profile already has `steam.app_id`).
- Launch behavior is unchanged — the field is metadata-only.

### US-2: Steam Gamer — Custom Portrait Override

> As a Steam gamer whose game has an ugly official portrait card, I want to upload my own portrait image for that profile so the Library grid shows my preferred artwork instead of the Steam default.

**Acceptance criteria:**

- A "Custom Portrait Art" upload slot exists in the profile editor Media section for `steam_applaunch` profiles.
- Uploading a custom portrait replaces the auto-downloaded portrait in the Library grid.
- Removing the custom portrait reverts to the auto-downloaded version.

### US-3: Power User — Full Art Customization (Mix-and-Match)

> As a power user, I want to override art independently per slot — maybe use a custom cover but keep the official portrait — so I have granular control without needing to fill all three slots.

**Acceptance criteria:**

- Custom art overrides operate per art type independently.
- Empty custom slots fall back to auto-downloaded art, then to a placeholder.
- No slot is required; all three default to empty and resolve through the fallback chain.

### US-4: Offline User — Graceful Degradation

> As a user who is sometimes offline, I want previously downloaded art to remain visible, and for missing art to show a placeholder (initials or icon) rather than a broken image or error.

**Acceptance criteria:**

- Valid cached images (within 24-hour TTL) are served from disk without a network call.
- Stale cached images are served as fallback if the refresh attempt fails.
- When no art is available, the Library card shows a text-initials fallback, not an error state.

### US-5: Community Profile Importer — Portable Art Metadata

> As a user importing a shared community profile with `runtime.steam_app_id` set, I want CrossHook to automatically download art for that game without extra steps.

**Acceptance criteria:**

- `runtime.steam_app_id` survives portable profile export/import (it is not a machine-specific path, so it lives in the base profile, not in `local_override`).
- Art download is triggered automatically when the Library page loads the imported profile.

---

## Business Rules

### Core Art Resolution Rules

**BR-1: Art source priority chain (per art type)**

For every art type (cover, portrait, background), resolution proceeds in this order and stops at the first non-empty, valid result:

1. Custom uploaded art path for this type (user-provided, always wins).
2. Auto-downloaded art keyed to the effective app_id (see BR-9: `steam.app_id` if non-empty, else `runtime.steam_app_id`).
3. Stale cached art (expired but file still on disk).
4. Placeholder / fallback (initials card, empty state).

Resolution is per art type independently. Setting a custom portrait does not affect cover or background resolution, and vice versa.

**BR-2: `runtime.steam_app_id` is media-only**

The field must never be read by the launch pipeline. `resolve_launch_method`, `LaunchRequest` construction, and all process-spawning code must remain unaware of it. Any code path that resolves an app_id for launch purposes reads only `steam.app_id`.

**BR-3: Art type definitions**

| Type       | Usage                                         | Steam CDN pattern                  | SteamGridDB endpoint                           |
| ---------- | --------------------------------------------- | ---------------------------------- | ---------------------------------------------- |
| Cover      | Profile editor header backdrop, grid fallback | `header.jpg` (460×215)             | `/grids/steam/{id}?dimensions=460x215,920x430` |
| Portrait   | Library grid card image                       | `library_600x900_2x.jpg` (600×900) | `/grids/steam/{id}?dimensions=342x482,600x900` |
| Background | Future use (hero/banner)                      | `library_hero.jpg`                 | `/heroes/steam/{id}`                           |

**BR-4: `steam_app_id` format validation**

Both `steam.app_id` and `runtime.steam_app_id` must be validated as pure ASCII decimal integers before any network call or filesystem path construction:

- Empty string is the valid "not set" state — treat as `None`, not as an error.
- Valid: non-empty string of ASCII digits only, 1–12 characters (Steam IDs are at most 10 digits in practice; 12 provides a safe cap against absurd inputs).
- Invalid: any non-digit character, spaces, leading zeros beyond "0" itself, or length exceeding 12 digits.
- Validation must be applied at **profile save time** to surface errors in the UI — not deferred to image-fetch time where errors are silently swallowed into a `None` result.
- The download pipeline (`download_and_cache_image`, `safe_image_cache_path`) already enforces numeric-only validation as a defence-in-depth layer, but this does not substitute for save-time validation.

**BR-5: Custom art import is idempotent**

When a user uploads art, the file is copied into the managed media directory (`~/.local/share/crosshook/media/covers/`) using a content-addressed filename (first 16 hex chars of SHA-256). Re-uploading the same file returns the existing path without re-writing. This rule applies independently to each art type slot (they will use type-segregated subdirectories: `media/covers/`, `media/portraits/`, `media/backgrounds/`).

**BR-6: Custom art paths are machine-local**

Custom art paths in `game.custom_cover_art_path` (and the forthcoming `custom_portrait_art_path`, `custom_background_art_path`) are machine-specific filesystem paths. They belong in `local_override.game.*` during storage and are cleared from the portable profile export. This matches the existing pattern for executable and trainer paths. `runtime.steam_app_id`, by contrast, is _not_ a local path — it is portable metadata and must stay in the base profile section.

The existing `custom_cover_art_path` is already cleared from community exports via a two-step mechanism (see `profile/models.rs:440-468`, `exchange.rs:257-264`):

1. `storage_profile()` explicitly copies `game.custom_cover_art_path` → `local_override.game.custom_cover_art_path`, then clears the base field.
2. `portable_profile()` then sets `local_override = LocalOverrideSection::default()`, wiping all local overrides wholesale.

The new portrait and background fields require changes in **four** places — omitting any one of them produces a silent bug:

- `LocalOverrideGameSection` struct: add `custom_portrait_art_path` and `custom_background_art_path` fields.
- `LocalOverrideGameSection::is_empty()`: include new fields — otherwise profiles with only portrait/background overrides suppress the entire `[local_override.game]` TOML block.
- `effective_profile()`: merge new override fields into base, following the same pattern as `custom_cover_art_path` (lines 416-418).
- `storage_profile()`: copy new fields to `local_override.game.*`, then clear from base (lines 445, 453 pattern).

`portable_profile()` itself needs no change — it wipes `local_override` entirely by replacing with `Default`, which covers all fields including future ones. The safety guarantee is: once the fields exist in `LocalOverrideGameSection`, they will be cleared by `portable_profile()` automatically. The only gap is in `storage_profile()` and `effective_profile()` which have explicit field lists.

**BR-7: Art cache TTL and stale fallback**

Cached images expire after 24 hours (constant in `client.rs`). On expiry the system attempts a fresh download; if the download fails, the stale file on disk is returned rather than `None`. If a cache row references a missing file, the row is deleted and `None` is returned (no stale fallback).

**BR-8: Image constraints**

All downloaded and user-imported images are subject to:

- Maximum size: 5 MB.
- Allowed formats: `image/jpeg`, `image/png`, `image/webp` (detected by magic bytes, not file extension).
- SVG and HTML are unconditionally rejected.

**BR-9: Effective media app_id resolution**

The effective app_id used for art download and metadata lookup is resolved as a single value: `steam.app_id` if non-empty, else `runtime.steam_app_id`. This resolution applies equally to `steam_applaunch` and `proton_run` profiles and is computed in one place (backend `profile_list_summaries` / a shared helper). The frontend always receives a single resolved `effectiveSteamAppId` — it does not implement this fallback itself.

This means existing `proton_run` profiles that already have `steam.app_id` set for ProtonDB lookup continue to drive art download without any migration. The new `runtime.steam_app_id` field is a clean alternative slot for profiles that prefer to keep the steam section empty.

**BR-10: Per-type art resolution is fully independent**

Each art type (cover, portrait, background) resolves its own custom path and auto-download independently. A profile with a custom portrait and no custom cover will show the custom portrait in the Library card and the auto-downloaded cover in the profile editor backdrop. No "all or nothing" constraint exists across art types.

**BR-11: SteamGridDB API key is user-scoped and opt-in**

CrossHook cannot bundle a shared SteamGridDB API key. Each user must supply their own key (obtained by creating a free SteamGridDB account). This is a one-time setup step, not a recurring cost. The business rules follow from this:

- Without a key, all art downloads use Steam CDN exclusively. Steam CDN covers Cover, Portrait (with three-URL fallback chain), and Background (`library_hero.jpg`) — sufficient for the large majority of games.
- With a key, SteamGridDB is tried first; Steam CDN is the fallback. SteamGridDB provides higher quality and more variety but does not unlock new art types.
- The presence or absence of a SteamGridDB key must never block the core feature (proton_run art display). Users who skip key setup must still get Steam CDN art automatically.
- SteamGridDB ToS permits community tool integration (established precedent: Heroic, SteamTinkerLaunch, Cartridges). Formal verification recommended before shipping.

**BR-12: Steam CDN URL patterns are stable but unofficial**

Steam CDN art URLs (e.g. `https://cdn.cloudflare.steamstatic.com/steam/apps/{id}/header.jpg`) are inferred from Steamworks asset specs and have been stable for years, but are not formally documented by Valve. The system must tolerate CDN URL changes gracefully — a failed download returns stale cache or `None` rather than an error state.

**BR-13: App ID validation at save time is format-only; art resolution is advisory**

Saving a profile with a valid-format but unresolvable App ID (e.g. a real integer that maps to no Steam game) is permitted. The profile saves successfully; art download is attempted lazily and silently falls back to placeholder on failure. The profile editor must not make a network call to verify App ID existence at save time. Only format validation (BR-4: non-empty decimal integer, ≤12 digits) is enforced as a hard save gate.

**BR-14: Custom art files are NOT deleted when a slot is cleared**

The managed media directory (`~/.local/share/crosshook/media/`) uses content-addressed filenames (first 16 hex chars of SHA-256). The same imported image shared across multiple profiles maps to the same file. Deleting the file on slot-clear would silently break all other profiles referencing it. Therefore:

- Clearing a custom art path field removes only the profile's reference to the file — the file on disk is preserved.
- Orphaned files (imported art no longer referenced by any profile) accumulate in the media dir over time. A future media garbage-collection pass can clean these safely by comparing all profile custom art paths against files on disk; this is deferred to a follow-up feature.
- Users wanting to reclaim disk space must do so manually (delete files in `~/.local/share/crosshook/media/`) or via a future "Clean up unused media" settings action.

### Edge-Case Rules

**EC-1: Both `steam.app_id` and `runtime.steam_app_id` set**

This is expected for `proton_run` profiles that already had `steam.app_id` set for ProtonDB lookup before this feature. `steam.app_id` takes priority (BR-9). The profile continues to work correctly with no migration. The user can optionally clear `steam.app_id` and move the value to `runtime.steam_app_id` if they prefer semantic clarity, but this is not required.

**EC-2: `proton_run` profile with no `steam_app_id` set**

Art resolution falls through to placeholder. No download is attempted. Library card shows initials fallback. This is the current behavior for all proton_run profiles and must remain unchanged.

**EC-3: Custom art points to a non-existent file**

`useGameCoverArt` returns `customUrl` when the path is non-empty, but the `<img>` element will fire `onError`. The `failed` state hides the broken image and falls back to the auto-downloaded art URL. This fallback is already implemented in `LibraryCard` and `GameCoverArt`.

**EC-4: `profile_list_summaries` currently only returns `steam.app_id`**

The Library page resolution currently passes `profile.steam.app_id` to `useGameCoverArt`. After this feature, `ProfileSummary` must return a single `effective_steam_app_id` field (computed backend-side per BR-9: `steam.app_id` if non-empty, else `runtime.steam_app_id`). The frontend receives one resolved value and does not implement the fallback logic itself.

**EC-5: Portrait uses candidate URL chain**

Steam CDN portrait download already uses a three-URL fallback chain (`library_600x900_2x.jpg` → `library_600x900.jpg` → `header.jpg`) because not all games publish the high-res variant. This applies equally when the portrait is fetched using `runtime.steam_app_id`.

**EC-6: Background art (hero) is future use**

The `GameImageType::Hero` variant and the `library_hero.jpg` CDN URL already exist in the backend. Background art is available on Steam CDN without a SteamGridDB API key (`library_hero.jpg`). SteamGridDB is not required for background art — it improves quality and variety but does not gate functionality for any art type. Background art support can be wired up at any time with no new infrastructure. For this feature scope, no background art UI need be implemented, but the data model should reserve the slot.

**EC-7: SteamGridDB key not configured**

When `settings.steamgriddb_api_key` is empty or absent, `download_and_cache_image` skips SteamGridDB entirely and fetches from Steam CDN directly. Art resolution still works for all three types. The user experience degrades gracefully — no error state, no broken UI. The only difference is image quality/variety. This is the expected default state for most users.

**EC-8: SteamGridDB API key must not be returned to the frontend**

The `SettingsStore` currently returns the full settings struct (including `steamgriddb_api_key`) via IPC. The frontend should only know whether a key is configured (`has_steamgriddb_api_key: bool`), not the key value itself. This is a pre-existing security gap (tracked in the security research doc as S-02) that is not introduced by this feature but should be fixed in the same release. The art download pipeline already reads the key server-side; the frontend never needs the raw value.

**EC-10: `ProfileSummary` DTO must carry resolved art paths, not raw TOML fields**

When `ProfileSummary` exposes custom art paths to the frontend, it must return the **resolved effective path** (the result of `effective_profile()`, which is already guaranteed to be inside the managed media directory after import) — not the raw `local_override.game.*` string from TOML. This closes the S-12 pattern by architecture: since `import_custom_cover_art` validates and copies any path into the managed media dir before it can be saved, the effective path is always either empty or a managed-media path. No separate sanitization of null bytes or `../` sequences is needed in the DTO layer, provided the import step is the only path into the custom art fields (enforced by `profile_save` in `commands/profile.rs:277-287`).

**EC-9: Art slot source state machine**

Each art slot (cover, portrait, background) independently occupies one of three states:

- **Not set** — no custom path, no app_id → placeholder/initials shown.
- **Auto** — app_id is set, download succeeded (or stale cache exists) → auto-downloaded art shown.
- **Custom** — custom art path is set (non-empty) → custom art shown regardless of auto-download state.

Transitions: Not Set → Auto (app_id saved and download completes on next Library load). Not Set or Auto → Custom (user uploads art). Custom → Auto (user clears custom path field and saves). Custom → Not Set (user clears custom path AND no app_id set). The backend does not need to model this state explicitly — it is derived at read time from the two inputs (custom path, effective app_id).

---

## Workflows

### WF-1: Setting `steam_app_id` on a proton_run Profile

1. User opens a `proton_run` profile in the editor.
2. "Proton Runtime" section displays a "Steam App ID" text field (new).
3. User types or pastes a numeric Steam App ID (e.g. `1245620`).
4. Optional: user clicks a ProtonDB / Steam lookup link next to the field to find their App ID.
5. User saves the profile (`profile_save` IPC command).
6. Backend validates format (digits only). If invalid, return an error; field remains focused.
7. Profile TOML is written with `[runtime] steam_app_id = "1245620"`.
8. On next Library page load, `profile_list_summaries` returns the new `steam_app_id`.
9. `LibraryCard` calls `useGameCoverArt` with the app id; a download is triggered.
10. Portrait art appears in the Library card.

### WF-2: Uploading Custom Art (Per Art Type)

1. User opens a profile in the editor, navigates to the "Media" tab/section.
2. User clicks "Browse" for a specific art slot (Cover, Portrait, or Background).
3. File picker opens filtered to `png`, `jpg`, `jpeg`, `webp`.
4. On selection, frontend calls `import_custom_cover_art` (or per-type variant) IPC command.
5. Backend reads file bytes, validates magic bytes and size, computes SHA-256, copies to managed media dir.
6. IPC returns the managed absolute path.
7. Frontend sets `game.custom_cover_art_path` (or the appropriate per-type field) to the returned path.
8. User saves profile. Profile TOML stores the managed path in `local_override.game.*`.
9. Library card immediately shows the custom art (converted via `convertFileSrc`).

### WF-3: Art Auto-Download on Library Page Load

1. Library page mounts; `useLibrarySummaries` calls `profile_list_summaries`.
2. Each `ProfileSummary` returns `steam_app_id` (effective: `steam.app_id` falling back to `runtime.steam_app_id`), `customCoverArtPath`, and `customPortraitArtPath` (new).
3. `LibraryCard` becomes visible via `IntersectionObserver`.
4. `useGameCoverArt(steamAppId, customPortraitArtPath, 'portrait')` hook runs:
   a. If `customPortraitArtPath` is non-empty: display custom art immediately, skip network.
   b. Else if `steamAppId` is non-empty: call `fetch_game_cover_art(appId, 'portrait')`.
5. Backend checks SQLite cache; if valid entry exists and file is on disk, returns path.
6. If no valid cache: attempts SteamGridDB (if API key configured), then Steam CDN.
7. On success: stores in DB cache, returns absolute path. Frontend converts to `tauri://asset` URL via `convertFileSrc`.
8. On failure: serves stale path if available, else `null`. Card shows initials fallback.

### WF-4: Removing Custom Art (Revert to Auto-Download)

1. User opens the profile editor Media section.
2. User clears the custom art path field (deletes text or clicks an "X" button).
3. User saves the profile. Profile TOML no longer contains the custom art path for that slot.
4. `local_override.game.*` for that art type is now empty.
5. Library card reverts to displaying auto-downloaded art (from `steam_app_id` cache) or initials fallback.
6. The imported file on disk is **not** deleted (see BR-14). The path reference is removed from the profile only. Other profiles sharing the same content-addressed file continue to work.

### WF-5: Error Recovery — Invalid App ID

1. User types a non-numeric value into the Steam App ID field.
2. User saves: `profile_save` IPC is called.
3. At the `download_and_cache_image` call site (which validates before any I/O), the invalid ID would fail. However, validation should be applied earlier — at save time — to surface the error in the UI.
4. Backend returns an error string.
5. Frontend displays the error; profile is not saved with invalid data.

---

## Domain Model

### Entities

**Profile** (`GameProfile` in Rust, `GameProfile` interface in TypeScript)

- Root aggregate for all game configuration.
- Has-one `GameSection`, `SteamSection`, `RuntimeSection`, `LaunchSection`, `TrainerSection`, `InjectionSection`, `LocalOverrideSection`.
- Identity: profile name (filename without `.toml`).
- Storage: TOML file in `ProfileStore` base path.
- Portable vs. machine-specific split enforced by `storage_profile()` / `effective_profile()`.

**ArtType** (enum)

- `Cover` — landscape header image (460×215 or 920×430).
- `Portrait` — vertical card image (600×900).
- `Background` — hero/banner image (library_hero.jpg); future use.
- Maps to `GameImageType` enum in `crates/crosshook-core/src/game_images/models.rs`.

**ArtSource** (enum)

- `Custom` — user-uploaded file in managed media directory; always wins.
- `AutoSteamCdn` — downloaded from Steam CDN for a given app_id.
- `AutoSteamGridDb` — downloaded from SteamGridDB for a given app_id.
- `Stale` — expired cache entry on disk, served as fallback.
- `None` — no art available; UI renders initials/placeholder.
- Maps to `GameImageSource` in `game_images/models.rs`.

**MediaAsset** (SQLite row in `game_image_cache`)

- Keyed by `(steam_app_id, image_type, source)`.
- Stores: file path on disk, file size, SHA-256 content hash, MIME type, source URL, expiration timestamp.
- Cache TTL: 24 hours. Stale entries are served as fallback, then evicted on miss.

**ProfileSummary** (IPC DTO)

- Lightweight view of a profile used by the Library page.
- Currently: `name`, `game_name`, `steam_app_id`, `custom_cover_art_path`.
- After this feature: must also expose `custom_portrait_art_path` (new) and an effective `steam_app_id` that resolves `steam.app_id` OR `runtime.steam_app_id`.

### State Transitions for Art Lifecycle

```
Not Set
  │ user sets steam_app_id (proton_run)
  ▼
App ID Present
  │ Library page loads → useGameCoverArt fires → fetch_game_cover_art
  ▼
Download Attempted
  ├─ Success → MediaAsset created (Valid, 24h TTL)
  │               ▼
  │           Cache Valid [served immediately on future loads]
  │               │ TTL expires
  │               ▼
  │           Cache Stale [re-download attempted; stale served on failure]
  │               │ file deleted from disk
  │               ▼
  │           Cache Evicted [no file path → art = None]
  │
  └─ Failure → Stale fallback returned if exists, else None
                   ▼ user uploads custom art
Custom Art Set → Custom Always Wins
                   │ custom art removed
                   ▼
              Reverts to Auto-download state
```

### Relationships

- One `GameProfile` → zero-or-one `runtime.steam_app_id` (new field on `RuntimeSection`).
- One `GameProfile` → zero-or-one `steam.app_id` (existing field on `SteamSection`).
- One `GameProfile` → zero-or-three custom art path fields (one per `ArtType`), stored in `LocalOverrideSection.game.*` for portability.
- One `(steam_app_id, ArtType)` pair → zero-or-many `MediaAsset` rows (one per source CDN/SteamGridDB); unique index on `(steam_app_id, image_type, source)`.

---

## Existing Codebase Integration

### Profile Model

- `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
  - `RuntimeSection` is the target for the new `steam_app_id` field.
  - Must update `RuntimeSection::is_empty()` to ignore `steam_app_id` (it is not a machine path — an empty check for serialization is sufficient, already handled by `skip_serializing_if = "String::is_empty"`).
  - `GameSection` currently has only `custom_cover_art_path`. Adding `custom_portrait_art_path` and `custom_background_art_path` (optional, skip_serializing_if empty) follows the same pattern.
  - `LocalOverrideGameSection` must gain matching fields.
  - `GameProfile::effective_profile()` and `storage_profile()` must be extended to merge/split the new custom art path fields.

### Art Download Infrastructure

- `/src/crosshook-native/crates/crosshook-core/src/game_images/client.rs`
  - `download_and_cache_image(store, app_id, image_type, api_key)` is the central download entry point.
  - Already supports `Cover`, `Hero`, `Capsule`, `Portrait` types.
  - No changes needed here for the proton app_id feature; caller just passes `runtime.steam_app_id`.

- `/src/crosshook-native/crates/crosshook-core/src/game_images/import.rs`
  - `import_custom_cover_art(source_path)` copies to `media/covers/`.
  - Will need parallel functions per art type (`import_custom_portrait_art`, etc.) or a single parameterized function.

- `/src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs`
  - `fetch_steamgriddb_image(api_key, app_id, image_type)` already handles all four types.

### IPC Layer

- `/src/crosshook-native/src-tauri/src/commands/game_metadata.rs`
  - `fetch_game_cover_art(app_id, image_type, ...)` — caller resolves `app_id` before invocation; no change needed here if callers are updated.

- `/src/crosshook-native/src-tauri/src/commands/profile.rs`
  - `profile_list_summaries`: must return effective `steam_app_id` (falling back to `runtime.steam_app_id`) and new custom art path fields.
  - `profile_save`: must auto-import new custom art path fields when outside managed media dir.
  - `ProfileSummary` struct must be extended.

### Frontend Hooks

- `/src/crosshook-native/src/hooks/useGameCoverArt.ts`
  - Currently accepts `steamAppId` (from `steam.app_id` via `ProfileSummary`).
  - Effective App ID resolution must happen before this hook is called — either in `ProfileSummary` on the backend or in `useLibrarySummaries` on the frontend. Backend resolution is preferred (single source of truth).

### Library Page

- `/src/crosshook-native/src/components/library/LibraryCard.tsx`
  - Passes `profile.steamAppId` and `profile.customCoverArtPath` to `useGameCoverArt` with `imageType = 'portrait'`.
  - After this feature: will also need to pass `profile.customPortraitArtPath` (new field).
  - No structural changes otherwise — the hook handles the resolution.

### Profile Editor UI

- `/src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx`
  - `proton_run` branch renders a "Steam App ID" field currently bound to `profile.steam.app_id` (placeholder: "Optional for ProtonDB lookup"). The migration plan: rebind this field to `profile.runtime.steam_app_id`. The effective resolution (BR-9) means ProtonDB lookup continues to work for existing profiles via `steam.app_id` while new profiles use `runtime.steam_app_id`. Both the ProtonDB query and art download must use the BR-9 effective app_id helper — neither should bind directly to only one field.

- `/src/crosshook-native/src/components/profile-sections/MediaSection.tsx`
  - Currently: single "Custom Cover Art" field for all launch methods.
  - After this feature: three fields — Cover, Portrait, Background (Background may be hidden until background art UI is ready).

---

## Persistence Classification

| Datum                                   | Storage type                                                         | Rationale                                                         |
| --------------------------------------- | -------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `runtime.steam_app_id`                  | TOML profile settings (base section, portable)                       | Identifies the game, not machine-specific; survives export/import |
| `game.custom_cover_art_path`            | TOML (local_override.game, machine-local)                            | Absolute filesystem path; not portable across machines            |
| `game.custom_portrait_art_path` (new)   | TOML (local_override.game, machine-local)                            | Same as cover — absolute path                                     |
| `game.custom_background_art_path` (new) | TOML (local_override.game, machine-local)                            | Same as cover                                                     |
| Downloaded art files                    | Filesystem cache (`~/.local/share/crosshook/cache/images/{app_id}/`) | Re-downloadable, TTL-managed                                      |
| Art cache metadata                      | SQLite metadata DB (`game_image_cache` table)                        | Operational cache — re-fetchable; schema at v14                   |

---

## Success Criteria

1. A `proton_run` profile with `runtime.steam_app_id` populated shows portrait art in the Library grid automatically on next page load.
2. A `proton_run` profile without `runtime.steam_app_id` continues to show the initials fallback (no regression).
3. A `steam_applaunch` profile continues to use `steam.app_id` for art; `runtime.steam_app_id` is ignored for that launch method in art resolution.
4. Custom art for any slot overrides auto-downloaded art for that slot; other slots are unaffected.
5. Removing a custom art path reverts that slot to auto-downloaded art.
6. Art download validation (numeric app_id, 5 MB limit, magic bytes) applies equally when `runtime.steam_app_id` is the source.
7. `runtime.steam_app_id` survives portable profile export/import unchanged.
8. Custom art paths are excluded from portable profile export (they are machine-local paths).
9. No launch behavior changes for any existing profile, regardless of `runtime.steam_app_id` presence.
10. Existing `steam_applaunch` profiles require no migration (additive change).
11. A user with no SteamGridDB API key configured still receives Steam CDN art for any profile with a valid `steam_app_id` or `runtime.steam_app_id` — the feature degrades gracefully to CDN-only, not to no-art.
12. `portable_profile()` / community export contains no `custom_*_art_path` values (all three slots cleared).
13. `ProfileSummary` DTO exposes resolved effective art paths only — no raw `local_override.game.*` strings.
14. SteamGridDB API key is not present in any IPC response to the frontend (only `has_steamgriddb_api_key: bool`).

---

## Open Questions

1. ~~**Effective App ID in `ProfileSummary`**~~ — **Resolved**: Backend computes a single `effective_steam_app_id` per BR-9. Frontend receives one value. (See BR-9, EC-4.)

2. ~~**`proton_run` App ID field binding**~~ — **Resolved**: The existing field in `RuntimeSection.tsx` is rebound to `profile.runtime.steam_app_id`. Both ProtonDB lookup and art download use the BR-9 effective app_id helper, so existing profiles with `steam.app_id` set continue working. No migration needed. (See EC-1, EC-4.)

3. **Background art UI timing**: The data model can accommodate `custom_background_art_path` now, but no display surface exists in the current UI. Recommendation: add the data model fields (they serialize as empty/omitted by default) but do not add the Media section UI slot until a consumer (e.g. a hero banner in the profile editor) exists. Defer to a follow-up issue.

4. **Portrait art auto-download on profile save**: Currently art downloads lazily on Library page load. Should saving a profile with a new `runtime.steam_app_id` trigger a background pre-fetch? Lazy fetch is acceptable for the initial implementation; a pre-fetch optimization can follow.

5. **Auto-suggest for Steam App ID**: The issue mentions auto-suggest/search via `auto_populate_steam` or Steam metadata lookup. Recommendation: out of scope for this iteration. A plain text field with a helper link to the Steam store search is sufficient. Auto-suggest can be a follow-up enhancement.
