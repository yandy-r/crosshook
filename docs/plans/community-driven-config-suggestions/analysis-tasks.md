# Task Structure Analysis: Community-Driven Configuration Suggestions

## Executive Summary

The backend ProtonDB pipeline is complete. The remaining work decomposes into **10 concrete tasks** across two hard-sequential phases: a security/foundation phase (Phase 0) that must fully complete before any apply-flow ships, and a feature delivery phase (Phase 1) that has significant internal parallelism. The schema is currently at v16 and needs a v17 migration for `suggestion_dismissals`. The `ConfigRevisionSource` enum currently has only `ManualSave` and `RollbackApply` variants — a `ProtonDbSuggestion` variant must be added before any write path ships.

**Dismissal persistence decision (resolved):** `feature-spec.md` is authoritative. The `suggestion_dismissals` SQLite table ships in Phase 1 with 30-day auto-expiry. The `protondb_dismiss_suggestion` Tauri command in T6 implements the full write path (not a no-op). The contradiction in `research-technical.md` is superseded by the spec.

The critical sequencing insight: **T1 (RESERVED_ENV_KEYS expansion) must ship first** as an isolated security fix. **T3 (TypeScript type interfaces) must land before T7 and T8 can proceed** because both frontend tasks mirror the Rust struct signatures. **T10 (OnboardingWizard integration) is a separate task** from T8 — `ProtonDbLookupCard` is not yet wired into `OnboardingWizard.tsx` Step 3, and that integration has distinct state threading concerns.

---

## Recommended Phase Structure

### Phase 0 — Security + Foundation (must complete before Phase 1)

All Phase 0 tasks are independent of each other and can run in parallel.

| ID  | Task                                   | Files Changed             | Est. Complexity |
| --- | -------------------------------------- | ------------------------- | --------------- |
| T1  | Expand RESERVED_ENV_KEYS blocklist     | `protondb/aggregation.rs` | Low             |
| T2  | Add ConfigRevisionSource variant       | `metadata/models.rs`      | Trivial         |
| T3  | Add TypeScript suggestion type mirrors | `src/types/protondb.ts`   | Low             |

**Phase 0 gate**: All three tasks must be merged before any Phase 1 task ships. T1 is the hard security gate. T2 and T3 are foundation work that unblocks the Phase 1 parallel fan-out.

### Phase 1 — Core Feature Delivery (after Phase 0)

Phase 1 has three sequential groups within it:

**Group A — Backend (can run in parallel after Phase 0):**

| ID  | Task                                   | Files Changed                                      | Est. Complexity |
| --- | -------------------------------------- | -------------------------------------------------- | --------------- |
| T4  | Create suggestions.rs engine           | `protondb/suggestions.rs` (new), `protondb/mod.rs` | Medium          |
| T5  | Schema v17 migration + dismissal store | `metadata/migrations.rs`, `metadata/mod.rs`        | Low-Medium      |

**Group B — IPC + Hook (T4 and T5 must complete; T3 must complete for T7):**

| ID  | Task                               | Files Changed                               | Est. Complexity |
| --- | ---------------------------------- | ------------------------------------------- | --------------- |
| T6  | Add 3 Tauri commands + register    | `commands/protondb.rs`, `lib.rs`            | Low-Medium      |
| T7  | Create useProtonDbSuggestions hook | `src/hooks/useProtonDbSuggestions.ts` (new) | Low             |

**Group C — UI wiring (after T6, T7 complete; T8/T9/T10 can run in parallel):**

| ID  | Task                                          | Files Changed                       | Est. Complexity |
| --- | --------------------------------------------- | ----------------------------------- | --------------- |
| T8  | Wire apply/dismiss into ProtonDbLookupCard    | `components/ProtonDbLookupCard.tsx` | Medium          |
| T9  | Unit tests (Rust blocklist + catalog bridge)  | `protondb/tests.rs` additions       | Low-Medium      |
| T10 | Wire suggestions into OnboardingWizard Step 3 | `components/OnboardingWizard.tsx`   | Low-Medium      |

---

## Task Granularity Recommendations

### T1 — Expand RESERVED_ENV_KEYS (aggregation.rs)

This is the highest-priority task and must be treated as a standalone, shippable security fix. It should not be bundled with other work.

**Scope:**

- Expand `RESERVED_ENV_KEYS` constant in `aggregation.rs:10-14` to add: `LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT`, `LD_DEBUG`, `LD_ORIGIN_PATH`, `LD_PROFILE`, `PATH`, `HOME`, `ZDOTDIR`, `SHELL`, `ENV`, `BASH_ENV`, `NODE_OPTIONS`, `PYTHONPATH`, `RUBYLIB`, `PERL5LIB`
- Add a `BLOCKED_ENV_KEY_PREFIXES: &[&str] = &["STEAM_COMPAT_", "LD_"]` constant for prefix-based blocking
- Update the guard in `safe_env_var_suggestions()` to check prefixes in addition to exact matches
- Existing tests in `protondb/tests.rs` must still pass; new tests for `LD_PRELOAD` and `PATH` rejection belong in T9

**Do NOT include in T1:** `make is_safe_env_key pub(crate)` — that is T4's concern when `suggestions.rs` needs it for re-validation.

### T2 — Add ConfigRevisionSource::ProtonDbSuggestion variant

Single-line enum extension in `metadata/models.rs`. The current variants are `ManualSave` and `RollbackApply` with `as_str()` returning `"manual_save"` and `"rollback_apply"`. The new variant must return `"protondb_suggestion"` from `as_str()`.

The `config_revisions` table stores the `source` column as a string — no schema migration required for this change. Verify `as_str()` is the only serialization point; if `impl Display` or `Serialize` exists separately, update those too.

### T3 — TypeScript type interfaces (src/types/protondb.ts)

Extend the existing TypeScript type file to mirror the Rust structs from `suggestions.rs`. Key types to add:

- `SuggestionStatus`: `'new' | 'already_applied' | 'conflict' | 'dismissed'`
- `CatalogSuggestionItem` (Tier 1)
- `EnvVarSuggestionItem` (Tier 2)
- `LaunchOptionSuggestionItem`
- `ProtonDbSuggestionSet`
- `AcceptSuggestionRequest` (tagged union: `{ kind: 'catalog', profileName, catalogEntryId }` | `{ kind: 'env_var', profileName, envKey, envValue }`)
- `AcceptSuggestionResult`

The `camelCase`/`snake_case` mapping matters — Tauri serializes Rust `snake_case` fields to `camelCase` by default via `serde(rename_all = "camelCase")`. Verify this is consistent with how `ProtonDbLookupResult` is currently typed in the same file.

**This task is a hard prerequisite for T7 (hook) and T8/T10 (UI).**

### T4 — Create suggestions.rs + update mod.rs

New file: `src/crosshook-native/crates/crosshook-core/src/protondb/suggestions.rs`

Core function signature from the spec:

```rust
pub fn derive_suggestions(
    lookup: &ProtonDbLookupResult,
    profile: &GameProfile,
    catalog: &[OptimizationEntry],
    dismissed_keys: &HashSet<String>,
) -> ProtonDbSuggestionSet
```

Also needed:

- `build_catalog_env_index(catalog: &[OptimizationEntry]) -> HashMap<(String, String), String>` (private helper)
- All structs defined in `research-technical.md`: `SuggestionStatus`, `CatalogSuggestionItem`, `EnvVarSuggestionItem`, `LaunchOptionSuggestionItem`, `ProtonDbSuggestionSet`, `AcceptSuggestionRequest`, `AcceptSuggestionResult`
- Make `is_safe_env_key()` and `is_safe_env_value()` `pub(crate)` in `aggregation.rs` so `suggestions.rs` can re-validate at write time
- Update `mod.rs` to add `pub mod suggestions;` and re-export the public types

**Dependency:** T1 must be complete so the expanded blocklist is in place before `suggestions.rs` references `safe_env_var_suggestions()` output.

### T5 — Schema v17 migration + suggestion dismissal store

**Migration in `metadata/migrations.rs`:**

- Add `if version < 17` block after the existing v16 block (line 154 is where the `Ok(())` return currently lives)
- Create `suggestion_dismissals` table:

```sql
CREATE TABLE IF NOT EXISTS suggestion_dismissals (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id     TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    app_id         TEXT NOT NULL,
    suggestion_key TEXT NOT NULL,
    dismissed_at   TEXT NOT NULL,
    expires_at     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_suggestion_dismissals_profile_app
    ON suggestion_dismissals(profile_id, app_id);
```

**Store methods in `metadata/mod.rs` (or a new `metadata/suggestion_store.rs`):**

- `dismiss_suggestion(profile_id, app_id, suggestion_key, ttl_days: u32) -> Result<(), MetadataStoreError>`
- `get_dismissed_keys(profile_id, app_id) -> Result<HashSet<String>, MetadataStoreError>` — must evict expired rows on read
- `evict_expired_dismissals() -> Result<usize, MetadataStoreError>`

**Decision (resolved):** feature-spec is authoritative. Implement the full write path in T5. The `protondb_dismiss_suggestion` command in T6 is a real write, not a no-op. The 30-day expiry and `ON DELETE CASCADE` on `profile_id` are required from day 1.

### T6 — Add 3 Tauri commands + register in lib.rs

Extend `src-tauri/src/commands/protondb.rs` with three commands following the existing `protondb_lookup` pattern (thin command, one `crosshook_core` call, map errors to `String`):

1. `protondb_get_suggestions(app_id, profile_name, force_refresh, metadata_store, profile_store)` — calls `lookup_protondb()` then `derive_suggestions()`
2. `protondb_accept_suggestion(request: AcceptSuggestionRequest, profile_store, metadata_store)` — tier-routing write path with re-validation
3. `protondb_dismiss_suggestion(profile_name, app_id, suggestion_key, metadata_store)` — writes to `suggestion_dismissals` via T5 store method

Register all three in `src-tauri/src/lib.rs` `invoke_handler!` macro.

**Critical:** `protondb_accept_suggestion` must re-run `is_safe_env_key()`, `is_safe_env_value()`, and the `RESERVED_ENV_KEYS` check (including the expanded T1 blocklist) at write time. Do not trust cached suggestion structs as pre-validated. Also records a config revision with `ConfigRevisionSource::ProtonDbSuggestion`.

**Dependency:** T2 (for enum variant), T4 (for `derive_suggestions()` signature and re-validation functions), T5 (for `dismiss_suggestion()` store method).

### T7 — Create useProtonDbSuggestions.ts hook

New file: `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts`

Follow the `useProtonDbLookup.ts` pattern exactly:

- `requestIdRef` counter for race-safety (discard stale responses)
- Loading/error/data state triple
- `forceRefresh` trigger
- Wrap `invoke<ProtonDbSuggestionSet>('protondb_get_suggestions', { appId, profileName, forceRefresh })`

Also expose:

- `acceptSuggestion(request: AcceptSuggestionRequest) => Promise<void>` — calls `invoke('protondb_accept_suggestion', { request })`
- `dismissSuggestion(profileName: string, appId: string, suggestionKey: string) => void` — calls `invoke('protondb_dismiss_suggestion', ...)` and updates local dismissed set optimistically

**Dependency:** T3 (for TypeScript types), T6 (for command names to exist in IPC layer).

### T8 — Wire apply/dismiss into ProtonDbLookupCard.tsx

`ProtonDbLookupCard.tsx` is 15k, currently renders recommendation groups with an `onApplyEnvVars` callback that is wired but the catalog-matching path is not implemented.

**Scope:**

- Accept `suggestionSet: ProtonDbSuggestionSet` as prop (passed from parent via `useProtonDbSuggestions`) or invoke the hook internally — follow whichever prop-threading pattern `LaunchPage.tsx` uses
- Render Tier 1 catalog suggestions as "Enable [Name]" toggle buttons (distinct visual treatment from Tier 2)
- Render Tier 2 env var suggestions with "Apply" button + conflict badge
- Status messaging on apply: "Applied X optimizations and Y env vars"
- Per-suggestion dismiss (X button) — calls `dismissSuggestion`
- Wire existing `ProtonDbOverwriteConfirmation` modal to the Tier 2 accept path for conflict resolution
- Cache staleness indicator (amber banner when `suggestionSet.cache?.isStale`)
- All ProtonDB-derived text rendered via plain React text interpolation — no `dangerouslySetInnerHTML`
- Any new `overflow-y: auto` container must be registered in `useScrollEnhance.ts::SCROLLABLE`

**Do NOT scope here:** Deduplication of the dual Apply paths between `LaunchPage.tsx` and `ProfileFormSections.tsx`. That is a follow-up refactor.

**Dependency:** T6 (commands), T7 (hook exposing `acceptSuggestion`/`dismissSuggestion`).

### T9 — Unit tests

Tests belong in `protondb/tests.rs` (existing file).

Required new test cases:

- `ld_preload_is_rejected_as_env_suggestion()` — feed with `LD_PRELOAD=/evil.so` must not appear in output
- `path_is_rejected_as_env_suggestion()` — similar for `PATH`
- `ld_prefix_keys_are_rejected()` — any `LD_*` key not explicitly listed is still blocked via prefix check
- `catalog_match_maps_dxvk_async()` — test `build_catalog_env_index` with a known mapping (`DXVK_ASYNC=1` → `enable_dxvk_async`)
- `catalog_match_unmapped_key_stays_in_tier2()` — env var with no catalog entry stays as `EnvVarSuggestionItem`
- `already_applied_status_when_key_matches_profile()` — status computed correctly against profile state
- `conflict_status_when_key_present_with_different_value()` — conflict detection correct

**Dependency:** T1 (blocklist in place for injection rejection tests), T4 (`derive_suggestions()` exists for status tests).

### T10 — Wire suggestions into OnboardingWizard.tsx Step 3

**Confirmed from codebase:** `ProtonDbLookupCard` is not yet referenced anywhere in `OnboardingWizard.tsx`. Step 3 already has the App ID input and is conditioned on `launchMethod === 'steam_applaunch'` (line 505). The suggestion panel must be inserted after the App ID input, also conditioned on `steam_applaunch` + a non-empty `app_id`.

**Scope:**

- Add `useProtonDbSuggestions` hook invocation inside the `steam_applaunch` condition in Step 3
- Render `ProtonDbLookupCard` with the suggestion set below the App ID field
- Wire the accept path so accepted suggestions update the in-progress `profile` state being built in the wizard (not a saved profile yet — use the wizard's local `updateProfile` pattern)
- Wire the dismiss path similarly
- Do NOT show the suggestion panel until `app_id.length >= 5` (avoid triggering lookups on partial input)
- Skeleton loader while fetching; form remains interactive
- All XSS and scroll container rules apply here as in T8

**Dependency:** T7 (hook), T8 (ProtonDbLookupCard must have the apply/dismiss actions before it's useful here).

---

## Dependency Analysis

```
T1 (blocklist)        ─────────────────────┐
T2 (enum variant)     ─────────────────────┤
T3 (TS types)         ─────────────────────┤
                                           ↓
T4 (suggestions.rs) ← T1              ──→ T6 (Tauri commands) ← T2, T4, T5
T5 (schema v17)                        ──→ T7 (hook)           ← T3, T6
                                            ↓                   ↓
                                         T8 (card UI)    ← T6, T7
                                         T9 (tests)      ← T1, T4
                                         T10 (wizard)    ← T7, T8
```

**Strict ordering:**

1. T1, T2, T3 — parallel, no dependencies (Phase 0)
2. T4, T5 — parallel after Phase 0 gate
3. T6 — after T2, T4, T5
4. T7 — after T3, T6
5. T8, T9 — parallel after T6+T7
6. T10 — after T7+T8 (wizard uses the card component)

---

## File-to-Task Mapping

| File                                    | Task(s) | Change Type                                               |
| --------------------------------------- | ------- | --------------------------------------------------------- |
| `protondb/aggregation.rs`               | T1, T4  | Modify: expand blocklist (T1), make fns `pub(crate)` (T4) |
| `protondb/suggestions.rs`               | T4      | Create: suggestion engine                                 |
| `protondb/mod.rs`                       | T4      | Modify: add `pub mod suggestions;`, re-export types       |
| `protondb/tests.rs`                     | T9      | Modify: add test cases                                    |
| `metadata/models.rs`                    | T2      | Modify: add `ConfigRevisionSource` variant                |
| `metadata/migrations.rs`                | T5      | Modify: add v17 migration block                           |
| `metadata/mod.rs`                       | T5      | Modify: expose dismissal store methods                    |
| `commands/protondb.rs`                  | T6      | Modify: add 3 commands                                    |
| `src-tauri/src/lib.rs`                  | T6      | Modify: register 3 commands in `invoke_handler`           |
| `src/types/protondb.ts`                 | T3      | Modify: add TS type interfaces                            |
| `src/hooks/useProtonDbSuggestions.ts`   | T7      | Create: frontend hook                                     |
| `src/components/ProtonDbLookupCard.tsx` | T8      | Modify: wire apply/dismiss UI                             |
| `src/components/OnboardingWizard.tsx`   | T10     | Modify: insert suggestion panel in Step 3                 |

**Files NOT in scope for this feature (confirmed):**

- `src/components/pages/LaunchPage.tsx` — dual apply path deduplication is a follow-up
- `src/components/ProfileFormSections.tsx` — same; shared `applyProtonDbSuggestions` utility extraction is a follow-up
- `src/components/CustomEnvironmentVariablesSection.tsx` — `RESERVED_CUSTOM_ENV_KEYS` mirror update is a low-priority follow-up after T1 ships

---

## Optimization Opportunities (Parallelism)

**Maximum parallel execution fan-out:**

Round 1 (all parallel, no dependencies):

- T1: Rust blocklist expansion
- T2: ConfigRevisionSource enum variant
- T3: TypeScript type interfaces

Round 2 (parallel, after Phase 0 gate):

- T4: suggestions.rs engine (depends on T1 being in place)
- T5: Schema v17 migration + dismissal store (independent of T1)

Round 3 (sequential convergence):

- T6: Tauri commands (depends on T2 + T4 + T5)

Round 4 (parallel after T6):

- T7: useProtonDbSuggestions hook (depends on T3 + T6)

Round 5 (parallel after T7):

- T8: ProtonDbLookupCard wiring (depends on T6 + T7)
- T9: Unit tests (depends on T1 + T4)

Round 6 (after T8):

- T10: OnboardingWizard Step 3 integration (depends on T7 + T8)

**Total minimum sequential depth:** 6 rounds. Practical critical path: T1 → T4 → T6 → T7 → T8 → T10.

---

## Implementation Strategy Recommendations

### 1. Ship T1 as an isolated security fix first

T1 (RESERVED_ENV_KEYS expansion) closes a CRITICAL security gap in the existing production ProtonDB module — not just the new feature. It should be committed and merged before any other work begins. It is independently reviewable, one file, and does not require the rest of the feature to be present.

### 2. Dismissal persistence is fully implemented in Phase 1 (resolved)

`feature-spec.md` is authoritative over `research-technical.md`. T5 implements the `suggestion_dismissals` table with 30-day TTL, and T6's `protondb_dismiss_suggestion` command writes to it immediately. No no-op, no follow-up migration needed.

### 3. Keep T8 scoped to ProtonDbLookupCard only

The dual Apply path in `LaunchPage.tsx` and `ProfileFormSections.tsx` (shared `applyProtonDbSuggestions` utility) is a clean-up refactor that belongs in a follow-up PR. Mixing it into T8 will expand scope and create merge conflicts with T10.

### 4. T4 (suggestions.rs) is the most complex task

The `derive_suggestions()` function must handle: catalog index building, tiered classification, profile state comparison, dismissal key filtering, status assignment, and sort ordering. The borrow patterns around `&ProtonDbLookupResult` vs owned types at the IPC boundary need care. Recommend a test-first approach: write T9's catalog matching tests alongside T4 to validate the engine before wiring IPC.

### 5. TypeScript type accuracy (T3) is load-bearing for T7, T8, and T10

If T3's `SuggestionStatus` string literals or struct field names don't exactly match what Tauri serializes, all three downstream tasks will have silent runtime type mismatches. Verify with a round-trip test against actual Tauri command output before merging T6. The `camelCase` transformation via `#[serde(rename_all = "camelCase")]` on the Rust structs is the primary divergence risk.

### 6. T10 must use the wizard's local profile update pattern, not a saved profile

`OnboardingWizard.tsx` builds a profile in local state before saving. Accepted suggestions during wizard flow must update that in-progress state via the wizard's `updateProfile` pattern — they must not trigger `profile_save` IPC calls. The accept path must be aware of this context distinction.

### 7. XSS and scroll container rules are cross-cutting

Both T8 and T10 render ProtonDB-derived text in React. Enforce: plain-text interpolation only (no `dangerouslySetInnerHTML`). Any new `overflow-y: auto` container in either component must be registered in `useScrollEnhance.ts::SCROLLABLE`.

---

## Key Technical Constraints (for implementers)

- `RESERVED_ENV_KEYS` is in `aggregation.rs:10-14` — the expansion in T1 is surgical (currently only 3 entries)
- `ConfigRevisionSource` enum is in `metadata/models.rs:382` — only `ManualSave` and `RollbackApply` exist
- Schema is at user_version 16; T5 targets user_version 17 (final migration block currently at `migrations.rs:147-154`)
- `protondb_lookup` command in `commands/protondb.rs` is 13 lines — T6 extends this file
- `useProtonDbLookup.ts` (`src/hooks/`, 4.4k) is the canonical hook pattern for T7
- `ProtonDbLookupCard.tsx` is 15k — T8 is a significant component modification
- `OnboardingWizard.tsx` Step 3 is at line 502; App ID input is at line 513; no `ProtonDbLookupCard` reference exists in this file yet
- `protondb/tests.rs` is 4.6k with existing test patterns — T9 adds to this file, not replaces
- No new crate dependencies — all needed crates already in `Cargo.toml`
- Catalog bridge index built at runtime from `global_catalog()` — not hardcoded

---

## Relevant Docs

- `docs/plans/community-driven-config-suggestions/shared.md` — file map and architectural patterns
- `docs/plans/community-driven-config-suggestions/feature-spec.md` — consolidated spec (authoritative for scope and persistence decisions)
- `docs/plans/community-driven-config-suggestions/research-technical.md` — exact struct definitions, command signatures, Tier 1/2 architecture
- `docs/plans/community-driven-config-suggestions/research-security.md` — S2 critical finding, exact blocklist values to add
- `docs/plans/community-driven-config-suggestions/research-recommendations.md` — phasing rationale, risk assessment
- `docs/plans/community-driven-config-suggestions/analysis-context.md` — cross-cutting concerns and parallelization constraints (context-synthesizer output)
