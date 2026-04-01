# UI Enhancements Implementation Plan

This plan restructures the Profiles page from a single collapsed "Advanced" wrapper into distinct `CollapsibleSection` cards, then layers on Steam Store API game metadata and cover art caching mirroring the ProtonDB lookup pattern end-to-end: `OnceLock` HTTP client + cache-first via `external_cache_entries` in Rust, thin IPC commands in `src-tauri`, and `useProtonDbLookup`-style hooks driving new card components on the frontend. The backend Rust track (new `steam_metadata/` and `game_images/` modules, `game_image_cache` SQLite table at schema v14) runs in parallel with the frontend restructuring track since they share zero files — the two tracks converge at Phase 2's IPC wiring. Phase 3 extracts 6 section components from the 41k `ProfileFormSections` monolith and adds `@radix-ui/react-tabs` sub-tab navigation using already-defined `.crosshook-subtab-*` CSS, with `display: none` (not conditional rendering) to preserve `CustomEnvironmentVariablesSection` local state.

## Critically Relevant Files and Documentation

- docs/plans/ui-enhancements/feature-spec.md: Authoritative implementation contract — data models, API signatures, phasing, risk assessment, persistence classification
- docs/plans/ui-enhancements/research-security.md: REQUIRED for Phase 2 — code-ready `validate_image_bytes()` and `safe_image_cache_path()` Rust snippets
- docs/plans/ui-enhancements/research-technical.md: Component tree, ProfileContext state flow, CSS pattern inventory
- docs/plans/ui-enhancements/research-practices.md: Reusable component inventory — prevents duplication
- docs/plans/protondb-lookup/research-technical.md: ProtonDB module layout — the exact pattern to mirror for steam_metadata
- AGENTS.md: Hard architectural constraints — crosshook-core owns logic, src-tauri is IPC-thin, 512 KiB cache cap
- src/crosshook-native/crates/crosshook-core/src/protondb/client.rs: Cache-first API client — the template for steam_metadata/client.rs
- src/crosshook-native/crates/crosshook-core/src/protondb/models.rs: Serde IPC types — template for Steam metadata types
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: MetadataStore public API — with_conn, put_cache_entry, get_cache_entry
- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs: Store submodule pattern — template for game_image_store.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Sequential if-version-lt-N migration pattern; currently v13
- src/crosshook-native/src-tauri/src/commands/protondb.rs: 13-line IPC command — template for new game_metadata commands
- src/crosshook-native/src/hooks/useProtonDbLookup.ts: Canonical frontend hook — requestIdRef race guard, stale-while-revalidating, refresh()
- src/crosshook-native/src/components/ProtonDbLookupCard.tsx: Card component template for GameMetadataCard
- src/crosshook-native/src/components/pages/ProfilesPage.tsx: Primary restructuring target — single collapsed Advanced wrapper
- src/crosshook-native/src/components/ProfileFormSections.tsx: 41k monolith shared with InstallPage via reviewMode prop
- src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx: Local rows state — must use CSS display:none during tab switches
- src/crosshook-native/src/components/ui/CollapsibleSection.tsx: Card primitive with meta slot
- src/crosshook-native/src/styles/theme.css: Contains unused crosshook-subtab-\* classes; new cover art/skeleton CSS goes here
- src/crosshook-native/src/styles/variables.css: Sub-tab CSS variables already defined; cover art variables go here
- src/crosshook-native/src-tauri/tauri.conf.json: CSP must add img-src asset: for cover art rendering
- src/crosshook-native/src-tauri/capabilities/default.json: Must add asset protocol + fs:allow-read-file scope

## Implementation Plan

### Phase 0: Foundation

#### Task 0.1: Extract formatProtonInstallLabel to Break Circular Import Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileFormSections.tsx
- src/crosshook-native/src/components/ui/ProtonPathField.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/utils/proton.ts

Files to Modify

- src/crosshook-native/src/components/ProfileFormSections.tsx
- src/crosshook-native/src/components/ui/ProtonPathField.tsx

Extract `formatProtonInstallLabel` function from `ProfileFormSections.tsx` into a new `src/utils/proton.ts` utility file. Update the import in `ProtonPathField.tsx` to point to `utils/proton.ts` instead of `ProfileFormSections`. Remove the function definition from `ProfileFormSections.tsx` and add an import from `utils/proton.ts` so existing call sites within `ProfileFormSections` still work. This is a mechanical move with no logic change — it fixes the `ProtonPathField → ProfileFormSections` circular dependency that would cause issues during Phase 3 section extraction. **Important**: Only move `formatProtonInstallLabel` — the `ProtonInstallOption` type must stay in `ProfileFormSections.tsx` because `OnboardingWizard.tsx` imports it from there and it is a profile form concern, not a utility. Verify no other files import `formatProtonInstallLabel` from `ProfileFormSections`, and confirm `ProtonInstallOption` exports remain intact.

#### Task 0.2: Add infer Crate and SQLite v14 Migration for game_image_cache Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/Cargo.toml
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs
- docs/plans/ui-enhancements/feature-spec.md (game_image_cache DDL section)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/Cargo.toml
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs

Add `infer = "~0.16"` to `[dependencies]` in `crosshook-core/Cargo.toml`. In `migrations.rs`, add a new `migrate_13_to_14(conn)` function that creates the `game_image_cache` table and its indexes. The exact DDL is in `feature-spec.md`. Add the `if version < 14` guard block in `run_migrations()` after the existing `if version < 13` block — use `if` (not `else if`) following the sequential migration pattern. The table stores filesystem paths to cached cover art images, keyed by `(steam_app_id, image_type, source)` with a unique index. Add migration tests following the existing pattern: call `MetadataStore::open_in_memory()`, verify `game_image_cache` table exists via `sqlite_master` query, and confirm the unique index `idx_game_image_cache_app_type_source` is present.

#### Task 0.3: Implement GameImageStore SQLite CRUD Depends on [0.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/game_image_store.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs

Create `game_image_store.rs` following the `health_store.rs` submodule pattern. Functions take `&Connection` directly (not `&MetadataStore`). Implement: `upsert_game_image(conn, steam_app_id, image_type, source, file_path, file_size, content_hash, mime_type, source_url, expires_at)` using `INSERT ... ON CONFLICT(steam_app_id, image_type, source) DO UPDATE`; `get_game_image(conn, steam_app_id, image_type)` returning `Option<GameImageCacheRow>`; `evict_expired_images(conn)` deleting rows where `expires_at < datetime('now')`. Define `GameImageCacheRow` struct in this file with all table columns. In `metadata/mod.rs`, add `mod game_image_store;` declaration and public delegation methods on `MetadataStore` using `self.with_conn()` for graceful degradation. Write unit tests using `MetadataStore::open_in_memory()` — test upsert, get-by-key, eviction, and graceful return of `None` on missing entries.

#### Task 0.4: Add steamgriddb_api_key to AppSettingsData Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs
- src/crosshook-native/src/types/settings.ts

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs
- src/crosshook-native/src/types/settings.ts

Add `pub steamgriddb_api_key: Option<String>` to `AppSettingsData` in `settings/mod.rs`. The struct already has `#[serde(default)]` at the struct level so no migration is needed — existing `settings.toml` files without this field will deserialize to `None`. On the frontend, add `steamgriddb_api_key?: string | null` to the `AppSettingsData` interface in `types/settings.ts`. This field must exist on both sides because settings are round-tripped through IPC — a missing field on either side silently drops it.

#### Task 0.5: Add id Prop to InstallField Component Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ui/InstallField.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/ui/InstallField.tsx

Add an optional `id?: string` prop to `InstallFieldProps` and pass it through to the root element or `<input>`. This is an additive change — no existing callers need updating. The `id` prop enables `<label htmlFor>` accessibility associations in the restructured Profiles page sections.

### Phase 1: ProfilesPage Card Layout

#### Task 1.1: Restructure ProfilesPage — Remove Advanced Wrapper, Promote to Section Cards Depends on [0.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/ProfilesPage.tsx
- src/crosshook-native/src/components/ProfileFormSections.tsx
- src/crosshook-native/src/components/ui/CollapsibleSection.tsx
- src/crosshook-native/src/components/ProfileActions.tsx
- docs/plans/ui-enhancements/research-ux.md

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

In `ProfilesPage.tsx` at ~line 622, remove the single `<CollapsibleSection title="Advanced" defaultOpen={false}>` wrapper that currently hides all editing fields. Replace with individual `<CollapsibleSection>` cards wrapping each logical group, each with `className="crosshook-panel"` and `defaultOpen={true}`:

1. **Core** card — profile name, game path, Steam App ID, runner method selector. Add conditional cover art slot (empty div with `className="crosshook-profile-cover-art"`, hidden when no `steam_app_id`) for Phase 2 wiring.
2. **Runtime** card — Working Directory + runner-method-conditional fields (Steam fields for `steam_applaunch`, Proton fields for `proton_run`, only Working Directory for `native`). Use `meta` prop to show active runner method badge.
3. **Environment & ProtonDB** card — `CustomEnvironmentVariablesSection` + `ProtonDbLookupCard` co-located (business rule: ProtonDB "Apply" writes to env vars).
4. **Trainer** card — conditionally rendered (hidden for `native` launch method). Use existing collapse logic.

Keep `ProfileActions` (Save/Delete/Duplicate/Rename) outside all cards — it must always be visible. Keep `ProfileFormSections` as the composition point wrapping the field JSX; only the outer `CollapsibleSection("Advanced")` is removed. Do NOT extract sections out of `ProfileFormSections` yet — that is Phase 3. Do NOT change the `reviewMode` contract with `InstallPage`.

#### Task 1.2: Add Cover Art CSS Stub Classes and Variables Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/styles/variables.css
- src/crosshook-native/src/styles/theme.css
- docs/plans/ui-enhancements/research-ux.md

**Instructions**

Files to Modify

- src/crosshook-native/src/styles/variables.css
- src/crosshook-native/src/styles/theme.css

In `variables.css`, add CSS custom properties: `--crosshook-profile-cover-art-aspect: 460 / 215`, `--crosshook-skeleton-duration: 1.8s`, `--crosshook-skeleton-color-from` and `--crosshook-skeleton-color-to` (using existing surface/accent color variable references). In `theme.css`, add:

- `.crosshook-profile-cover-art` — `aspect-ratio: var(--crosshook-profile-cover-art-aspect)`, `border-radius`, `overflow: hidden`, `object-fit: cover`
- `.crosshook-profile-cover-art--hidden` — `display: none` (when no art available)
- `@keyframes crosshook-skeleton-shimmer` — standard shimmer animation (left-to-right gradient sweep)
- `.crosshook-skeleton` — background animation using the keyframe + `background-size: 200%`
- `.crosshook-game-metadata-bar` — flex row for genre chips + metadata summary
- Controller mode override: larger cover art dimensions under `[data-crosshook-controller-mode='true']`

These are stubs — no components reference them yet. They establish the visual foundation so Phase 2 components slot in without layout rework.

#### Task 1.3: Verify InstallPage and OnboardingWizard Compatibility Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/InstallPage.tsx
- src/crosshook-native/src/components/ProfileFormSections.tsx

**Instructions**

Files to Modify

(none — verification task)

After Task 1.1, run `./scripts/dev-native.sh` and manually verify:

1. `InstallPage` renders `ProfileFormSections` with `reviewMode={true}` correctly — all fields display, optional empty fields are collapsed, "Apply" for ProtonDB env vars is disabled.
2. `OnboardingWizard` profile creation flow still works — specifically verify that `ProtonInstallOption` (imported from `ProfileFormSections`) renders correctly in the wizard's proton install selector.
3. Keyboard navigation (F2 rename, Tab focus cycling) functions on the restructured ProfilesPage.
4. Controller mode (set `data-crosshook-controller-mode='true'` on root) shows appropriate touch target sizing on section cards.

If any issue is found, fix it before proceeding. The `ProfileFormSections` `reviewMode` contract and all its type exports (including `ProtonInstallOption`) must remain intact.

### Phase 2: Steam Metadata + Cover Art Backend

#### Task 2.1: Implement steam_metadata Rust Module Depends on [0.2, 0.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/protondb/client.rs
- src/crosshook-native/crates/crosshook-core/src/protondb/models.rs
- src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs
- docs/plans/ui-enhancements/feature-spec.md (API section)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/steam_metadata/mod.rs
- src/crosshook-native/crates/crosshook-core/src/steam_metadata/client.rs
- src/crosshook-native/crates/crosshook-core/src/steam_metadata/models.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/lib.rs

Mirror the `protondb/` module structure exactly. In `models.rs`: define `SteamMetadataLookupState` enum (`Idle/Loading/Ready/Stale/Unavailable` with `#[serde(rename_all = "snake_case")]`), `SteamMetadataLookupResult` (app_id, state, app_details, from_cache, is_stale), `SteamAppDetails` (name, short_description, header_image, genres as `Vec<SteamGenre>`), and `SteamGenre` (id, description). All types derive `Serialize + Deserialize + Default + Clone`.

In `client.rs`: implement `OnceLock<reqwest::Client>` singleton (`steam_metadata_http_client()`), `normalize_app_id()` validation, and `pub async fn lookup_steam_metadata(store: &MetadataStore, app_id: &str, force_refresh: bool) -> SteamMetadataLookupResult`. Cache key: `steam:appdetails:v1:{app_id}`. TTL: 24 hours. Follow the exact cache-first pattern from `protondb/client.rs:85-130`: valid cache → live fetch → persist → stale fallback → unavailable. Endpoint: `GET https://store.steampowered.com/api/appdetails?appids={app_id}`. Parse the outer response envelope `{ "{app_id}": { "success": bool, "data": {...} } }`. Define private `SteamMetadataError` enum mirroring `ProtonDbError`.

In `mod.rs`: `pub mod client; pub mod models;` and re-export the public lookup function and result types.

Add `pub mod steam_metadata;` to `lib.rs`. Write unit tests in a `tests.rs` file: seed `MetadataStore::open_in_memory()` with valid/expired cache entries, assert on `state`, `from_cache`, `is_stale`, and `app_details` fields. Run: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.

#### Task 2.2: Implement game_images Rust Module with Security Mitigations Depends on [0.2, 0.3]

**READ THESE BEFORE TASK**

- docs/plans/ui-enhancements/research-security.md (REQUIRED — I1 SVG rejection, I2 path traversal)
- src/crosshook-native/crates/crosshook-core/src/protondb/client.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/game_image_store.rs (from Task 0.3)
- docs/plans/ui-enhancements/feature-spec.md (image caching section)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/game_images/mod.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/client.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/models.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/lib.rs

Create `game_images/` module. In `models.rs`: define `GameImageType` enum (`Cover/Hero/Capsule`), `GameImageSource` enum (`SteamCdn/SteamGridDb`), and result types with Serde annotations.

In `client.rs`: implement `OnceLock<reqwest::Client>` singleton, `pub async fn download_and_cache_image(store: &MetadataStore, app_id: &str, image_type: GameImageType) -> Result<Option<String>, String>`. The function must:

1. **Validate `app_id`**: `app_id.chars().all(|c| c.is_ascii_digit())` — reject non-numeric input immediately (path traversal mitigation I2).
2. **Check cached entry**: `store.get_game_image(app_id, image_type_str)` — if valid (non-expired), return the file path.
3. **Construct download URL**: `https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/header.jpg` for Cover type.
4. **Download**: HTTP GET with 5 MB response size limit.
5. **Validate image bytes**: Use `infer::get(&bytes)` to check magic bytes. Allowlist ONLY `image/jpeg`, `image/png`, `image/webp`. Reject all others including SVG (has no magic bytes). Use the `validate_image_bytes()` snippet from `research-security.md`.
6. **Construct safe path**: Use `safe_image_cache_path()` from `research-security.md`. Base dir: `~/.local/share/crosshook/cache/images/`. Full path: `{base}/{app_id}/cover_steam_cdn.jpg`. Canonicalize and assert prefix after construction.
7. **Write to disk**: `tokio::fs::create_dir_all` for parent, then `tokio::fs::write`.
8. **Persist metadata**: `store.upsert_game_image(...)` with SHA-256 content hash, MIME type, absolute path, 24h TTL.
9. **Return**: `Ok(Some(absolute_path))` on success; `Ok(None)` on network failure or validation rejection.

Stale fallback: if download fails but a non-expired cache entry exists with a valid file on disk, return the cached path. If the cached file is missing from disk, delete the DB row and return `None`.

Add `pub mod game_images;` to `lib.rs`. Write tests covering: valid image download (mock with known JPEG magic bytes), SVG rejection, numeric-only app_id validation, stale fallback when file exists, cleanup when cached file is missing.

#### Task 2.3: Register Tauri IPC Commands for Game Metadata Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/protondb.rs
- src/crosshook-native/src-tauri/src/commands/mod.rs
- src/crosshook-native/src-tauri/src/lib.rs

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/game_metadata.rs

Files to Modify

- src/crosshook-native/src-tauri/src/commands/mod.rs
- src/crosshook-native/src-tauri/src/lib.rs

Create `game_metadata.rs` following the `protondb.rs` 13-line template. Implement two commands:

```rust
#[tauri::command]
pub async fn fetch_game_metadata(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<SteamMetadataLookupResult, String> {
    let store = metadata_store.inner().clone();
    Ok(lookup_steam_metadata(&store, &app_id, force_refresh.unwrap_or(false)).await)
}

#[tauri::command]
pub async fn fetch_game_cover_art(
    app_id: String,
    image_type: Option<String>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Option<String>, String> {
    let store = metadata_store.inner().clone();
    let image_type = match image_type.as_deref().unwrap_or("cover") {
        "hero" => GameImageType::Hero,
        "capsule" => GameImageType::Capsule,
        _ => GameImageType::Cover,
    };
    download_and_cache_image(&store, &app_id, image_type)
        .await
        .map_err(|e| e.to_string())
}
```

Add `pub mod game_metadata;` to `commands/mod.rs`. Register both commands in `lib.rs`'s `invoke_handler!(tauri::generate_handler![...])` list. Import the command functions appropriately. Run `cargo check` to verify compilation.

#### Task 2.4: Configure Tauri Asset Protocol for Cover Art Rendering Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/tauri.conf.json
- src/crosshook-native/src-tauri/capabilities/default.json
- docs/plans/ui-enhancements/research-security.md (sections C1, C2)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/tauri.conf.json
- src/crosshook-native/src-tauri/capabilities/default.json

In `tauri.conf.json`, extend the CSP to add `img-src 'self' asset: http://asset.localhost`. The existing CSP is `default-src 'self'; script-src 'self'` — append the `img-src` directive.

In `capabilities/default.json`, add permissions for:

1. Filesystem read scope: `{ "identifier": "fs:allow-read-file", "allow": [{ "path": "$LOCALDATA/crosshook/cache/images/**" }] }` — scoped to the image cache directory only.
2. Asset protocol scope: **consult `research-security.md` sections C1 and C2 for the exact Tauri v2 permission identifiers**. The correct permission depends on the Tauri v2 version in use — do NOT guess the identifier. Read the current `capabilities/default.json` to see the existing permission format, then check `research-security.md` for the tested asset protocol configuration. If `research-security.md` does not have the exact identifier, check the Tauri v2 documentation at `https://v2.tauri.app/security/csp/` and the `tauri-plugin-fs` permissions list.

Keep the scope narrow — `$LOCALDATA/crosshook/cache/images/**` only. A broader scope grants webview read access to arbitrary user files. Verify with `./scripts/dev-native.sh` that the app still launches without CSP errors in the console and that `convertFileSrc('/tmp/test.jpg')` produces a valid `asset://` URL in the browser console.

### Phase 3: Steam Metadata + Cover Art Frontend

#### Task 3.1: Create TypeScript Types and Frontend Hooks for Game Metadata Depends on [2.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useProtonDbLookup.ts
- src/crosshook-native/src/types/protondb.ts
- src/crosshook-native/crates/crosshook-core/src/steam_metadata/models.rs (from Task 2.1)

**Instructions**

Files to Create

- src/crosshook-native/src/types/game-metadata.ts
- src/crosshook-native/src/hooks/useGameMetadata.ts
- src/crosshook-native/src/hooks/useGameCoverArt.ts

In `types/game-metadata.ts`: define TypeScript interfaces mirroring the Rust Serde output — `SteamMetadataLookupState` (union type `'idle' | 'loading' | 'ready' | 'stale' | 'unavailable'`), `SteamMetadataLookupResult`, `SteamAppDetails`, `SteamGenre`. Field names are `snake_case` matching Serde `rename_all`.

In `useGameMetadata.ts`: clone `useProtonDbLookup.ts` exactly, adapting types and command name. Implement `requestIdRef` race guard, `idle/loading/ready/stale/unavailable` state machine, preserve previous snapshot during loading (stale-while-revalidating), expose `refresh()` callback. Call `invoke<SteamMetadataLookupResult>('fetch_game_metadata', { appId, forceRefresh })`. Trigger on `steamAppId` change via `useEffect`.

In `useGameCoverArt.ts`: simpler hook — `useState<string | null>(null)` for the filesystem path, `useState<boolean>(false)` for loading. Call `invoke<string | null>('fetch_game_cover_art', { appId, imageType: 'cover' })`. On success, convert path to displayable URL via `convertFileSrc(path)` from `@tauri-apps/api/core`. Re-fetch when `steamAppId` changes. Include `requestIdRef` race guard.

#### Task 3.2: Build GameCoverArt and GameMetadataBar Components Depends on [3.1, 1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProtonDbLookupCard.tsx
- src/crosshook-native/src/styles/theme.css (crosshook-profile-cover-art, crosshook-skeleton classes)
- docs/plans/ui-enhancements/research-ux.md (cover art loading patterns)

**Instructions**

Files to Create

- src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx
- src/crosshook-native/src/components/profile-sections/GameMetadataBar.tsx

`GameCoverArt.tsx`: Takes `steamAppId: string | undefined`. Uses `useGameCoverArt(steamAppId)`. Renders:

- When loading: `<div className="crosshook-profile-cover-art crosshook-skeleton" />` (shimmer placeholder)
- When loaded: `<img src={coverArtUrl} className="crosshook-profile-cover-art" alt="Game cover art" />`
- When unavailable or no steamAppId: render nothing (return `null`) — no broken image placeholders per business rule.

`GameMetadataBar.tsx`: Takes `steamAppId: string | undefined`. Uses `useGameMetadata(steamAppId)`. Renders:

- Game name as heading text (if available from metadata)
- Genre chips using `className="crosshook-status-chip"` (the established badge pattern from CompatibilityViewer)
- Stale indicator badge when `state === 'stale'`
- Nothing when state is `idle` or `unavailable`

Both components must be enhancement-only — they never block profile rendering.

#### Task 3.3: Wire Cover Art and Metadata into ProfilesPage Depends on [3.2, 1.1, 2.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/ProfilesPage.tsx (after Task 1.1 restructuring)
- src/crosshook-native/src/context/ProfileContext.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

In the Core section card (created in Task 1.1), add `<GameCoverArt steamAppId={profile?.steam?.app_id} />` at the top of the card and `<GameMetadataBar steamAppId={profile?.steam?.app_id} />` below it. The cover art slot created in Task 1.1 (the conditional div with `crosshook-profile-cover-art` class) is replaced with the actual `GameCoverArt` component. The `steamAppId` comes from the active profile in `ProfileContext`. When `steam.app_id` is empty or undefined, both components render nothing. Import `GameCoverArt` and `GameMetadataBar` from `components/profile-sections/`. Verify the layout works with and without cover art present — the card should not have empty whitespace when art is unavailable.

### Phase 4: ProfileFormSections Extraction + Sub-Tab Navigation

#### Task 4.1: Extract ProfileIdentitySection Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileFormSections.tsx
- src/crosshook-native/src/components/pages/ProfilesPage.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/components/profile-sections/ProfileIdentitySection.tsx

Files to Modify

- src/crosshook-native/src/components/ProfileFormSections.tsx

Extract the profile identity fields (profile name input, game name input) from `ProfileFormSections` into a new `ProfileIdentitySection` component. Props: `profile: GameProfile`, `onUpdateProfile: (updater) => void`, `reviewMode?: boolean`, `dirty?: boolean`. The component wraps these fields in a `<CollapsibleSection title="Identity" className="crosshook-panel" defaultOpen>`. In `ProfileFormSections`, replace the inline identity fields with `<ProfileIdentitySection {...props} />`. Preserve all existing behavior including validation states and `reviewMode` collapse logic.

#### Task 4.2: Extract GameSection Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileFormSections.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/components/profile-sections/GameSection.tsx

Files to Modify

- src/crosshook-native/src/components/ProfileFormSections.tsx

Extract game path and Steam App ID fields into `GameSection`. Props: `profile: GameProfile`, `onUpdateProfile: (updater) => void`, `reviewMode?: boolean`. Include the game path browse button and Steam App ID validation. Working Directory belongs in `RuntimeSection` (Task 4.4), not here — it is runner-method-dependent and was placed in the Runtime card in Phase 1. Replace inline in `ProfileFormSections`.

#### Task 4.3: Extract RunnerMethodSection Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileFormSections.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/components/profile-sections/RunnerMethodSection.tsx

Files to Modify

- src/crosshook-native/src/components/ProfileFormSections.tsx

Extract the runner method selector (Steam AppLaunch / Proton Run / Native dropdown) into `RunnerMethodSection`. Props: `profile: GameProfile`, `onUpdateProfile: (updater) => void`, `reviewMode?: boolean`. This is the dropdown that gates which runtime fields are visible — it must remain its own section for clarity.

#### Task 4.4: Extract RuntimeSection Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileFormSections.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx

Files to Modify

- src/crosshook-native/src/components/ProfileFormSections.tsx

Extract the runner-method-conditional fields into `RuntimeSection`. This is the most complex extraction because it contains conditional rendering: Steam fields for `steam_applaunch`, Proton fields for `proton_run`, only Working Directory for `native`. Props: `profile: GameProfile`, `onUpdateProfile: (updater) => void`, `reviewMode?: boolean`, `launchMethod: string`, `steamClientInstallPath: string`, `targetHomePath: string`. Preserve all conditional logic and field visibility gating.

#### Task 4.5: Extract TrainerSection Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileFormSections.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/components/profile-sections/TrainerSection.tsx

Files to Modify

- src/crosshook-native/src/components/ProfileFormSections.tsx

Extract trainer configuration fields into `TrainerSection`. Props: `profile: GameProfile`, `onUpdateProfile: (updater) => void`, `reviewMode?: boolean`, `launchMethod: string`. The section is conditionally hidden for `native` launch method. Preserve `trainerCollapsed = reviewMode && profile.trainer.path.trim().length === 0` logic.

#### Task 4.6: Reduce ProfileFormSections to Thin Composition Wrapper Depends on [4.1, 4.2, 4.3, 4.4, 4.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileFormSections.tsx
- src/crosshook-native/src/components/pages/InstallPage.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/ProfileFormSections.tsx

After all 5 section extractions are complete, reduce `ProfileFormSections` to a thin composition wrapper that imports and renders each extracted section component, threading `profile`, `onUpdateProfile`, `reviewMode`, `launchMethod`, and other props through. The existing `ProfileFormSectionsBaseProps` interface and `reviewMode` behavior must be preserved exactly as `InstallPage` depends on it. The `CustomEnvironmentVariablesSection` is NOT extracted (it's already its own component) — it stays inline in the composition. `ProtonDbLookupCard` stays co-located with env vars. Verify `InstallPage` renders correctly with `reviewMode={true}` after this change.

#### Task 4.7: Add ProfileSubTabs with Radix Tabs Depends on [4.6]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/styles/theme.css (crosshook-subtab-\* classes)
- src/crosshook-native/src/styles/variables.css (subtab variables)
- docs/plans/ui-enhancements/research-external.md (Radix Tabs JSX reference)
- src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/components/ProfileSubTabs.tsx

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

Create `ProfileSubTabs.tsx` using `@radix-ui/react-tabs` (`Tabs.Root`, `Tabs.List`, `Tabs.Trigger`, `Tabs.Content`). Map `Tabs.List` to `className="crosshook-subtab-row"` and `Tabs.Trigger` to `className="crosshook-subtab"` with active state via `data-state="active"` or the `--active` modifier class. Tab state uses `useState` (not URL-based).

**CRITICAL**: Tab panels MUST use CSS `display: none` on inactive panels, NOT conditional rendering. The `CustomEnvironmentVariablesSection` holds local `rows` state that would be lost on unmount. Render ALL tab panel contents always, toggle visibility with `style={{ display: activeTab === 'tabId' ? 'block' : 'none' }}` or equivalent CSS class toggle.

Tabs: "Setup" (Identity + Game + RunnerMethod), "Runtime" (RuntimeSection), "Environment" (EnvVars + ProtonDB), "Trainer" (TrainerSection). Wire into `ProfilesPage` inside the restructured card area. `ProfileSubTabs` only renders on `ProfilesPage` — `InstallPage` continues using `ProfileFormSections` directly without tabs.

#### Task 4.8: SteamGridDB Rust Client and Settings UI Depends on [2.2, 0.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/game_images/client.rs (from Task 2.2)
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs
- docs/plans/ui-enhancements/feature-spec.md (SteamGridDB section)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/game_images/mod.rs
- src/crosshook-native/src/components/pages/SettingsPage.tsx

Add `steamgriddb.rs` to the `game_images` module. Implement `pub async fn fetch_steamgriddb_image(api_key: &str, app_id: &str, image_type: GameImageType) -> Result<Vec<u8>, GameImageError>`. Use Bearer token auth: `Authorization: Bearer {api_key}`. Endpoint: `GET https://www.steamgriddb.com/api/v2/grids/steam/{app_id}`. Parse the response to extract the first grid URL, download the image bytes. Apply the same `validate_image_bytes()` check (SVG rejection) and size limit (5 MB). Annotate the client function with `#[tracing::instrument(skip(api_key))]` to prevent key logging.

Update `game_images/client.rs` fallback chain: if `steamgriddb_api_key` is present in settings, try SteamGridDB first → Steam CDN fallback → stale cache → None. The `game_image_cache.source` column distinguishes between `steam_cdn` and `steamgriddb`.

In `SettingsPage.tsx`, add a "SteamGridDB API Key" input field that reads/writes `steamgriddb_api_key` from settings. Include a "Get API Key" link to `https://www.steamgriddb.com/`. Add `https://www.steamgriddb.com/**` to the `shell:allow-open` list in `capabilities/default.json` if needed for external link opening.

## Advice

- **Copy protondb/ as a scaffold, don't write from scratch**: The `steam_metadata/` module is structurally identical to `protondb/`. Copy the 3 files (`mod.rs`, `client.rs`, `models.rs`), rename types and endpoints, and adjust the cache key and TTL. This eliminates design-from-scratch risk and ensures pattern consistency.
- **The circular import in Task 0.1 is the single highest-friction blocker**: If `formatProtonInstallLabel` is not extracted before Phase 4 begins, section extraction will create import cycles. Do this first even if nothing else in Phase 0 is ready.
- **CSS `display: none` in Task 4.7 is a hard requirement, not a preference**: The `CustomEnvironmentVariablesSection` holds local `rows` state for env var editing. Conditional rendering (unmount/remount) silently discards in-progress edits. This is a known risk (W1 in the feature spec). Every PR that touches tab panel visibility must be verified against this constraint.
- **Phase 2 Rust work (Tasks 2.1, 2.2) can start immediately after Phase 0**: These modules share zero files with Phase 1's frontend restructuring. Assign backend and frontend tracks to separate agents/developers for maximum parallelism.
- **Security mitigations in Task 2.2 are gate items, not optional**: The `validate_image_bytes()` and `safe_image_cache_path()` functions from `research-security.md` must be implemented verbatim. SVG rejection (I1) and path traversal prevention (I2) are WARNING-level findings that must ship with Phase 2.
- **`injection.*` fields must never appear in any new section component**: `GameProfile` has `injection.dll_paths` and `injection.inject_on_launch` that are managed by the install pipeline. During section extraction (Phase 4), explicitly verify no section component renders or edits `injection` fields — they must remain hidden from the user form.
- **Test `InstallPage` after Tasks 1.1, 4.6, and 4.7**: These are the three moments when `ProfileFormSections`' contract with `InstallPage` could break. Run the install/onboarding flow each time. The `reviewMode` prop must continue to gate field collapse and "Apply" button behavior.
- **The `game_image_cache.source` column future-proofs for SteamGridDB**: Even though SteamGridDB ships in Task 4.8, the table schema (Task 0.2) already has the `source` column. This means no v15 migration is needed when SteamGridDB support lands.
- **Phase 4 section extractions (Tasks 4.1-4.5) are fully parallelizable**: Each extracts a distinct, non-overlapping block from `ProfileFormSections`. Up to 5 agents can work these simultaneously with zero file conflicts, provided they all complete before Task 4.6 begins.
- **Do not plan for Phase 5 (visual polish)**: Gradient overlays, portrait card layout, and grid/list toggle are explicitly deferred. Do not stub, create files, or allocate tasks for these until Phase 3 ships and user feedback is collected.
