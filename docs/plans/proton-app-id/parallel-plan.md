# Proton App ID & Tri-Art System Implementation Plan

This feature adds `runtime.steam_app_id` to `proton_run` profiles so the existing art download/cache pipeline can resolve cover, portrait, and background art without a `[steam]` section — then extends custom art from a single cover slot to three independent slots (cover, portrait, background) with per-type upload and mix-and-match resolution. The primary integration surface is `crosshook-core/src/profile/models.rs` (profile section structs + three-layer merge), `crosshook-core/src/game_images/` (download pipeline, import, SteamGridDB), and the Tauri IPC layer (`commands/profile.rs`, `commands/game_metadata.rs`). All required crates and the SQLite `game_image_cache` table already exist — no new dependencies or migrations needed. Security findings (redirect policy, API key exposure, export sanitization, auth failure handling, silent type default) are folded into Phases 2–4.

## Critically Relevant Files and Documentation

- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: All profile section structs (`RuntimeSection`, `GameSection`, `LocalOverrideGameSection`), `effective_profile()`, `storage_profile()`, `portable_profile()`, `resolve_launch_method()` — primary change surface for Phases 1 and 2
- src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs: `sanitize_profile_for_community_export` — must verify custom art paths cleared (S-03)
- src/crosshook-native/crates/crosshook-core/src/game_images/models.rs: `GameImageType` enum + `Display`, `GameImageError`, `GameImageSource`
- src/crosshook-native/crates/crosshook-core/src/game_images/client.rs: `download_and_cache_image`, `build_download_url`, `filename_for`, HTTP singleton, stale fallback
- src/crosshook-native/crates/crosshook-core/src/game_images/import.rs: `import_custom_cover_art`, `is_in_managed_media_dir`, `media_base_dir`
- src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs: `fetch_steamgriddb_image`, `build_endpoint`
- src/crosshook-native/crates/crosshook-core/src/game_images/mod.rs: Public re-exports
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: `AppSettingsData` with `steamgriddb_api_key`
- src/crosshook-native/src-tauri/src/commands/game_metadata.rs: `fetch_game_cover_art`, `import_custom_cover_art` Tauri commands
- src/crosshook-native/src-tauri/src/commands/profile.rs: `ProfileSummary` DTO, `profile_list_summaries`, `profile_save` with auto-import
- src/crosshook-native/src-tauri/src/commands/settings.rs: `settings_load` — exposes raw API key (S-02)
- src/crosshook-native/src-tauri/src/lib.rs: `invoke_handler!` macro for command registration
- src/crosshook-native/src/types/profile.ts: `GameProfile` TS interface
- src/crosshook-native/src/types/library.ts: `LibraryCardData` interface
- src/crosshook-native/src/hooks/useGameCoverArt.ts: Art fetch hook — already accepts `imageType` param
- src/crosshook-native/src/hooks/useLibrarySummaries.ts: Invokes `profile_list_summaries`
- src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx: proton_run App ID field (~line 188)
- src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx: Null-gate bug
- src/crosshook-native/src/components/profile-sections/MediaSection.tsx: Single cover slot — expand to three
- docs/plans/proton-app-id/feature-spec.md: Authoritative feature contract — business rules, data models, phasing
- docs/plans/proton-app-id/research-security.md: Security findings S-01 through S-15 with code snippets
- docs/plans/proton-app-id/research-technical.md: Full struct change specs and integration points
- docs/plans/proton-app-id/research-ux.md: Three-slot media section design, App ID field placement
- AGENTS.md: Hard architectural constraints — `crosshook-core` owns logic, IPC-thin `src-tauri`

## Implementation Plan

### Phase 1: Proton App ID End-to-End

#### Task 1.1: Add `steam_app_id` to RuntimeSection + resolve_art_app_id helper Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/models.rs
- docs/plans/proton-app-id/feature-spec.md (BR-2, BR-4, BR-9)
- docs/plans/proton-app-id/analysis-code.md (Pattern 2: Adding a Field)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/models.rs

1. Add `pub steam_app_id: String` to `RuntimeSection` with `#[serde(rename = "steam_app_id", default, skip_serializing_if = "String::is_empty")]`.

2. **Update `RuntimeSection::is_empty()`** to include `&& self.steam_app_id.trim().is_empty()`. This is critical: without it, a profile with only `steam_app_id` set has `is_empty()` return `true`, the `[runtime]` TOML section is skipped, and the field is silently lost.

3. Add `pub fn resolve_art_app_id(profile: &GameProfile) -> &str` as a free function alongside `resolve_launch_method` (~line 511). Returns `steam.app_id.trim()` if non-empty, else `runtime.steam_app_id.trim()`.

4. Add `pub fn validate_steam_app_id(value: &str) -> Result<(), String>` — validates ASCII decimal digits only, 1-12 chars, empty string allowed (means "not set"). This is called at profile-save time (BR-4).

5. **Do NOT** add `steam_app_id` to `LocalOverrideRuntimeSection` — it is portable metadata (media ID only), not a machine-local path. It stays in the base `RuntimeSection` and intentionally survives portable export.

6. **Tests**: Add unit tests:
   - `RuntimeSection::is_empty()` returns `false` when only `steam_app_id` is set
   - `resolve_art_app_id` returns `steam.app_id` when both are set; falls back to `runtime.steam_app_id` when `steam.app_id` is empty
   - `validate_steam_app_id` rejects non-numeric, >12 digit, and accepts empty/valid IDs
   - Verify `steam_app_id` round-trips through TOML serialization

Run: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

#### Task 1.2: Fix GameCoverArt null-gate bug Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx
- docs/plans/proton-app-id/analysis-code.md (Pattern 8: GameCoverArt Null-Gate Bug)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx

The component currently returns `null` when `steamAppId` is falsy, even when `customCoverArtPath` is set. Fix the early-return guard:

```tsx
// Before:
if (!steamAppId) {
  return null;
}

// After:
if (!steamAppId && !customCoverArtPath?.trim()) {
  return null;
}
```

The `useGameCoverArt` hook already handles `steamAppId = undefined` correctly (skips IPC fetch, returns custom path via `customUrl ?? coverArtUrl`). This fix only changes the guard in the component itself.

This is a 2-line quick win that immediately improves art display for existing `proton_run` profiles with custom cover art but no `steam.app_id`.

**Manual verification** (no frontend test framework): Open a `proton_run` profile with `custom_cover_art_path` set but no `steam.app_id` — cover art should now appear in the profile editor header.

#### Task 1.3: Update ProfileSummary DTO and profile_list_summaries Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/profile.rs
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs (resolve_art_app_id from Task 1.1)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/profile.rs

1. In `profile_list_summaries`, replace the current `steam_app_id: effective.steam.app_id.clone()` with `steam_app_id: resolve_art_app_id(&effective).to_string()`. Import `resolve_art_app_id` from `crosshook_core::profile`.

2. The `ProfileSummary` DTO field `steam_app_id` now carries the effective media app ID for both `steam_applaunch` (from `steam.app_id`) and `proton_run` (from `runtime.steam_app_id`) profiles. No field rename needed — the frontend's `steamAppId` (camelCase via `rename_all`) already feeds `useGameCoverArt`.

3. Add `validate_steam_app_id` call in `profile_save` before writing to disk — reject profiles with invalid `runtime.steam_app_id` values at save time (BR-4).

4. **Tests**: Add or update tests verifying `profile_list_summaries` returns `runtime.steam_app_id` for `proton_run` profiles where `steam.app_id` is empty.

#### Task 1.4: Add steam_app_id to TypeScript types + create art utilities Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/profile.ts
- src/crosshook-native/src/types/library.ts
- src/crosshook-native/src/utils/steam.ts (pattern reference for utility function)

**Instructions**

Files to Modify

- src/crosshook-native/src/types/profile.ts

Files to Create

- src/crosshook-native/src/utils/art.ts

1. In `profile.ts`, add `steam_app_id?: string` to the `runtime` section of the `GameProfile` interface.

2. Create `src/utils/art.ts` with `resolveArtAppId(profile: GameProfile): string` — mirrors the Rust helper: returns `profile.steam?.app_id` if non-empty, else `profile.runtime?.steam_app_id ?? ''`. Follow the pattern in `utils/steam.ts` (pure function, named export).

3. Add `validateSteamAppId(value: string): boolean` to `art.ts` — mirrors Rust validation: ASCII digits only, 1-12 chars, empty allowed.

#### Task 1.5: Rebind RuntimeSection.tsx proton_run Steam App ID field Depends on [1.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx
- docs/plans/proton-app-id/research-ux.md (App ID field placement)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx

1. Find the `proton_run` block (~line 155-250). Locate the "Steam App ID" `FieldRow` (~line 188) currently bound to `profile.steam.app_id`.

2. Rebind to `profile.runtime.steam_app_id`:

   ```tsx
   value={profile.runtime?.steam_app_id ?? ''}
   onChange={(value) =>
     onUpdateProfile((current) => ({
       ...current,
       runtime: { ...current.runtime, steam_app_id: value },
     }))
   }
   ```

3. Add inline validation using `validateSteamAppId` from `utils/art.ts`. Show a validation hint if the value contains non-numeric characters.

4. Update the placeholder text to "Optional — used for art and metadata lookup" (reflects the new semantic: media ID, not launch ID).

5. **Do NOT touch** the `steam_applaunch` "Steam App ID" field (~line 60) — it must remain bound to `steam.app_id`.

#### Task 1.6: Update useLibrarySummaries to consume effective app ID Depends on [1.3, 1.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useLibrarySummaries.ts
- src/crosshook-native/src/types/library.ts

**Instructions**

Files to Modify

- src/crosshook-native/src/hooks/useLibrarySummaries.ts

1. The backend `profile_list_summaries` (updated in Task 1.3) now returns the effective app ID via `steamAppId`. The hook maps this directly to `LibraryCardData.steamAppId` — verify the mapping is correct. No structural change should be needed if the field name hasn't changed, but confirm the hook passes the value through correctly.

2. `LibraryCard` already calls `useGameCoverArt(profile.steamAppId, profile.customCoverArtPath, 'portrait')` — with the backend now returning the effective app ID, `proton_run` profiles will automatically display portrait art in the Library grid.

3. **This task is the Phase 1 end-to-end smoke test.** Verify the exact field: `steamAppId` on `LibraryCardData` (mapped from `ProfileSummary.steam_app_id`). If a `proton_run` profile with `runtime.steam_app_id` set shows portrait art in the Library grid, Phase 1 is complete. If not, trace the field from `profile_list_summaries` → `useLibrarySummaries` → `LibraryCard` → `useGameCoverArt` to find where the effective app ID is lost.

### Phase 2: Tri-Art Custom Upload

#### Task 2.1: Add portrait/background art path fields + three-profile merge logic Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/models.rs
- docs/plans/proton-app-id/analysis-code.md (Pattern 2 and Pattern 3)
- docs/plans/proton-app-id/feature-spec.md (BR-5, BR-6, BR-10)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/models.rs

1. Add to `GameSection`:

   ```rust
   #[serde(rename = "custom_portrait_art_path", default, skip_serializing_if = "String::is_empty")]
   pub custom_portrait_art_path: String,
   #[serde(rename = "custom_background_art_path", default, skip_serializing_if = "String::is_empty")]
   pub custom_background_art_path: String,
   ```

2. Add the same two fields to `LocalOverrideGameSection` with `#[serde(rename = "…", default)]`.

3. **Update `LocalOverrideGameSection::is_empty()`** to include the new fields:

   ```rust
   && self.custom_portrait_art_path.trim().is_empty()
   && self.custom_background_art_path.trim().is_empty()
   ```

4. **Update `effective_profile()`** — add merge logic for both new fields, following the exact `custom_cover_art_path` pattern:

   ```rust
   if !self.local_override.game.custom_portrait_art_path.trim().is_empty() {
       merged.game.custom_portrait_art_path = self.local_override.game.custom_portrait_art_path.clone();
   }
   // Same for custom_background_art_path
   ```

5. **Update `storage_profile()`** — move both new path fields to `local_override`, clear base:

   ```rust
   storage.local_override.game.custom_portrait_art_path = effective.game.custom_portrait_art_path.clone();
   storage.game.custom_portrait_art_path.clear();
   // Same for custom_background_art_path
   ```

6. **`portable_profile()`** requires NO changes — it calls `storage_profile()` then resets all of `local_override` via `Default::default()`, which covers the new fields automatically.

7. **Tests**: Extend existing `storage_profile` / `effective_profile` round-trip tests to cover portrait and background art paths. Verify they move to `local_override` in storage and merge correctly in effective.

Run: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

#### Task 2.2: Generalize import_custom_cover_art to import_custom_art Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/game_images/import.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/mod.rs
- docs/plans/proton-app-id/analysis-code.md (Pattern 4)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/game_images/import.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/mod.rs

1. Create `pub fn import_custom_art(source_path: &str, art_type: GameImageType) -> Result<String, String>`. Extract the current `import_custom_cover_art` body, replace the hardcoded `"covers"` subdir with a match:

   ```rust
   let subdir = match art_type {
       GameImageType::Cover => "covers",
       GameImageType::Portrait => "portraits",
       GameImageType::Background => "backgrounds",
       _ => return Err(format!("Unsupported art type for custom import: {art_type}")),
   };
   let dest_dir = media_base_dir()?.join(subdir);
   ```

2. Convert `import_custom_cover_art` to a backward-compat wrapper:

   ```rust
   pub fn import_custom_cover_art(source_path: &str) -> Result<String, String> {
       import_custom_art(source_path, GameImageType::Cover)
   }
   ```

3. In `mod.rs`, add `import_custom_art` to the public re-exports.

4. `is_in_managed_media_dir` checks against `media_base_dir()` root — it already covers all subdirectories. No change needed.

5. **Tests**: Add tests for `import_custom_art` with `Portrait` and `Background` types, verifying they create the correct subdirectories and that content-addressed naming works. Follow the existing `tempfile::tempdir()` pattern.

#### Task 2.3: Fix S-03 — Clear custom art paths in community export Depends on [2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs
- docs/plans/proton-app-id/research-security.md (S-03)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs

1. In `sanitize_profile_for_community_export`, after the `portable_profile()` call and existing clears, add explicit clears for all three custom art path fields as a defense-in-depth measure:

   ```rust
   out.game.custom_cover_art_path.clear();
   out.game.custom_portrait_art_path.clear();
   out.game.custom_background_art_path.clear();
   ```

2. `portable_profile()` already clears `local_override` via `Default::default()`, so the local_override fields are already handled. The explicit clears above guard against the case where a path leaks into the base `game` section.

3. Verify `runtime.steam_app_id` is NOT cleared — it is portable media metadata that should survive export (BR-2, BR-6).

4. **Tests**: Add a test that creates a profile with all three custom art paths set (both base and local_override), exports via `sanitize_profile_for_community_export`, and asserts all three paths are empty in the result.

#### Task 2.4: Add import_custom_art Tauri IPC command Depends on [2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/game_metadata.rs
- src/crosshook-native/src-tauri/src/lib.rs

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/game_metadata.rs
- src/crosshook-native/src-tauri/src/lib.rs

1. Add a new `#[tauri::command]` in `game_metadata.rs`:

   ```rust
   #[tauri::command]
   pub fn import_custom_art(source_path: String, art_type: Option<String>) -> Result<String, String> {
       let image_type = match art_type.as_deref().unwrap_or("cover") {
           "cover" => GameImageType::Cover,
           "portrait" => GameImageType::Portrait,
           "background" => GameImageType::Background,
           other => return Err(format!("Unknown art type: {other}")),
       };
       crosshook_core::game_images::import_custom_art(&source_path, image_type)
   }
   ```

2. Register `import_custom_art` in the `invoke_handler!` macro in `lib.rs`.

3. Keep the existing `import_custom_cover_art` IPC command — it still works and existing callers use it. It can be deprecated later.

#### Task 2.5: Extend profile_save auto-import + ProfileSummary portrait path Depends on [2.1, 2.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/profile.rs (auto-import block ~line 279)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/profile.rs

1. In `profile_save`, replicate the existing `custom_cover_art_path` auto-import pattern for portrait and background:

   ```rust
   // Portrait auto-import
   let portrait = data.game.custom_portrait_art_path.trim().to_string();
   if !portrait.is_empty() && !is_in_managed_media_dir(&portrait) {
       match import_custom_art(&portrait, GameImageType::Portrait) {
           Ok(imported) => data.game.custom_portrait_art_path = imported,
           Err(e) => tracing::warn!("Failed to auto-import portrait art: {e}"),
       }
   }
   // Same pattern for custom_background_art_path with GameImageType::Background
   ```

2. Add `custom_portrait_art_path: Option<String>` to `ProfileSummary` DTO. Populate from the effective profile in `profile_list_summaries`. The `#[serde(rename_all = "camelCase")]` on the struct maps it to `customPortraitArtPath` in TS automatically. **Note**: `custom_background_art_path` is intentionally NOT added to `ProfileSummary` — background art is only displayed in the game detail/profile view, not the library card grid.

3. Import `import_custom_art` from `crosshook_core::game_images` alongside the existing `import_custom_cover_art` import.

#### Task 2.6: Update TypeScript types for tri-art fields Depends on [2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/profile.ts
- src/crosshook-native/src/types/library.ts

**Instructions**

Files to Modify

- src/crosshook-native/src/types/profile.ts
- src/crosshook-native/src/types/library.ts

1. In `profile.ts`, add to the `game` section of `GameProfile`:

   ```ts
   custom_portrait_art_path?: string;
   custom_background_art_path?: string;
   ```

2. Add the same two fields to `local_override.game` section.

3. In `library.ts`, add `customPortraitArtPath?: string` to `LibraryCardData`. This mirrors the new `ProfileSummary` DTO field from Task 2.5.

#### Task 2.7: Expand MediaSection to three art slots Depends on [2.4, 2.6]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/profile-sections/MediaSection.tsx
- docs/plans/proton-app-id/research-ux.md (three-slot design, source badge, thumbnail preview)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/profile-sections/MediaSection.tsx
- src/crosshook-native/src/styles/variables.css (if new CSS variables needed for slot dimensions)

1. Expand from a single cover art slot to three: **Cover** (preview: ~230x107px, 2.14:1), **Portrait** (preview: ~120x180px, 2:3), **Background** (preview: ~240x80px, ~3:1). Each slot has Browse/Clear/Preview controls.

2. **Preserve existing cover art save path**: The current cover slot's `onUpdateProfile` logic for `game.custom_cover_art_path` must continue working identically. Extract the shared Browse/Clear/Preview pattern into a reusable slot component or repeated block — don't break the existing cover flow.

3. Each slot invokes `import_custom_art` IPC with the appropriate `art_type` parameter:

   ```ts
   const importedPath = await invoke<string>('import_custom_art', {
     sourcePath: selectedFile,
     artType: 'portrait', // or 'cover', 'background'
   });
   ```

4. Update the profile via `onUpdateProfile` for the corresponding field:
   - Cover: `game.custom_cover_art_path`
   - Portrait: `game.custom_portrait_art_path`
   - Background: `game.custom_background_art_path`

5. Add a source badge per slot indicating "Custom" (has path), "Auto" (has effective app_id but no custom path), or "Not Set" (neither).

6. Show thumbnail previews using `convertFileSrc` for existing art paths — all raw file paths MUST go through `convertFileSrc` for the Tauri asset protocol. For slots without a custom path, show a placeholder with the expected aspect ratio.

7. Use BEM-like `crosshook-media-section__slot`, `crosshook-media-section__slot--cover`, etc. Add CSS variables in `variables.css` if new slot dimension or spacing values are needed.

### Phase 3: Background Image Type + Download Pipeline

#### Task 3.1: Add GameImageType::Background + all match sites Depends on [2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/game_images/models.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/client.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs
- docs/plans/proton-app-id/analysis-code.md (Pattern 1: Adding a GameImageType Variant)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/game_images/models.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/client.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs

1. **models.rs**: Add `Background` variant to `GameImageType` enum. Add `Display` arm: `Self::Background => write!(f, "background")`.

2. **client.rs — `build_download_url`**: Add `GameImageType::Background => format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_hero.jpg")`. Same CDN path as Hero — Background uses the hero banner image. Add a comment: `// Background uses same CDN file as Hero (library_hero.jpg, 3840x1240)`.

3. **client.rs — `filename_for`**: Add `GameImageType::Background => "background"`. This produces cache filenames like `background_steam_cdn.jpg`.

4. **steamgriddb.rs — `build_endpoint`**: Add `GameImageType::Background => ("heroes", None)`. Same SteamGridDB endpoint as Hero, no dimension filter.

5. **Note**: The `download_and_cache_image` function dispatches to these helpers — no changes to the orchestration function itself. Portrait's special `try_portrait_candidates` fallback chain is only triggered for `GameImageType::Portrait` — Background uses the single CDN URL, no fallback chain.

6. **Tests**:
   - `build_download_url` returns correct URL for `Background`
   - `filename_for` returns `"background"` prefix for `Background`
   - `build_endpoint` returns `("heroes", None)` for `Background`
   - The `Display` impl produces `"background"` string

Run: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

#### Task 3.2: Add "background" to IPC dispatch + fix S-05 silent default Depends on [3.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/game_metadata.rs
- docs/plans/proton-app-id/research-security.md (S-05)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/game_metadata.rs

1. Add `"background" => GameImageType::Background` to the `image_type` match dispatch in `fetch_game_cover_art`.

2. Fix the silent default (S-05): Change the `_ => GameImageType::Cover` catch-all to log a warning and return an error for unrecognized types:

   ```rust
   let image_type = match image_type.as_deref().unwrap_or("cover") {
       "cover" => GameImageType::Cover,
       "hero" => GameImageType::Hero,
       "capsule" => GameImageType::Capsule,
       "portrait" => GameImageType::Portrait,
       "background" => GameImageType::Background,
       other => {
           tracing::warn!("Unknown image_type requested: {other}");
           return Err(format!("Unknown image type: {other}"));
       }
   };
   ```

3. This is an IPC string dispatch — the Rust compiler does NOT check it for exhaustiveness. Any future `GameImageType` variant additions must update this match manually.

4. **Smoke test**: From the dev console, invoke `fetch_game_cover_art` with `imageType: 'invalid'` and verify it returns an error (not a cover image). Then invoke with `imageType: 'background'` and a valid app ID and verify a background image path is returned.

### Phase 4: Security Hardening

#### Task 4.1: S-01/S-06 — Add redirect-policy domain allow-list Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/game_images/client.rs (HTTP singleton ~line 22)
- docs/plans/proton-app-id/research-security.md (S-01, S-06)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/game_images/client.rs

1. Add a custom redirect policy to the `reqwest::Client` builder in the `http_client()` singleton:

   ```rust
   use reqwest::redirect::Policy;
   use url::Url;

   let allowed_hosts: &[&str] = &[
       "cdn.cloudflare.steamstatic.com",
       "steamcdn-a.akamaihd.net",
       "www.steamgriddb.com",
       "cdn2.steamgriddb.com",
   ];

   .redirect(Policy::custom(move |attempt| {
       let url = attempt.url();
       if url.scheme() != "https" {
           return attempt.stop();
       }
       if let Some(host) = url.host_str() {
           if allowed_hosts.iter().any(|&h| h == host) {
               return attempt.follow();
           }
       }
       attempt.stop()
   }))
   ```

2. This prevents the HTTP client from following redirects to arbitrary domains, mitigating SSRF risk from CDN misconfiguration.

3. **Tests**: Add a unit test that constructs the redirect policy closure, simulates redirect attempts to both allowed and non-allowed domains, and asserts the correct allow/stop behavior. Do NOT use `wiremock` (not in `Cargo.toml`). Test the policy logic directly by verifying the allow-list filtering — extract the host-check logic into a testable helper function `is_allowed_redirect_host(host: &str) -> bool` and unit-test that.

#### Task 4.2: S-02 — Filter API key at IPC boundary Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/settings.rs
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs
- docs/plans/proton-app-id/research-security.md (S-02)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/settings.rs
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs
- src/crosshook-native/src/types/settings.ts
- src/crosshook-native/src/contexts/PreferencesContext.tsx

1. Create an IPC-safe DTO in `settings.rs` (or `settings/mod.rs`) that replaces the raw API key with a boolean:

   ```rust
   #[derive(Serialize, Deserialize)]
   #[serde(rename_all = "camelCase")]
   pub struct AppSettingsIpcData {
       // ...all existing fields except steamgriddb_api_key...
       pub has_steamgriddb_api_key: bool,
   }
   ```

2. Update `settings_load` to convert `AppSettingsData` → `AppSettingsIpcData` before returning, mapping `steamgriddb_api_key.is_some()` to `has_steamgriddb_api_key`.

3. Add a separate `settings_save_steamgriddb_key` IPC command for setting/clearing the key (write-only, never returned to frontend). Also update `settings_save` to strip `steamgriddb_api_key` from the payload if present — the key should only be written through the dedicated command, never round-tripped through `settings_save`.

4. Update `src/types/settings.ts`: Change `steamgriddb_api_key?: string | null` to `has_steamgriddb_api_key: boolean` in the `AppSettingsData` TS interface.

5. **Frontend consumers that call `settings_load`** (must update all):
   - `src/crosshook-native/src/contexts/PreferencesContext.tsx` (~line 50): Currently loads `steamgriddb_api_key` from settings and populates the Preferences UI. Must change to: show a "Key is set" / "No key configured" indicator based on `hasSteamgriddbApiKey: boolean`. The `handleSteamGridDbApiKeyChange` save path (~line 133) must call the new `settings_save_steamgriddb_key` IPC command instead of bundling the key into `settings_save`.
   - `src/crosshook-native/src/hooks/useCommunityProfiles.ts` (~lines 224, 391): Check if these reference the API key field; update type if needed.
   - `src/crosshook-native/src/hooks/useProfile.ts` (~lines 502, 626): Check if these reference the API key field; update type if needed.

6. Register `settings_save_steamgriddb_key` in the `invoke_handler!` macro in `lib.rs`.

#### Task 4.3: S-12 — AuthFailure error variant + CDN fallback on 401/403 Depends on [3.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/game_images/models.rs (GameImageError)
- src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/client.rs
- docs/plans/proton-app-id/research-security.md (S-12)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/game_images/models.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/client.rs

1. Add `AuthFailure { status: u16, message: String }` variant to `GameImageError`.

2. In `steamgriddb.rs` (`fetch_steamgriddb_image`), detect HTTP 401/403 responses and return `Err(GameImageError::AuthFailure { ... })` instead of a generic network error.

3. In `client.rs` (`download_and_cache_image`), when the SteamGridDB call returns `AuthFailure`, fall back to Steam CDN (skip stale cache) and log a warning: `"SteamGridDB auth failed (status {status}). Falling back to Steam CDN. Check your API key."`. Do NOT treat auth failure as a cache-worthy result — the user should see the auth hint.

4. **Tests**: Test that `AuthFailure` from SGDB triggers CDN fallback, not stale cache return.

## Advice

- **The `is_empty()` gotcha is the highest-risk correctness issue**: If `RuntimeSection::is_empty()` does not include `steam_app_id` in its check, and `LocalOverrideGameSection::is_empty()` does not include the new art path fields, profile data will be silently lost during TOML serialization. Both Task 1.1 and Task 2.1 must include explicit unit tests for this. The pattern is: set only the new field, call `is_empty()`, assert it returns `false`.

- **IPC string dispatch is NOT compiler-checked**: When adding `GameImageType::Background` in Task 3.1, the Rust compiler will enforce exhaustiveness in `build_download_url`, `filename_for`, and `build_endpoint` via match arms. But the `"background"` string in `game_metadata.rs` (Task 3.2) is a plain `&str` match — the compiler will not warn if it's missing. Always update both in the same PR.

- **`runtime.steam_app_id` is intentionally portable**: Unlike `custom_cover_art_path` (machine-local, moves to `local_override`), `steam_app_id` stays in the base `RuntimeSection` and survives portable export. This is by design (BR-2: media-only, not machine-specific). Do NOT add it to `LocalOverrideRuntimeSection`. Do NOT clear it in `sanitize_profile_for_community_export`.

- **`profile_save` auto-import scope**: The existing auto-import in `profile.rs` only handles `custom_cover_art_path`. Task 2.5 must replicate this for portrait and background. If any auto-import is missed, users who paste a raw file path into the profile TOML will see the path stored but the file not managed — a subtle data integrity issue.

- **Phase parallelism boundaries**: Within Phase 1, Tasks 1.2 (GameCoverArt fix) is fully independent and can be done first as a quick win. Tasks 1.3 and 1.4 can run in parallel once 1.1 lands. Within Phase 2, Tasks 2.1/2.2 and 2.2 can run in parallel (separate Rust modules). Phase 4 tasks are all independent of each other.

- **Background CDN URL = Hero CDN URL**: `library_hero.jpg` is used for both `Hero` and `Background` types. This is intentional — they share the same source asset. They are distinguished by `GameImageType` and stored in separate cache rows (`image_type TEXT` in `game_image_cache`).

- **Test command**: After every Rust change, run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`. There is no configured frontend test framework — verify UI changes with `./scripts/dev-native.sh`.

- **`GameCoverArt.tsx` vs `LibraryCard.tsx`**: Both consume art but serve different contexts. `LibraryCard` uses `useGameCoverArt` with `'portrait'` type and IntersectionObserver. `GameCoverArt` is used in the profile editor header. The null-gate fix (Task 1.2) only affects `GameCoverArt` — `LibraryCard` has its own null handling via the `imgFailed` state.
