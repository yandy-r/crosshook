# Pattern Research: community-driven-config-suggestions

## Overview

CrossHook uses a layered architecture with business logic in `crosshook-core` (Rust), a thin Tauri IPC layer in `src-tauri`, and a React/TypeScript frontend. The existing ProtonDB pipeline is fully implemented: the backend aggregates env-var suggestions by frequency, normalizes them into `ProtonDbRecommendationGroup` structs, caches with TTL in SQLite via `MetadataStore`, and exposes them through a single `protondb_lookup` Tauri command. The frontend already has types, a hook, a card component, and a merge utility for applying suggestions into profiles. The remaining feature work is wiring suggestions into the profile editor at the right trigger points.

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs`: Public API for the protondb module — re-exports `lookup_protondb` and all models
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs`: Rust structs for `ProtonDbRecommendationGroup`, `ProtonDbEnvVarSuggestion`, `ProtonDbLaunchOptionSuggestion`, `ProtonDbSnapshot`, etc.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs`: Report-feed aggregation logic — parses raw launch option strings into env-var or copy-only groups, with `RESERVED_ENV_KEYS` guard and frequency counting via `supporting_report_count`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: HTTP client and cache logic for ProtonDB lookups
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/tests.rs`: Unit tests for aggregation and cache fallback
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs`: Single Tauri command `protondb_lookup` — thin wrapper over `crosshook_core::protondb::lookup_protondb`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/protondb.ts`: Frontend TypeScript types mirroring Rust models: `ProtonDbLookupResult`, `ProtonDbSnapshot`, `ProtonDbRecommendationGroup`, `ProtonDbEnvVarSuggestion`, etc.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbLookup.ts`: Custom hook wrapping `invoke('protondb_lookup')` — race-condition-safe with `requestIdRef`, normalizes results, exposes `refresh()`, `recommendationGroups`, `loading`, `isStale`, `isUnavailable`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProtonDbLookupCard.tsx`: Presentational card rendering recommendation groups, with per-group "Apply Suggested Env Vars" button and copy-to-clipboard for launch options
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/utils/protondb.ts`: `mergeProtonDbEnvVarGroup()` — conflict-aware env-var merge utility used by `ProfileFormSections`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileFormSections.tsx`: Profile editing composition — wires `ProtonDbLookupCard` + conflict resolution state + `mergeProtonDbEnvVarGroup` into the profile editor
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx`: Row-based env-var editor — exposes `onAutoSaveBlur` for autosave on blur
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts`: Central profile state hook — manages `updateProfile(updater)` pattern with immutable spread updates
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile.ts`: `GameProfile` type definition — `launch.custom_env_vars: Record<string, string>` is the target for env-var suggestions
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/variables.css`: CSS custom properties for tokens — includes ProtonDB-specific vars like `--crosshook-protondb-panel-gap`, `--crosshook-community-cache-border`, etc.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css`: BEM-style component styles — `crosshook-protondb-card` and its modifiers fully defined here

## Architectural Patterns

**Thin Tauri Command Layer**: Commands in `src-tauri/src/commands/` are thin wrappers. They receive `State<'_, Store>` injected by Tauri, call a single `crosshook_core` function, and map errors to `String` with a `map_error` helper. No business logic lives here.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs`

**Store Injection via Tauri State**: Shared state (e.g., `MetadataStore`, `ProfileStore`, `SettingsStore`) is registered as managed state in `lib.rs` and injected via `State<'_, T>` into commands. Commands clone the store reference when passing to async tasks.

- Example: `lib.rs` registers `MetadataStore::try_new()` and commands receive `metadata_store: State<'_, MetadataStore>`

**Immutable Profile Update Pattern**: All profile field mutations go through `updateProfile(updater: (current: GameProfile) => GameProfile)`. Updater functions always spread-copy the whole profile and the changed nested section.

```typescript
// Pattern used throughout ProfileFormSections.tsx and useProfile.ts
onUpdateProfile((current) => ({
  ...current,
  launch: {
    ...current.launch,
    custom_env_vars: nextEnvVars,
  },
}));
```

**invoke() Wrapped in Custom Hooks**: Every Tauri IPC call is encapsulated in a dedicated hook. Hooks manage loading state, error state, race cancellation (via `requestIdRef`), and normalization. Components never call `invoke()` directly.

- Example: `useProtonDbLookup.ts` — race-safe pattern with `requestId = ++requestIdRef.current`

**Race-Condition Guard for Async IPC**: Hooks that fire multiple concurrent IPC requests use a ref counter to discard stale responses:

```typescript
const requestIdRef = useRef(0);
const requestId = ++requestIdRef.current;
// ... after await, check: if (requestId !== requestIdRef.current) return;
```

**Serde snake_case Serialization**: All Rust structs crossing the IPC boundary use `#[serde(rename_all = "snake_case")]` or custom serialization. Frontend TypeScript types mirror the snake_case field names exactly. Enums use `#[serde(rename_all = "snake_case")]`.

**Module-per-Domain Organization (Rust)**: Each domain has its own subdirectory with `mod.rs`, and the public API is re-exported explicitly from `mod.rs`. Internal implementation submodules (`aggregation.rs`, `client.rs`) are `pub(crate)` or private.

**BEM-style CSS Classes with `crosshook-` Prefix**: All component classes use `crosshook-<component>` and `crosshook-<component>__<element>` and `crosshook-<component>--<modifier>` patterns. CSS variables for component-specific tokens are declared in `variables.css`.

## Code Conventions

**Naming**

- Rust: `snake_case` for everything (functions, fields, modules); `PascalCase` for types/enums/structs
- TypeScript: `PascalCase` for components and interfaces; `camelCase` for hooks, functions, and variables
- Hooks: `use` prefix (e.g., `useProtonDbLookup`, `useProfile`)
- CSS: `crosshook-` prefixed BEM classes; component-level CSS vars in `variables.css`
- Tauri commands: `snake_case` function name in Rust matches frontend `invoke('snake_case_name', { camelCaseArgs })`

**File Organization**

- Frontend components: flat in `src/components/` with subdirectory `profile-sections/` for sub-components
- Custom hooks: `src/hooks/use<Name>.ts`
- TypeScript types: `src/types/<domain>.ts`, with a barrel `index.ts` for common exports
- Utilities: `src/utils/<domain>.ts` (pure functions)
- Rust domain modules: `crates/crosshook-core/src/<domain>/mod.rs`

**Props Pattern**

- Component props are typed with a named interface `<ComponentName>Props`
- Callback props for profile mutation always receive an updater: `(updater: (current: GameProfile) => GameProfile) => void`

**Error Display**

- Rust commands map errors to `String` (`.map_err(map_error)` or `map_err(|e| e.to_string())`)
- Frontend: errors surface as `string | null` state; rendered as `<p className="crosshook-danger">`
- Success feedback: `<p className="crosshook-help-text" role="status">`

**Loading States**

- IPC hooks expose `loading: boolean`; components disable action buttons and show `'Saving...'` / `'Refreshing…'` text labels while loading

## Error Handling

**Rust Pattern**

- Commands return `Result<T, String>` — always map domain errors to `String` before returning
- Internal crosshook-core functions use `anyhow::Result` with `.context()` for chained error context
- Cache-layer errors are typically soft-failed (logged via `tracing::warn!`) rather than propagated

**Frontend Pattern**

- `invoke()` calls are wrapped in `try/catch`; errors are logged with `console.error()` and trigger fallback state (e.g., `unavailableLookup()`)
- Tauri invoke errors are sometimes plain objects, not `Error` instances — `formatInvokeError()` in `useProfile.ts` handles this safely with type narrowing
- Status messages are shown inline via `role="status"` `<p>` tags, not toasts

## Testing Approach

**Rust Unit Tests**

- Tests live in a `tests.rs` file within the module directory, included via `#[cfg(test)] mod tests;`
- Use `tokio::runtime::Builder::new_current_thread().enable_all().build()` to run async tests synchronously via `block_on()`
- `MetadataStore::disabled()` and `MetadataStore::open_in_memory()` are the test fixtures — no real SQLite file needed
- Test naming: descriptive, behavior-driven snake_case (e.g., `empty_app_id_short_circuits_to_default_result`)

**Frontend Testing**

- No configured frontend test framework in this project; rely on dev/build scripts for UI behavior verification
- Core logic in utility functions (e.g., `mergeProtonDbEnvVarGroup`) could be unit-tested if a test framework is added

## Frontend Patterns

**Custom Hook for IPC**
Every feature that calls Tauri commands gets a dedicated hook:

```typescript
export function useProtonDbLookup(appId: string): UseProtonDbLookupResult {
  const [lookup, setLookup] = useState<ProtonDbLookupResult>(() => idleLookup(appId));
  const requestIdRef = useRef(0);
  // ... invoke, set state, expose derived values
}
```

**Hook Return Interface**
Hooks always export a named return interface:

```typescript
export interface UseProtonDbLookupResult {
  appId: string;
  state: ProtonDbLookupState;
  loading: boolean;
  recommendationGroups: ProtonDbRecommendationGroup[];
  refresh: () => Promise<void>;
  // ...
}
```

**Presentational / Container Split**

- Presentational components (e.g., `ProtonDbLookupCard`) receive data and callbacks as props; no `invoke()` calls
- Container composition happens in `ProfileFormSections.tsx` or page-level components

**Tauri invoke() Call Pattern**

```typescript
const result = await invoke<ReturnType>('command_name', {
  camelCaseArg1: value1,
  camelCaseArg2: value2,
});
```

Arguments use camelCase on the frontend; Tauri serializes them to snake_case for Rust.

**State for Async Confirmation Flows**
Multi-step confirmation UI (e.g., conflict resolution before applying env vars) uses local `useState` holding a `Pending*` type:

```typescript
const [pendingProtonDbOverwrite, setPendingProtonDbOverwrite] = useState<PendingProtonDbOverwrite | null>(null);
```

When non-null, a confirmation component renders. On confirm/cancel, state is set back to `null`.

**CSS Pattern**

- All component styles in `theme.css` using BEM with `crosshook-` prefix
- Component-specific token variables in `variables.css`
- State/modifier classes as BEM modifiers: `crosshook-protondb-card--platinum`, `crosshook-protondb-card--loading`
- New component sections follow: declare CSS vars in `variables.css`, add component block in `theme.css`

## IPC Patterns

**Command Structure**

```rust
#[tauri::command]
pub async fn my_command(
    arg1: TypeFromFrontend,
    store: State<'_, SomeStore>,
) -> Result<ReturnType, String> {
    let store = store.inner().clone();
    crosshook_core::module::do_thing(&store, &arg1)
        .await
        .map_err(|e| e.to_string())
}
```

**Command Registration**
Commands are declared in `commands/mod.rs` as `pub mod <domain>` and registered in `lib.rs` via `.invoke_handler(tauri::generate_handler![...])`. Any new command must be added to the handler list in `lib.rs`.

**Event Emission Pattern**
For push events from backend to frontend, `app_handle.emit("event-name", &payload)` is used. Frontend listens with `listen('event-name', callback)` from `@tauri-apps/api/event`.

## Patterns to Follow for This Feature

1. **No new Tauri command needed** — the existing `protondb_lookup` command returns `recommendation_groups` in its `ProtonDbLookupResult`. The backend work is already done.

2. **New hook if adding a new trigger point** — if config suggestions need to be surfaced from a different entry point (e.g., a dedicated suggestions panel), create `useConfigSuggestions.ts` following the `useProtonDbLookup.ts` pattern with race-safe `requestIdRef`.

3. **Extend `ProtonDbLookupCard` via props** — new suggestion-display behavior should be added as optional callback props (e.g., `onApplyLaunchOption?: (text: string) => void`) to keep the component presentational.

4. **Conflict detection before applying** — use `mergeProtonDbEnvVarGroup()` in `src/utils/protondb.ts` to detect conflicts before applying; render a `ProtonDbOverwriteConfirmation`-style modal for resolution.

5. **Profile update via updater pattern** — all env-var mutations must go through `onUpdateProfile((current) => ({ ...current, launch: { ...current.launch, custom_env_vars: ... } }))` — never mutate state directly.

6. **CSS new tokens in `variables.css`, styles in `theme.css`** — add `--crosshook-config-suggestions-*` token variables to `variables.css`; add component block in `theme.css` using `crosshook-config-suggestions-*` BEM classes.

7. **Scroll container registration** — if adding a new scrollable container, add its CSS selector to the `SCROLLABLE` selector list in `useScrollEnhance.ts` per CLAUDE.md requirement.

8. **Rust test fixtures** — use `MetadataStore::open_in_memory()` for tests that need a real store, `MetadataStore::disabled()` for tests that only need the function to run without persistence.

## Gotchas & Edge Cases

- **ProtonDB apply logic is duplicated across two containers**: `applyProtonDbGroup` + `handleApplyProtonDbEnvVars` are independently implemented in both `ProfileFormSections.tsx` (profile editor) and `LaunchPage.tsx` (launch page). Any new trigger surface or behavior change must be applied in both files, or the logic should be extracted to a shared utility hook first. The existing `mergeProtonDbEnvVarGroup()` in `src/utils/protondb.ts` is already shared — the state-management layer around it is not.

- **`ConfigRevisionSource` needs a new enum variant**: Profile saves that result from applying community suggestions should be recorded with a distinct revision source for auditability. The current variants are `ManualSave`, `Import`, `PresetApply`, `LaunchOptimizationSave`, `RollbackApply`. A new variant (e.g., `CommunityConfigApply`) must be added in `crates/crosshook-core/src/metadata/config_history_store.rs` and used when the apply path triggers a save.

- **Reserved env keys are guarded in two places**: `RESERVED_ENV_KEYS` in `crates/crosshook-core/src/protondb/aggregation.rs` prevents them from appearing in suggestions. `RESERVED_CUSTOM_ENV_KEYS` in `CustomEnvironmentVariablesSection.tsx` prevents the user from entering them manually. Any new apply path must respect the same guard — do not apply suggestions whose key is in the reserved set.

- **`LaunchPage.tsx` calls `invoke()` directly in one place**: `check_gamescope_session` is called with a raw `invoke<boolean>()` in `LaunchPage.tsx` rather than through a hook. This is an existing deviation from the hook-wraps-invoke convention — do not replicate it for new suggestion apply calls.

- **ProtonDB panel only renders for Steam launch methods**: In `ProfileFormSections.tsx`, `showProtonDbLookup` is `true` only when `launchMethod === 'steam_applaunch' || launchMethod === 'proton_run'`. Config suggestions wired into a new surface must apply the same method guard to avoid surfacing Steam-specific env-var suggestions for `native` launch profiles.
