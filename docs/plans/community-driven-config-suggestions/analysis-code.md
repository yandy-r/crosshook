# Code Analysis: Community-Driven Config Suggestions

## Executive Summary

The existing ProtonDB pipeline is fully implemented end-to-end. The remaining work concentrates on four discrete gaps: expanding the `RESERVED_ENV_KEYS` blocklist (security-critical), adding catalog-matching logic to redirect catalog-matching env vars to `enabled_option_ids` instead of `custom_env_vars`, adding a `ConfigRevisionSource` variant for suggestion-apply tracking, and collapsing the duplicated `applyProtonDbGroup` function present in both `LaunchPage.tsx` and `ProfileFormSections.tsx` into a shared utility.

---

## Relevant Files

### Rust Backend

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs`: Env var extraction from ProtonDB report feeds; `RESERVED_ENV_KEYS` blocklist (3 entries); `safe_env_var_suggestions()` and its helpers `is_safe_env_key()` / `is_safe_env_value()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`: `OptimizationCatalog` and `OptimizationEntry`; `global_catalog()` singleton; `allowed_env_keys: HashSet<String>` derived from all `entry.env` pairs
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/env.rs`: `RESERVED_ENV_KEYS` in `aggregation.rs` must be expanded to mirror the dangerous keys listed in `WINE_ENV_VARS_TO_CLEAR` here (`LD_PRELOAD`, `LD_LIBRARY_PATH`, `PATH`, `HOME`, `LD_*` prefix)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs:382`: `ConfigRevisionSource` enum — must add `ProtonDbSuggestionApply` variant
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store.rs`: `insert_config_revision()` — the store function; no changes needed here but all callers pass a `ConfigRevisionSource`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs`: `capture_config_revision()` private helper; new `profile_apply_protondb_suggestions` Tauri command should follow the `profile_save_launch_optimizations` pattern

### Frontend

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/utils/protondb.ts`: `mergeProtonDbEnvVarGroup()` — the merge utility; must grow a catalog-matching variant or caller-side routing
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/pages/LaunchPage.tsx:194`: First copy of `applyProtonDbGroup` (inline `useCallback`); only writes to `custom_env_vars`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileFormSections.tsx:395`: Second copy of `applyProtonDbGroup` (plain function); identical behavior, same gap
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/utils/optimization-catalog.ts`: `fetchOptimizationCatalog()`, `buildOptionsById()`, `buildConflictMatrix()` — catalog access utilities for the frontend
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProtonDbLookupCard.tsx`: Renders recommendation groups; `onApplyEnvVars` callback wired but no catalog-matching path
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx:6`: `RESERVED_CUSTOM_ENV_KEYS` Set (3 keys) — must expand to mirror the expanded Rust blocklist
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts:676`: `updateProfile(updater)` — the canonical immutable profile update hook

---

## Architectural Patterns

### Pattern 1 — Immutable Profile Update

All profile mutations go through `updateProfile(updater: (current: GameProfile) => GameProfile)` via `setProfile(current => updater(current))`. No mutation in place. The existing `applyProtonDbGroup` in both components already follows this contract correctly:

```tsx
// LaunchPage.tsx:200 / ProfileFormSections.tsx:400 — identical shape
onUpdateProfile((current) => {
  const nextMerge = mergeProtonDbEnvVarGroup(current.launch.custom_env_vars, group, overwriteKeys);
  return {
    ...current,
    launch: {
      ...current.launch,
      custom_env_vars: nextMerge.mergedEnvVars,
    },
  };
});
```

The new catalog-matching apply path must follow the same spread-copy pattern, extending the return value to also update `launch.optimizations.enabled_option_ids`.

### Pattern 2 — Thin Tauri Command Layer

Every command receives `State<'_, Store>`, calls exactly one `crosshook_core` function, and maps errors to `String`. The command for suggestion-apply should follow `profile_save_launch_optimizations`:

```rust
#[tauri::command]
pub fn profile_apply_protondb_suggestions(
    name: String,
    custom_env_vars: HashMap<String, String>,
    enabled_option_ids: Vec<String>,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<GameProfile, String> {
    // load, patch two fields, save, capture_config_revision, return updated
}
```

Returns the updated `GameProfile` so the frontend can `setProfile(normalizeProfileForEdit(updated, optionsById))` without a separate `profile_load` call — matching the `profile_apply_bundled_optimization_preset` return pattern.

### Pattern 3 — `capture_config_revision` Call Convention

Every command that writes a profile calls `capture_config_revision()` with a `ConfigRevisionSource` variant. The private helper is:

```rust
fn capture_config_revision(
    profile_name: &str,
    profile: &GameProfile,
    source: ConfigRevisionSource,
    source_revision_id: Option<i64>,
    metadata_store: &MetadataStore,
) -> Option<i64>
```

Never returns an error — failures are logged with `tracing::warn!` and silently skipped so snapshot capture never blocks a user-facing save.

### Pattern 4 — Pending State Confirmation Modal

Multi-step confirmations use `useState<PendingType | null>(null)`: non-null renders a modal; confirm resets to null. The `PendingProtonDbOverwrite` type and `ProtonDbOverwriteConfirmation` component are the reference implementation. New catalog-match confirmation UI (if any) should follow this exact pattern.

### Pattern 5 — `RESERVED_ENV_KEYS` Dual Enforcement

Safety filtering happens in Rust at aggregation time (`aggregation.rs:10`) and again client-side in `CustomEnvironmentVariablesSection.tsx:6`. Both lists currently contain only 3 keys. The new feature must expand both lists in sync, or the frontend will allow a user to manually enter a dangerous key that the Rust backend would also (should) block at the suggestion stage. The research doc identifies `LD_PRELOAD`, `PATH`, `HOME`, and the `LD_*` prefix as the critical additions.

### Pattern 6 — Race-Safe IPC with `requestIdRef`

`useProtonDbLookup` uses a `requestIdRef` counter to discard stale responses. Any new hook that wraps the `profile_apply_protondb_suggestions` command should guard against concurrent invocations with the same mechanism (or simpler: an `isApplying: boolean` flag, as the apply operation is user-triggered, not polling-based).

### Pattern 7 — Catalog Singleton Access

The Rust-side catalog is a `OnceLock<OptimizationCatalog>` with `global_catalog()` returning a `&'static OptimizationCatalog`. On the frontend, `fetchOptimizationCatalog()` wraps the `get_optimization_catalog` IPC call with a module-level cache (`_cached`). The `useProfile` hook already holds the catalog via `useLaunchOptimizationCatalog()` and exposes `catalog: OptimizationCatalogPayload | null`. Both apply paths can read `catalog.entries` from the parent hook.

---

## Dual Apply Path Gap — Detailed Analysis

The `applyProtonDbGroup` function exists in **two independent locations**, both with identical logic and the same missing catalog-matching step:

### Copy 1 — `LaunchPage.tsx:194-236`

```tsx
const applyProtonDbGroup = useCallback(
  (group: ProtonDbRecommendationGroup, overwriteKeys: readonly string[]) => {
    profileState.updateProfile((current) => {
      const nextMerge = mergeProtonDbEnvVarGroup(current.launch.custom_env_vars, group, overwriteKeys);
      return {
        ...current,
        launch: { ...current.launch, custom_env_vars: nextMerge.mergedEnvVars },
      };
    });
    // sets status messages...
  },
  [profileState.updateProfile]
);
```

Context: `LaunchPage` has access to `profileState` (full `UseProfileResult`), including `catalog`.

### Copy 2 — `ProfileFormSections.tsx:395-434`

```tsx
const applyProtonDbGroup = (group: ProtonDbRecommendationGroup, overwriteKeys: readonly string[]) => {
  onUpdateProfile((current) => {
    const nextMerge = mergeProtonDbEnvVarGroup(current.launch.custom_env_vars, group, overwriteKeys);
    return {
      ...current,
      launch: { ...current.launch, custom_env_vars: nextMerge.mergedEnvVars },
    };
  });
  // sets status messages...
};
```

Context: `ProfileFormSections` receives `onUpdateProfile` as a prop and does **not** receive the catalog. The catalog would need to be threaded in as a new prop, or the logic extracted to a utility that accepts the catalog explicitly.

### Recommended Extraction Strategy

Extract a pure function `applyProtonDbGroupToProfile(current: GameProfile, group, overwriteKeys, catalog)` into `src/crosshook-native/src/utils/protondb.ts` that:

1. Runs `mergeProtonDbEnvVarGroup()` to split suggestions into catalog-matched and custom
2. For keys in `catalog.allowed_env_keys`: toggle corresponding `enabled_option_ids`
3. For remaining keys: merge into `custom_env_vars`
4. Returns `{ nextProfile, appliedKeys, unchangedKeys, enabledOptionIds }`

Both `LaunchPage.applyProtonDbGroup` and `ProfileFormSections.applyProtonDbGroup` become thin wrappers calling this utility. The status message logic (pure string computation) can also be extracted.

`ProfileFormSections` must receive a new `catalog: OptimizationCatalogPayload | null` prop so the utility can perform catalog matching. Currently the component receives no catalog reference.

---

## Integration Points

### Files to Modify

| File                                                     | Change                                                                                                                            |
| -------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `aggregation.rs`                                         | Expand `RESERVED_ENV_KEYS` to include `LD_PRELOAD`, `LD_LIBRARY_PATH`, `PATH`, `HOME`, and block `LD_*` prefix                    |
| `metadata/models.rs:382`                                 | Add `ProtonDbSuggestionApply` variant to `ConfigRevisionSource` enum and its `as_str()` match arm                                 |
| `src-tauri/src/commands/profile.rs`                      | Add `profile_apply_protondb_suggestions` command; follows `profile_save_launch_optimizations` + `capture_config_revision` pattern |
| `src-tauri/src/lib.rs`                                   | Register new command in `invoke_handler!` macro                                                                                   |
| `src/utils/protondb.ts`                                  | Add `applyProtonDbGroupToProfile()` pure function; thread catalog reference; fix the catalog-matching routing                     |
| `src/components/pages/LaunchPage.tsx:194`                | Replace inline `applyProtonDbGroup` with a call to the extracted utility                                                          |
| `src/components/ProfileFormSections.tsx:395`             | Replace inline `applyProtonDbGroup` with a call to the extracted utility; add `catalog` prop                                      |
| `src/components/CustomEnvironmentVariablesSection.tsx:6` | Expand `RESERVED_CUSTOM_ENV_KEYS` Set to mirror expanded Rust blocklist                                                           |

### Files to Create

None required for the core implementation. The catalog-matching and apply logic can live in the existing `src/utils/protondb.ts`.

---

## Code Conventions

- **Rust command return type**: `Result<GameProfile, String>` for commands that mutate and return the updated profile (matches `profile_apply_bundled_optimization_preset`)
- **Rust error mapping**: `map_err(map_error)` where `fn map_error(e: ProfileStoreError) -> String { e.to_string() }`
- **TS immutable update**: spread all nested objects; never mutate `current` directly
- **TS hook callbacks**: `useCallback` with explicit dependency arrays; no closures over stale refs
- **TS prop threading**: new optional props go at the end of the props interface; use `catalog: OptimizationCatalogPayload | null` so the component renders safely before the catalog loads
- **BEM CSS**: component classes use `crosshook-<component>__<element>--<modifier>` pattern
- **Rust `pub(crate)` visibility**: all module-internal functions use `pub(crate)`, not `pub`
- **Rust `as_str()` pattern**: `ConfigRevisionSource::as_str()` returns `&'static str`; new variant must add a snake_case arm (e.g., `"protondb_suggestion_apply"`)

---

## Dependencies and Services

### Rust side

- `global_catalog()` is available anywhere in `crosshook-core` after startup init
- `MetadataStore` is injected as `State<'_, MetadataStore>` into Tauri commands
- `ProfileStore` is injected as `State<'_, ProfileStore>` into Tauri commands
- No new crate dependencies needed

### Frontend side

- `fetchOptimizationCatalog()` is already called via `useLaunchOptimizationCatalog()`; the result is exposed as `catalog` on `UseProfileResult`
- `mergeProtonDbEnvVarGroup()` in `src/utils/protondb.ts` is the anchor point for the new utility
- `buildOptionsById(catalog.entries)` + `buildConflictMatrix(catalog.entries)` already exist in `src/utils/optimization-catalog.ts` for toggle conflict detection
- `invoke` from `@tauri-apps/api/core` is the standard IPC entry point

---

## Gotchas and Warnings

1. **`RESERVED_ENV_KEYS` in `aggregation.rs` is NOT the same list as `WINE_ENV_VARS_TO_CLEAR` in `env.rs`.** The aggregation blocklist currently only blocks Proton-managed paths (3 keys). The security research doc (S2) requires blocking `LD_PRELOAD`, `LD_LIBRARY_PATH`, `PATH`, `HOME`, and any key matching the `LD_*` prefix. These are not in the current `RESERVED_ENV_KEYS`. This must be addressed before any apply flow ships.

2. **The two `RESERVED_ENV_KEYS` lists must be kept in sync manually** — the Rust `aggregation.rs` constant and the TypeScript `RESERVED_CUSTOM_ENV_KEYS` Set in `CustomEnvironmentVariablesSection.tsx`. There is no compile-time enforcement. A comment in each file references the other; this convention must be maintained and extended.

3. **`is_safe_env_value()` in `aggregation.rs:310` blocks whitespace, `$`, `;`, shell metacharacters.** This means suggestion values with spaces are dropped at aggregation time. The frontend `mergeProtonDbEnvVarGroup()` does not re-validate; it trusts the Rust-side pre-filtering. Do not add client-side value validation that contradicts the Rust filtering — the frontend receives only already-validated suggestions.

4. **`ProfileFormSections` does not currently receive `catalog`.** Adding catalog-matching to the `ProfileFormSections.applyProtonDbGroup` path requires adding a `catalog: OptimizationCatalogPayload | null` prop and threading it down from callers. The component is used in at least the Onboarding Wizard (step 3) and the Profiles page — both callers must be updated.

5. **`profile_apply_protondb_suggestions` must call `observe_profile_write()` before `capture_config_revision()`.** Every command that writes a profile in `profile.rs` follows: `store.save()` → `metadata_store.observe_profile_write()` → `capture_config_revision()`. Skipping `observe_profile_write` breaks the metadata sync pipeline (profile hash tracking, drift detection).

6. **`OptimizationCatalog.allowed_env_keys` is the Rust-side source of truth** for which env keys belong to catalog entries. On the frontend, `catalog.entries[i].env` is the equivalent. When routing a ProtonDB suggestion: check `catalog.entries.flatMap(e => e.env).some(([k]) => k === suggestedKey)` — if yes, find the matching entry and toggle `enabled_option_ids`, not `custom_env_vars`.

7. **Multiple catalog entries may share the same env key** (e.g., different values for the same key). When a ProtonDB suggestion matches a key shared across multiple entries, the routing logic must select the entry whose `env` pair value matches the suggestion value exactly. If no value match, fall back to `custom_env_vars`.

8. **`LaunchPage.tsx:71` and `ProfileFormSections.tsx:375` both independently reset ProtonDB modal state** on `profileName` / `resolvedAppId` / `launchMethod` change via a `useEffect`. The shared utility must not carry any component-local state — it must remain a pure transformation function.

9. **`enqueueLaunchProfileWrite` serializes disk writes.** Any new `invoke` call that writes to a profile in `LaunchPage` or `useProfile` should be enqueued through this chain reference to prevent optimizations-autosave from clobbering a concurrent suggestion-apply write.

10. **Content-hash deduplication in `insert_config_revision`** means that if a suggestion-apply produces no net change to the profile TOML (e.g., all keys already matched), the revision row is skipped. This is correct behavior — no action needed.

---

## Task-Specific Guidance

### S2 — Expand `RESERVED_ENV_KEYS` (security-critical, must ship first)

**Rust change** (`aggregation.rs:10`): Add at minimum `LD_PRELOAD`, `PATH`, `HOME`, `LD_LIBRARY_PATH`. Add a prefix-check branch alongside the existing `starts_with("STEAM_COMPAT_")` branch to block all `LD_`-prefixed keys:

```rust
if RESERVED_ENV_KEYS.contains(&normalized_key)
    || normalized_key.starts_with("STEAM_COMPAT_")
    || normalized_key.starts_with("LD_")     // new: block all LD_ vars
    || normalized_key == "PATH"               // new
    || normalized_key == "HOME"               // new
{
    continue;
}
```

**Frontend change** (`CustomEnvironmentVariablesSection.tsx:6`): Expand the `Set` to match. The comment referencing `launch/request.rs` must be updated to also reference `protondb/aggregation.rs`.

**Tests** (`protondb/tests.rs`): Add test cases asserting that `LD_PRELOAD=something`, `PATH=/usr/bin`, `HOME=/root` are not present in the output of `safe_env_var_suggestions()`.

### Catalog Matching — New Tauri Command

Pattern: mirror `profile_save_launch_optimizations` but also accept `custom_env_vars_patch: HashMap<String, String>` in addition to `enabled_option_ids: Vec<String>`. Load profile, apply both patches, save, observe, capture revision with `ConfigRevisionSource::ProtonDbSuggestionApply`, return updated profile.

### `ConfigRevisionSource` — New Variant

`metadata/models.rs:382`: Add variant and `as_str()` arm. Because `ConfigRevisionSource` is serialized to SQLite as a TEXT column, no migration is needed — adding a new variant does not break existing rows.

### Frontend Deduplication

Create `applyProtonDbGroupToProfile` in `src/utils/protondb.ts`. Signature:

```ts
export function applyProtonDbGroupToProfile(
  current: GameProfile,
  group: ProtonDbRecommendationGroup,
  overwriteKeys: readonly string[],
  catalog: OptimizationCatalogPayload | null
): { nextProfile: GameProfile; appliedKeys: string[]; unchangedKeys: string[]; toggledOptionIds: string[] };
```

Both `LaunchPage` and `ProfileFormSections` replace their inline `applyProtonDbGroup` with a call to this function inside their respective `updateProfile` / `onUpdateProfile` wrappers.

---

## Supplemental Findings

### `protondb_lookup` Command — Exact Pattern for New ProtonDB Commands

`src-tauri/src/commands/protondb.rs` is the thinnest command in the codebase (13 lines total):

```rust
#[tauri::command]
pub async fn protondb_lookup(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProtonDbLookupResult, String> {
    let metadata_store = metadata_store.inner().clone();
    Ok(lookup_protondb(&metadata_store, &app_id, force_refresh.unwrap_or(false)).await)
}
```

Key observations:

- `async fn` only because `lookup_protondb` is async (HTTP fetch). The new `profile_apply_protondb_suggestions` command is synchronous — use `fn`, not `async fn`.
- `metadata_store.inner().clone()` is used to move the store into the async block. For sync commands, pass `&metadata_store` directly as done in `profile.rs`.
- Returns `Ok(...)` directly — no `map_err` needed here because `lookup_protondb` returns `ProtonDbLookupResult` (not `Result`). The apply command will return `Result<GameProfile, String>` and use `map_err(map_error)`.

### SQLite Migration Pattern — Current Schema v16, Next is v17

The `run_migrations()` function in `migrations.rs` uses sequential `if version < N` guards with `PRAGMA user_version` updates. Current schema is **v16** (last block at line 147-154). Adding the `suggestion_dismissals` table for the dismissal feature requires:

1. Add `fn migrate_16_to_17(conn: &Connection) -> Result<(), MetadataStoreError>` with `CREATE TABLE IF NOT EXISTS suggestion_dismissals (...)` and `conn.execute_batch(...)` returning `map_err` with action `"run metadata migration 16 to 17"`.
2. Add an `if version < 17` block in `run_migrations()` after the v16 block, calling `migrate_16_to_17(conn)?` then `pragma_update(17)`.
3. Add a test `fn migration_16_to_17_creates_suggestion_dismissals_table()` following the pattern in existing tests (lines 770–). Tests call `open_test_db()` (which calls `run_migrations()`), then verify the table exists via `conn.execute("SELECT 1 FROM suggestion_dismissals LIMIT 0", [])`.

No FOREIGN KEY constraints exist in earlier migrations without explicit `PRAGMA foreign_keys = ON` — the codebase does not enable FK enforcement at runtime so FK declarations are for documentation only.

### Test Patterns in `protondb/tests.rs`

Two test patterns used:

**Pattern A — Pure aggregation test** (no store needed):

```rust
#[test]
fn safe_env_suggestion_parsing_accepts_supported_key_value_fragments() {
    let groups = normalize_report_feed(feed(vec![ProtonDbReportEntry { ... }]));
    assert_eq!(groups[0].env_vars[0].key, "PROTON_USE_WINED3D");
}
```

Uses `feed(Vec<ProtonDbReportEntry>)` helper + `normalize_report_feed()` directly. No async, no store.

**Pattern B — Store-backed integration test** (cache behavior):

```rust
#[test]
fn stale_cache_is_returned_when_live_lookup_fails() {
    let store = MetadataStore::open_in_memory().expect("open metadata store");
    // seed cache entry
    store.put_cache_entry(...).expect("seed");
    let result = runtime().block_on(super::lookup_protondb(&store, app_id, false));
    assert_eq!(result.state, ProtonDbLookupState::Stale);
}
```

Uses `MetadataStore::open_in_memory()` + `runtime().block_on(...)` for async. The `runtime()` helper is `Builder::new_current_thread().enable_all().build()`.

New tests for the expanded `RESERVED_ENV_KEYS` security gate should follow **Pattern A** — they test `safe_env_var_suggestions()` directly (or indirectly via `normalize_report_feed()`) without needing a store:

```rust
#[test]
fn safe_env_suggestion_blocks_ld_preload() {
    let groups = normalize_report_feed(feed(vec![ProtonDbReportEntry {
        responses: ProtonDbReportResponses {
            launch_options: "LD_PRELOAD=/evil.so DXVK_ASYNC=1 %command%".to_string(),
            ..Default::default()
        },
        ..Default::default()
    }]));
    // LD_PRELOAD should be blocked; DXVK_ASYNC=1 should pass
    let keys: Vec<&str> = groups.iter()
        .flat_map(|g| g.env_vars.iter())
        .map(|e| e.key.as_str())
        .collect();
    assert!(!keys.contains(&"LD_PRELOAD"), "LD_PRELOAD must be blocked");
    assert!(keys.contains(&"DXVK_ASYNC"), "safe key must pass through");
}
```

### Function Visibility for Accept Command Re-validation

`is_safe_env_key()` and `is_safe_env_value()` in `aggregation.rs` are currently private (`fn`, not `pub(crate)`). The accept/apply Tauri command in `profile.rs` (in the `src-tauri` crate) needs to re-validate keys at write time. Two options:

1. **Re-export as `pub(crate)`** — change `fn is_safe_env_key` and `fn is_safe_env_value` to `pub(crate) fn` in `aggregation.rs` so the `protondb` module can expose them for use in crosshook-core business logic. The Tauri command then calls a core-layer function that uses them.
2. **Wrap in a core-layer validation function** — add `pub(crate) fn validate_suggestion_env_vars(vars: &HashMap<String, String>) -> HashMap<String, String>` in `aggregation.rs` or a new `protondb/validation.rs` that filters the map through both the key/value safety checks and the expanded `RESERVED_ENV_KEYS`. The Tauri command calls this wrapper. This is the cleaner boundary.

Option 2 is preferred — it keeps the validation logic in the protondb module and gives the command a single call site that applies all security rules atomically.
