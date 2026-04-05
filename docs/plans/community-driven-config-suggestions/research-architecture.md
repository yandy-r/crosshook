# Architecture Research: community-driven-config-suggestions

## System Overview

CrossHook is a native Linux Tauri v2 desktop app (AppImage) with a layered architecture: business logic lives entirely in the `crosshook-core` Rust crate; `src-tauri` provides thin Tauri command wrappers that manage app state and route IPC; the React/TypeScript frontend (`src/crosshook-native/src/`) invokes those commands via `invoke()` hooks. The ProtonDB feature is already fully implemented end-to-end — `protondb/client.rs` fetches and caches reports, `protondb/aggregation.rs` extracts and frequency-ranks env vars, and `ProtonDbLookupCard.tsx` renders them. The remaining work is wiring catalog-matching for known optimization IDs, expanding the env var blocklist, and confirming the Apply flow is complete in all profile-editing surfaces.

## Relevant Components

### Backend (Rust — crosshook-core)

- `/src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs`: Public exports for the protondb module; exposes `lookup_protondb`, all model types, and cache utilities.
- `/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: `lookup_protondb()` async function — cache-read → live fetch → cache-write → stale-fallback. Uses `MetadataStore` for SQLite-backed `external_cache_entries` table with 6h TTL.
- `/src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs`: `normalize_report_feed()` — parses raw report feed JSON, extracts `KEY=value` tokens from `launch_options`, applies `safe_env_var_suggestions()` safety filter, groups/ranks by frequency. Key blocklist: `WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, and anything starting with `STEAM_COMPAT_`.
- `/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs`: All IPC-crossing types: `ProtonDbLookupResult`, `ProtonDbSnapshot`, `ProtonDbRecommendationGroup`, `ProtonDbEnvVarSuggestion`, `ProtonDbLaunchOptionSuggestion`, `ProtonDbAdvisoryNote`, `ProtonDbTier`, `ProtonDbLookupState`, `ProtonDbCacheState`.
- `/src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`: `OptimizationCatalog` and `OptimizationEntry` — the static catalog of known optimization toggles. `global_catalog()` returns the process-wide instance. `OptimizationEntry.env` stores `Vec<[String; 2]>` (key-value pairs). The `allowed_env_keys` set is derived from all catalog entries at startup.
- `/src/crosshook-native/crates/crosshook-core/src/launch/env.rs`: `RESERVED_ENV_KEYS` list (`WINEPREFIX`, `STEAM_COMPAT_*`), `BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS` constant (22 keys), `WINE_ENV_VARS_TO_CLEAR` (31 keys cleared before launch). These inform the expanded blocklist requirement.
- `/src/crosshook-native/assets/default_optimization_catalog.toml`: 25 built-in optimization entries. Each entry maps to one or more env var key-value pairs. Used to match ProtonDB suggestions back to known optimization IDs.
- `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: `put_cache_entry` / `load_cached_lookup_row` — SQLite `external_cache_entries` table ops used by the ProtonDB client.
- `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `GameProfile` Rust struct with `launch.custom_env_vars: BTreeMap<String, String>` and `launch.optimizations.enabled_option_ids: Vec<String>`. Both are the write targets for accepted suggestions.

### Tauri IPC Layer (src-tauri/src/commands/)

- `/src/crosshook-native/src-tauri/src/commands/protondb.rs`: Single command `protondb_lookup(app_id, force_refresh)` — thin wrapper that injects `MetadataStore` state and delegates to `lookup_protondb()`. Returns `ProtonDbLookupResult`.
- `/src/crosshook-native/src-tauri/src/commands/catalog.rs`: `get_optimization_catalog()` — returns `OptimizationCatalogPayload { catalog_version, entries }` from the global in-process catalog singleton.
- `/src/crosshook-native/src-tauri/src/commands/profile.rs`: `profile_save()` and `profile_save_launch_optimizations()` — the two write paths for persisting env var and optimization ID changes. Both call into `ProfileStore`.
- `/src/crosshook-native/src-tauri/src/lib.rs`: Full `invoke_handler!` macro registration list. All IPC commands are registered here.

### Frontend (React/TypeScript)

- `/src/crosshook-native/src/types/protondb.ts`: TypeScript mirror of the Rust models — `ProtonDbRecommendationGroup`, `ProtonDbEnvVarSuggestion`, `ProtonDbLookupResult`, etc.
- `/src/crosshook-native/src/types/profile.ts`: `GameProfile` TS type — `launch.custom_env_vars: Record<string, string>` and `launch.optimizations.enabled_option_ids: string[]`.
- `/src/crosshook-native/src/types/launch-optimizations.ts`: `LaunchOptimizations`, `LaunchOptimizationOption`, `BUILTIN_LAUNCH_OPTIMIZATION_IDS` list, conflict matrix utilities.
- `/src/crosshook-native/src/hooks/useProtonDbLookup.ts`: `useProtonDbLookup(appId)` — calls `invoke('protondb_lookup', ...)`, manages loading/stale/unavailable states, normalizes result, exposes `recommendationGroups`.
- `/src/crosshook-native/src/utils/optimization-catalog.ts`: `fetchOptimizationCatalog()`, `buildOptionsById()`, `buildConflictMatrix()` — in-memory cache of the catalog, used by `LaunchOptimizationsPanel`.
- `/src/crosshook-native/src/utils/protondb.ts`: `mergeProtonDbEnvVarGroup()` — merge logic with conflict detection. Returns `{ mergedEnvVars, conflicts, appliedKeys, unchangedKeys }`. Already handles the key-collision and same-value cases.
- `/src/crosshook-native/src/components/ProtonDbLookupCard.tsx`: Main display component. Props: `appId`, `onApplyEnvVars?: (group) => void`, `applyingGroupId?`. Renders recommendation groups with per-env-var Copy buttons and a group-level "Apply Suggested Env Vars" button. **The `onApplyEnvVars` callback is already wired but the catalog-matching path for `optimizations.enabled_option_ids` is not implemented.**
- `/src/crosshook-native/src/components/ProtonDbOverwriteConfirmation.tsx`: Conflict resolution UI — shown when `mergeProtonDbEnvVarGroup` returns conflicts; offers per-key keep/overwrite choice.
- `/src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx`: Renders and manages `launch.custom_env_vars`. Uses `RESERVED_CUSTOM_ENV_KEYS` Set (mirrored from the Rust blocklist). Has `onAutoSaveBlur` for debounced persistence.
- `/src/crosshook-native/src/components/LaunchSubTabs.tsx`: Hosts `ProtonDbLookupCard` and `ProtonDbOverwriteConfirmation` in the "Community" sub-tab. Accepts `onApplyProtonDbEnvVars`, `pendingProtonDbOverwrite`, overwrite confirmation/cancel callbacks.
- `/src/crosshook-native/src/components/pages/LaunchPage.tsx`: Orchestrates the full Apply flow — `applyProtonDbGroup()` and `handleApplyProtonDbEnvVars()`. Already handles conflict → overwrite modal → apply sequence. Writes to `profile.launch.custom_env_vars`.
- `/src/crosshook-native/src/components/ProfileFormSections.tsx`: Duplicates the same `applyProtonDbGroup` / `handleApplyProtonDbEnvVars` pattern for the profile form context. Also wires `onApplyEnvVars` to `ProtonDbLookupCard`.
- `/src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`: Renders optimization toggles using the catalog. Consumes `OptimizationCatalogPayload` and `enabledOptionIds`. `onToggleOption` callback is the write path for `enabled_option_ids`.

## Data Flow

```
ProtonDB API
  → protondb/client.rs: fetch_live_lookup()
  → protondb/aggregation.rs: normalize_report_feed() / safe_env_var_suggestions()
  → MetadataStore: put_cache_entry() → SQLite external_cache_entries
  → ProtonDbLookupResult (IPC boundary, serde)
  → Tauri command: protondb_lookup
  → useProtonDbLookup hook → ProtonDbLookupResult
  → ProtonDbLookupCard: renders recommendation groups
  → User clicks "Apply Suggested Env Vars"
  → onApplyEnvVars callback → mergeProtonDbEnvVarGroup()
    → if conflicts: ProtonDbOverwriteConfirmation modal
    → applyProtonDbGroup(): updates profile.launch.custom_env_vars
  → profile_save() IPC → ProfileStore → TOML file
```

Optimization catalog lookup-to-toggle path (missing link for catalog-matching):

```
ProtonDbEnvVarSuggestion { key, value }
  → match against OptimizationCatalog.entries[*].env (key-value pairs)
  → if match found: toggle launch.optimizations.enabled_option_ids
  → if no match: fall back to custom_env_vars
  → profile_save_launch_optimizations() or profile_save() IPC
```

## Integration Points

1. **Env var blocklist expansion** (`protondb/aggregation.rs:RESERVED_ENV_KEYS`): Add `LD_PRELOAD`, `PATH`, `HOME`, `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`. The feature spec (BR-2) requires these to be filtered at extraction time, not just at write time.

2. **Catalog-matching in the Apply flow** (`src/utils/protondb.ts` or a new utility): When `mergeProtonDbEnvVarGroup` processes env vars, check each `(key, value)` pair against the optimization catalog entries (`OptimizationEntry.env`). Matching entries should produce toggles for `enabled_option_ids`, not additions to `custom_env_vars`.

3. **ProtonDbLookupCard prop extension** (`ProtonDbLookupCard.tsx`): The `onApplyEnvVars` prop only passes a `ProtonDbRecommendationGroup`. Catalog-matching either happens inside the callback (in `LaunchPage.tsx` / `ProfileFormSections.tsx`) or the prop type is extended to pass the catalog. The catalog is already fetched in `LaunchPage.tsx` via `fetchOptimizationCatalog()`.

4. **LaunchPage / ProfileFormSections apply callback** (`LaunchPage.tsx:194`, `ProfileFormSections.tsx:395`): `applyProtonDbGroup()` currently only writes to `custom_env_vars`. It needs an additional branch to write catalog-matched keys to `optimizations.enabled_option_ids` via `onToggleOption` (or directly into the profile draft).

5. **Status messaging**: `protonDbStatusMessage` state already exists in both `LaunchPage` and `ProfileFormSections`. Catalog-match applies should report "Applied X optimization toggles and Y env vars" for clear UX.

## Key Dependencies

- **MetadataStore** (`crosshook-core/src/metadata/`): SQLite-backed store; `external_cache_entries` table holds ProtonDB cache with `cache_key`, `payload_json`, `fetched_at`, `expires_at`. Schema is at v13 with 18 tables. No new tables are needed for Phase 1.
- **ProfileStore** (`crosshook-core/src/profile/`): TOML-based profile persistence. `profile_save` and `profile_save_launch_optimizations` are the write-path commands. Both emit metadata sync side effects.
- **OptimizationCatalog** (global singleton): Initialized at startup from embedded TOML + optional user overrides. 25 built-in entries. Accessed via `global_catalog()` (Rust) or `fetchOptimizationCatalog()` / `getCachedCatalog()` (TS).
- **reqwest** (HTTP): Used by protondb/client.rs for ProtonDB API calls. Already in Cargo.toml.
- **rusqlite**: Used by MetadataStore for cache persistence. Already in Cargo.toml.
- **serde / serde_json**: All IPC-crossing types use Serde derive macros. Already in Cargo.toml.
- **Tauri v2 IPC**: `invoke()` in frontend, `#[tauri::command]` + `invoke_handler!` in backend. All command registration in `src-tauri/src/lib.rs`.
- **@tauri-apps/api/core**: Frontend JS SDK for `invoke()` calls.

## Architectural Gotchas

- **Dual Apply paths**: Both `LaunchPage.tsx` and `ProfileFormSections.tsx` independently implement `applyProtonDbGroup` and `handleApplyProtonDbEnvVars`. Any catalog-matching extension must be applied in both places, or the logic must be extracted to a shared utility.
- **Custom env vars take precedence over catalog env vars at launch time** (`CustomEnvironmentVariablesSection.tsx:5-6` comment). When the same key exists in both `custom_env_vars` and an enabled catalog entry, `custom_env_vars` wins. This means adding to `custom_env_vars` and `enabled_option_ids` for the same key could produce unexpected precedence at runtime.
- **RESERVED_CUSTOM_ENV_KEYS mirror**: The TS `CustomEnvironmentVariablesSection.tsx` maintains a hardcoded `Set` mirroring the Rust `RESERVED_ENV_KEYS` constant. Expanding the Rust blocklist requires a matching TS update.
- **`is_safe_env_key` only allows `UPPER_SNAKE_CASE`**: Keys must start with `_` or uppercase ASCII, followed by uppercase letters, digits, or underscores. This blocks lowercase env keys entirely — any ProtonDB suggestion with a lowercase key (e.g., `gamemoderun`) is silently dropped by `safe_env_var_suggestions()`. This is intentional but worth documenting.
- **Catalog env matching is key+value, not key-only**: An optimization entry may set `PROTON_NO_ESYNC=1`. A ProtonDB suggestion of `PROTON_NO_ESYNC=0` should NOT match the same catalog entry — matching must be exact key-value pair comparison.
- **ODbL compliance (BR-7)**: ProtonDB-derived data must never be serialized into profile TOML exports or community tap distributions. Accepted suggestions that write to `custom_env_vars` become profile data, which is fine (user deliberately applied them). The raw cache payload must not be included in any export.
- **Scroll container registration**: Any new scrollable containers added to the ProtonDB panel must be added to the `SCROLLABLE` selector in `src/crosshook-native/src/hooks/useScrollEnhance.ts` per the CLAUDE.md requirement.
- **No frontend test framework**: The CLAUDE.md notes there is no configured frontend test framework. Verification of UI behavior relies on dev/build scripts.
