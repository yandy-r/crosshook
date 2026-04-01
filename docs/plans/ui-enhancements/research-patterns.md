# Pattern Research: ui-enhancements

## Overview

CrossHook uses a consistent set of patterns across Rust (crosshook-core + src-tauri) and React/TypeScript (src/crosshook-native/src). The backend follows a thin-IPC model where `src-tauri/src/commands/` are pure pass-throughs to `crosshook-core`, and the frontend wraps `invoke()` in domain hooks. The ProtonDB lookup is the canonical reference pattern for the Steam Store API integration — it covers HTTP client initialization, cache-first lookup, stale fallback, and IPC surface.

## Architectural Patterns

**Thin IPC Command Layer**: Tauri commands in `src-tauri/src/commands/` do no business logic. They extract state via `tauri::State<'_>`, clone it if needed for async spawn, and delegate to `crosshook-core` functions. Error types are mapped to `String` at the IPC boundary using `.map_err(|e| e.to_string())`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs`

**MetadataStore Access Pattern**: All SQLite access goes through `MetadataStore::with_conn()` (returns `T: Default` when store unavailable) or `MetadataStore::with_sqlite_conn()` (returns `Err` when unavailable). The inner implementation locks a `Arc<Mutex<Connection>>`. Store submodules (`cache_store`, `health_store`) are private functions accepting `&Connection` directly, called from `MetadataStore` public methods.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:91-153`
- Store submodule: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`

**Cache-First with Stale Fallback**: The canonical pattern used by ProtonDB and which the Steam Store metadata must mirror: (1) check valid cache (expires_at > now), (2) if miss, fetch live, (3) on network failure, load expired cache and mark `is_stale=true`, (4) on total failure, return `Unavailable` state. Results are stored in `external_cache_entries` via `MetadataStore::put_cache_entry()`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs:85-130`

**OnceLock HTTP Client**: Singleton reqwest client initialized with `OnceLock<reqwest::Client>` inside the module. Includes timeout and `user_agent` headers.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs:26,175-190`

**Module-Level Error Enum (private)**: Each module defines a private `enum` for its specific error cases (e.g., `ProtonDbError`), implements `Display`, then converts to `String` at the IPC boundary. `MetadataStoreError` is the shared storage error type with `Database { action: &'static str, source }` for contextual SQL failures.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs:28-53`
- MetadataStoreError: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs:8-63`

**Sequential Migration Pattern**: `migrations.rs` uses `PRAGMA user_version` for schema version tracking. Each version is a guarded `if version < N { migrate_N_minus_1_to_N(conn)?; pragma_update(user_version, N); }` block. Migration functions call `conn.execute_batch()` directly with inline SQL.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`

**Managed State via `.manage()`**: All long-lived stores (`MetadataStore`, `ProfileStore`, `SettingsStore`) are registered with Tauri via `.manage()` in `lib.rs` and accessed in commands as `State<'_, T>`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs:180-184`

## Code Conventions

**Rust naming**: `snake_case` everywhere. Module directories use `mod.rs`. All IPC command function names use `snake_case` which must exactly match the frontend `invoke('command_name')` string.

**Serde on IPC types**: All types crossing the IPC boundary derive `Serialize + Deserialize`. Enums use `#[serde(rename_all = "snake_case")]`. Optional fields use `#[serde(default, skip_serializing_if = "Option::is_none")]`. Vec fields use `#[serde(default, skip_serializing_if = "Vec::is_empty")]`. String fields use `#[serde(default, skip_serializing_if = "String::is_empty")]`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs:114-241`

**AppSettingsData extension**: New optional fields added to `AppSettingsData` must use `#[serde(default)]` at the struct level to ensure backward-compatible deserialization. The struct already has `#[serde(default)]` on the struct itself.

- File: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs:19-27`

**CSS naming convention**: All CSS classes use `crosshook-` prefix with BEM-like modifiers (`crosshook-panel`, `crosshook-panel--active`). CSS variables are defined in `variables.css` and consumed everywhere else. Component-specific variables are named semantically (e.g., `--crosshook-subtab-min-height`). The `crosshook-status-chip` class is the canonical badge/chip pattern.

- Variables: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/variables.css`
- Theme: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css`

**React component naming**: `PascalCase` components, `camelCase` hooks and functions. Hooks live in `src/hooks/` and are prefixed with `use`. Component files use `.tsx`, plain functions use `.ts`.

**React import ordering**: `react` imports first, then `@tauri-apps/api` imports, then relative type imports, then relative hook/util imports, then relative component imports.

**Controller mode**: All touch targets scale via CSS variables (`--crosshook-touch-target-min`, `--crosshook-touch-target-compact`). The `data-crosshook-controller-mode='true'` attribute on `:root` drives responsive overrides for all layout vars.

## Error Handling

**Rust layer**: Use `Result<T, MetadataStoreError>` for SQLite operations. For HTTP client errors, use private module enums and log with `tracing::warn!()` structured fields: `tracing::warn!(app_id, %error, "message")`. Errors are NOT surfaced to users unless critical. The `MetadataStoreError::Database { action: &'static str, source }` pattern provides context for all SQL failures.

**IPC boundary**: Commands return `Result<T, String>`, converting all internal errors via `.map_err(|e| e.to_string())`. The frontend receives the error string directly. This is the universal pattern — never use custom error types at the IPC boundary.

**Frontend layer**: Hooks manage their own error state. Pattern is `useState<string | null>(null)` for error, set in `catch` blocks. Non-critical failures (e.g., cached snapshots load failure) use empty `catch {}` with comments explaining why. Network failures degrade gracefully to stale data rather than throwing.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbLookup.ts:120-135`
- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfileHealth.ts:165-170`

**Race condition guard**: Async hooks use a `requestIdRef = useRef(0)` incremented on each call; the response is discarded if `requestId !== requestIdRef.current`. This prevents stale responses overwriting newer state.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbLookup.ts:106-118`

**Stale-while-revalidating UI**: Hook state machines expose `state: 'idle' | 'loading' | 'ready' | 'stale' | 'unavailable'`. During loading, the previous snapshot and cache are preserved in state. This allows UI to show stale data without flickering to empty.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbLookup.ts:65-76`

## Testing Approach

**Rust unit tests**: Tests live in the same file using `#[cfg(test)] mod tests { ... }` at the bottom, or in dedicated `tests.rs` files within the module directory. Tests use `MetadataStore::open_in_memory()` to get a real SQLite connection without filesystem access. Async tests use a locally constructed `tokio::runtime::Builder::new_current_thread()` runtime (not `#[tokio::test]` macro).

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/tests.rs`
- Migration tests: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs:633-670`

**Test pattern for cache lookup**: Seed `MetadataStore::open_in_memory()` via `store.put_cache_entry(...)` with an expired `expires_at`, then call the lookup function. Assert on `result.state`, `result.cache.is_stale`, and snapshot fields.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/tests.rs:40-82`

**Migration test pattern**: Call `db::open_in_memory()`, run `run_migrations(&conn)`, then query `sqlite_master` or the specific table to confirm it exists with expected structure.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs:651-669`

**Frontend testing**: No configured test framework. Behavioral testing is done via dev/build scripts (`./scripts/dev-native.sh`). Type-level correctness via TypeScript strict mode.

## Patterns to Follow for This Feature

### Steam Metadata Module (mirrors ProtonDB)

The new `steam_metadata/` module must follow the exact structure of `protondb/`:

1. **`models.rs`**: Define `SteamMetadataLookupState` enum (same states: `Idle/Loading/Ready/Stale/Unavailable`), `SteamMetadataLookupResult`, `SteamAppDetails`, `SteamGenre`. Use identical Serde annotations (`snake_case`, `skip_serializing_if`).
2. **`client.rs`**: One `OnceLock<reqwest::Client>` per module. Main `lookup_steam_metadata(store, app_id, force_refresh)` public async function with same cache-first pattern. Cache key: `steam:appdetails:v1:{app_id}` via `external_cache_entries`.
3. **`mod.rs`**: Re-export public lookup function and result types.

### Game Image Store (follows health_store/cache_store pattern)

The new `metadata/game_image_store.rs` must:

1. Take `&Connection` parameters directly (not `MetadataStore`).
2. Expose `upsert_game_image`, `get_game_image`, `evict_expired_images` functions.
3. Add new `MetadataStore` public methods that delegate via `self.with_conn()` or `self.with_sqlite_conn()`.
4. Add `migrate_13_to_14(conn)` in `migrations.rs` following the sequential guard pattern.

### IPC Command Registration

New commands go in a new `src-tauri/src/commands/game_metadata.rs` file. Must be:

1. Added to `invoke_handler!(tauri::generate_handler![...])` in `lib.rs`.
2. Declared as `pub async fn` with `#[tauri::command]`.
3. Take `metadata_store: State<'_, MetadataStore>` as parameter.
4. Return `Result<T, String>`.

### Frontend Hook Pattern (mirrors useProtonDbLookup)

`useGameMetadata` must implement:

1. `requestIdRef` race guard.
2. State machine: `idle → loading → ready/stale/unavailable`.
3. Preserve previous snapshot during loading transitions (stale-while-revalidating).
4. `refresh()` function callable by the UI for force-refresh.

`useGameCoverArt` may be simpler — `useState<string | null>` for the filesystem path, `useState<boolean>` for loading, with direct `invoke('fetch_game_cover_art', ...)`.

### CSS Variables for Cover Art

Add to `variables.css` (not inline styles):

- `--crosshook-profile-cover-art-aspect`: `460 / 215`
- `--crosshook-skeleton-duration`: `1.8s`
- `--crosshook-skeleton-gradient`: standard shimmer gradient values

Add classes to `theme.css`:

- `.crosshook-profile-cover-art`: `aspect-ratio` + `border-radius` + `overflow: hidden`
- `.crosshook-skeleton`: `@keyframes crosshook-skeleton-shimmer` + animation

### Component Composition

`CollapsibleSection` (`src/crosshook-native/src/components/ui/CollapsibleSection.tsx`) is the canonical container for each section card. Pass `className="crosshook-panel"` for card styling. Use the `meta` prop for one-line section summaries in collapsed headers.

Status chips for genres use `className="crosshook-status-chip crosshook-compatibility-chip"` (established pattern from `CompatibilityViewer`).

### Tab Infrastructure

`crosshook-subtab-row` and `crosshook-subtab` / `crosshook-subtab--active` CSS classes already exist in `theme.css`. For Phase 3, `@radix-ui/react-tabs` wraps these classes. The `TabsList` maps to `crosshook-subtab-row`, `TabsTrigger` maps to `crosshook-subtab`. Use `data-state="active"` CSS attribute selector or the `--active` modifier class to style the active tab.

- CSS: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css:104-135`
- Variables: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/variables.css:45-46`
