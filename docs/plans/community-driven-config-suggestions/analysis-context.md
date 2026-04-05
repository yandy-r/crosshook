# Context Analysis: community-driven-config-suggestions

## Executive Summary

CrossHook's ProtonDB fetch-and-aggregate backend pipeline is already complete. The remaining work is a layered set of changes: (1) a mandatory security blocklist expansion in `aggregation.rs` that must ship first, (2) a new `suggestions.rs` engine that classifies ProtonDB env vars against the optimization catalog and the profile's current state, (3) three new Tauri commands for get/accept/dismiss, (4) a `suggestion_dismissals` SQLite table (schema v17), and (5) frontend wiring to thread the catalog-match path through the existing Apply flow and dedup the dual Apply paths in `LaunchPage.tsx` / `ProfileFormSections.tsx`.

---

## Architecture Context

- **System Structure**: Business logic lives entirely in `crosshook-core`. `src-tauri/src/commands/` is a thin IPC shim that calls one `crosshook_core` function and maps errors to `String`. The ProtonDB sub-module (`protondb/`) is already split: `client.rs` (HTTP + cache), `aggregation.rs` (parsing + safety filtering), `models.rs` (Serde types), `tests.rs`. The new `suggestions.rs` module adds a classification + comparison layer on top of the existing aggregation output.

- **Data Flow**: ProtonDB servers → `client.rs` (6h TTL cache in `external_cache_entries`) → `aggregation.rs` (`normalize_report_feed` → `ProtonDbRecommendationGroup` with `env_vars`) → `suggestions.rs` (NEW: catalog bridge + profile-state comparison → `ProtonDbSuggestionSet`) → Tauri IPC (Serde JSON) → frontend hook → UI.

- **Integration Points**:
  - **Security gate**: `aggregation.rs:10` `RESERVED_ENV_KEYS` currently has only 3 entries — must be expanded with `LD_*` prefix block before any accept path ships. `CustomEnvironmentVariablesSection.tsx:6` has the same 3-key frontend Set and must be expanded to match.
  - **Catalog bridge**: `launch/catalog.rs::global_catalog()` is the singleton source for `OptimizationEntry` structs; `suggestions.rs` builds an `(env_key, env_value) → catalog_entry_id` index from it at runtime. Multiple entries may share a key with different values — match on full `[key, value]` pair.
  - **Profile write target**: `profile/models.rs::LaunchSection.custom_env_vars` for raw env vars; `LaunchOptimizationsSection.enabled_option_ids` for catalog-matched toggles. Both written via `ProfileStore::save()` then `metadata_store.observe_profile_write()` then `capture_config_revision()` — all three steps required every time.
  - **Config revision tracking**: `capture_config_revision()` is fire-and-forget (logs warning on failure, never propagates). New `ConfigRevisionSource::ProtonDbSuggestionApply` variant needed at `metadata/models.rs:382` — not `config_history_store.rs` as previously stated.
  - **Schema migration**: `metadata/migrations.rs` is currently at v16; the `suggestion_dismissals` table requires v17.
  - **Scroll containers**: any new `overflow-y: auto` container must register in `useScrollEnhance.ts::SCROLLABLE`.

- **Canonical new command pattern**: Follow `profile_apply_bundled_optimization_preset` — accepts profile name + computed diffs, loads profile, applies both `custom_env_vars` patch and `enabled_option_ids` patch atomically, saves, observes, captures revision, returns updated `GameProfile`. Frontend calls `normalizeProfileForEdit(updated, optionsById)` then `setProfile()`.

---

## Critical Files Reference

- `/src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs`: `RESERVED_ENV_KEYS` at line 10 (only 3 entries — S2 fix lands here); `is_safe_env_key()`, `safe_env_var_suggestions()` — visibility must change to `pub(crate)` for accept-time re-validation.
- `/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs`: All IPC-crossing ProtonDB types; new suggestion types extend this.
- `/src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs`: Must add `pub mod suggestions;` and re-export new types.
- `/src/crosshook-native/crates/crosshook-core/src/protondb/suggestions.rs`: NEW — `derive_suggestions()`, catalog bridge, `SuggestionStatus`, `ProtonDbSuggestionSet`.
- `/src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`: `OptimizationEntry` with `env: Vec<[String; 2]>`, `global_catalog()` — source for Tier 1 bridge index.
- `/src/crosshook-native/assets/default_optimization_catalog.toml`: 25 built-in optimization entries; 21 have known ProtonDB env var mappings.
- `/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`: `ConfigRevisionSource` enum at line 382 — add `ProtonDbSuggestionApply` variant here (not `config_history_store.rs`).
- `/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Schema migration point for `suggestion_dismissals` table (v17).
- `/src/crosshook-native/src-tauri/src/commands/protondb.rs`: Extend with 3 new commands: `protondb_get_suggestions`, `protondb_accept_suggestion`, `protondb_dismiss_suggestion`.
- `/src/crosshook-native/src-tauri/src/commands/profile.rs`: Contains `profile_apply_bundled_optimization_preset` — the canonical pattern to follow for the new accept command.
- `/src/crosshook-native/src-tauri/src/lib.rs`: `invoke_handler!` macro — register all 3 new commands here.
- `/src/crosshook-native/src/components/pages/LaunchPage.tsx`: First duplicate Apply path at line 194 (`applyProtonDbGroup`) — writes only to `custom_env_vars`, no catalog-match.
- `/src/crosshook-native/src/components/ProfileFormSections.tsx`: Second duplicate Apply path at line 395 — identical to `LaunchPage.tsx`; does NOT receive `catalog` prop currently, so prop addition required for catalog-matching.
- `/src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx`: Line 6 — frontend `RESERVED_CUSTOM_ENV_KEYS` Set has same 3-entry gap as Rust; must be expanded to match the expanded Rust blocklist.
- `/src/crosshook-native/src/utils/protondb.ts`: `mergeProtonDbEnvVarGroup()` — reuse as-is; do not duplicate.
- `/src/crosshook-native/src/components/ProtonDbOverwriteConfirmation.tsx`: Existing conflict modal — reuse as-is.
- `/src/crosshook-native/src/hooks/useProtonDbLookup.ts`: Pattern to follow for `useProtonDbSuggestions.ts` (requestIdRef race-safety).
- `/src/crosshook-native/src/components/OnboardingWizard.tsx`: Profile creation wizard — integrate `ProtonDbLookupCard` in Step 3, after App ID input, conditioned on `launchMethod === 'steam_applaunch'`.
- `/src/crosshook-native/src/types/protondb.ts`: Extend with TypeScript mirrors of new Rust suggestion types.
- `/src/crosshook-native/src/hooks/useScrollEnhance.ts`: Register any new scroll containers here.

---

## Patterns to Follow

- **Immutable Profile Update**: All profile mutations via `updateProfile((current) => GameProfile)` spread-copy pattern. `src/hooks/useProfile.ts`. Every apply path must use this contract — no direct state mutation.
- **Tauri Command Write Sequence**: `store.save()` → `metadata_store.observe_profile_write()` → `capture_config_revision(source, ...)` — all three steps, every profile-mutating command. Skipping any step breaks metadata sync. Pattern: `profile_apply_bundled_optimization_preset` in `commands/profile.rs`.
- **Race-Safe IPC Hook**: `requestIdRef` counter to discard stale responses. Mirror `useProtonDbLookup.ts` in new `useProtonDbSuggestions.ts`.
- **Thin Tauri Command Layer**: Command receives `State<'_, Store>`, calls one `crosshook_core` fn, maps errors to `String`. `src-tauri/src/commands/protondb.rs`.
- **Pending State for Confirmation Flows**: `useState<PendingType | null>(null)` — non-null renders modal, confirm/cancel resets to null. `ProfileFormSections.tsx`.
- **MetadataStore::open_in_memory() in tests**: Never mock the store; use the in-memory variant. `protondb/tests.rs`.
- **BEM CSS with `crosshook-` prefix**: Variables in `variables.css`, styles in `theme.css`. New UI components must follow `crosshook-<component>__<element>--<modifier>`.
- **Frontend post-save normalization**: After a profile-mutating command returns the updated `GameProfile`, call `normalizeProfileForEdit(updated, optionsById)` then `setProfile()` — mirrors the bundled preset apply flow.

---

## Cross-Cutting Concerns

- **S2 Security (CRITICAL — blocks all other work)**: `RESERVED_ENV_KEYS` at `aggregation.rs:10` currently has only 3 entries. Must be expanded with `LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT`, `LD_DEBUG`, `PATH`, `HOME`, `ZDOTDIR`, `SHELL`, `NODE_OPTIONS`, `PYTHONPATH`, `RUBYLIB`, `PERL5LIB`, plus a `BLOCKED_ENV_KEY_PREFIXES: &[&str] = &["STEAM_COMPAT_", "LD_"]` prefix check. `CustomEnvironmentVariablesSection.tsx:6` frontend Set must be expanded to match. Add `ld_preload_is_rejected_as_env_suggestion` unit test. This must ship before `protondb_accept_suggestion` is exposed.

- **Re-validate at Write Time**: `protondb_accept_suggestion` must re-run `is_safe_env_key()`, `is_safe_env_value()`, and the expanded `RESERVED_ENV_KEYS` check at write time — do not trust the cached suggestion struct.

- **Dual Apply Path Deduplication**: `applyProtonDbGroup` is copy-pasted identically at `LaunchPage.tsx:194` and `ProfileFormSections.tsx:395`. Both only write to `custom_env_vars`. `ProfileFormSections` does not currently receive a `catalog` prop — adding catalog-matching requires a prop addition there. Extract a shared hook (`useProtonDbApply`) that both components consume to prevent a third divergence point.

- **ConfigRevisionSource location correction**: The enum is at `metadata/models.rs:382`, not `config_history_store.rs`. Add `ProtonDbSuggestionApply` variant there. Must land before the accept command ships.

- **ODbL compliance**: The cached `external_cache_entries` payload must never appear in profile exports or community tap distributions. Only user-accepted values written to TOML fields may be exported.

- **XSS (S5)**: All ProtonDB-derived text (notes, source labels, launch strings) must use React plain-text interpolation — never `dangerouslySetInnerHTML`. Audit `ProtonDbLookupCard.tsx` and all components rendering `ProtonDbRecommendationGroup` fields before any new text fields are surfaced.

- **Dismissal scope**: `feature-spec.md` specifies `suggestion_dismissals` as a new SQLite table (schema v17) with 30-day auto-expiry and `ON DELETE CASCADE`. Feature-spec is authoritative — treat the SQLite table as Phase 1 scope.

- **Catalog key collision**: Multiple catalog entries may share the same env key with different values. The matching oracle is the full `[key, value]` pair — match on both, not key alone.

- **Scroll containers**: Any new panel with `overflow-y: auto` must be added to `useScrollEnhance.ts::SCROLLABLE`.

---

## Parallelization Opportunities

1. **S2 security fix** (`aggregation.rs` + `CustomEnvironmentVariablesSection.tsx`) — fully independent, start immediately.
2. **`ConfigRevisionSource::ProtonDbSuggestionApply` enum variant** (`metadata/models.rs:382`) — one-file Rust change, independent of all other work.
3. **`suggestion_dismissals` schema migration** (v17) — independent of the Rust logic in `suggestions.rs`.
4. **TypeScript type definitions** (`src/types/protondb.ts`) — once Rust structs in `suggestions.rs` are defined, frontend and backend can proceed in parallel using the agreed interfaces.
5. **`useProtonDbSuggestions.ts` hook** — writable in parallel with backend commands once TypeScript interfaces exist.
6. **UI deduplication** of `LaunchPage.tsx` and `ProfileFormSections.tsx` Apply paths — pure frontend refactor; independent of suggestion engine but requires adding `catalog` prop to `ProfileFormSections`.

**Sequential dependencies**:

- S2 security fix → `protondb_accept_suggestion` command
- `suggestions.rs` data models agreed → TypeScript interfaces → frontend hook + UI in parallel
- Schema v17 migration → `protondb_dismiss_suggestion` write path

---

## Implementation Constraints

- **No new crate dependencies**: all required crates (`serde`, `serde_json`, `reqwest`, `rusqlite`, `chrono`) are already in `Cargo.toml`.
- **No new external APIs**: only existing ProtonDB endpoints.
- **Catalog bridge is runtime-built**: `(env_key, env_value) → catalog_entry_id` index built from `global_catalog()` at call time — not hardcoded — so user-added catalog entries are automatically included.
- **On-demand derivation only**: `ProtonDbSuggestionSet` computed on each `protondb_get_suggestions` call; no background caching of the derived set.
- **Visibility gate**: ProtonDB panel only shown when `launchMethod ∈ {steam_applaunch, proton_run}` AND `steam.app_id` is non-empty. Step 3 of `OnboardingWizard.tsx` is the correct creation wizard integration point.
- **No auto-apply anywhere**: BR-14 is a hard constraint — no code path may write to profile state without explicit user action.
- **Schema current version is v16**: the migration to v17 adds only `suggestion_dismissals`.
- **`ProfileFormSections` needs `catalog` prop**: catalog-matching in the dedup refactor requires adding this prop; it is currently absent.

---

## Key Recommendations

- **Ship S2 as a standalone PR first** — pure `aggregation.rs` + `CustomEnvironmentVariablesSection.tsx` change with tests; no UI dependency.
- **Agree on Rust struct signatures in `suggestions.rs` before frontend work begins** — TypeScript interfaces are a direct Serde mirror; defining them together avoids a round-trip.
- **Use `profile_apply_bundled_optimization_preset` as the command template** — it already handles the dual write path (both `custom_env_vars` and `enabled_option_ids`) and the full save + observe + revise sequence.
- **Use tagged enum `AcceptSuggestionRequest`** — `{ kind: "catalog" }` / `{ kind: "env_var" }` discriminator enables tier-aware routing in a single command.
- **For the dedup task**: extract the Apply handler into a shared `useProtonDbApply` hook; add the `catalog` prop to `ProfileFormSections` in the same PR to avoid two passes over the same files.
- **Test pattern**: extend `protondb/tests.rs` with `MetadataStore::open_in_memory()` fixtures for the accept and suggest paths; add golden fixture JSON files under `src/protondb/fixtures/` for extraction edge cases.
- **Dismissal expiry**: implement 30-day `expires_at` and evict expired rows on read in the `protondb_get_suggestions` call path; no background cleanup job.

---

## Sources

- `docs/plans/community-driven-config-suggestions/shared.md`
- `docs/plans/community-driven-config-suggestions/feature-spec.md`
- `docs/plans/community-driven-config-suggestions/research-technical.md`
- `docs/plans/community-driven-config-suggestions/research-security.md`
- `docs/plans/community-driven-config-suggestions/research-practices.md`
- `docs/plans/community-driven-config-suggestions/research-business.md`
- `docs/plans/community-driven-config-suggestions/research-ux.md`
- `docs/plans/community-driven-config-suggestions/research-recommendations.md`
- `docs/plans/community-driven-config-suggestions/analysis-code.md` (code-analyzer findings)
