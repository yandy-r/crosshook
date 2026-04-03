# Code Analysis: Proton App ID & Tri-Art System

## Executive Summary

The feature touches a well-established pipeline: all image logic lives in `crosshook-core/src/game_images/`, profiles follow a three-representation pattern (effective/storage/portable), and Tauri commands are thin dispatchers. Adding `steam_app_id` to `RuntimeSection`, extending `GameImageType` with `Background`, and generalizing `import_custom_cover_art` to `import_custom_art` are the core changes, each with clear analogues already in the codebase. Every match arm site must be updated when adding a new `GameImageType` variant — compiler exhaustiveness enforcement makes this safe but requires visiting four files: `models.rs` (Display), `client.rs` (build_download_url, filename_for), `steamgriddb.rs` (build_endpoint), and `game_metadata.rs` (image_type dispatch string).

---

## Existing Code Structure

### Rust Backend

```
crosshook-core/src/
  game_images/
    mod.rs           — pub re-exports (download_and_cache_image, import_custom_cover_art, is_in_managed_media_dir, GameImageType, GameImageError, GameImageSource)
    models.rs        — GameImageType enum + Display, GameImageSource enum, GameImageError custom error
    client.rs        — download_and_cache_image (public), validate_image_bytes, safe_image_cache_path, build_download_url, filename_for, image_cache_base_dir
    import.rs        — import_custom_cover_art, is_in_managed_media_dir, media_base_dir
    steamgriddb.rs   — fetch_steamgriddb_image, build_endpoint
  profile/
    models.rs        — all section structs (GameSection, RuntimeSection, LocalOverrideGameSection, etc.), effective_profile(), storage_profile(), portable_profile()
    exchange.rs      — sanitize_profile_for_community_export, export_community_profile, import_community_profile
  settings/
    mod.rs           — AppSettingsData (contains steamgriddb_api_key: Option<String>), SettingsStore

src-tauri/src/commands/
  game_metadata.rs   — fetch_game_cover_art (IPC), import_custom_cover_art (IPC)
  profile.rs         — ProfileSummary DTO, profile_list_summaries, profile_save (with auto-import)
  settings.rs        — settings_load, settings_save (passes AppSettingsData directly — API key exposed)
  lib.rs             — invoke_handler! macro — all command registrations
```

### Frontend

```
src/crosshook-native/src/
  types/
    profile.ts       — GameProfile TS interface (runtime section has prefix_path, proton_path, working_directory — missing steam_app_id)
    library.ts       — LibraryCardData (missing customPortraitArtPath)
  hooks/
    useGameCoverArt.ts  — accepts (steamAppId, customArtPath, imageType); prioritizes custom path; race-safe
  components/profile-sections/
    GameCoverArt.tsx    — null-gate bug: returns null when !steamAppId even if customCoverArtPath is set
    MediaSection.tsx    — single cover slot only; invokes import_custom_cover_art directly
    RuntimeSection.tsx  — proton_run binds Steam App ID to profile.steam.app_id (must rebind to runtime.steam_app_id)
```

---

## Implementation Patterns

### Pattern 1: Adding a `GameImageType` Variant

All four match sites must be updated together. The compiler enforces exhaustiveness.

**models.rs** — enum definition + Display:

```rust
// Current
pub enum GameImageType { Cover, Hero, Capsule, Portrait }
impl fmt::Display for GameImageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cover => write!(f, "cover"),
            Self::Portrait => write!(f, "portrait"),
            // ... add: Self::Background => write!(f, "background"),
        }
    }
}
```

**client.rs** — `build_download_url` (Steam CDN URL per type):

```rust
// Current pattern — each arm returns a CDN URL string
fn build_download_url(app_id: &str, image_type: GameImageType) -> String {
    match image_type {
        GameImageType::Cover => format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/header.jpg"),
        GameImageType::Hero => format!("...library_hero.jpg"),
        GameImageType::Capsule => format!("...capsule_616x353.jpg"),
        GameImageType::Portrait => format!("...library_600x900_2x.jpg"),
        // Add: GameImageType::Background => format!("...library_hero.jpg") — same CDN path as Hero
    }
}
```

**client.rs** — `filename_for` (cached filename on disk):

```rust
fn filename_for(image_type: GameImageType, source: GameImageSource, extension: &str) -> String {
    let type_prefix = match image_type {
        GameImageType::Cover => "cover",
        GameImageType::Portrait => "portrait",
        // Add: GameImageType::Background => "background",
    };
    format!("{type_prefix}_{source_suffix}.{extension}")
}
```

**steamgriddb.rs** — `build_endpoint` (SteamGridDB API path segment + dimensions):

```rust
fn build_endpoint(app_id: &str, image_type: &GameImageType) -> String {
    let (path_segment, dimensions) = match image_type {
        GameImageType::Cover => ("grids", Some("460x215,920x430")),
        GameImageType::Hero => ("heroes", None),
        GameImageType::Capsule => ("grids", Some("342x482,600x900")),
        GameImageType::Portrait => ("grids", Some("342x482,600x900")),
        // Add: GameImageType::Background => ("heroes", None)
    };
    // ...
}
```

**game_metadata.rs** — IPC string dispatch (NOT compiler-checked — must update manually):

```rust
let image_type = match image_type.as_deref().unwrap_or("cover") {
    "hero" => GameImageType::Hero,
    "capsule" => GameImageType::Capsule,
    "portrait" => GameImageType::Portrait,
    _ => GameImageType::Cover,  // SILENT DEFAULT — security issue S-05
    // Add explicit: "background" => GameImageType::Background,
    // THEN change _ arm to: _ => GameImageType::Cover with a warning log
};
```

---

### Pattern 2: Adding a Field to a Profile Section

Use `custom_cover_art_path` in `GameSection` as the canonical example:

```rust
// GameSection — base (portable) field
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GameSection {
    #[serde(rename = "custom_cover_art_path", default, skip_serializing_if = "String::is_empty")]
    pub custom_cover_art_path: String,
}

// LocalOverrideGameSection — machine-local override mirror
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalOverrideGameSection {
    #[serde(rename = "custom_cover_art_path", default)]
    pub custom_cover_art_path: String,
}

impl LocalOverrideGameSection {
    pub fn is_empty(&self) -> bool {
        self.executable_path.trim().is_empty() && self.custom_cover_art_path.trim().is_empty()
        // extend: && self.custom_portrait_art_path.trim().is_empty() && self.custom_background_art_path.trim().is_empty()
    }
}
```

For `RuntimeSection::steam_app_id`: the field is portable (not machine-local), so it goes only in `RuntimeSection` — NOT in `LocalOverrideRuntimeSection`. It uses `#[serde(rename = "steam_app_id", default, skip_serializing_if = "String::is_empty")]`. The `is_empty()` method must include it.

---

### Pattern 3: Three-Profile Representation (effective/storage/portable)

Custom art paths follow the machine-local path pattern. For each new path field, touch all three places in `GameProfile` impl:

**effective_profile()** — local override wins if non-empty:

```rust
if !self.local_override.game.custom_cover_art_path.trim().is_empty() {
    merged.game.custom_cover_art_path = self.local_override.game.custom_cover_art_path.clone();
}
// Add for portrait:
if !self.local_override.game.custom_portrait_art_path.trim().is_empty() {
    merged.game.custom_portrait_art_path = self.local_override.game.custom_portrait_art_path.clone();
}
```

**storage_profile()** — move all path fields to local_override, clear base:

```rust
storage.local_override.game.custom_cover_art_path = effective.game.custom_cover_art_path.clone();
storage.game.custom_cover_art_path.clear();
// Add for portrait and background — same pattern
```

**portable_profile()** — calls `storage_profile()` then resets all of `local_override`:

```rust
pub fn portable_profile(&self) -> Self {
    let mut portable = self.storage_profile();
    portable.local_override = LocalOverrideSection::default();  // clears everything including new art paths
    portable
}
```

`portable_profile()` requires no changes as long as new fields are in `local_override` — the `Default::default()` reset covers them automatically.

---

### Pattern 4: Generalizing `import_custom_cover_art` to `import_custom_art`

Current function (`import.rs:32`):

- Reads bytes, validates (magic + size), SHA-256 hashes, writes to `media/covers/{hash[..16]}.{ext}`
- Idempotent: skips write if content-addressed file exists
- Returns absolute path string

Generalization: add `art_type: GameImageType` parameter, route to type-segregated subdirectory:

```rust
// New signature
pub fn import_custom_art(source_path: &str, art_type: GameImageType) -> Result<String, String> {
    // ...same validation logic...
    let subdir = match art_type {
        GameImageType::Cover => "covers",
        GameImageType::Portrait => "portraits",
        GameImageType::Background => "backgrounds",
        _ => "covers",  // Hero/Capsule not imported by users
    };
    let dest_dir = media_base_dir()?.join(subdir);
    // ...same hash + write logic...
}

// Keep backward-compat wrapper
pub fn import_custom_cover_art(source_path: &str) -> Result<String, String> {
    import_custom_art(source_path, GameImageType::Cover)
}
```

`is_in_managed_media_dir` already checks against `media_base_dir()` root — works for all subdirectories without change.

---

### Pattern 5: ProfileSummary DTO

The IPC DTO uses `#[serde(rename_all = "camelCase")]` — all Rust `snake_case` fields become camelCase in TS:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub name: String,
    pub game_name: String,
    pub steam_app_id: String,           // → steamAppId in TS
    pub custom_cover_art_path: Option<String>,  // → customCoverArtPath in TS
    // Add: pub custom_portrait_art_path: Option<String>, // → customPortraitArtPath
}
```

`profile_list_summaries` currently reads `effective.steam.app_id`. After the feature, it must call `resolve_art_app_id()` which returns `steam.app_id || runtime.steam_app_id`.

---

### Pattern 6: API Key Security at IPC Boundary

`settings_load` currently returns `AppSettingsData` directly, which includes `steamgriddb_api_key: Option<String>`. Security finding S-02 requires filtering it before returning. Pattern: create a new IPC-safe DTO:

```rust
// Proposed new IPC DTO in settings.rs command
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsIpcData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,
    pub offline_mode: bool,
    pub has_steamgriddb_api_key: bool,  // Never send the key itself
}
```

---

### Pattern 7: Frontend Art Hook

`useGameCoverArt(steamAppId, customArtPath, imageType)` at `hooks/useGameCoverArt.ts:13`:

- Custom path takes priority — if `customUrl` resolves, IPC fetch is skipped entirely
- Race protection via `requestIdRef` — stale responses are discarded
- `convertFileSrc` converts absolute paths for Tauri's asset protocol

For tri-art, consumers call the hook three times with different `imageType` and `customArtPath` arguments — the hook itself needs no changes.

---

### Pattern 8: `GameCoverArt.tsx` Null-Gate Bug

At `GameCoverArt.tsx:18`:

```tsx
if (!steamAppId) {
  return null; // BUG: returns null even when customCoverArtPath is set
}
```

Fix: check custom path first:

```tsx
if (!steamAppId && !customCoverArtPath?.trim()) {
  return null;
}
```

The hook already handles `steamAppId = undefined` gracefully (skips IPC, falls back to custom path), so the fix is in the guard condition only.

---

### Pattern 9: `sanitize_profile_for_community_export`

At `exchange.rs:257`:

```rust
fn sanitize_profile_for_community_export(profile: &GameProfile) -> GameProfile {
    let mut out = profile.portable_profile();
    out.injection.dll_paths.clear();
    out.steam.launcher.icon_path.clear();
    out.runtime.proton_path.clear();
    out.runtime.working_directory.clear();
    out
}
```

`portable_profile()` already calls `storage_profile()` then resets `local_override` — new custom art path fields in `local_override.game` will be cleared automatically. However, the base `game.custom_portrait_art_path` and `game.custom_background_art_path` fields in `storage_profile()` will have been cleared (following the same move-to-local pattern). So new art path fields are covered without explicit additions to this function IF they follow the storage_profile pattern correctly. Verify after implementation.

Note: `runtime.steam_app_id` is portable (media ID only, not machine-local) — it should NOT be cleared by sanitize. This is intentional: it's used for art resolution, not machine configuration.

---

## Integration Points

### Files to Modify

| File                                             | Change                                                                                                                                                                                  | Priority           |
| ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------ |
| `game_images/models.rs`                          | Add `Background` to `GameImageType` enum + `Display` arm                                                                                                                                | Phase 1            |
| `game_images/client.rs`                          | Add `Background` arm to `build_download_url` and `filename_for`                                                                                                                         | Phase 1            |
| `game_images/steamgriddb.rs`                     | Add `Background => ("heroes", None)` arm to `build_endpoint`                                                                                                                            | Phase 1            |
| `game_images/import.rs`                          | Generalize to `import_custom_art(source_path, art_type)`                                                                                                                                | Phase 1            |
| `game_images/mod.rs`                             | Export `import_custom_art` alongside `import_custom_cover_art`                                                                                                                          | Phase 1            |
| `profile/models.rs`                              | Add `steam_app_id` to `RuntimeSection`; add portrait/background fields to `GameSection` and `LocalOverrideGameSection`; update `is_empty()`, `effective_profile()`, `storage_profile()` | Phase 1            |
| `profile/exchange.rs`                            | Verify new custom art paths cleared by existing sanitize logic; add `resolve_art_app_id()` helper                                                                                       | Phase 2            |
| `commands/game_metadata.rs`                      | Add `"background"` arm + explicit default warning; rename IPC command to `import_custom_art`                                                                                            | Phase 1/2          |
| `commands/profile.rs`                            | Update `ProfileSummary` DTO; `profile_list_summaries` uses `resolve_art_app_id()`; `profile_save` auto-imports portrait/background art                                                  | Phase 2            |
| `commands/settings.rs`                           | Filter API key — return `has_steamgriddb_api_key: bool` instead of key string                                                                                                           | Phase 1 (security) |
| `src-tauri/src/lib.rs`                           | Register new `import_custom_art` IPC command if renamed                                                                                                                                 | Phase 2            |
| `types/profile.ts`                               | Add `runtime.steam_app_id`, `game.custom_portrait_art_path`, `game.custom_background_art_path`; mirror in `local_override.game`                                                         | Phase 2            |
| `types/library.ts`                               | Add `customPortraitArtPath?: string` to `LibraryCardData`                                                                                                                               | Phase 3            |
| `components/profile-sections/GameCoverArt.tsx`   | Fix null-gate bug                                                                                                                                                                       | Phase 1            |
| `components/profile-sections/MediaSection.tsx`   | Expand to three art slots                                                                                                                                                               | Phase 3            |
| `components/profile-sections/RuntimeSection.tsx` | Add `steam_app_id` field to `proton_run` section bound to `runtime.steam_app_id`                                                                                                        | Phase 2            |

### New Helper to Create

`resolve_art_app_id(profile: &GameProfile) -> &str` — pure function, lives in `crosshook-core`:

```rust
pub fn resolve_art_app_id(profile: &GameProfile) -> &str {
    let steam_id = profile.steam.app_id.trim();
    if !steam_id.is_empty() {
        return steam_id;
    }
    profile.runtime.steam_app_id.trim()
}
```

Called in `profile_list_summaries` and anywhere `steam.app_id` is currently used for art lookup.

---

## Code Conventions

### Rust

- **TOML field naming**: `#[serde(rename = "field_name", default, skip_serializing_if = "String::is_empty")]` for optional String fields, `#[serde(default, skip_serializing_if = "…::is_empty")]` for sections.
- **Error types**: Custom enum with `Display` + `From<io::Error>` + `From<reqwest::Error>`. Non-fatal errors use `tracing::warn!` and return `Ok(None)` or `Ok(Some(fallback))`. Fatal config/IO errors return `Err(String)` at the IPC boundary.
- **Struct naming**: Sections are `FooSection`, overrides are `LocalOverrideFooSection`.
- **Module structure**: `pub mod` in `mod.rs`, `pub use` for the public surface.
- **Tests**: `#[cfg(test)] mod tests` in-file, `MetadataStore::open_in_memory()` for DB tests, `tempfile::tempdir()` for filesystem tests.

### TypeScript / React

- **IPC wrappers**: Business-logic hooks wrap `invoke()`. Direct `invoke` in components only for simple fire-and-forget calls (see `MediaSection.tsx:35`).
- **Profile update pattern**: `onUpdateProfile((current) => ({ ...current, section: { ...current.section, field: value } }))` — always spread entire section.
- **Optional fields with `?`**: TS interfaces use `field?: string` for fields that may be absent (matching Rust `skip_serializing_if`).
- **camelCase mapping**: All IPC DTOs with `#[serde(rename_all = "camelCase")]` map to camelCase TS fields. Profile section fields use `snake_case` directly (no rename_all on `GameProfile`).

---

## Dependencies and Services

### State Handles (Tauri managed state)

- `State<'_, ProfileStore>` — profile TOML CRUD
- `State<'_, MetadataStore>` — SQLite metadata (game_image_cache, profiles, config_revisions, etc.)
- `State<'_, SettingsStore>` — settings TOML (API key lives here)

### Key Functions Used by the Feature

- `download_and_cache_image(store, app_id, image_type, api_key)` — full pipeline, returns `Ok(Some(path))`
- `import_custom_cover_art(source_path)` — to be generalized to `import_custom_art(source_path, art_type)`
- `is_in_managed_media_dir(path)` — guard in `profile_save` to skip re-import of already-managed paths
- `validate_image_bytes(bytes)` — shared between download pipeline and import; already used by both
- `profile.effective_profile()` — called before reading any path for display
- `profile.storage_profile()` — called before saving to disk

### HTTP Client

- Singleton `OnceLock<reqwest::Client>` at `client.rs:22`; 15s timeout; no redirect policy (security S-01/S-06 requires `redirect::Policy::none()` addition)
- SteamGridDB uses Bearer auth; API key is excluded from tracing via `#[tracing::instrument(skip(api_key))]`

---

## Gotchas and Warnings

1. **Four match sites for `GameImageType`**: Compiler enforces exhaustiveness for the enum arms in Rust, but the IPC string dispatch in `game_metadata.rs:27` is a plain `match on &str` with a silent `_ => Cover` default. Adding `"background"` to the string match MUST be done manually — the compiler will not warn about missing it.

2. **`RuntimeSection::is_empty()` only checks three fields**: Current impl at `models.rs:272` checks `prefix_path`, `proton_path`, `working_directory`. Adding `steam_app_id` means `is_empty()` must also check it, or a profile with only `runtime.steam_app_id` set will be skipped in TOML serialization (`skip_serializing_if = "RuntimeSection::is_empty"`).

3. **`LocalOverrideGameSection::is_empty()` must be extended**: Current check is `executable_path.is_empty() && custom_cover_art_path.is_empty()`. New portrait/background fields must be included or the local_override section may be incorrectly suppressed.

4. **`sanitize_profile_for_community_export` coverage**: New art path fields added to `GameSection` (base fields) will be cleared by `storage_profile()` which moves them to `local_override`, which is then reset by `portable_profile()`. BUT: if a field is portable (like `runtime.steam_app_id`), it will survive export — verify this is intentional per the business rules (it should survive; it's a media ID).

5. **`GameCoverArt.tsx` null-gate**: Returns `null` when `steamAppId` is falsy even when `customCoverArtPath` is present. The fix is a one-line guard change, but it affects the profile editor header art visibility for all `proton_run` profiles that have custom art but no `steam.app_id` set.

6. **`profile_save` auto-import scope**: Currently only auto-imports `custom_cover_art_path`. When portrait/background fields are added, the auto-import block must be duplicated for each. Pattern is already clear in the existing block at `profile.rs:279-287`.

7. **`ProfileSummary::steam_app_id` source change**: Currently reads `effective.steam.app_id`. After the feature, it must use `resolve_art_app_id(&effective)`. This is a behavioral change for `proton_run` profiles — the frontend will receive the resolved ID for art fetching. Existing `steam_applaunch` profiles are unaffected (`steam.app_id` is always non-empty for them).

8. **`settings_load` leaks the API key**: `AppSettingsData` is returned directly to the frontend. The key is present in the TOML response payload. The fix (security S-02) requires a new IPC-safe DTO or a dedicated filter function. Must be done before ship.

9. **Portrait CDN fallback chain**: `Portrait` already has special handling in `download_and_cache_image` via `try_portrait_candidates` (three CDN URLs in order). `Background` uses `build_download_url` directly — the Hero CDN URL (`library_hero.jpg`) is a single URL with no fallback chain. This is intentional but should be documented in the match arm comment.

10. **`is_in_managed_media_dir` covers all subdirectories**: The check is a `starts_with(media_base_dir())` — it correctly matches `media/covers/`, `media/portraits/`, and `media/backgrounds/` without any changes.

---

## Task-Specific Guidance

### Adding `GameImageType::Background` (Phase 1 core)

Touch exactly four files in this order to maintain compilation:

1. `game_images/models.rs` — add variant + Display arm (`"background"`)
2. `game_images/steamgriddb.rs` — add `Background => ("heroes", None)` (no dimensions for heroes endpoint)
3. `game_images/client.rs` — add `Background => "https://...library_hero.jpg"` to `build_download_url`, and `Background => "background"` to `filename_for`
4. `commands/game_metadata.rs` — add explicit `"background" => GameImageType::Background` arm and change the silent default to a logged fallback

Run `cargo test -p crosshook-core` after step 3 to catch any missed match sites before touching IPC.

### Adding `steam_app_id` to `RuntimeSection` (Phase 1 core)

- Add `pub steam_app_id: String` with `#[serde(rename = "steam_app_id", default, skip_serializing_if = "String::is_empty")]`
- Update `RuntimeSection::is_empty()` to include `&& self.steam_app_id.trim().is_empty()`
- `LocalOverrideRuntimeSection` does NOT get this field — it's portable, not machine-local
- Do NOT add it to `effective_profile()` or `storage_profile()` merge logic — portable fields are carried as-is
- Add `resolve_art_app_id()` free function in `profile/models.rs` or a new `profile/art.rs`

### Generalizing `import_custom_cover_art` (Phase 1 core)

- The inner logic (validate → hash → write to subdir) is identical; only the subdirectory path changes
- Add `pub fn import_custom_art(source_path: &str, art_type: GameImageType) -> Result<String, String>` with a subdir dispatch match
- Keep `import_custom_cover_art` as a thin wrapper to avoid breaking the existing IPC command and its test
- Export both from `game_images/mod.rs`
- The IPC command `import_custom_cover_art` in `game_metadata.rs` can be extended or a new `import_custom_art` command added; coordinate with the frontend slot changes

### `profile_save` auto-import extension (Phase 2)

The existing pattern at `profile.rs:279-287` is the template. Replicate for portrait and background:

```rust
let portrait = data.game.custom_portrait_art_path.trim().to_string();
if !portrait.is_empty() && !is_in_managed_media_dir(&portrait) {
    match import_custom_art(&portrait, GameImageType::Portrait) {
        Ok(imported) => data.game.custom_portrait_art_path = imported,
        Err(e) => tracing::warn!(...),
    }
}
```

### Frontend `RuntimeSection.tsx` change (Phase 2)

The `proton_run` block at `RuntimeSection.tsx:155-250` has a `Steam App ID` field bound to `profile.steam.app_id`. Rebind to `profile.runtime.steam_app_id`:

```tsx
value={profile.runtime.steam_app_id ?? ''}
onChange={(value) =>
  onUpdateProfile((current) => ({
    ...current,
    runtime: { ...current.runtime, steam_app_id: value },
  }))
}
```

Note the placeholder should change from "Optional for ProtonDB lookup" to "Optional — used for art resolution" to match the new semantic.

### Testing

- New `GameImageType::Background` arms: add unit tests to `steamgriddb.rs` (`build_endpoint_background_uses_heroes`) and `client.rs` (`filename_for_background_type`) following the existing test pattern
- New profile fields: extend `storage_profile_moves_machine_paths_to_local_override` test in `models.rs` — add portrait/background paths, verify they move to local_override; verify `runtime.steam_app_id` does NOT move
- `resolve_art_app_id`: unit tests for both priority cases (`steam.app_id` wins, fallback to `runtime.steam_app_id`)
- `import_custom_art`: test that `portraits/` and `backgrounds/` subdirectories are created correctly (follow `is_in_managed_media_dir_detects_managed_paths` pattern)
