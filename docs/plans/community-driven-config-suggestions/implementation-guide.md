# Community-Driven Configuration Suggestions — Implementation Guide

This guide synthesizes all planning, analysis, and validation work into a single source of truth for implementers. It is the authoritative reference for task sequencing, file modifications, decision points, and readiness checks.

---

## Overview

The community-driven configuration suggestions feature adds a suggestions engine to CrossHook that:

1. Analyzes ProtonDB environment variable recommendations against the user's profile
2. Classifies them into two tiers: catalog-matched optimizations (Tier 1) and raw env vars (Tier 2)
3. Presents suggestions with automatic dismissal after 30 days
4. Allows users to accept suggestions, which apply both environment variables and optimization toggles atomically

The existing ProtonDB pipeline (fetch, cache, aggregate) is complete. This feature is a **security-gated** implementation: the RESERVED_ENV_KEYS blocklist expansion (T1) must ship first as an isolated fix. Subsequent work is organized into Phase 0 (foundations) and Phase 1 (delivery) with 6 sequential rounds of parallelizable tasks.

---

## Phase Structure

### Phase 0 — Security & Foundation Gate (Tasks T1, T2, T3)

**These three tasks must complete and merge before any Phase 1 task ships.**

All Phase 0 tasks are **independent and run in parallel**:

| Task | Title                                                | Complexity | Est. Effort | Files                   |
| ---- | ---------------------------------------------------- | ---------- | ----------- | ----------------------- |
| T1   | Expand RESERVED_ENV_KEYS blocklist                   | Low        | 1-2 hours   | `aggregation.rs`        |
| T2   | Add ConfigRevisionSource::ProtonDbSuggestion variant | Trivial    | 15 min      | `metadata/models.rs`    |
| T3   | Add TypeScript suggestion type mirrors               | Low        | 1 hour      | `src/types/protondb.ts` |

**Phase 0 Gate**: All three PRs merged before opening any Phase 1 PR.

---

### Phase 1 — Core Feature Delivery (Tasks T4–T10)

#### Group A — Backend Engine (Tasks T4, T5)

**Parallel after Phase 0 gate. No dependencies between T4 and T5.**

| Task | Title                                  | Complexity | Est. Effort | Files                                              |
| ---- | -------------------------------------- | ---------- | ----------- | -------------------------------------------------- |
| T4   | Create suggestions.rs engine           | Medium     | 3-4 hours   | `suggestions.rs` (new), `aggregation.rs`, `mod.rs` |
| T5   | Schema v17 migration + dismissal store | Low-Medium | 1-2 hours   | `migrations.rs`, `metadata/mod.rs`                 |

#### Group B — IPC Layer (Tasks T6, T7)

**Sequential: T4 + T5 must complete before T6 starts.**

| Task | Title                              | Complexity | Est. Effort | Files                                          |
| ---- | ---------------------------------- | ---------- | ----------- | ---------------------------------------------- |
| T6   | Add 3 Tauri commands + register    | Low-Medium | 1.5-2 hours | `commands/protondb.rs`, `src-tauri/src/lib.rs` |
| T7   | Create useProtonDbSuggestions hook | Low        | 1-1.5 hours | `useProtonDbSuggestions.ts` (new)              |

**Dependency**: T3 (for TS types) + T6 (for command names) required before T7.

#### Group C — UI Wiring & Tests (Tasks T8, T9, T10)

**T8 and T9 run in parallel after T6+T7; T10 runs after T8.**

| Task | Title                                         | Complexity | Est. Effort | Files                    |
| ---- | --------------------------------------------- | ---------- | ----------- | ------------------------ |
| T8   | Wire apply/dismiss into ProtonDbLookupCard    | Medium     | 2-3 hours   | `ProtonDbLookupCard.tsx` |
| T9   | Unit tests (blocklist + catalog bridge)       | Low-Medium | 1.5-2 hours | `protondb/tests.rs`      |
| T10  | Wire suggestions into OnboardingWizard Step 3 | Low-Medium | 1.5-2 hours | `OnboardingWizard.tsx`   |

**Dependencies**: T9 depends on T1 + T4; T10 depends on T7 + T8.

---

## Critical Path & Sequencing

```
PHASE 0 (parallel):  T1 ┐
                     T2 ├─→ [Phase 0 gate]
                     T3 ┘

PHASE 1:
  Round 2 (parallel): T4, T5 ──→ T6 ──→ T7 ──→ T8 ┐ ──→ [Done]
  Round 3:                                    T9 ┘
                                             T10 ──→ [Done]

Strict critical path: T1 → T4 → T6 → T7 → T8 → T10 (minimum 6 sequential rounds)
Practical duration: ~12–15 hours sequential, ~8–10 hours with max parallelism.
```

---

## Resolved Decisions

### Decision 1: Dismissal Persistence (RESOLVED — `feature-spec.md` is authoritative)

- **Issue**: `research-technical.md` suggested dismissals might be in-memory only or deferred.
- **Resolution**: `feature-spec.md` is the single source of truth. Dismissals are a full SQLite table (`suggestion_dismissals`, v17) with 30-day auto-expiry, implemented in T5. The `protondb_dismiss_suggestion` command in T6 is a real write, not a no-op.
- **Impact**: T5 and T6 both include dismissal write logic.

### Decision 2: ConfigRevisionSource Enum Location (RESOLVED — `metadata/models.rs:382`)

- **Issue**: Earlier docs referred to `config_history_store.rs` as the location.
- **Resolution**: The enum is at `metadata/models.rs:382`. The `config_history_store.rs` file only uses the enum; it does not define it. T2 modifies `models.rs` exclusively.
- **Impact**: T2 scope is precise and isolated.

### Decision 3: T10 is a Separate Task from T8 (RESOLVED)

- **Issue**: `ProtonDbLookupCard` wiring and `OnboardingWizard` integration seemed like one task.
- **Resolution**: They are distinct concerns. T8 focuses on the card component UI in isolation; T10 handles wizard-specific state threading (local profile building, no profile save calls during wizard flow). Splitting prevents scope creep and merge conflicts.
- **Impact**: T8 has tighter scope; T10 has clear wizard-specific constraints.

### Decision 4: T1 Ships as Isolated Security Fix First (RESOLVED)

- **Reason**: RESERVED_ENV_KEYS expansion closes a critical security gap in _existing_ production ProtonDB code, independent of this feature.
- **Action**: T1 is reviewed, tested, and merged as a standalone PR before any other work begins.
- **Impact**: Phase 0 gate is unblocked on T1 shipping; other Phase 0 work can proceed in parallel.

---

## Task-by-Task Specifications

### T1 — Expand RESERVED_ENV_KEYS Blocklist

**Priority**: CRITICAL — blocks all apply flows until merged.

**File**: `src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs`

**Line numbers**: `RESERVED_ENV_KEYS` at lines 10–14; `safe_env_var_suggestions()` function.

**Changes**:

1. Replace the 3-entry `RESERVED_ENV_KEYS` list with the expanded list:
   - `LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT`, `LD_DEBUG`, `LD_ORIGIN_PATH`, `LD_PROFILE`, `PATH`, `HOME`, `ZDOTDIR`, `SHELL`, `ENV`, `BASH_ENV`, `NODE_OPTIONS`, `PYTHONPATH`, `RUBYLIB`, `PERL5LIB`

2. Add a new constant after `RESERVED_ENV_KEYS`:

   ```rust
   const BLOCKED_ENV_KEY_PREFIXES: &[&str] = &["STEAM_COMPAT_", "LD_"];
   ```

3. Update `safe_env_var_suggestions()` guard to check both exact matches and prefixes:

   ```rust
   if RESERVED_ENV_KEYS.contains(&key) || BLOCKED_ENV_KEY_PREFIXES.iter().any(|p| key.starts_with(p)) {
       // reject
   }
   ```

4. **Do NOT** include in T1: making `is_safe_env_key()` or `is_safe_env_value()` public. That is T4's concern.

**Tests** (added in T9, not here):

- `ld_preload_is_rejected_as_env_suggestion()`
- `path_is_rejected_as_env_suggestion()`
- `ld_prefix_keys_are_rejected()`

**Readiness checklist**:

- [ ] Reviewed `research-security.md` for exact blocklist values
- [ ] Confirmed no other code references `RESERVED_ENV_KEYS` outside `aggregation.rs` and `CustomEnvironmentVariablesSection.tsx`
- [ ] Compiled locally: `cargo build --manifest-path src/crosshook-native/Cargo.toml`
- [ ] Existing tests still pass: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

**Merge criteria**:

- All tests pass
- Code review approved
- No blocking CI errors
- Ship in isolation — do not mix with other Phase 1 work

---

### T2 — Add ConfigRevisionSource::ProtonDbSuggestion Variant

**Priority**: CRITICAL — required for T6 to compile.

**File**: `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`

**Line number**: `ConfigRevisionSource` enum at line 382.

**Changes**:

1. Add new variant to enum:

   ```rust
   ProtonDbSuggestion,
   ```

2. Update the `as_str()` method to return `"protondb_suggestion"` for the new variant.

3. If a `Display` or `Serialize` impl exists separately, update those too (verify by searching for `impl` near line 382).

**Scope**:

- **Include**: Enum variant definition + `as_str()` method
- **Exclude**: Any config history logic or `config_revisions` table schema (no schema change needed — the column is already `TEXT`)

**Readiness checklist**:

- [ ] Located enum at exact line 382
- [ ] Confirmed `as_str()` is the only serialization method
- [ ] Compiled: `cargo build --manifest-path src/crosshook-native/Cargo.toml`
- [ ] No existing tests reference `ConfigRevisionSource` values that would break

**Merge criteria**:

- Compiles cleanly
- Code review approved
- No test regressions

---

### T3 — Add TypeScript Suggestion Type Mirrors

**Priority**: CRITICAL — blocks T7, T8, T10 from type-checking.

**File**: `src/crosshook-native/src/types/protondb.ts`

**Changes**:
Add or extend type definitions to match the Rust structs from `suggestions.rs`. Key types (use **camelCase** for all properties — Tauri serializes Rust `snake_case` to JS `camelCase` via `#[serde(rename_all = "camelCase")]`):

```typescript
// Status enum
export type SuggestionStatus = 'new' | 'already_applied' | 'conflict' | 'dismissed';

// Tier 1 (Catalog)
export interface CatalogSuggestionItem {
  catalogEntryId: string; // ID from optimization catalog
  catalogEntryName: string;
  status: SuggestionStatus;
  notes?: string; // Optional confidence/warning note
}

// Tier 2 (Env vars)
export interface EnvVarSuggestionItem {
  envKey: string;
  envValue: string;
  sourceLabel: string; // e.g. "Reported by 127 users"
  status: SuggestionStatus;
  notes?: string;
}

// Wrapper (only one of the above per suggestion)
export type SuggestionItem = CatalogSuggestionItem | EnvVarSuggestionItem;

// Full result set
export interface ProtonDbSuggestionSet {
  appId: string;
  suggestions: SuggestionItem[];
  cache?: {
    fetchedAt: string; // ISO timestamp
    expiresAt: string;
    isStale: boolean; // TTL exceeded but data returned
  };
}

// Request to accept a suggestion (tagged union for tier routing)
export type AcceptSuggestionRequest =
  | { kind: 'catalog'; profileName: string; catalogEntryId: string }
  | { kind: 'env_var'; profileName: string; envKey: string; envValue: string };

// Result after accept
export interface AcceptSuggestionResult {
  profileName: string;
  appliedCatalogIds: string[];
  appliedEnvKeys: string[];
  conflicts?: Record<string, { existing: string; new: string }>;
}
```

**Scope**:

- **Include**: All types needed for T6 command signatures and T7 hook state
- **Exclude**: No changes to existing types like `ProtonDbLookupResult` or `ProtonDbEnvVarSuggestion` (those are separate)

**Readiness checklist**:

- [ ] Confirmed `camelCase` serialization by checking existing `ProtonDbLookupResult` types in same file
- [ ] Verified enum string literals match Rust struct definitions from `research-technical.md`
- [ ] Type-checked: `npm run type-check` or equivalent
- [ ] Confirmed no conflicts with existing type names in `protondb.ts`

**Merge criteria**:

- Compiles without TS errors
- Code review approved (especially serialization consistency)
- Can be round-trip tested with T6 once commands exist

---

### T4 — Create suggestions.rs Engine

**Priority**: HIGH — gate on T1 completion; gates T6.

**Files**:

- `src/crosshook-native/crates/crosshook-core/src/protondb/suggestions.rs` (new)
- `src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs` (modify to make fns `pub(crate)`)
- `src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs` (modify to add `pub mod suggestions;`)

**Core function signature**:

```rust
pub fn derive_suggestions(
    lookup: &ProtonDbLookupResult,
    profile: &GameProfile,
    catalog: &[OptimizationEntry],
    dismissed_keys: &HashSet<String>,
) -> ProtonDbSuggestionSet
```

**Key structs** (copy exact definitions from `research-technical.md`):

- `SuggestionStatus` enum: `New`, `AlreadyApplied`, `Conflict`, `Dismissed`
- `CatalogSuggestionItem`
- `EnvVarSuggestionItem`
- `LaunchOptionSuggestionItem` (if catalog entry has launch options)
- `ProtonDbSuggestionSet` — wraps `Vec<SuggestionItem>`
- `AcceptSuggestionRequest` — tagged enum for tier routing
- `AcceptSuggestionResult` — write result with applied IDs and conflicts

**Helper functions** (private within suggestions.rs):

- `build_catalog_env_index(catalog: &[OptimizationEntry]) -> HashMap<(String, String), String>` — build Tier 1 oracle at runtime
- `status_from_profile(key, value, profile) -> SuggestionStatus` — compute `AlreadyApplied` vs `Conflict` vs `New`

**Other changes**:

1. In `aggregation.rs`: Change `is_safe_env_key()` and `is_safe_env_value()` visibility to `pub(crate)` (so `suggestions.rs` can re-validate at write time in T6).
2. In `mod.rs`: Add `pub mod suggestions;` and re-export public types.

**Scope**:

- **Include**: All suggestion derivation logic, tiering, classification, status computation
- **Exclude**: Tauri IPC layer (T6), dismissal storage (T5), dismissal read filtering (T7 will pass dismissed set)

**Dependencies**:

- **Hard**: T1 (so the expanded blocklist is in place when `suggestions.rs` references `safe_env_var_suggestions()` output)

**Readiness checklist**:

- [ ] Reviewed `research-technical.md` sections on Tier 1/2 classification
- [ ] Confirmed `OptimizationEntry` struct definition and `env: Vec<[String; 2]>` field
- [ ] Confirmed `global_catalog()` function exists in `launch/catalog.rs`
- [ ] Test-first approach: write T9 test cases in parallel before finalizing implementation
- [ ] Compiled: `cargo build --manifest-path src/crosshook-native/Cargo.toml`
- [ ] Unit tests pass (including new T9 tests if written in parallel)

**Merge criteria**:

- All tests pass (including T9)
- Code review approved
- No unsafe code without justification
- Cargo clippy clean

---

### T5 — Schema v17 Migration + Dismissal Store

**Priority**: HIGH — gates T6.

**Files**:

- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`

**Migration in migrations.rs**:

Location: After the existing v16 block (around line 147–154). Add:

```rust
if version < 17 {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS suggestion_dismissals (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id     TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            app_id         TEXT NOT NULL,
            suggestion_key TEXT NOT NULL,
            dismissed_at   TEXT NOT NULL,
            expires_at     TEXT NOT NULL
        );",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_suggestion_dismissals_profile_app
            ON suggestion_dismissals(profile_id, app_id);",
        [],
    )?;
    conn.pragma_update(None, "user_version", 17)?;
}
```

**Store methods in metadata/mod.rs** (or optional new `metadata/suggestion_store.rs`):

```rust
pub fn dismiss_suggestion(
    &self,
    profile_id: &str,
    app_id: &str,
    suggestion_key: &str,
    ttl_days: u32,
) -> Result<(), MetadataStoreError>

pub fn get_dismissed_keys(
    &self,
    profile_id: &str,
    app_id: &str,
) -> Result<HashSet<String>, MetadataStoreError>
    // Must evict expired rows on read

pub fn evict_expired_dismissals(&self) -> Result<usize, MetadataStoreError>
```

**Scope**:

- **Include**: Migration logic, table creation, index, store methods for dismissal read/write
- **Exclude**: Tauri command implementation (T6), dismissal UI flow (T8)

**Dependencies**:

- None (independent of T1, T2, T3, T4)

**Readiness checklist**:

- [ ] Confirmed current schema version is v16 (line ~147 in migrations.rs)
- [ ] Reviewed `feature-spec.md` dismissal persistence section for TTL (30 days)
- [ ] Confirmed `profiles` table has `profile_id` column for foreign key
- [ ] Compiled: `cargo build --manifest-path src/crosshook-native/Cargo.toml`
- [ ] Tested migration: created in-memory store and ran full migration suite
- [ ] Verified `ON DELETE CASCADE` is correctly applied

**Merge criteria**:

- Compiles cleanly
- Migration tested (forward migration to v17, no regressions)
- Code review approved
- New store methods tested with in-memory metadata store

---

### T6 — Add 3 Tauri Commands + Register

**Priority**: HIGH — gates T7; depends on T2, T4, T5.

**Files**:

- `src/crosshook-native/src-tauri/src/commands/protondb.rs`
- `src/crosshook-native/src-tauri/src/lib.rs`

**New commands** (follow the existing `protondb_lookup` pattern exactly):

```rust
#[tauri::command]
pub async fn protondb_get_suggestions(
    app_id: String,
    profile_name: String,
    force_refresh: Option<bool>,
    state: tauri::State<'_, Store>,
) -> Result<ProtonDbSuggestionSet, String> {
    // 1. Call lookup_protondb() with force_refresh
    // 2. Call derive_suggestions() with dismissed keys from metadata store
    // 3. Return ProtonDbSuggestionSet (maps errors to String)
}

#[tauri::command]
pub async fn protondb_accept_suggestion(
    request: AcceptSuggestionRequest,
    profile_name: String,
    state: tauri::State<'_, Store>,
) -> Result<AcceptSuggestionResult, String> {
    // 1. Load profile from store
    // 2. Re-validate env key/value with is_safe_env_key(), is_safe_env_value(), RESERVED_ENV_KEYS
    // 3. Route on request.kind (catalog vs env_var)
    // 4. Apply to profile: custom_env_vars and/or enabled_option_ids
    // 5. Save profile via store.save()
    // 6. Call observe_profile_write() then capture_config_revision(ConfigRevisionSource::ProtonDbSuggestion)
    // 7. Return updated profile (normalized for frontend)
}

#[tauri::command]
pub async fn protondb_dismiss_suggestion(
    profile_name: String,
    app_id: String,
    suggestion_key: String,
    state: tauri::State<'_, Store>,
) -> Result<(), String> {
    // 1. Load profile to get profile_id
    // 2. Call metadata_store.dismiss_suggestion(profile_id, app_id, suggestion_key, ttl_days=30)
    // 3. Return () or error string
}
```

**Register in lib.rs `invoke_handler!` macro**:

```rust
invoke_handler![
    // ... existing commands ...
    protondb_get_suggestions,
    protondb_accept_suggestion,
    protondb_dismiss_suggestion,
]
```

**CRITICAL**: `protondb_accept_suggestion` MUST:

- Re-run `is_safe_env_key()`, `is_safe_env_value()`, and the expanded `RESERVED_ENV_KEYS` check (plus prefix check) at write time
- **Do NOT trust the suggestion struct as pre-validated**
- Record config revision with `ConfigRevisionSource::ProtonDbSuggestion` (added in T2)
- Follow the canonical pattern from `profile_apply_bundled_optimization_preset` for the atomic write path

**Scope**:

- **Include**: All three command implementations, error mapping, IPC registration
- **Exclude**: Frontend hook (T7), UI rendering (T8)

**Dependencies**:

- **Hard**: T2 (ConfigRevisionSource variant), T4 (derive_suggestions, re-validation functions), T5 (dismissal store methods)

**Readiness checklist**:

- [ ] Reviewed existing `protondb_lookup` command pattern in `commands/protondb.rs`
- [ ] Reviewed `profile_apply_bundled_optimization_preset` for canonical write-path pattern
- [ ] Confirmed `State<'_, Store>` pattern matches existing commands
- [ ] Confirmed Serde serialization on `AcceptSuggestionRequest` enum matches T3 TypeScript types
- [ ] Compiled: `cargo build --manifest-path src/crosshook-native/Cargo.toml`
- [ ] Commands appear in Tauri command list: verify via Tauri logs or DevTools

**Merge criteria**:

- Compiles cleanly
- Error mapping is complete (no panics)
- Code review approved (especially re-validation logic)
- IPC registration verified
- Manual testing: can invoke commands from frontend

---

### T7 — Create useProtonDbSuggestions Hook

**Priority**: HIGH — gates T8, T10; depends on T3, T6.

**File**: `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts` (new)

**Pattern**: Mirror `useProtonDbLookup.ts` exactly, but for suggestions.

```typescript
import { useState, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ProtonDbSuggestionSet, AcceptSuggestionRequest } from '../types/protondb';

export function useProtonDbSuggestions() {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [data, setData] = useState<ProtonDbSuggestionSet | null>(null);
  const requestIdRef = useRef(0);

  const getSuggestions = useCallback(async (appId: string, profileName: string, forceRefresh?: boolean) => {
    const requestId = ++requestIdRef.current;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<ProtonDbSuggestionSet>('protondb_get_suggestions', {
        app_id: appId,
        profile_name: profileName,
        force_refresh: forceRefresh ?? false,
      });
      if (requestIdRef.current === requestId) {
        setData(result);
      }
    } catch (err) {
      if (requestIdRef.current === requestId) {
        setError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      if (requestIdRef.current === requestId) {
        setLoading(false);
      }
    }
  }, []);

  const acceptSuggestion = useCallback(async (request: AcceptSuggestionRequest) => {
    try {
      await invoke('protondb_accept_suggestion', { request });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      throw err;
    }
  }, []);

  const dismissSuggestion = useCallback(async (profileName: string, appId: string, suggestionKey: string) => {
    try {
      await invoke('protondb_dismiss_suggestion', {
        profile_name: profileName,
        app_id: appId,
        suggestion_key: suggestionKey,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      throw err;
    }
  }, []);

  return {
    loading,
    error,
    data,
    getSuggestions,
    acceptSuggestion,
    dismissSuggestion,
  };
}
```

**Scope**:

- **Include**: Hook state, race-safe request ID, callback methods, IPC wrapping
- **Exclude**: UI rendering (T8), integration into components (T8, T10)

**Dependencies**:

- **Hard**: T3 (TypeScript types), T6 (command names and signatures)

**Readiness checklist**:

- [ ] Located `useProtonDbLookup.ts` for reference pattern
- [ ] Confirmed `requestIdRef` race-safety pattern
- [ ] Verified TypeScript types from T3 are available
- [ ] Type-checked: `npm run type-check`
- [ ] Tested hook locally: can invoke via Tauri DevTools

**Merge criteria**:

- Compiles without TS errors
- Code review approved
- Matches `useProtonDbLookup` pattern closely
- Tested with mock IPC layer

---

### T8 — Wire Apply/Dismiss into ProtonDbLookupCard

**Priority**: HIGH — gates T10; depends on T6, T7.

**File**: `src/crosshook-native/src/components/ProtonDbLookupCard.tsx`

**Current state**: Component renders recommendation groups; `onApplyEnvVars` callback exists but catalog-matching path is not implemented.

**Changes**:

1. **Accept `suggestionSet` prop** (or invoke `useProtonDbSuggestions` hook internally):

   ```typescript
   interface ProtonDbLookupCardProps {
     appId: string;
     suggestionSet: ProtonDbSuggestionSet;
     onApplySuggestion?: (request: AcceptSuggestionRequest) => Promise<void>;
     onDismissSuggestion?: (profileName: string, appId: string, key: string) => void;
   }
   ```

2. **Render Tier 1 suggestions** (catalog-matched optimizations):
   - Distinct visual treatment from Tier 2 (e.g., button-style toggles, different badge color)
   - Button text: "Enable [catalogEntryName]"
   - Click → calls `acceptSuggestion({ kind: 'catalog', ... })`

3. **Render Tier 2 suggestions** (raw env vars):
   - Existing "Apply" button for each env var
   - Show source label (e.g., "Reported by 127 users")
   - Click → opens `ProtonDbOverwriteConfirmation` modal if conflicts exist
   - Modal confirm → calls `acceptSuggestion({ kind: 'env_var', ... })`

4. **Status messaging** on apply:
   - "Applied X optimizations and Y env vars"
   - Clear any error state after 3 seconds

5. **Per-suggestion dismiss**:
   - "X" button or trash icon on each suggestion
   - Click → calls `dismissSuggestion(profileName, appId, suggestionKey)`
   - Optimistic update: remove from list immediately, revert on error

6. **Cache staleness indicator**:
   - If `suggestionSet.cache?.isStale` is true, show amber/warning banner: "This data may be outdated; consider refreshing."

7. **XSS prevention**:
   - All ProtonDB-derived text (notes, source labels, names) rendered via plain React text interpolation
   - **Never use `dangerouslySetInnerHTML`**

8. **Scroll container** (if needed):
   - If a new `overflow-y: auto` container is added for suggestions list, register it in `useScrollEnhance.ts::SCROLLABLE`

**Scope**:

- **Include**: UI wiring, suggestion rendering, apply/dismiss callbacks, modal integration, staleness indicator, XSS protection
- **Exclude**: Deduplication of dual Apply paths (`LaunchPage.tsx` / `ProfileFormSections.tsx`) — that is a follow-up refactor

**Dependencies**:

- **Hard**: T6 (commands exist), T7 (hook with `acceptSuggestion`/`dismissSuggestion`)

**Readiness checklist**:

- [ ] Reviewed existing `ProtonDbLookupCard.tsx` (~15k) structure
- [ ] Reviewed `ProtonDbOverwriteConfirmation.tsx` modal for reuse
- [ ] Confirmed TypeScript types from T3 are accurately represented in UI
- [ ] Tested with mock suggestion data: catalog items, env vars, conflicts, stale data
- [ ] XSS audit: no `dangerouslySetInnerHTML`, all text interpolated plainly
- [ ] Scroll container registered in `useScrollEnhance.ts` if needed

**Merge criteria**:

- Renders suggestions correctly (both tiers)
- Apply/dismiss workflows tested
- Modal integration works
- No XSS vulnerabilities
- Code review approved
- Snapshot tests (if using visual regression testing)

---

### T9 — Unit Tests (Blocklist + Catalog Bridge)

**Priority**: MEDIUM — depends on T1, T4.

**File**: `src/crosshook-native/crates/crosshook-core/src/protondb/tests.rs`

**New test cases** (add to existing file; do not replace):

```rust
#[test]
fn ld_preload_is_rejected_as_env_suggestion() {
    let mut feed = vec![/* ProtonDbEnvVarSuggestion with LD_PRELOAD=/evil.so */];
    let result = safe_env_var_suggestions(&feed);
    assert!(!result.iter().any(|s| s.env_key == "LD_PRELOAD"));
}

#[test]
fn path_is_rejected_as_env_suggestion() {
    let mut feed = vec![/* ProtonDbEnvVarSuggestion with PATH=... */];
    let result = safe_env_var_suggestions(&feed);
    assert!(!result.iter().any(|s| s.env_key == "PATH"));
}

#[test]
fn ld_prefix_keys_are_rejected() {
    let feed = vec![
        /* LD_LIBRARY_PATH, LD_DEBUG, LD_ORIGIN_PATH, etc. */
    ];
    let result = safe_env_var_suggestions(&feed);
    assert!(result.is_empty() || result.iter().all(|s| !s.env_key.starts_with("LD_")));
}

#[test]
fn catalog_match_maps_dxvk_async() {
    let lookup = ProtonDbLookupResult {
        /* with env var DXVK_ASYNC=1 */
    };
    let profile = GameProfile { /* ... */ };
    let catalog = vec![
        OptimizationEntry {
            id: "enable_dxvk_async",
            env: vec![["DXVK_ASYNC".into(), "1".into()]],
            // ...
        },
    ];
    let suggestions = derive_suggestions(&lookup, &profile, &catalog, &HashSet::new());

    let catalog_items: Vec<_> = suggestions.suggestions.iter()
        .filter_map(|s| match s {
            SuggestionItem::Catalog(c) => Some(c),
            _ => None,
        })
        .collect();
    assert!(catalog_items.iter().any(|c| c.catalog_entry_id == "enable_dxvk_async"));
}

#[test]
fn catalog_match_unmapped_key_stays_in_tier2() {
    let lookup = ProtonDbLookupResult {
        /* with env var UNMAPPED_KEY=value */
    };
    let profile = GameProfile { /* ... */ };
    let catalog = vec![/* no entry with UNMAPPED_KEY */];
    let suggestions = derive_suggestions(&lookup, &profile, &catalog, &HashSet::new());

    let env_items: Vec<_> = suggestions.suggestions.iter()
        .filter_map(|s| match s {
            SuggestionItem::EnvVar(e) => Some(e),
            _ => None,
        })
        .collect();
    assert!(env_items.iter().any(|e| e.env_key == "UNMAPPED_KEY"));
}

#[test]
fn already_applied_status_when_key_matches_profile() {
    let lookup = ProtonDbLookupResult { /* with DXVK_ASYNC=1 */ };
    let mut profile = GameProfile { /* ... */ };
    profile.launch.custom_env_vars.insert("DXVK_ASYNC".into(), "1".into());
    let catalog = vec![/* DXVK_ASYNC→enable_dxvk_async mapping */];

    let suggestions = derive_suggestions(&lookup, &profile, &catalog, &HashSet::new());
    // Check that the status is AlreadyApplied
    assert!(suggestions.suggestions.iter().any(|s| {
        match s {
            SuggestionItem::Catalog(c) if c.catalog_entry_id == "enable_dxvk_async" => {
                c.status == SuggestionStatus::AlreadyApplied
            },
            _ => false,
        }
    }));
}

#[test]
fn conflict_status_when_key_present_with_different_value() {
    let lookup = ProtonDbLookupResult { /* with DXVK_ASYNC=1 */ };
    let mut profile = GameProfile { /* ... */ };
    profile.launch.custom_env_vars.insert("DXVK_ASYNC".into(), "0".into());
    let catalog = vec![/* DXVK_ASYNC→enable_dxvk_async mapping */];

    let suggestions = derive_suggestions(&lookup, &profile, &catalog, &HashSet::new());
    // Check that the status is Conflict
    assert!(suggestions.suggestions.iter().any(|s| {
        match s {
            SuggestionItem::EnvVar(e) if e.env_key == "DXVK_ASYNC" => {
                e.status == SuggestionStatus::Conflict
            },
            _ => false,
        }
    }));
}
```

**Scope**:

- **Include**: All unit tests for blocklist expansion, catalog bridge, status computation
- **Exclude**: Integration tests (those are implicit in T6/T7 when commands are called)

**Dependencies**:

- **Hard**: T1 (blocklist in place), T4 (derive_suggestions function exists)

**Readiness checklist**:

- [ ] Located `protondb/tests.rs` and reviewed existing test patterns
- [ ] Confirmed fixture data availability (e.g., sample `ProtonDbLookupResult`, `GameProfile`)
- [ ] Confirmed `MetadataStore::open_in_memory()` for in-memory test setup
- [ ] Compiled: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- [ ] All tests pass

**Merge criteria**:

- All tests pass
- Coverage for blocklist rejection, catalog matching, status computation
- Code review approved
- Proper test organization and documentation

---

### T10 — Wire Suggestions into OnboardingWizard Step 3

**Priority**: MEDIUM — depends on T7, T8.

**File**: `src/crosshook-native/src/components/OnboardingWizard.tsx`

**Current state**: Step 3 has App ID input at line 513; no `ProtonDbLookupCard` reference.

**Location**: After App ID input in Step 3, also conditioned on `launchMethod === 'steam_applaunch'`.

**Changes**:

1. **Invoke `useProtonDbSuggestions` hook** in Step 3:

   ```typescript
   const {
     data: suggestionSet,
     loading,
     error,
     getSuggestions,
     acceptSuggestion,
     dismissSuggestion,
   } = useProtonDbSuggestions();
   ```

2. **Render `ProtonDbLookupCard`** below the App ID field:
   - Only show if `appId.length >= 5` (avoid noise from partial input)
   - Pass `suggestionSet`, `acceptSuggestion`, `dismissSuggestion` as props
   - Show skeleton loader while `loading` is true

3. **Wire accept path** for wizard context:
   - When user accepts a suggestion, update the in-progress `profile` state (local wizard state, not saved)
   - Use the wizard's `updateProfile` immutable-spread pattern
   - **Do NOT trigger `profile_save` IPC calls** during wizard flow
   - Status: "Applied X optimizations and Y env vars" (local update only)

4. **Wire dismiss path** similarly:
   - Dismiss calls the store (via `dismissSuggestion`)
   - Local UI state updates to hide dismissed suggestion

5. **Form remains interactive**:
   - Suggestion panel does not block other wizard inputs
   - User can apply suggestions in any order with other form steps

6. **XSS and scroll container rules**:
   - All ProtonDB text via plain React interpolation
   - Register any new `overflow-y: auto` container in `useScrollEnhance.ts::SCROLLABLE`

**Scope**:

- **Include**: Hook invocation, card rendering, accept/dismiss wiring, local profile state updates
- **Exclude**: Cross-step integration or changes to other wizard steps (unrelated)

**Dependencies**:

- **Hard**: T7 (hook exists), T8 (ProtonDbLookupCard fully wired)

**Readiness checklist**:

- [ ] Located `OnboardingWizard.tsx` and reviewed Step 3 structure
- [ ] Confirmed wizard's `updateProfile` pattern (immutable spread)
- [ ] Confirmed `launchMethod === 'steam_applaunch'` condition at Step 3
- [ ] Tested with mock suggestion data during wizard flow
- [ ] Verified form remains interactive during suggestion operations
- [ ] XSS audit: plain text interpolation only
- [ ] Type-checked: `npm run type-check`

**Merge criteria**:

- Suggestions panel appears in wizard at correct step
- Apply/dismiss work in wizard context (no profile_save calls)
- Form remains interactive
- No XSS vulnerabilities
- Code review approved
- Tested user flow: complete wizard with suggestions applied

---

## Cross-Cutting Concerns

### Security (S2 + S5)

**S2 — RESERVED_ENV_KEYS Expansion** (T1 critical gate):

- All dangerous keys blocked at aggregation time (T1) and again at accept time (T6 re-validation)
- Expanded blocklist includes `LD_*` prefix check
- Tests verify rejection of `LD_PRELOAD`, `PATH`, and unknown `LD_*` keys

**S5 — XSS Prevention**:

- All ProtonDB-derived text (names, notes, labels, source) use plain React text interpolation
- No `dangerouslySetInnerHTML` anywhere in T8 or T10
- Audit `ProtonDbLookupCard.tsx` before T8 merges

### Type Safety

- **T3 serialization**: Rust `snake_case` → JS `camelCase` via `#[serde(rename_all = "camelCase")]`
- Verify round-trip with T6 commands before T7 merges
- TS types must exactly match Rust struct definitions from `research-technical.md`

### Persistence & Backward Compatibility

- Schema v17 migration (T5) is forward-only; no rollback mechanism needed
- `suggestion_dismissals` table has 30-day TTL; expired rows auto-evicted on read
- Profile saves during suggestions include config revision tracking (T2 variant in T6)
- No changes to existing profile schema or ProtonDB types

### Scroll Containers

- Any new `overflow-y: auto` container in T8 or T10 must be registered in `useScrollEnhance.ts::SCROLLABLE`
- WebKitGTK scroll handling requires this registration

---

## Implementation Checklist

### Pre-Implementation (All Teams)

- [ ] Read `feature-spec.md` (authoritative for scope and decisions)
- [ ] Read `research-security.md` (S2 blocklist expansion details)
- [ ] Read `research-technical.md` (exact struct definitions, command signatures)
- [ ] All teammates have access to this guide

### Phase 0 Gate (T1, T2, T3 must merge first)

- [ ] T1: RESERVED_ENV_KEYS blocklist expansion ships as isolated security fix
- [ ] T2: ConfigRevisionSource variant added
- [ ] T3: TypeScript types defined
- [ ] All Phase 0 PRs merged before opening any Phase 1 PR

### Phase 1 Execution (After Phase 0 gate)

**Round 1 (Parallel):**

- [ ] T4: suggestions.rs engine (test-first with T9 tests in parallel)
- [ ] T5: Schema v17 migration + dismissal store

**Round 2 (Sequential):**

- [ ] T6: 3 Tauri commands + registration (after T2, T4, T5 merged)

**Round 3 (Sequential):**

- [ ] T7: useProtonDbSuggestions hook (after T3, T6 merged)

**Round 4 (Parallel):**

- [ ] T8: ProtonDbLookupCard wiring (after T6, T7 merged)
- [ ] T9: Unit tests (if not done in parallel with T4)

**Round 5 (Sequential):**

- [ ] T10: OnboardingWizard Step 3 integration (after T7, T8 merged)

### Verification Steps

- [ ] All tests pass: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- [ ] TypeScript compilation clean: `npm run type-check`
- [ ] No clippy warnings: `cargo clippy --manifest-path src/crosshook-native/Cargo.toml`
- [ ] All commands appear in Tauri DevTools
- [ ] No XSS vulnerabilities in T8/T10 (code review)
- [ ] Scroll containers registered in useScrollEnhance.ts if added
- [ ] Config revisions recorded correctly with `ProtonDbSuggestion` source

---

## Decision Log

| Decision                                                                                      | Status   | Authority         | Date       |
| --------------------------------------------------------------------------------------------- | -------- | ----------------- | ---------- |
| Dismissal persistence is full SQLite table with 30-day TTL, not deferred                      | RESOLVED | `feature-spec.md` | 2026-04-04 |
| ConfigRevisionSource enum location is `metadata/models.rs:382`, not `config_history_store.rs` | RESOLVED | Code analysis     | 2026-04-04 |
| T10 is separate task from T8 (distinct wizard state threading concerns)                       | RESOLVED | Task analysis     | 2026-04-04 |
| T1 ships as isolated security fix first, unblocking Phase 0 gate                              | RESOLVED | Security review   | 2026-04-04 |
| Frontend apply path deduplication (LaunchPage/ProfileFormSections) is follow-up work          | RESOLVED | Scope analysis    | 2026-04-04 |

---

## References

- `docs/plans/community-driven-config-suggestions/feature-spec.md` — Consolidated spec (authoritative)
- `docs/plans/community-driven-config-suggestions/shared.md` — Architectural patterns and file map
- `docs/plans/community-driven-config-suggestions/research-technical.md` — Exact struct definitions and command signatures
- `docs/plans/community-driven-config-suggestions/research-security.md` — S2 blocklist expansion, exact values
- `docs/plans/community-driven-config-suggestions/analysis-context.md` — Cross-cutting concerns and constraints
- `docs/plans/community-driven-config-suggestions/analysis-tasks.md` — Detailed task granularity and dependencies
- `AGENTS.md` — Architecture rules, directory map, schema inventory
- `CLAUDE.md` — Agent guidelines for this repository

---

**Generated**: 2026-04-04  
**Version**: 1.0 (Synthesis of validation outputs)
