# Engineering Practices Research: ML-Assisted Configuration

## Executive Summary

The codebase already has a near-complete infrastructure for this feature. The `protondb` module fetches community reports, parses raw launch strings, extracts env vars, and surfaces them as `ProtonDbRecommendationGroup` objects. The frontend already applies those suggestions to `launch.custom_env_vars` via `mergeProtonDbEnvVarGroup` and handles overwrite conflicts. What is missing is a single "suggest from ProtonDB" entry point that maps the existing `ProtonDbEnvVarSuggestion` values onto profile fields, surfaced during profile creation — not a new text extraction engine, just a new Tauri command and a UI step that wires the existing pieces together.

No regex crate, no NLP, and no new HTTP infrastructure are needed for the first version.

---

## Existing Reusable Code

| Module / File                                                                        | Purpose                                                                                                                                                                              | How to Reuse                                                                                                     |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`                  | `lookup_protondb(store, app_id, force_refresh)` — fetches summary + report feed, caches in `external_cache_entries`, returns `ProtonDbLookupResult`                                  | Call as-is; already handles stale cache fallback and offline graceful degradation                                |
| `src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs`             | `normalize_report_feed` — tokenizes raw `launch_options` strings, extracts `KEY=VALUE` pairs, validates keys/values via `is_safe_env_key` / `is_safe_env_value`, groups by signature | Already produces the `ProtonDbRecommendationGroup` with `env_vars` and `launch_options` the UI needs             |
| `src/crosshook-native/crates/crosshook-core/src/protondb/models.rs`                  | `ProtonDbEnvVarSuggestion`, `ProtonDbRecommendationGroup`, `ProtonDbLookupResult`, `ProtonDbSnapshot` — all Serde-ready and IPC-serializable                                         | Use `env_vars` field on each group as direct suggestions                                                         |
| `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`             | `put_cache_entry` / `get_cache_entry` — upsert/read from `external_cache_entries`, enforces 512 KiB payload cap, checks `expires_at`                                                 | ProtonDB client already calls these; no new cache plumbing needed                                                |
| `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — `LaunchSection` | `custom_env_vars: BTreeMap<String, String>` — the canonical location for user-defined env vars applied at launch                                                                     | Suggestions should write into this field                                                                         |
| `src/crosshook-native/src-tauri/src/commands/protondb.rs`                            | `protondb_lookup` Tauri command — thin wrapper, already registered                                                                                                                   | Extend or companion command for "suggest from snapshot"                                                          |
| `src/crosshook-native/src/hooks/useProtonDbLookup.ts`                                | React hook managing `invoke('protondb_lookup', ...)` lifecycle (loading, stale, unavailable states, request-id cancellation)                                                         | Reuse as the data source for a new suggestion step in the profile creation wizard                                |
| `src/crosshook-native/src/utils/protondb.ts`                                         | `mergeProtonDbEnvVarGroup` — pure utility that computes `appliedKeys`, `unchangedKeys`, `conflicts` for a `ProtonDbRecommendationGroup` against existing env vars                    | Reuse verbatim in the profile creation flow                                                                      |
| `src/crosshook-native/src/components/ProtonDbLookupCard.tsx`                         | Renders recommendation groups with Apply buttons; `onApplyEnvVars` callback prop                                                                                                     | Already wired to `handleApplyProtonDbEnvVars` in `ProfileFormSections.tsx`; adapt or reuse for the creation step |
| `src/crosshook-native/src/components/ProtonDbOverwriteConfirmation.tsx`              | Conflict-resolution UI for env-var overwrites                                                                                                                                        | Reuse as-is in the creation wizard                                                                               |
| `src/crosshook-native/src/components/ProfileFormSections.tsx`                        | Contains `applyProtonDbGroup`, `handleApplyProtonDbEnvVars`, and state (`pendingProtonDbOverwrite`, `applyingProtonDbGroupId`)                                                       | Extract or adapt these handlers into the profile creation path                                                   |

---

## Profile Creation Wizard Integration Point

The profile creation wizard is `src/crosshook-native/src/components/OnboardingWizard.tsx`. It has three steps: Game Setup (step 1), Trainer Setup (step 2), and Runtime Setup (step 3). The wizard is opened from `ProfilesPage.tsx` via `<OnboardingWizard mode="create" ... />`.

### Where the Steam App ID lives in the wizard

- For `steam_applaunch`: the Steam App ID field (`profile.steam.app_id`) is rendered in **Step 3 (Runtime Setup)** at lines 511-519 of `OnboardingWizard.tsx`.
- For `proton_run`: there is no App ID field in the wizard at all.
- `CustomEnvironmentVariablesSection` is rendered in **Step 1 (Game Setup)** at line 451.

### Correct integration point: Step 3, below the App ID input

The `ProtonDbLookupCard` (inside a `CollapsibleSection`) should be placed in Step 3, immediately after the App ID input, conditioned on `launchMethod === 'steam_applaunch'`. This is the first moment in the wizard when an App ID value is available to feed into `useProtonDbLookup`.

For `proton_run`, the wizard has no App ID input. If the product decides to support ProtonDB lookups during `proton_run` creation, an optional App ID field would need to be added to Step 3 — but that is a product decision, not a prerequisite for the `steam_applaunch` path.

### Wire pattern (Step 3, steam_applaunch section)

```tsx
// After the existing App ID + Prefix Path inputs in OnboardingWizard.tsx Step 3:
{profile.steam.app_id.trim().length > 0 && (
  <CollapsibleSection title="ProtonDB Suggestions" meta={recommendationGroups.length > 0 ? `${recommendationGroups.length}` : undefined}>
    <ProtonDbLookupCard
      appId={profile.steam.app_id}
      lookupState={lookupState}
      recommendationGroups={recommendationGroups}
      onApplyEnvVars={(group, overwriteKeys) => {
        const result = mergeProtonDbEnvVarGroup(profile.launch.custom_env_vars, group, overwriteKeys);
        if (result.conflicts.length > 0) {
          setPendingProtonDbOverwrite({ group, conflicts: result.conflicts });
        } else {
          updateProfile((current) => ({ ...current, launch: { ...current.launch, custom_env_vars: result.mergedEnvVars } }));
        }
      }}
      applyingGroupId={applyingProtonDbGroupId}
    />
    {pendingProtonDbOverwrite && (
      <ProtonDbOverwriteConfirmation ... />
    )}
  </CollapsibleSection>
)}
```

All referenced components (`CollapsibleSection`, `ProtonDbLookupCard`, `ProtonDbOverwriteConfirmation`), hooks (`useProtonDbLookup`), and utilities (`mergeProtonDbEnvVarGroup`) already exist. The state variables (`pendingProtonDbOverwrite`, `applyingProtonDbGroupId`) mirror what `ProfileFormSections.tsx` already uses — add them locally in `OnboardingWizard.tsx`.

---

## Modularity Design

### Recommended module boundaries

The extraction engine already exists inside `protondb/aggregation.rs`. No new module is needed for v1.

**Backend: add `protondb/suggestions.rs` (or inline in `client.rs`)**

A new `pub fn top_env_var_suggestions(snapshot: &ProtonDbSnapshot) -> Vec<ProtonDbEnvVarSuggestion>` that flattens the recommendation groups and deduplicates env-var suggestions by key, ranked by `supporting_report_count`. This is a pure, stateless function over the existing snapshot type.

If this function grows beyond ~50 lines (adding filtering, scoring, grouping by confidence), extract it as a separate `suggestions.rs` submodule under `protondb/`. It should not own any I/O.

**Backend: add one Tauri command `protondb_suggest_env_vars`**

```
protondb_suggest_env_vars(app_id: String, force_refresh: Option<bool>) -> ProtonDbLookupResult
```

This can be the same as `protondb_lookup` (it already returns the full snapshot including `recommendation_groups`). The frontend already reads `env_vars` from each group. No new command may be needed at all — the caller just reads `snapshot.recommendation_groups[*].env_vars`.

**Frontend: `useSuggestProfileEnvVars` hook (new, small)**

Wraps `useProtonDbLookup` and exposes a `topSuggestions: ProtonDbEnvVarSuggestion[]` derived value (flattened from all groups, deduplicated by key, sorted by `supporting_report_count` descending). This isolates the derivation logic from rendering.

**Relation to `community/` and `offline/` modules**

- `community/`: Community profiles (git-tap based). ProtonDB suggestions are a separate data source — do not mix. They target different user actions (applying env vars during creation vs. importing a full TOML profile).
- `offline/`: The ProtonDB client already handles stale/offline gracefully (`ProtonDbLookupState::Stale`, `ProtonDbLookupState::Unavailable`). The suggestion step should simply pass through the existing offline state rather than adding new offline logic.

---

## KISS Assessment

| Option                                            | Complexity                                               | Usefulness                                                           | Verdict                                                          |
| ------------------------------------------------- | -------------------------------------------------------- | -------------------------------------------------------------------- | ---------------------------------------------------------------- |
| Regex-based extraction (current `aggregation.rs`) | Low — already implemented                                | High — covers `KEY=VALUE %command%` patterns used in 90%+ of reports | **Ship this first**                                              |
| NLP / ML model                                    | Very high — new dependency, training data, runtime       | Marginal uplift for v1                                               | Defer indefinitely until regex clearly misses important patterns |
| Full-text search over report notes                | Medium — would require new indexing                      | Moderate — notes are free-form and noisy                             | Defer to v2                                                      |
| Embed report notes as "advisory text" (copy-only) | Zero extra work — already in `notes` field of each group | Low-medium (user must read and act manually)                         | Already done                                                     |

The existing `safe_env_var_suggestions` function in `aggregation.rs` already does the right thing: it tokenizes on whitespace, splits on `=`, validates keys against `[A-Z_][A-Z0-9_]*`, rejects shell-special characters in values, and skips reserved Steam keys. This is more careful than most regex approaches. **No new extraction logic is needed for v1.**

---

## Abstraction vs. Repetition

### Apply the Rule of Three

`mergeProtonDbEnvVarGroup` already exists in `src/utils/protondb.ts` and is consumed in `ProfileFormSections.tsx`. If profile creation needs the same merge behavior, reuse the utility directly — do not duplicate.

The conflict resolution UX (`ProtonDbOverwriteConfirmation`, `PendingProtonDbOverwrite` type, `resolutions` record) is used in exactly one place today (profile editing). Adding it to profile creation is the second use. Still too early to abstract — share the existing components without extracting a new abstraction.

**Do not create** a `SuggestionEngine` trait or abstraction layer until there are at least three distinct suggestion sources (ProtonDB, community taps, user history). Today there is one source.

---

## Interface Design

The key design principle for future-proofing against an ML swap is: **callers should only consume `ProtonDbRecommendationGroup` values, not implementation details of how they were generated.** The aggregation code is already encapsulated behind `normalize_report_feed` and the model types are already in `models.rs`. Nothing exposes the tokenization logic to callers.

### Recommended trait shape (only if a second data source arrives)

```rust
// Only extract this when there are two concrete impls
pub trait ConfigSuggestionSource {
    fn recommendation_groups(&self) -> &[ProtonDbRecommendationGroup];
}
```

Both ProtonDB and a hypothetical ML-based source would produce the same `ProtonDbRecommendationGroup` shape (which already carries `env_vars`, `launch_options`, `notes`, `supporting_report_count`). Rename the type to `ConfigRecommendationGroup` only when the second source ships.

### For v1, the interface is already in place

- Backend: `lookup_protondb(store, app_id, force_refresh) -> ProtonDbLookupResult`
- Frontend: `useProtonDbLookup(appId)` returns `recommendationGroups: ProtonDbRecommendationGroup[]`
- Apply: `mergeProtonDbEnvVarGroup(currentEnvVars, group, overwriteKeys)` returns `ProtonDbEnvVarMergeResult`

The only new surface needed is wiring these three into the profile creation wizard.

---

## Testability Patterns

### Recommended patterns

**Golden test files for extraction (Rust)**

Add fixtures under `src/crosshook-native/crates/crosshook-core/src/protondb/fixtures/`:

- `report_feed_env_vars_only.json` — feed where all reports have parseable `KEY=VALUE %command%` lines
- `report_feed_copy_only.json` — feed where launch strings have shell syntax that blocks parsing
- `report_feed_mixed.json` — mix of parseable, copy-only, and note-only reports
- `report_feed_empty.json` — empty or missing reports

Pass each through `normalize_report_feed` in unit tests and assert the exact output. `aggregation.rs` already tests inline (`tests.rs`) using hardcoded report structs — migrate to fixtures for the complex cases.

**Cache behavior (Rust, existing pattern)**

`protondb/tests.rs` already demonstrates the correct pattern: create an in-memory `MetadataStore::open_in_memory()`, seed a cache entry with a known payload, call `lookup_protondb`, assert the state. Extend this for the suggestion path.

**`mergeProtonDbEnvVarGroup` (TypeScript, unit test)**

Pure function — test with plain objects, no mocks needed. Cover:

- New key (no conflict)
- Existing key, same value (unchanged)
- Existing key, different value, no overwrite (conflict)
- Existing key, different value, with overwrite key in set (applies)

**Anti-patterns to avoid**

- Do not mock the `MetadataStore` in Rust tests — use `MetadataStore::open_in_memory()` (the test suite already establishes this pattern).
- Do not test the HTTP layer in CI — `lookup_protondb` with a disabled store short-circuits before any network call.
- Do not use snapshot testing on the full `ProtonDbLookupResult` JSON — test specific fields. The ProtonDB response format is stable but not guaranteed.

---

## Build vs. Depend

| Need                    | Current status                                                                 | Decision                                                                                                                                                                          |
| ----------------------- | ------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| HTTP client             | `reqwest 0.12` already in `Cargo.toml` with `json` and `rustls-tls` features   | Reuse. No new dependency.                                                                                                                                                         |
| JSON parsing            | `serde_json 1` already in `Cargo.toml`                                         | Reuse.                                                                                                                                                                            |
| Regex / text extraction | Not in `Cargo.toml` — not needed                                               | The current char-by-char tokenizer in `safe_env_var_suggestions` is sufficient and has zero dependency cost. Add `regex` only if a specific pattern is genuinely unrepresentable. |
| NLP / ML inference      | Not in `Cargo.toml`                                                            | Do not add. Regex extraction covers the practical ProtonDB report format.                                                                                                         |
| Cache storage           | `rusqlite 0.39` already in `Cargo.toml`, `external_cache_entries` table exists | Reuse `put_cache_entry` / `get_cache_entry`.                                                                                                                                      |
| UUID generation         | `uuid 1` already in `Cargo.toml`                                               | Reuse.                                                                                                                                                                            |

---

## Open Questions

1. **Profile creation entry point (RESOLVED)**: The `OnboardingWizard.tsx` is the profile creation wizard. The Steam App ID (`profile.steam.app_id`) is collected in **Step 3 (Runtime Setup)** for `steam_applaunch`. The `ProtonDbLookupCard` should be placed in Step 3, immediately after the App ID input, conditioned on `profile.steam.app_id.trim().length > 0 && launchMethod === 'steam_applaunch'`. For `proton_run` creation, there is no App ID field in the wizard — ProtonDB suggestions during `proton_run` creation require an additional product decision to add an optional App ID field.

2. **Opt-in vs. auto-apply**: Should suggestions be shown proactively (user must dismiss) or only on demand (user clicks "Fetch suggestions")? The existing editing flow is on-demand. Consistency favors the same pattern in creation.

3. **Interaction with optimization catalog entries**: The ProtonDB `env_vars` suggestions produce raw `KEY=VALUE` entries that go into `launch.custom_env_vars`. The optimization catalog entries also inject env vars at launch, but are gated behind `launch.optimizations.enabled_option_ids`. A user could end up with `PROTON_USE_WINED3D=1` in both places. The backend already deduplicates at launch time via `env.rs` — confirm this precedence order is documented and intentional before the creation wizard ships.

4. **`supporting_report_count` ranking threshold**: How many supporting reports should a suggestion require to be surfaced in creation? The current editing view shows all groups regardless of count. A minimum threshold (e.g., 2+ reports) would reduce noise during creation.

5. **Cache TTL during creation**: `CACHE_TTL_HOURS = 6` is set in `client.rs`. During profile creation, a freshly-fetched result will always be live. This is fine — no change needed.
