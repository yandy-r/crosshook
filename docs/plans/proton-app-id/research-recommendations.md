# Proton App ID & Tri-Art System: Recommendations & Risk Assessment

## Executive Summary

The proton-app-id feature has a surprisingly low barrier to entry because much of the infrastructure already exists. The `proton_run` RuntimeSection UI already writes to `profile.steam.app_id`, `profile_list_summaries` already reads it for Library art, and the `useGameCoverArt` hook already accepts an `imageType` parameter. The primary work is: (1) formalizing where the proton_run app_id lives in the data model, (2) extending custom art from cover-only to tri-art, and (3) adding a `Background` image type with corresponding API mappings. A phased approach is strongly recommended, with proton app_id and portrait/cover art normalization as Phase 1, tri-art custom upload as Phase 2, and background art as Phase 3.

The team has a key architectural decision to make on field placement (section 1.1) where research and architecture perspectives diverge. Both options are viable; the trade-off is semantic clarity vs. implementation simplicity. Business analysis has identified launch pipeline contamination as the highest-severity risk if the field placement is handled incorrectly.

---

## 1. Implementation Recommendations

### 1.1 App ID Field Placement: Two Viable Approaches

This is the most consequential architectural decision for Phase 1. The research and architecture roles have differing recommendations. Business analysis has flagged additional risks that inform both options.

**Option A: New `runtime.steam_app_id` field** (Architecture + Business recommendation)
- Add a dedicated `steam_app_id: String` field to `RuntimeSection` in `profile/models.rs`
- Update `RuntimeSection::is_empty()` to include the new field
- Update `effective_profile()`, `storage_profile()`, `portable_profile()` to handle it
- Update `LocalOverrideRuntimeSection` for portability
- Frontend: RuntimeSection.tsx proton_run path writes to `runtime.steam_app_id` instead of `steam.app_id`
- `profile_list_summaries` resolves effective app_id as `steam.app_id || runtime.steam_app_id`

*Pros*: Clean semantic separation -- `steam.*` is for steam_applaunch, `runtime.*` is for proton_run. Avoids conflating launch-critical fields with metadata-only fields. Follows issue #142's original design. Eliminates launch pipeline contamination risk (see section 3.5).

*Cons*: Requires model changes, Serde annotations, `is_empty()` update, effective/storage/portable profile propagation, local override additions, frontend rewiring. More code to review and test. **Migration concern**: the proton_run section of `RuntimeSection.tsx` already binds its "Steam App ID" field to `steam.app_id` -- changing this binding for existing profiles needs careful handling to avoid silent data loss (see section 3.5, risk #3).

**Option B: Reuse existing `steam.app_id`** (Research recommendation)
- Zero backend model changes for Phase 1
- The `proton_run` RuntimeSection UI already writes to `profile.steam.app_id` (`RuntimeSection.tsx:183-191`)
- `profile_list_summaries` (`profile.rs:253`) already reads `effective.steam.app_id`
- `useGameCoverArt` already uses whatever `steamAppId` is passed

*Pros*: Extremely low implementation cost. Already wired end-to-end. The existing UI label already says "Steam App ID" in the proton_run section. No migration needed.

*Cons*: Semantic confusion -- `proton_run` profiles would store metadata-only data in a `steam.*` section that implies launch behavior. Risk that future launch logic changes could accidentally read `steam.app_id` on proton_run profiles and cause silent failures (see section 3.5, risk #1).

**Resolution approach**: Regardless of which option is chosen:
1. A frontend utility function `resolveArtAppId(profile)` should centralize effective app_id resolution for art lookup: `steam.app_id || runtime.steam_app_id || null`. This insulates the art pipeline from the field placement decision.
2. The backend `profile_list_summaries` should compute and return a single `effective_art_app_id` field rather than leaving resolution to the frontend. This prevents inconsistency if resolution rules change later (Business recommendation).
3. If Option A is chosen, a test must assert that launch request construction ignores `runtime.steam_app_id` (Business recommendation).

### 1.2 Phasing Strategy

**Phase 1: Proton App ID + Art Normalization** (Low complexity)
- Implement the chosen app_id field placement (section 1.1)
- Validate that proton_run profiles with app_id trigger art download end-to-end
- Ensure `GameCoverArt` and `GameMetadataBar` render correctly for `proton_run` profiles
- Fix Library card to show portrait art for proton_run games that have an app_id
- Add frontend validation (numeric-only) on the proton_run App ID field
- Wire up `GameMetadataBar` for proton_run profiles (game name, genres from Steam API)

**Phase 2: Tri-Art Custom Upload** (Medium complexity)
- Extend `GameSection` with `custom_portrait_art_path` and `custom_background_art_path`
- Generalize `import_custom_cover_art` to `import_custom_art(source_path, art_type)`
- Add Tauri commands for importing portrait and background art
- Update profile editor UI with art upload controls for each type
- Implement art resolution chain: custom -> auto-downloaded -> placeholder

**Phase 3: Background Art Infrastructure** (Medium complexity)
- Add `GameImageType::Background` variant
- Map to SteamGridDB `/heroes/` endpoint and Steam CDN `library_hero.jpg`
- Build UI consumer for background art (profile detail page, launch page backdrop)
- Consider `GameImageType::Hero` already exists and may serve this purpose
- **Note**: Defer background art UI upload to after a display surface exists. Adding the data model slot (empty-skipped in TOML) is safe, but adding it to the Media upload UI with no consumer creates user confusion ("Where does this show up?") (Business recommendation).

### 1.3 Quick Wins

1. **Immediate (no code changes)**: Test if proton_run profiles with a manually-entered `steam.app_id` already get art in the Library. The pipeline may already work end-to-end since `profile_list_summaries` reads `effective.steam.app_id`.
2. **Small PR**: Add numeric validation to the proton_run "Steam App ID" field on the frontend (matching the backend's `app_id.chars().all(|c| c.is_ascii_digit())` check).
3. **Small PR**: Update `GameCoverArt` component to not return `null` when `steamAppId` is missing but `customCoverArtPath` exists (currently gated on `steamAppId` at line 19).

### 1.4 Technology Choices

- **No new dependencies needed**: `reqwest`, `infer`, `chrono`, `rusqlite` already handle HTTP, image validation, caching, and storage.
- **No new Tauri plugins needed**: File dialog, IPC, and asset protocol are already in use.
- **No DB schema changes needed**: `game_image_cache` already supports multiple image types per app_id. New TOML fields use Serde `#[serde(default)]` -- zero migration risk.
- **No SQLite migration needed for Phase 2**: Custom art paths live in TOML, not SQLite. The `game_image_cache` table already has a generic `image_type` TEXT column.

---

## 2. Improvement Ideas

### 2.1 Related Features That Could Benefit

- **SteamGridDB Search/Browse**: Let users search SteamGridDB by game name and pick from multiple art options rather than auto-selecting the first result. The current `fetch_steamgriddb_image` only takes the first item from the response (`items.into_iter().next()`) -- extending this to return multiple candidates would enable a picker UI.
- **Batch Art Download**: On first launch or after adding SteamGridDB API key, offer to download art for all profiles that have a steam app_id. The `profile_list_summaries` already enumerates all profiles with their app_ids.
- **Art Refresh/Re-download**: Add a "Refresh Art" button per profile that forces cache invalidation (the `useGameMetadata` hook already has a `refresh` callback with `forceRefresh: true`). A similar pattern could be added to `useGameCoverArt`.
- **Art Export**: Allow exporting art assets for backup or sharing. The content-addressed storage in `~/.local/share/crosshook/media/` makes this straightforward.

### 2.2 Background Art Usage Ideas

- **Profile Detail Backdrop**: Use background/hero art as a blurred backdrop behind the profile editor form.
- **Launch Page Hero**: Display background art on the launch confirmation screen.
- **Library Card Hover**: On hover/focus, fade from portrait to background art for a dynamic effect.
- **Steam Deck Game Mode**: Full-screen background art when launching from gamepad-focused mode.

### 2.3 Art System Enhancements

- **Dominant Color Extraction**: `useImageDominantColor.ts` hook already exists. Use it to tint UI chrome based on the game's art palette.
- **Art Preloading**: The Library already uses IntersectionObserver for lazy loading. Consider prefetching art for off-screen cards in a low-priority queue.
- **Offline Art Persistence**: The offline readiness system (`offline_readiness_snapshots`) could include art availability as a readiness factor, ensuring games have cached art for offline use.

---

## 3. Risk Assessment

### 3.1 Technical Risks

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| SteamGridDB API availability/rate limits | Medium | Medium | Existing fallback chain (SteamGridDB -> Steam CDN -> stale cache -> None) handles this gracefully |
| Image format edge cases (animated WebP, truncated downloads) | Low | Low | Magic-byte validation via `infer` crate + size limit already robust; `validate_image_bytes` rejects unknown formats |
| Large art collections (100+ profiles) | Medium | Low | IntersectionObserver lazy loading already in place; content-addressed dedup prevents duplicate files; 24h TTL prevents unbounded growth |
| Steam CDN URL format changes | Low | Low | CDN URLs are well-established and rarely change; SteamGridDB as fallback provides redundancy |
| TOML profile backward compatibility | Low | High (will happen) | Serde `#[serde(default)]` on new fields ensures old profiles parse correctly; no data migration needed |
| Portrait art missing for some games on Steam CDN | Medium | Medium | `portrait_candidate_urls` already has 3-URL fallback chain; SteamGridDB fills gaps |
| Proton_run profiles without app_id show no art | Low | High (by design) | Already the case; no regression. Placeholder/initials fallback in LibraryCard handles this |
| Field placement refactoring (if Option A chosen) | Low | Medium | Well-understood pattern; `effective_profile()` / `storage_profile()` already handle 7 similar fields |

### 3.2 Integration Challenges

- **Profile Editor UI Complexity**: Adding three art upload controls (cover, portrait, background) to the profile form increases visual density. The `GameCoverArt` component currently renders a single image. Tri-art will need a more sophisticated art management section, possibly a collapsible panel.
- **Library Page Performance**: Each LibraryCard already makes a `fetch_game_cover_art` invoke call. Adding a second art type per card (e.g., portrait + cover) would double the IPC calls. Recommendation: Library cards should use only portrait, profile editor uses cover. Don't fetch multiple types simultaneously unless the UI displays them.
- **profile_list_summaries Resolution**: This command loads every profile from disk to extract summaries. It should compute the effective art app_id on the backend (`steam.app_id || runtime.steam_app_id`) and return it as a single `effective_art_app_id` field. This keeps the frontend simple and prevents resolution inconsistency (Business recommendation).
- **Custom Art Path in Local Override**: The `effective_profile()` method already handles `custom_cover_art_path` via `LocalOverrideGameSection`. New custom art fields need the same treatment for portability. **Critical**: If `custom_portrait_art_path` or `custom_background_art_path` are accidentally placed in the base profile section instead of `local_override.game`, they will be included in community profile exports, leaking local filesystem paths. The existing `storage_profile()` / `portable_profile()` test patterns must cover all three art type fields (Business recommendation).
- **Art Resolution Across Launch Methods**: A frontend helper `resolveArtAppId(profile)` should centralize the logic for determining which app_id to use for art lookup. However, the backend `profile_list_summaries` should also perform this resolution, so both layers agree on the effective app_id.

### 3.3 Performance Risks

- **Storage Growth**: Each game image is ~50KB-500KB. With 3 art types per game and 2 sources (CDN + SteamGridDB), worst case is ~3MB per game. 100 games = ~300MB. Acceptable for desktop but worth monitoring.
- **24h Cache TTL**: Current TTL means art is re-downloaded daily. For custom art (imported, not downloaded), TTL should be infinite (NULL expires_at). The existing eviction logic already skips NULL expires_at rows.
- **Startup Art Loading**: Library page loads all profile summaries on mount. Art loading is deferred via IntersectionObserver. No startup performance regression expected.

### 3.4 Security Considerations

- **App ID Validation**: Robust on backend (`app_id.chars().all(|c| c.is_ascii_digit())`). Frontend should mirror this validation.
- **Path Traversal in Import**: `safe_image_cache_path` validates app_id as pure digits and filename as a single component. Same pattern should apply to art-type subdirectory routing. The generalized `import_custom_art` function must use a closed enum for art type (not a free-form string) to prevent directory traversal.
- **Custom Art Upload**: Content-addressed naming (SHA256 hash) prevents filename injection. The `validate_image_bytes` function rejects non-image content. Both patterns should be preserved in the generalized import function.
- **API Key Handling**: SteamGridDB API key is already redacted in Debug output (`AppSettingsData` Debug impl), passed via Bearer auth, and excluded from tracing spans via `skip(api_key)`. No changes needed.

### 3.5 Business Logic Risks (from domain analysis)

These risks are specific to the business rules and data model decisions. They are ordered by severity.

1. **Launch pipeline contamination** (Severity: HIGH if violated)
   `runtime.steam_app_id` (if Option A) must be strictly media-only. If any launch logic reads it and misinterprets it as the Steam app_id for launch, it could cause silent failures (wrong compatdata path, wrong Proton env vars). **Mitigation**: Add a test that asserts `LaunchRequest` construction ignores `runtime.steam_app_id`. Enforce with code review gate. If Option B is chosen, this risk exists in reverse -- future launch logic changes must not accidentally consume `steam.app_id` on proton_run profiles for launch behavior.

2. **ProfileSummary resolution inconsistency** (Severity: MEDIUM)
   The Library page currently passes `steam.app_id` directly to `useGameCoverArt`. If the effective app_id resolution is done solely on the frontend, there is risk of inconsistency when resolution rules change. **Mitigation**: Backend `profile_list_summaries` should compute and return a single `effective_art_app_id` field. Frontend uses it directly without re-resolving.

3. **Existing proton_run UI binding migration** (Severity: MEDIUM, Option A only)
   `RuntimeSection.tsx` already renders a "Steam App ID" field under `proton_run` bound to `profile.steam.app_id` (used for ProtonDB lookup). If Option A is chosen, changing this binding to `runtime.steam_app_id` for existing profiles could silently orphan the value previously stored in `steam.app_id`. **Mitigation**: Either (a) read from both fields during a transition period with `steam.app_id` taking precedence, or (b) add a one-time data migration that copies `steam.app_id` to `runtime.steam_app_id` for proton_run profiles, or (c) leave the ProtonDB lookup reading `steam.app_id` while art lookup reads `runtime.steam_app_id` and document the distinction.

4. **Custom art portability misclassification** (Severity: LOW-MEDIUM)
   If `custom_portrait_art_path` or `custom_background_art_path` are placed in the base `GameSection` instead of being routed through `local_override.game`, community profile exports will leak local filesystem paths. **Mitigation**: The existing `storage_profile()` / `portable_profile()` test pattern must be extended to cover all three art type fields. Add explicit test assertions.

5. **Background art scope creep** (Severity: LOW)
   Adding a background art upload slot to the UI without a display surface creates user confusion. **Mitigation**: Include the background field in the data model (skip-serialized when empty), but defer adding it to the upload UI until a display surface exists in Phase 3.

---

## 4. Alternative Approaches

### 4.1 App ID Field Placement

| Approach | Pros | Cons | Effort |
|----------|------|------|--------|
| **A: New `runtime.steam_app_id`** | Clean semantic separation; follows issue #142 design; `steam.*` stays launch-only; eliminates launch contamination risk | Model changes, is_empty() update, effective/storage/portable profile propagation, frontend rewiring; UI binding migration needed for existing proton_run profiles | Medium |
| **B: Reuse `steam.app_id`** | Zero backend changes; already wired end-to-end; immediate testing possible; no migration risk | Semantic confusion; `steam.app_id` on proton_run implies launch behavior it doesn't have; future launch logic changes could accidentally consume it | Very Low |
| **C: New top-level `metadata` section** | Future-proof for non-Steam metadata | Over-engineered; requires extensive refactoring | High |

**Team perspective**: Architecture and Business recommend Option A for semantic clarity and launch pipeline safety. Research recommends Option B for implementation simplicity and zero migration risk. Both are viable. A `resolveArtAppId(profile)` frontend utility and backend `effective_art_app_id` in ProfileSummary should be created regardless to insulate the art pipeline from this choice. This is a decision for the project owner.

### 4.2 Custom Art Storage Strategy

| Approach | Pros | Cons | Effort |
|----------|------|------|--------|
| **A: Concrete fields per type (recommended)** | Clear, explicit, easy to validate, TOML-friendly | Grows GameSection struct | Low |
| **B: BTreeMap\<String, String\>** | Extensible; single field | Complex TOML syntax; harder to validate; opaque types | Medium |
| **C: External art manifest JSON** | Unlimited art types; no profile bloat | New file format; synchronization complexity | High |
| **D: SQLite-only custom art tracking** | Structured queries; no TOML growth | Art config separated from profile; portability harder | Medium |

**Recommendation**: Option A. Three explicit fields (`custom_cover_art_path`, `custom_portrait_art_path`, `custom_background_art_path`) with Serde defaults. TOML readability matters for a user-editable config format.

### 4.3 Art Resolution Chain Design

| Approach | Pros | Cons | Effort |
|----------|------|------|--------|
| **A: Frontend-driven resolution** | Existing `useGameCoverArt` already implements custom -> auto fallback; simple to extend | Resolution logic split across hooks; risk of inconsistency with backend | Low |
| **B: Backend-driven app_id resolution + frontend art resolution (recommended)** | Backend computes effective_art_app_id in ProfileSummary; frontend handles custom -> auto -> placeholder chain using existing hooks | Slight change to ProfileSummary struct | Low-Medium |
| **C: Fully backend unified resolver** | Single source of truth; consistent behavior | Requires new Tauri command; more complex backend | Medium |

**Recommendation**: Option B (hybrid). The backend resolves the effective art app_id in `profile_list_summaries` and returns it as `effective_art_app_id`. The frontend uses the existing `useGameCoverArt` hook pattern for custom -> auto -> placeholder resolution. This gives the best of both worlds: consistent app_id resolution and reuse of existing frontend art hooks.

### 4.4 Background Art Type Mapping

| Approach | Pros | Cons | Effort |
|----------|------|------|--------|
| **A: New `GameImageType::Background` variant** | Explicit; can diverge from Hero in future | New variant + new CDN/SteamGridDB mappings, though they currently match Hero | Low-Medium |
| **B: Repurpose existing `GameImageType::Hero`** | No new variant; already mapped to `library_hero.jpg` and SteamGridDB `/heroes/` | Semantic overloading; if background needs diverge later, requires refactoring | Very Low |

**Recommendation**: Option A. A dedicated `Background` variant is cleaner and future-proof, even though the initial CDN/SteamGridDB mappings will be identical to `Hero`. The cost of adding a new enum variant is negligible.

---

## 5. Task Breakdown Preview

### Phase 1: Proton App ID + Art Normalization
**Estimated Complexity**: Low (Option B) or Low-Medium (Option A) | **Tasks**: 5-10

- **Task Group 1.1: Data Model** (0-4 tasks depending on field placement choice)
  - If Option A: Add `steam_app_id` to `RuntimeSection`, update `is_empty()`, propagate through effective/storage/portable/local_override
  - If Option A: Handle existing proton_run profile migration (existing `steam.app_id` values)
  - If Option A: Add test asserting `LaunchRequest` construction ignores `runtime.steam_app_id`
  - If Option B: No model changes needed
  - Both options: Update `profile_list_summaries` to return `effective_art_app_id`
  - Both options: Create `resolveArtAppId(profile)` frontend utility

- **Task Group 1.2: Validation & Wiring** (1-2 tasks)
  - Verify proton_run profiles with app_id trigger art download end-to-end
  - Add numeric-only validation to proton_run "Steam App ID" field on frontend

- **Task Group 1.3: Art Display for Proton_Run** (2-3 tasks)
  - Ensure `GameCoverArt` component renders for proton_run profiles with app_id
  - Ensure `GameMetadataBar` shows game name/genres for proton_run profiles
  - Fix `GameCoverArt` to not return null when only `customCoverArtPath` is present (no app_id)

- **Task Group 1.4: Library Integration** (1-2 tasks)
  - Update LibraryCard and useLibrarySummaries to use `effective_art_app_id` from backend
  - Verify Library cards show portrait art for proton_run profiles with app_id

### Phase 2: Tri-Art Custom Upload
**Estimated Complexity**: Medium | **Tasks**: 8-12

- **Task Group 2.1: Backend Art Infrastructure** (4-5 tasks)
  - Add `custom_portrait_art_path` and `custom_background_art_path` to `GameSection` model
  - Propagate through `effective_profile()`, `storage_profile()`, `portable_profile()`, `LocalOverrideGameSection`
  - Add tests asserting all three custom art paths are correctly handled in storage/portable profiles (prevents path leakage in exports)
  - Generalize `import_custom_cover_art` -> `import_custom_art(source_path, art_type)` with type-based subdirectory routing
  - Add `import_custom_art` Tauri command with art type parameter (or separate commands per type)

- **Task Group 2.2: Frontend Art Management** (3-4 tasks)
  - Create art management section in profile editor (cover, portrait upload controls; background deferred to Phase 3)
  - Implement art preview for each type
  - Wire import commands to file dialog
  - Handle art resolution chain in UI (custom -> auto -> placeholder per type)

- **Task Group 2.3: Profile Save Integration** (2-3 tasks)
  - Update `profile_save` command to auto-import custom art for all three types
  - Update `profile_list_summaries` to include custom portrait/background paths
  - Update `LibraryCardData` type to include new art paths

### Phase 3: Background Art Infrastructure
**Estimated Complexity**: Medium | **Tasks**: 6-8
**Dependency**: Phase 2

- **Task Group 3.1: Download Pipeline** (2-3 tasks)
  - Add `GameImageType::Background` variant
  - Map to SteamGridDB `/heroes/` endpoint and Steam CDN `library_hero.jpg`
  - Add `build_download_url` and `build_endpoint` cases for background

- **Task Group 3.2: UI Consumers** (3-4 tasks)
  - Profile detail page backdrop using background art
  - Launch page hero image
  - Library card hover effect (optional)
  - Dominant color integration with background art
  - Add background art upload to profile editor Media section (only after display surface exists)

- **Task Group 3.3: Polish** (1 task)
  - Art refresh button per profile (force cache invalidation for all types)

### Phase 4 (Future): Art Browser & Batch Operations
**Estimated Complexity**: High | **Tasks**: 10+
**Dependency**: Phases 1-3

- SteamGridDB search/browse UI for art selection
- Batch art download for all profiles
- Art export/backup
- Art sharing via community profiles

---

## 6. Key Decisions Needed

1. **Field placement**: New `runtime.steam_app_id` (Option A: semantic clarity, launch safety, medium effort, migration concern) vs. reuse `steam.app_id` (Option B: minimal effort, zero migration, semantic trade-off). See section 1.1 for full analysis. Architecture + Business favor A; Research favors B.
2. **Background art type**: Add new `GameImageType::Background` variant (recommended) or repurpose existing `GameImageType::Hero`? They map to the same Steam CDN asset (`library_hero.jpg`).
3. **Phase 1 scope**: Should Phase 1 include any custom art upload work, or strictly limit to proton app_id + existing art pipeline for proton_run?
4. **Art display locations**: Where does each art type appear? Suggested: cover in profile editor, portrait in Library cards (already the case), background as detail page backdrop (Phase 3).
5. **SteamGridDB key UX**: Should there be an in-app prompt or onboarding step for users to configure their SteamGridDB API key, or remain a settings-only option?
6. **ProfileSummary resolution**: Should effective art app_id be computed on the backend (recommended) or resolved on the frontend? Backend resolution prevents inconsistency.

---

## 7. Open Questions

1. **Does the existing pipeline already work for proton_run?** The code path from LibraryCard -> `useGameCoverArt` -> `fetch_game_cover_art` -> `download_and_cache_image` appears to work for any profile that has a `steamAppId`. If `profile_list_summaries` returns a non-empty `steam_app_id` for proton_run profiles, art download may already be functional. This should be tested before writing any code.

2. **Community profiles and app_id**: Do community profile manifests include a steam app_id field? If so, imported community profiles for proton_run games could automatically get art.

3. **Offline mode interaction**: When `offline_mode` is true, should art download be skipped entirely? The existing `download_and_cache_image` would fail on network requests but fall back to stale cache. Is this sufficient?

4. **Art storage limits**: Should there be a maximum total size for cached art? The current eviction logic only removes expired entries. A size-based eviction policy may be needed for users with 100+ profiles.

5. **Custom art portability**: When a profile is exported (portable_profile), custom art paths become invalid on the receiving machine. Should custom art be embedded in the export, or should the import process re-download art using the app_id?

6. **Existing proton_run profile migration (Option A only)**: How many existing proton_run profiles have values in `steam.app_id`? If significant, what is the migration strategy -- one-time copy on load, explicit migration step, or dual-read transition period?

---

## Appendix: Cross-References

- Business analysis: `docs/plans/proton-app-id/research-business.md`
- Technical specifications: `docs/plans/proton-app-id/research-technical-spec.md` (if produced)
- API research: `docs/plans/proton-app-id/research-api.md` (if produced)
- Security evaluation: `docs/plans/proton-app-id/research-security.md` (if produced)
- UX research: `docs/plans/proton-app-id/research-ux.md` (if produced)
- Engineering practices: `docs/plans/proton-app-id/research-practices.md` (if produced)
