# Community-Driven Configuration Suggestions Implementation Plan

CrossHook's ProtonDB backend pipeline is complete — reports are fetched, cached with 6h TTL, env vars extracted and frequency-ranked, and results exposed through `protondb_lookup`. The remaining work is a layered set of changes across two hard-sequential phases: Phase 1 (security + foundation) expands the `RESERVED_ENV_KEYS` blocklist to close the S2 security gap, adds a `ConfigRevisionSource` variant for audit tracking, and defines TypeScript type interfaces — all independent and parallelizable. Phase 2 (feature delivery) builds the `suggestions.rs` catalog-bridge engine, adds a `suggestion_dismissals` schema migration, wires 3 new Tauri commands, creates a frontend hook, and integrates the apply/dismiss flow into the existing `ProtonDbLookupCard` with unit tests. The critical path is T1.1→T2.1→T2.3→T2.4→T2.5, with significant parallel fan-out at each phase boundary.

## Critically Relevant Files and Documentation

- src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs: `RESERVED_ENV_KEYS` blocklist (3 entries — must expand), `safe_env_var_suggestions()`, `is_safe_env_key()`, `is_safe_env_value()`
- src/crosshook-native/crates/crosshook-core/src/protondb/models.rs: All ProtonDB IPC-crossing types (Serde-derived structs)
- src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs: Public module re-exports — must add `pub mod suggestions`
- src/crosshook-native/crates/crosshook-core/src/protondb/tests.rs: Existing unit tests for aggregation/cache — extend here
- src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs: `OptimizationCatalog`, `OptimizationEntry` with `env: Vec<[String; 2]>`, `global_catalog()` singleton
- src/crosshook-native/crates/crosshook-core/src/launch/env.rs: `RESERVED_ENV_KEYS`, `BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS`, `WINE_ENV_VARS_TO_CLEAR`
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile` with `launch.custom_env_vars` and `launch.optimizations.enabled_option_ids`
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs: `ConfigRevisionSource` enum (add `ProtonDbSuggestionApply` variant)
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Schema at v16 — add v17 for `suggestion_dismissals`
- src/crosshook-native/src-tauri/src/commands/protondb.rs: Existing `protondb_lookup` command — extend with 3 new commands
- src/crosshook-native/src-tauri/src/commands/profile.rs: `profile_apply_bundled_optimization_preset` — canonical pattern for the new accept command
- src/crosshook-native/src-tauri/src/lib.rs: `invoke_handler!` macro — register new commands here
- src/crosshook-native/src/types/protondb.ts: TypeScript mirror of Rust ProtonDB types — extend with suggestion types
- src/crosshook-native/src/hooks/useProtonDbLookup.ts: Race-safe IPC hook pattern to follow for new hook
- src/crosshook-native/src/utils/protondb.ts: `mergeProtonDbEnvVarGroup()` — extend with catalog-aware apply utility
- src/crosshook-native/src/components/ProtonDbLookupCard.tsx: Main display component — wire apply/dismiss actions
- src/crosshook-native/src/components/ProtonDbOverwriteConfirmation.tsx: Existing conflict resolution modal — reuse as-is
- src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx: Frontend `RESERVED_CUSTOM_ENV_KEYS` Set — must expand to match Rust blocklist
- src/crosshook-native/src/components/pages/LaunchPage.tsx: First Apply path — will consume new utility
- src/crosshook-native/src/components/ProfileFormSections.tsx: Second Apply path — will consume new utility, needs `catalog` prop
- src/crosshook-native/src/components/pages/InstallPage.tsx: Imports `ProfileFormSections` — must pass new `catalog` prop when added
- docs/plans/community-driven-config-suggestions/feature-spec.md: Authoritative feature spec — all business rules, data models, API design
- docs/plans/community-driven-config-suggestions/research-security.md: S2 critical finding — blocklist expansion details
- docs/plans/community-driven-config-suggestions/research-technical.md: Rust struct definitions, Tauri command signatures, three-tier architecture
- AGENTS.md: Architecture rules, IPC conventions, scroll container requirements

## Implementation Plan

### Phase 1: Security and Foundation

#### Task 1.1: Expand RESERVED_ENV_KEYS blocklist (S2 security fix) Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs
- docs/plans/community-driven-config-suggestions/research-security.md
- src/crosshook-native/crates/crosshook-core/src/launch/env.rs (reference only — contains `WINE_ENV_VARS_TO_CLEAR` and `BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS` for context on dangerous keys, but the blocklist to modify is in `aggregation.rs`)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs

This is the highest-priority task and a hard security prerequisite for the entire feature.

Expand `RESERVED_ENV_KEYS` constant at line 10-14 of `aggregation.rs` to add: `LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT`, `LD_DEBUG`, `PATH`, `HOME`, `SHELL`, `NODE_OPTIONS`, `PYTHONPATH`.

Add a `BLOCKED_ENV_KEY_PREFIXES` constant: `&["STEAM_COMPAT_", "LD_"]`.

Update the guard in `safe_env_var_suggestions()` to check both exact matches and prefix matches:

```rust
if RESERVED_ENV_KEYS.contains(&normalized_key.as_str())
    || BLOCKED_ENV_KEY_PREFIXES.iter().any(|p| normalized_key.starts_with(p))
{
    continue;
}
```

This replaces the existing `starts_with("STEAM_COMPAT_")` check with a generalized prefix array.

Add unit tests in `protondb/tests.rs`:

- `ld_preload_is_rejected_as_env_suggestion` — feed containing `LD_PRELOAD=/evil.so` must not appear in output
- `path_is_rejected_as_env_suggestion` — `PATH=/usr/bin` must be filtered
- `ld_prefix_keys_are_rejected` — `LD_LIBRARY_PATH=/tmp`, `LD_AUDIT=foo` both filtered via prefix check

Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` to verify all existing and new tests pass.

#### Task 1.2: Add ConfigRevisionSource::ProtonDbSuggestionApply variant Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs

Single-file change. The `ConfigRevisionSource` enum is at approximately line 382. Add a new variant `ProtonDbSuggestionApply`.

Update the `as_str()` implementation to return `"protondb_suggestion_apply"` for the new variant.

Check if there is a `Display` impl, `Serialize` impl, or any other serialization point for this enum beyond `as_str()` — if so, update those too. Note: the enum has `#[serde(rename_all = "snake_case")]` which would auto-derive `"proton_db_suggestion_apply"` (with underscore between `db` and `suggestion`). The `as_str()` method must be explicitly updated to return the desired `"protondb_suggestion_apply"` — do not rely on serde derive for the stored string. The `config_revisions.source` column stores TEXT, so no schema migration is required.

Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` to verify compilation and existing tests pass.

#### Task 1.3: Mirror expanded blocklist in frontend RESERVED_CUSTOM_ENV_KEYS Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx

At line 6, the `RESERVED_CUSTOM_ENV_KEYS` Set currently has 3 entries matching the old Rust blocklist. Expand it to include all keys added in Task 1.1: `LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT`, `LD_DEBUG`, `PATH`, `HOME`, `SHELL`, `NODE_OPTIONS`, `PYTHONPATH`.

Also add a prefix-check function that mirrors the Rust `BLOCKED_ENV_KEY_PREFIXES` behavior — check if the key starts with `STEAM_COMPAT_` or `LD_`. This may be a helper `isReservedEnvKey(key: string): boolean` that combines Set lookup and prefix checks.

Update the existing comment at line 5 which currently references `launch/request.rs` — change it to reference `protondb/aggregation.rs` instead (the actual location of the Rust blocklist). This comment is the only cross-reference between the frontend and backend blocklists, so accuracy is critical for future maintainers.

#### Task 1.4: Define TypeScript suggestion type interfaces Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/protondb.ts
- docs/plans/community-driven-config-suggestions/research-technical.md

**Instructions**

Files to Modify

- src/crosshook-native/src/types/protondb.ts

Extend the existing TypeScript type file with interfaces mirroring the Rust structs that will be defined in `suggestions.rs`. These types are the contract between backend and frontend.

Add these types:

- `SuggestionStatus`: `'new' | 'already_applied' | 'conflict' | 'dismissed'`
- `CatalogSuggestionItem`: `{ catalogEntryId: string; label: string; description: string; envPairs: [string, string][]; status: SuggestionStatus; supportingReportCount: number }`
- `EnvVarSuggestionItem`: `{ key: string; value: string; status: SuggestionStatus; supportingReportCount: number }`
- `LaunchOptionSuggestionItem`: `{ rawText: string; supportingReportCount: number }`
- `ProtonDbSuggestionSet`: `{ catalogSuggestions: CatalogSuggestionItem[]; envVarSuggestions: EnvVarSuggestionItem[]; launchOptionSuggestions: LaunchOptionSuggestionItem[]; tier: ProtonDbTier; totalReports: number; isStale: boolean }`
- `AcceptSuggestionRequest`: Tagged union — `{ kind: 'catalog'; profileName: string; catalogEntryId: string } | { kind: 'env_var'; profileName: string; envKey: string; envValue: string }`
- `AcceptSuggestionResult`: `{ updatedProfile: GameProfile; appliedKeys: string[]; toggledOptionIds: string[] }`

Verify `camelCase` naming matches Tauri's Serde serialization — Rust structs use `#[serde(rename_all = "camelCase")]`, which means Rust `supporting_report_count` → TS `supportingReportCount`. Cross-check with existing types in the same file for the established convention.

### Phase 2: Core Feature Delivery

#### Task 2.1: Create suggestions.rs catalog-bridge engine Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs
- src/crosshook-native/crates/crosshook-core/src/protondb/models.rs
- src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs
- docs/plans/community-driven-config-suggestions/feature-spec.md
- docs/plans/community-driven-config-suggestions/research-technical.md

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/protondb/suggestions.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs
- src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs

This is the most algorithmically complex task. Create `suggestions.rs` with:

1. **Struct definitions** (all with `#[derive(Debug, Clone, Serialize, Deserialize)]` and `#[serde(rename_all = "camelCase")]`):
   - `SuggestionStatus` enum: `New`, `AlreadyApplied`, `Conflict`, `Dismissed`
   - `CatalogSuggestionItem`: catalog-matched suggestion with entry ID, label, env pairs, status, report count
   - `EnvVarSuggestionItem`: raw env var suggestion with key, value, status, report count
   - `LaunchOptionSuggestionItem`: copy-only launch option text with report count
   - `ProtonDbSuggestionSet`: container for all three suggestion tiers plus metadata
   - `AcceptSuggestionRequest` (tagged enum): `Catalog { profile_name, catalog_entry_id }` | `EnvVar { profile_name, env_key, env_value }`
   - `AcceptSuggestionResult`: updated profile + applied keys + toggled option IDs

2. **Private helper**: `build_catalog_env_index(catalog: &[OptimizationEntry]) -> HashMap<(String, String), String>` — maps `(env_key, env_value)` → `catalog_entry_id`. Match on full key+value pair, not key alone.

3. **Core function**: `derive_suggestions(lookup: &ProtonDbLookupResult, profile: &GameProfile, catalog: &[OptimizationEntry], dismissed_keys: &HashSet<String>) -> ProtonDbSuggestionSet`:
   - Build catalog env index
   - For each `ProtonDbRecommendationGroup.env_vars`, check each `(key, value)` pair against the catalog index
   - If match: create `CatalogSuggestionItem` with status based on whether `catalog_entry_id` is already in `profile.launch.optimizations.enabled_option_ids`
   - If no match: create `EnvVarSuggestionItem` with status based on whether `key` exists in `profile.launch.custom_env_vars` (same value → `AlreadyApplied`, different value → `Conflict`, absent → `New`)
   - If `key` is in `dismissed_keys`: set status to `Dismissed`
   - Sort each tier by `supporting_report_count` descending

4. **Visibility change in aggregation.rs**: Make `is_safe_env_key()` and `is_safe_env_value()` `pub(crate)` so `suggestions.rs` can re-validate at accept time.

5. **Module registration in mod.rs**: Add `pub mod suggestions;` and re-export public types.

Run tests after creation.

#### Task 2.2: Schema v17 migration + dismissal store methods Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs

Add `if version < 17` block after the existing v16 block in `migrations.rs`:

```sql
CREATE TABLE IF NOT EXISTS suggestion_dismissals (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id     TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    app_id         TEXT NOT NULL,
    suggestion_key TEXT NOT NULL,
    dismissed_at   TEXT NOT NULL,
    expires_at     TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_suggestion_dismissals_unique
    ON suggestion_dismissals(profile_id, app_id, suggestion_key);
```

Update `PRAGMA user_version = 17`.

Add store methods in a new `metadata/suggestion_store.rs` submodule (`metadata/mod.rs` is 3200+ lines — do not add more methods there). Register the submodule in `mod.rs` and re-export public methods. Methods to implement:

- `dismiss_suggestion(profile_id: &str, app_id: &str, suggestion_key: &str, ttl_days: u32) -> Result<()>` — upsert with 30-day `expires_at`
- `get_dismissed_keys(profile_id: &str, app_id: &str) -> Result<HashSet<String>>` — evict expired rows before returning
- `evict_expired_dismissals() -> Result<usize>` — cleanup helper

Run tests to verify migration applies cleanly using `MetadataStore::open_in_memory()`.

#### Task 2.3: Add 3 Tauri commands + register in invoke_handler Depends on [1.2, 2.1, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/protondb.rs
- src/crosshook-native/src-tauri/src/commands/profile.rs
- src/crosshook-native/src-tauri/src/lib.rs

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/protondb.rs
- src/crosshook-native/src-tauri/src/lib.rs

Extend `commands/protondb.rs` with 3 new thin Tauri commands:

1. **`protondb_get_suggestions`**: `(app_id: String, profile_name: String, force_refresh: Option<bool>, metadata_store: State<MetadataStore>, profile_store: State<ProfileStore>) -> Result<ProtonDbSuggestionSet, String>`. Calls `lookup_protondb()`, loads profile, gets dismissed keys, then calls `derive_suggestions()`.

2. **`protondb_accept_suggestion`**: `(request: AcceptSuggestionRequest, profile_store: State<ProfileStore>, metadata_store: State<MetadataStore>) -> Result<AcceptSuggestionResult, String>`. Follow the `profile_apply_bundled_optimization_preset` pattern:
   - Load profile by name
   - Re-validate the env key/value with `is_safe_env_key()` + `is_safe_env_value()` + `RESERVED_ENV_KEYS` check at write time — do NOT trust the cached suggestion
   - For `Catalog` kind: add `catalog_entry_id` to `enabled_option_ids`
   - For `EnvVar` kind: insert into `custom_env_vars`
   - Save profile, call `observe_profile_write()`, call `capture_config_revision()` with `ConfigRevisionSource::ProtonDbSuggestionApply`
   - Return `AcceptSuggestionResult` with updated profile

3. **`protondb_dismiss_suggestion`**: `(profile_name: String, app_id: String, suggestion_key: String, metadata_store: State<MetadataStore>) -> Result<(), String>`. Resolves `profile_id` from `profile_name`, calls `metadata_store.dismiss_suggestion()` with 30-day TTL.

Register all 3 commands in `lib.rs` `invoke_handler!` macro alongside existing `protondb_lookup`.

Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` and `cargo build --manifest-path src/crosshook-native/Cargo.toml` to verify.

#### Task 2.4: Create useProtonDbSuggestions hook Depends on [1.4, 2.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useProtonDbLookup.ts
- src/crosshook-native/src/types/protondb.ts

**Instructions**

Files to Create

- src/crosshook-native/src/hooks/useProtonDbSuggestions.ts

Follow the `useProtonDbLookup.ts` pattern exactly:

1. **State**: `suggestionSet: ProtonDbSuggestionSet | null`, `loading: boolean`, `error: string | null`
2. **Race safety**: `requestIdRef` counter — discard stale responses from concurrent fetches
3. **Fetch**: `invoke<ProtonDbSuggestionSet>('protondb_get_suggestions', { appId, profileName, forceRefresh })` triggered on `appId`/`profileName` change
4. **Accept**: `acceptSuggestion(request: AcceptSuggestionRequest): Promise<AcceptSuggestionResult>` — calls `invoke('protondb_accept_suggestion', { request })`, returns the result so the caller can update profile state
5. **Dismiss**: `dismissSuggestion(suggestionKey: string): void` — calls `invoke('protondb_dismiss_suggestion', { profileName, appId, suggestionKey })` fire-and-forget; optimistically removes from local `suggestionSet` state
6. **Refresh**: `refresh(): Promise<void>` — re-fetches with `forceRefresh: true`

Export interface `UseProtonDbSuggestionsResult` with all state fields and action methods.

The hook should only activate when `appId` is non-empty and `profileName` is non-empty — return idle state otherwise.

#### Task 2.5: Wire apply/dismiss into ProtonDbLookupCard and extract shared apply utility Depends on [2.3, 2.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProtonDbLookupCard.tsx
- src/crosshook-native/src/utils/protondb.ts
- src/crosshook-native/src/components/pages/LaunchPage.tsx
- src/crosshook-native/src/components/ProfileFormSections.tsx
- src/crosshook-native/src/utils/optimization-catalog.ts

**Instructions**

Files to Modify

- src/crosshook-native/src/utils/protondb.ts
- src/crosshook-native/src/components/ProtonDbLookupCard.tsx
- src/crosshook-native/src/components/pages/LaunchPage.tsx
- src/crosshook-native/src/components/ProfileFormSections.tsx
- src/crosshook-native/src/components/pages/InstallPage.tsx (imports `ProfileFormSections` — must pass new `catalog` prop to avoid TypeScript build break)

**Step 1 — Extract shared apply utility** in `src/utils/protondb.ts`:

Create `applyProtonDbGroupToProfile(current: GameProfile, group: ProtonDbRecommendationGroup, overwriteKeys: readonly string[], catalog: OptimizationCatalogPayload | null): { nextProfile: GameProfile; appliedKeys: string[]; unchangedKeys: string[]; toggledOptionIds: string[] }`:

- Run existing `mergeProtonDbEnvVarGroup()` for env var merge
- For each applied key, check if `(key, value)` matches any catalog entry's `env` pair
- Catalog-matched keys: add entry ID to `enabled_option_ids` instead of `custom_env_vars`
- Non-matched keys: add to `custom_env_vars` as before
- Return the updated profile plus metadata for status messaging

**Step 2 — Replace dual Apply paths**:

In `LaunchPage.tsx`: Replace inline `applyProtonDbGroup` callback at line ~194 with a call to `applyProtonDbGroupToProfile`. The catalog is already available via `profileState.catalog`.

In `ProfileFormSections.tsx`: Add `catalog: OptimizationCatalogPayload | null` to the component's props interface. Replace inline `applyProtonDbGroup` at line ~395 with a call to `applyProtonDbGroupToProfile`. Thread `catalog` from all call sites.

**Step 3 — Wire suggestion UI into ProtonDbLookupCard**:

Extend `ProtonDbLookupCard` props:

- `suggestionSet?: ProtonDbSuggestionSet` — optional, renders suggestion sections when present
- `onAcceptSuggestion?: (request: AcceptSuggestionRequest) => Promise<void>`
- `onDismissSuggestion?: (suggestionKey: string) => void`

Render in the card:

- **Catalog suggestions** (Tier 1): "Enable [label]" toggle buttons with status badges (`AlreadyApplied` → checkmark, `Conflict` → warning, `New` → action button)
- **Env var suggestions** (Tier 2): key=value display with "Apply" button + conflict detection
- **Report count attribution**: show `supportingReportCount` for each suggestion
- **Dismiss**: X button on each suggestion item calls `onDismissSuggestion`
- **Staleness indicator**: amber banner when `suggestionSet.isStale`

**Step 4 — Status messaging**:

Update the `protonDbStatusMessage` state in both `LaunchPage` and `ProfileFormSections` to report catalog-aware results: "Applied X optimization(s) and Y env var(s)" or "Enabled [optimization name]".

Register any new scroll containers in `useScrollEnhance.ts::SCROLLABLE` if `overflow-y: auto` is added.

#### Task 2.6: Unit tests for blocklist, catalog bridge, and suggestions engine Depends on [1.1, 2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/protondb/tests.rs
- src/crosshook-native/crates/crosshook-core/src/protondb/suggestions.rs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/protondb/tests.rs

Extend the existing test file with these test cases:

**Catalog bridge tests:**

- `catalog_match_maps_known_optimization` — `DXVK_ASYNC=1` maps to the `enable_dxvk_async` catalog entry
- `catalog_match_value_mismatch_stays_tier2` — `DXVK_ASYNC=0` does NOT match `enable_dxvk_async` (which sets `DXVK_ASYNC=1`); stays as `EnvVarSuggestionItem`
- `catalog_match_unmapped_key_stays_tier2` — env var with no catalog entry stays as `EnvVarSuggestionItem`

**Status computation tests:**

- `already_applied_when_key_matches_profile` — env var already in `custom_env_vars` with same value → `AlreadyApplied`
- `conflict_when_key_present_with_different_value` — env var in `custom_env_vars` with different value → `Conflict`
- `new_when_key_absent` — env var not in profile → `New`
- `dismissed_status_overrides` — dismissed key shows `Dismissed` regardless of profile state

**Sorting test:**

- `suggestions_sorted_by_report_count_descending` — verify each tier is sorted by `supporting_report_count` desc

Use `MetadataStore::open_in_memory()` for any tests that need a store. Build test `OptimizationEntry` vectors from `default_optimization_catalog.toml` entries for realistic matching.

Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.

### Phase 3: Verification and Build

#### Task 3.1: Build verification and integration check Depends on [2.5, 2.6]

**READ THESE BEFORE TASK**

- CLAUDE.md
- AGENTS.md

**Instructions**

Files to Modify

- (none — verification only)

Run the full verification sequence:

1. `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` — all Rust tests pass
2. `./scripts/build-native.sh --binary-only` — full build succeeds (skips AppImage bundling for speed)
3. Verify no compiler warnings in `crosshook-core` or `src-tauri`
4. Spot-check that `ProtonDbLookupCard` renders without errors in dev mode (`./scripts/dev-native.sh`)

If any test or build fails, fix before marking complete.

## Advice

- **Ship Task 1.1 as a standalone commit/PR first** — it closes a security gap in existing production code, independent of the rest of the feature. The `LD_PRELOAD` family is injectable today; this should not wait for the full feature to land.
- **The `ConfigRevisionSource` enum is serialized as TEXT** — adding a new variant does not require a schema migration. It is safe to add at any time without backward compatibility concerns.
- **Catalog env matching must be key+value, never key-only** — `PROTON_NO_ESYNC=1` and `PROTON_NO_ESYNC=0` are different suggestions that map to different catalog entries (or no entry). The `build_catalog_env_index` function must use `(String, String)` keys.
- **`ProfileFormSections` does not currently receive `catalog`** — Task 2.5 adds this prop. All call sites of `ProfileFormSections` (including `OnboardingWizard.tsx` step 3) must be updated to pass it. Check `ProfileFormSections` usage with a grep before modifying.
- **Profile write serialization** — `LaunchPage.tsx` may use a write serialization pattern to prevent race conditions with optimization autosave. Check the current implementation before adding new `invoke` calls that write profiles — concurrent writes can cause clobbering.
- **Content-hash deduplication in `insert_config_revision`** means applying suggestions that produce no net change to the TOML will not create a revision row. This is correct behavior — no special handling needed.
- **The `RESERVED_CUSTOM_ENV_KEYS` Set in the frontend and `RESERVED_ENV_KEYS` in Rust must be kept in sync manually** — there is no compile-time enforcement. Add comments in both files cross-referencing each other.
- **Test with `MetadataStore::open_in_memory()`** — never mock the store; the in-memory SQLite variant exercises real migration and query paths, catching schema issues that mocks would hide.
- **`ProtonDbLookupCard` is ~400 lines** — keep changes surgical. Wire suggestion props as optional additions; do not restructure existing rendering logic.
- **ODbL compliance reminder** — cached ProtonDB data in `external_cache_entries.payload_json` must never appear in profile exports or community tap distributions. Only user-accepted values written to TOML fields may be exported.
