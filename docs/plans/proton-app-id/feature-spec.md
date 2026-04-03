# Feature Spec: Proton App ID & Tri-Art System

## Executive Summary

This feature adds optional Steam App ID support to `proton_run` profiles and extends art management to a tri-art system (cover, portrait, background) with per-type custom upload and mix-and-match resolution. Most download/cache infrastructure already exists in `crosshook-core` — the primary work is adding `steam_app_id` to `RuntimeSection`, extending custom art from cover-only to three types, adding a `Background` image type variant, and building UI surfaces. A 4-phase rollout is recommended. No new dependencies or SQLite migrations are required.

## External Dependencies

### APIs and Services

#### Steam CDN (Primary — No Auth)

- **Documentation**: [Steamworks Library Assets](https://partner.steamgames.com/doc/store/assets/libraryassets)
- **Authentication**: None — all URLs are publicly accessible
- **CDN Base**: `https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/`
- **Key Endpoints (URL patterns)**:
  - `header.jpg` (920x430): Cover art
  - `library_600x900.jpg` / `library_600x900_2x.jpg`: Portrait art (with fallback chain)
  - `library_hero.jpg` (3840x1240): Background art
- **Rate Limits**: None documented; CDN via Cloudflare/Akamai
- **Pricing**: Free

#### SteamGridDB API (Secondary — API Key Required)

- **Documentation**: <https://www.steamgriddb.com/api/v2>
- **Authentication**: `Authorization: Bearer <api_key>` (free account at <https://www.steamgriddb.com/profile/preferences/api>)
- **Key Endpoints**:
  - `GET /api/v2/grids/steam/{app_id}?dimensions=600x900`: Portrait grids
  - `GET /api/v2/grids/steam/{app_id}?dimensions=460x215,920x430`: Cover grids
  - `GET /api/v2/heroes/steam/{app_id}`: Background heroes
  - `GET /api/v2/search/autocomplete/{term}`: Game search (future auto-suggest)
- **Rate Limits**: Not publicly documented; implement 429 backoff
- **Pricing**: Free API key; no paid tiers
- **CDN Domain**: `cdn2.steamgriddb.com` (image delivery)

### Libraries and SDKs

| Library    | Version           | Purpose                                                    | Status          |
| ---------- | ----------------- | ---------------------------------------------------------- | --------------- |
| `reqwest`  | 0.12 (rustls-tls) | HTTP downloads — already in Cargo.toml                     | Active, no CVEs |
| `infer`    | ~0.16             | Magic-byte image validation — already in Cargo.toml        | Active, no CVEs |
| `sha2`     | 0.11              | Content-addressed import filenames — already in Cargo.toml | Active, audited |
| `rusqlite` | (existing)        | game_image_cache DB operations — already in Cargo.toml     | Active          |

No new crate or npm dependencies required.

### External Documentation

- [Steamworks Library Assets Spec](https://partner.steamgames.com/doc/store/assets/libraryassets): Official image dimensions and formats
- [SteamGridDB API v2 Docs](https://www.steamgriddb.com/api/v2): Endpoint reference
- [Steam Web API Terms](https://steamcommunity.com/dev/apiterms): 100k calls/day limit (applies to api.steampowered.com, not CDN)

## Business Requirements

### User Stories

**Primary User: Proton Gamer (umu-run, Heroic, Lutris)**

- As a gamer who runs titles via umu-run, Heroic, or Lutris with a manual Proton prefix, I want to set my game's Steam App ID once so the Library page fills in cover and portrait art automatically — without having to find and upload image files myself.
- As a user importing a community profile with `runtime.steam_app_id` set, I want CrossHook to automatically download art for that game on Library page load.

**Secondary User: Steam Gamer Wanting Custom Art**

- As a Steam gamer whose game has an ugly official portrait card, I want to upload my own portrait image so the Library grid shows my preferred artwork instead of the Steam default.

**Tertiary User: Power User (Full Art Customization)**

- As a power user, I want to override art independently per slot (e.g., custom cover but auto portrait) so I have granular control without filling all three slots.
- As a user who is sometimes offline, I want previously downloaded art to remain visible and missing art to show a placeholder rather than a broken image.

### Business Rules

1. **BR-1: Art source priority chain (per art type)**: For every art type (cover, portrait, background), resolution stops at the first valid result: (1) custom uploaded art path, (2) auto-downloaded art from effective app_id, (3) stale cached art, (4) placeholder/initials.

2. **BR-2: `runtime.steam_app_id` is media-only**: The field must never be read by the launch pipeline. It does not affect how games launch.

3. **BR-3: Art type definitions**:

   | Type       | Usage                 | Steam CDN                      | SteamGridDB                         |
   | ---------- | --------------------- | ------------------------------ | ----------------------------------- |
   | Cover      | Profile editor header | `header.jpg` (920x430)         | `/grids?dimensions=460x215,920x430` |
   | Portrait   | Library grid card     | `library_600x900_2x.jpg`       | `/grids?dimensions=342x482,600x900` |
   | Background | Future hero/banner    | `library_hero.jpg` (3840x1240) | `/heroes`                           |

4. **BR-4: App ID validation**: Pure ASCII decimal integers, 1-12 digits. Empty string = "not set." Validated at profile-save time (not deferred to fetch time).

5. **BR-5: Custom art import is idempotent**: Content-addressed filenames (first 16 hex chars of SHA-256). Re-uploading returns existing path.

6. **BR-6: Custom art paths are machine-local**: Stored in `local_override.game.*`. Cleared from portable profile exports. `runtime.steam_app_id` is portable metadata (stays in base profile).

7. **BR-7: Art cache TTL 24h**: Stale entries served as fallback on download failure. Missing-file entries evicted.

8. **BR-8: Image constraints**: 5 MB max, allowed formats: JPEG, PNG, WebP (magic-byte detection). SVG/HTML unconditionally rejected.

9. **BR-9: Effective media app_id resolution**: `steam.app_id` if non-empty, else `runtime.steam_app_id`. Computed backend-side; frontend receives one resolved value.

10. **BR-10: Per-type independence**: Each art type resolves independently. Custom portrait + auto cover is a valid state requiring no special handling.

11. **BR-11: SteamGridDB API key is opt-in**: Without a key, all art uses Steam CDN exclusively (sufficient for most games). The feature must never require a SteamGridDB key to function.

12. **BR-12: Custom art files not deleted on clear**: Only the profile reference is removed. Content-addressed files may be shared across profiles.

### Edge Cases

| Scenario                                             | Expected Behavior                                                  | Notes                                                          |
| ---------------------------------------------------- | ------------------------------------------------------------------ | -------------------------------------------------------------- |
| Both `steam.app_id` and `runtime.steam_app_id` set   | `steam.app_id` takes priority (BR-9)                               | Expected for existing proton_run profiles; no migration needed |
| proton_run with no app_id                            | Placeholder/initials shown                                         | No download attempted; no regression                           |
| Custom art points to non-existent file               | Falls back to auto-downloaded art                                  | `onError` handler in LibraryCard already implemented           |
| SteamGridDB key not configured                       | Steam CDN only; no error state                                     | Graceful degradation                                           |
| Valid-format but non-existent app_id (e.g., 9999999) | Profile saves successfully; art falls back to placeholder          | No save-time existence check; lazy resolution                  |
| Pre-2019 game missing library_600x900                | Fallback chain: `_2x.jpg` -> `library_600x900.jpg` -> `header.jpg` | Already implemented in `portrait_candidate_urls()`             |

### Success Criteria

- [ ] proton_run profile with `runtime.steam_app_id` shows portrait art in Library grid automatically
- [ ] proton_run profile without app_id shows initials fallback (no regression)
- [ ] Custom art for any slot overrides auto-downloaded art for that slot only
- [ ] Removing custom art reverts to auto-downloaded art
- [ ] `runtime.steam_app_id` survives portable profile export/import
- [ ] Custom art paths excluded from portable export
- [ ] No launch behavior changes for any profile
- [ ] Users without SteamGridDB key still receive Steam CDN art
- [ ] All existing profiles require no migration

## Technical Specifications

### Architecture Overview

```
                      +-----------------------+
                      |   Frontend (React)    |
                      |                       |
                      |  useGameCoverArt()    |
                      |  resolveArtAppId()    |
                      +----------+------------+
                                 |  invoke()
                      +----------v------------+
                      |  Tauri IPC Commands   |
                      |                       |
                      |  fetch_game_cover_art |
                      |  import_custom_art    |
                      +----------+------------+
                                 |
                      +----------v------------+
                      |   crosshook-core      |
                      |                       |
                      |  game_images/         |
                      |    client.rs          |
                      |    import.rs          |
                      |    steamgriddb.rs     |
                      |  profile/models.rs    |
                      |  metadata/ (SQLite)   |
                      +----------+------------+
                                 |
                 +---------------+---------------+
                 |                               |
          +------v------+              +---------v--------+
          | Steam CDN   |              | SteamGridDB API  |
          | (no auth)   |              | (Bearer token)   |
          +-------------+              +------------------+
```

### Data Models

#### `RuntimeSection` — New `steam_app_id` field

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuntimeSection {
    #[serde(rename = "prefix_path", default)]
    pub prefix_path: String,
    #[serde(rename = "proton_path", default)]
    pub proton_path: String,
    #[serde(rename = "working_directory", default)]
    pub working_directory: String,
    /// Optional Steam App ID for media/metadata lookup only.
    /// Does NOT affect launch behavior.
    #[serde(rename = "steam_app_id", default, skip_serializing_if = "String::is_empty")]
    pub steam_app_id: String,
}
```

`is_empty()` must **exclude** `steam_app_id` — a profile with only `steam_app_id` set must still emit the `[runtime]` section.

#### `GameSection` — Per-type custom art paths

```rust
pub struct GameSection {
    // ... existing fields ...
    #[serde(rename = "custom_cover_art_path", default, skip_serializing_if = "String::is_empty")]
    pub custom_cover_art_path: String,
    #[serde(rename = "custom_portrait_art_path", default, skip_serializing_if = "String::is_empty")]
    pub custom_portrait_art_path: String,       // NEW
    #[serde(rename = "custom_background_art_path", default, skip_serializing_if = "String::is_empty")]
    pub custom_background_art_path: String,     // NEW
}
```

Matching fields added to `LocalOverrideGameSection`. Propagated through `effective_profile()`, `storage_profile()`, `portable_profile()`.

#### `GameImageType` — New `Background` variant

```rust
pub enum GameImageType {
    Cover,
    Hero,
    Capsule,
    Portrait,
    Background,  // NEW — maps to library_hero.jpg / heroes endpoint
}
```

#### `resolve_art_app_id` helper

```rust
pub fn resolve_art_app_id(profile: &GameProfile) -> &str {
    let steam = profile.steam.app_id.trim();
    if !steam.is_empty() { return steam; }
    profile.runtime.steam_app_id.trim()
}
```

#### SQLite — No migration needed

The existing `game_image_cache` table uses `image_type TEXT`, which already accepts arbitrary type strings. `"background"` fits naturally alongside `"cover"`, `"portrait"`, `"hero"`.

#### TOML Profile Format

```toml
[runtime]
steam_app_id = "1245620"            # NEW (omitted when empty)

[game]
custom_cover_art_path = ""
custom_portrait_art_path = ""       # NEW (omitted when empty)
custom_background_art_path = ""     # NEW (omitted when empty)
```

Existing profiles remain unchanged via `#[serde(default)]`.

### API Design

#### `import_custom_art` (generalized from `import_custom_cover_art`)

```rust
#[tauri::command]
pub fn import_custom_art(
    source_path: String,
    art_type: Option<String>,  // "cover" | "portrait" | "background"; defaults to "cover"
) -> Result<String, String>
```

Routes to type-segregated subdirectories: `media/covers/`, `media/portraits/`, `media/backgrounds/`. Art type matched against a closed set (no arbitrary strings). Backward-compat wrapper `import_custom_cover_art` preserved.

#### `fetch_game_cover_art` — Add `"background"` arm

Already accepts `image_type: Option<String>`. Add `"background" => GameImageType::Background` to the match.

#### `profile_list_summaries` — Resolve effective app_id

Returns `effective_steam_app_id` computed per BR-9. Frontend receives one resolved value.

### System Integration

#### Files to Modify (~16)

| File                                                   | Change                                                                                                                                                                                                                                    |
| ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/models.rs`          | Add `steam_app_id` to `RuntimeSection`; add per-type custom art paths to `GameSection` and `LocalOverrideGameSection`; update `is_empty()`, `effective_profile()`, `storage_profile()`, `portable_profile()`. Add `resolve_art_app_id()`. |
| `crates/crosshook-core/src/game_images/models.rs`      | Add `Background` to `GameImageType` enum                                                                                                                                                                                                  |
| `crates/crosshook-core/src/game_images/client.rs`      | Add `Background` to `build_download_url()`, `filename_for()`                                                                                                                                                                              |
| `crates/crosshook-core/src/game_images/steamgriddb.rs` | Add `Background` to `build_endpoint()`                                                                                                                                                                                                    |
| `crates/crosshook-core/src/game_images/import.rs`      | Generalize to `import_custom_art(source_path, art_type)`                                                                                                                                                                                  |
| `src-tauri/src/commands/game_metadata.rs`              | Add `"background"` match arm; add `import_custom_art` command                                                                                                                                                                             |
| `src-tauri/src/commands/profile.rs`                    | Update `profile_list_summaries` and `profile_save` for effective app_id and tri-art auto-import                                                                                                                                           |
| `src-tauri/src/lib.rs`                                 | Register `import_custom_art` command                                                                                                                                                                                                      |
| `src/types/profile.ts`                                 | Add `steam_app_id` to runtime; per-type custom art paths to game                                                                                                                                                                          |
| `src/components/profile-sections/RuntimeSection.tsx`   | Rebind proton_run App ID field to `runtime.steam_app_id`                                                                                                                                                                                  |
| `src/components/profile-sections/MediaSection.tsx`     | Expand to three art type fields                                                                                                                                                                                                           |
| `src/components/library/LibraryCard.tsx`               | Use backend-resolved `steamAppId` (minor, if needed)                                                                                                                                                                                      |

#### Files to Create (1)

| File               | Purpose                                                             |
| ------------------ | ------------------------------------------------------------------- |
| `src/utils/art.ts` | `resolveArtAppId()` and `resolveCustomArtPath()` frontend utilities |

#### Configuration

No new environment variables, feature flags, or configuration files.

## UX Considerations

### User Workflows

#### Primary: Adding Steam App ID to a proton_run Profile

1. User opens a `proton_run` profile in the editor
2. "Proton Runtime" section displays a "Steam App ID" text field
3. User types or pastes a numeric App ID (helper link to Steam store search)
4. User saves; backend validates format (digits only, 1-12 chars)
5. On next Library page load, portrait art appears automatically

#### Primary: Uploading Custom Art (Per Type)

1. User opens profile editor, navigates to Media section
2. Three art slots shown: Cover, Portrait, Background — each with Browse/Clear/Preview
3. User clicks Browse for a slot; native file picker opens (filtered to png/jpg/jpeg/webp)
4. Backend validates (magic bytes, 5MB limit), copies to managed media dir
5. Preview thumbnail appears inline; user saves profile

#### Error Recovery

| Error                          | UX Response                                                      |
| ------------------------------ | ---------------------------------------------------------------- |
| Invalid App ID format          | Inline validation on field; save blocked with error message      |
| Art download fails (network)   | Stale cache served; if none, initials placeholder shown silently |
| Corrupt/oversized image upload | Inline error: "Image must be JPEG, PNG, or WebP, under 5 MB"     |
| SteamGridDB 401/403            | Fall back to Steam CDN; surface "API key invalid" in Settings    |

### UI Patterns

| Component            | Pattern                                                  | Notes                                                            |
| -------------------- | -------------------------------------------------------- | ---------------------------------------------------------------- |
| App ID field         | Text input with numeric validation, helper link          | Not a search; plain input for Phase 1                            |
| Art upload slot      | Browse button + clear button + inline thumbnail preview  | Per-type, three slots in Media section                           |
| Library card         | Skeleton -> portrait art -> initials fallback            | Already implemented via `useGameCoverArt` + IntersectionObserver |
| Art source indicator | Small badge showing "Custom" vs "Steam" vs "SteamGridDB" | Nice-to-have; defer to Phase 2+                                  |

### Accessibility Requirements

- Art upload slots: keyboard-navigable, labeled for screen readers
- App ID field: `aria-describedby` linking to helper text
- Library card: alt text derived from game name for all art images

### Performance UX

- **Loading States**: Skeleton loading on Library grid (already implemented)
- **Lazy Loading**: IntersectionObserver triggers art fetch only for visible cards (already implemented)
- **Cache**: 24h TTL prevents redundant CDN hits; stale fallback on failure

## Recommendations

### Implementation Approach

**Recommended Strategy**: 4-phase rollout, from low to high complexity.

**Phasing:**

1. **Phase 1 — Proton App ID + Art Normalization** (Low complexity, 5-9 tasks): Add `runtime.steam_app_id` field, create `resolveArtAppId()` utility, validate proton_run profiles get art end-to-end, add frontend validation. No new art types or custom upload changes.

2. **Phase 2 — Tri-Art Custom Upload** (Medium complexity, 8-12 tasks): Add per-type custom art paths to `GameSection`, generalize `import_custom_cover_art`, build Media section UI with three art slots, wire auto-import at save time.

3. **Phase 3 — Background Art Infrastructure** (Medium complexity, 6-8 tasks): Add `GameImageType::Background`, map to CDN/SteamGridDB, build UI consumers (profile detail backdrop, launch page hero).

4. **Phase 4 — Art Browser & Batch Operations** (High complexity, future): SteamGridDB search/browse picker, batch art download, art refresh/re-download.

### Technology Decisions

| Decision                   | Recommendation                                    | Rationale                                                                                                                                                                   |
| -------------------------- | ------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| App ID field placement     | New `runtime.steam_app_id` (Option A)             | Clean semantic separation; `steam.*` stays launch-specific; matches issue #142 design. `resolveArtAppId()` fallback chain provides natural migration for existing profiles. |
| Custom art field structure | Flat per-type fields                              | Matches existing `custom_cover_art_path` pattern; simple TOML layout; easy `skip_serializing_if`. Three fixed types don't justify a map.                                    |
| Background vs Hero type    | New `GameImageType::Background` variant           | Explicit semantic distinction for UI use, even though CDN/SteamGridDB mappings match Hero initially.                                                                        |
| Art resolution location    | Frontend-driven + backend summary                 | Existing `useGameCoverArt` hook pattern; `profile_list_summaries` resolves effective app_id server-side. No new IPC round-trips.                                            |
| Import generalization      | Parameterized `import_custom_art(path, art_type)` | Reuses all existing validation/hashing logic; type-based subdirectory routing. Closed enum prevents directory traversal.                                                    |

### Quick Wins

- **Test existing pipeline**: proton_run profiles with `steam.app_id` may already get Library art — verify before writing code
- **Add numeric validation**: Frontend App ID field validation matching backend's `chars().all(is_ascii_digit)` check
- **Fix GameCoverArt null gate**: Component returns null when `steamAppId` is missing even if `customCoverArtPath` exists

### Future Enhancements

- **SteamGridDB Browse/Pick**: Let users choose from multiple art options instead of auto-selecting first result
- **Batch Art Download**: On SteamGridDB key setup, offer to download art for all profiles with app_ids
- **Art Refresh**: Per-profile "Refresh Art" button that forces cache invalidation
- **Dominant Color**: Use existing `useImageDominantColor.ts` hook to tint UI chrome from art palette

## Risk Assessment

### Technical Risks

| Risk                                     | Likelihood         | Impact | Mitigation                                                                       |
| ---------------------------------------- | ------------------ | ------ | -------------------------------------------------------------------------------- |
| SteamGridDB API availability/rate limits | Medium             | Medium | Existing fallback chain: SteamGridDB -> Steam CDN -> stale cache -> None         |
| Steam CDN URL format changes             | Low                | Low    | URLs stable since 2019; SteamGridDB provides redundancy                          |
| TOML backward compatibility breakage     | High (will happen) | Low    | Serde `#[serde(default)]` on all new fields; empty-string skip-serialization     |
| Portrait art missing for older games     | Medium             | Medium | 3-URL fallback chain already in `portrait_candidate_urls()`                      |
| Large art collections (100+ profiles)    | Low                | Medium | IntersectionObserver lazy loading; content-addressed dedup; 24h TTL              |
| Field placement refactoring (Option A)   | Medium             | Low    | Well-understood pattern; 7 similar fields already handled in effective_profile() |

### Integration Challenges

- **Profile editor UI density**: Three art upload controls increase visual complexity — consider collapsible Media panel
- **Library page IPC**: Each LibraryCard makes one `fetch_game_cover_art` call — keep it to one art type per card (portrait for grid)
- **Local override propagation**: New custom art fields must be added to `storage_profile()` and `portable_profile()` — omission leaks local paths in exports

### Security Considerations

#### Critical — Hard Stops

| Finding         | Risk | Required Mitigation |
| --------------- | ---- | ------------------- |
| None identified | —    | —                   |

#### Warnings — Must Address

| Finding                                                          | Risk                                                  | Mitigation                                                                                                 | Alternatives                                      |
| ---------------------------------------------------------------- | ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| S-01/S-06: No redirect-policy domain allow-list on HTTP client   | SSRF via compromised SteamGridDB response             | Add `.redirect(Policy::custom(...))` restricting to 4 allowed domains, HTTPS only                          | Disable all redirects (may break SteamGridDB CDN) |
| S-02: `settings_load` IPC returns raw SteamGridDB API key        | Key exposure to frontend                              | Return `has_steamgriddb_api_key: bool` only; accept key only in `settings_save`                            | `#[serde(skip_serializing)]` on key field         |
| S-03: Community export doesn't clear `custom_cover_art_path`     | Local filesystem path disclosure                      | Add `.clear()` for all custom art paths in `sanitize_profile_for_community_export`                         | —                                                 |
| S-05: Unknown `image_type` silently defaults to Cover            | Surprising behavior for callers                       | Return typed error for unrecognized `image_type` values                                                    | —                                                 |
| S-12: HTTP 401/403 from SteamGridDB falls through to stale cache | User can't distinguish expired key from network error | Add `AuthFailure` error variant; fall back to Steam CDN (not stale cache); surface "API key invalid" in UI | —                                                 |

#### Advisories — Best Practices

- S-04: Pixel flood protection — enforce max dimensions (8192x8192) if `image` crate is ever added for decoding (not currently a dependency)
- S-07: Track `infer` crate in `cargo audit` CI
- S-08: Add `cargo audit` to CI pipeline
- S-13: Add 12-digit length cap to `steam_app_id` validation
- S-15: SGDB key in plaintext TOML — acceptable for single-user desktop app; document `chmod 600` for shared systems

## Storage Boundary

| Datum                                   | Storage Type                                                         | Rationale                                            |
| --------------------------------------- | -------------------------------------------------------------------- | ---------------------------------------------------- |
| `runtime.steam_app_id`                  | TOML profile settings (base section, portable)                       | Identifies the game; not machine-specific            |
| `game.custom_cover_art_path`            | TOML (local_override.game, machine-local)                            | Absolute filesystem path; not portable               |
| `game.custom_portrait_art_path` (new)   | TOML (local_override.game, machine-local)                            | Same as cover                                        |
| `game.custom_background_art_path` (new) | TOML (local_override.game, machine-local)                            | Same as cover                                        |
| Downloaded art files                    | Filesystem cache (`~/.local/share/crosshook/cache/images/{app_id}/`) | Re-downloadable, TTL-managed                         |
| Art cache metadata                      | SQLite metadata DB (`game_image_cache` table)                        | Operational cache; re-fetchable; no schema migration |

## Persistence & Usability

- **Migration**: No migration needed. All new fields default to empty string via `#[serde(default)]`; existing profiles unaffected. `game_image_cache` TEXT column accepts new `"background"` type without schema change.
- **Backward compatibility**: Unknown TOML keys ignored by older CrossHook versions. `resolveArtAppId()` fallback chain handles existing proton_run profiles with `steam.app_id` set.
- **Offline**: Previously downloaded art remains available indefinitely. Missing art shows placeholder/initials. SteamGridDB API key absence doesn't block Steam CDN art.
- **User visibility**: `runtime.steam_app_id` editable in profile editor (proton_run only). Custom art paths editable via Browse/Clear in Media section. Art source visible as auto-downloaded vs custom in Library grid.

## Task Breakdown Preview

### Phase 1: Proton App ID + Art Normalization

**Focus**: Wire `runtime.steam_app_id` end-to-end for proton_run profiles
**Tasks**: 5-9 (depending on field placement choice)

- Add `steam_app_id` to `RuntimeSection` + update `is_empty()`
- Create `resolve_art_app_id()` Rust helper and `resolveArtAppId()` frontend utility
- Update `profile_list_summaries` to return effective app_id
- Rebind RuntimeSection.tsx proton_run App ID field to `runtime.steam_app_id`
- Add frontend numeric validation on App ID field
- Verify Library cards show portrait art for proton_run profiles with app_id
- Fix `GameCoverArt` null gate when only customCoverArtPath exists

**Parallelization**: Rust model changes + frontend utility can run in parallel

### Phase 2: Tri-Art Custom Upload

**Focus**: Per-type custom art with mix-and-match
**Dependencies**: Phase 1 complete
**Tasks**: 8-12

- Add `custom_portrait_art_path`, `custom_background_art_path` to `GameSection` + `LocalOverrideGameSection`
- Update `effective_profile()`, `storage_profile()`, `portable_profile()` for new fields
- Generalize `import_custom_cover_art` -> `import_custom_art(source_path, art_type)`
- Add `import_custom_art` Tauri command
- Update `profile_save` to auto-import all three art types
- Build Media section UI with three art slots (Browse/Clear/Preview per type)
- Update `profile_list_summaries` to include `customPortraitArtPath`
- Address S-03: clear all custom art paths in community export sanitization

**Parallelization**: Backend import generalization + frontend Media section can run in parallel

### Phase 3: Background Art Infrastructure

**Focus**: Add Background image type and UI consumers
**Dependencies**: Phase 2 complete
**Tasks**: 6-8

- Add `GameImageType::Background` variant to enum
- Map to Steam CDN `library_hero.jpg` and SteamGridDB `/heroes/` endpoint
- Add `"background"` arm to `fetch_game_cover_art` IPC command
- Build profile detail page backdrop using background art
- Build launch page hero image surface
- Address S-01/S-06: Add redirect-policy domain allow-list to HTTP client
- Address S-02: Sanitize `settings_load` to not return raw API key

### Phase 4: Art Browser & Batch Operations

**Focus**: Enhanced art discovery and management
**Dependencies**: Phases 1-3
**Tasks**: 10+

- SteamGridDB search/browse UI for art selection
- Batch art download for all profiles with app_ids
- Art refresh/re-download per profile
- Art export/backup
- Media garbage collection (orphaned content-addressed files)

## Decisions (Resolved)

1. **Field placement**: **Option A — New `runtime.steam_app_id`** field for clean semantic separation and future-proofing for additional backends. `resolveArtAppId()` fallback chain provides migration for existing profiles.

2. **Background art UI timing**: **Option A — Ship data model fields in Phase 2, UI in Phase 3.** Exercises the data model early with no UI cost (fields serialize as empty).

3. **Phase 1 scope**: **Option A — Strictly proton app_id + existing pipeline.** Ship as standalone improvement, then iterate through subsequent phases.

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Steam CDN and SteamGridDB API details, existing codebase implementation state
- [research-business.md](./research-business.md): User stories, business rules, workflows, domain model
- [research-technical.md](./research-technical.md): Data models, API design, codebase changes, technical decisions
- [research-ux.md](./research-ux.md): User workflows, competitive analysis, error handling UX
- [research-security.md](./research-security.md): Security findings by severity, secure coding guidelines
- [research-practices.md](./research-practices.md): Code reuse, modularity, KISS assessment, build-vs-depend
- [research-recommendations.md](./research-recommendations.md): Phasing strategy, risk assessment, alternative approaches
