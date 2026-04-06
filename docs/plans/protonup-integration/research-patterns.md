# Pattern Research: protonup-integration

## Overview

CrossHook follows a strict three-layer architecture: `crosshook-core` owns all business logic, `src-tauri/src/commands/` provides thin IPC wrappers, and the React frontend consumes those commands through typed hooks. The patterns below are directly observable from analogous modules (install, update, steam, protondb, discovery) and must be replicated exactly for the protonup-integration feature.

---

## Architectural Patterns

**Request/Result/Error Triple**: Every feature domain defines three types in `models.rs`: a `*Request` struct (Serde `Default`), a `*Result` struct, and an enum-per-variant `*Error` with a nested `Validation` variant. The `*Error` enum derives `Serialize + Deserialize` with `#[serde(rename_all = "snake_case")]` and implements `std::error::Error` + `Display` via a `.message()` method. A `From<*ValidationError> for *Error` impl is required.

- Example (install): `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/models.rs`
- Example (update): `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/update/models.rs`

**Validate-then-Execute service pattern**: Core service functions begin with an explicit `validate_*` call that returns a typed validation error before touching the filesystem or spawning processes. The validate function is also re-exported individually so the frontend can call it cheaply before the full operation.

- Example: `validate_install_request` / `install_game` in `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/service.rs`
- Example: `validate_update_request` / `update_game` in `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/update/service.rs`

**Module-per-domain with `mod.rs` re-exports**: Each domain is a directory with `mod.rs`, `models.rs`, `service.rs`, and optionally `discovery.rs`, `client.rs`, or `tests.rs`. Public surface is re-exported from `mod.rs`, not scattered across internal files. New protonup domain should follow: `src/crosshook-native/crates/crosshook-core/src/protonup/`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/mod.rs`
- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/update/mod.rs`

**Thin Tauri command layer**: `src-tauri/src/commands/*.rs` files contain only `#[tauri::command]` functions. Each function calls the equivalent `crosshook-core` function and maps `error.to_string()` for the `Err` arm. No business logic lives here. Blocking operations are wrapped in `tauri::async_runtime::spawn_blocking`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/install.rs`
- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/steam.rs`

**Cache-first fetch (3-stage flow)**: Network-backed features follow a cache→live→stale-fallback pattern. Stage 1: return valid (non-expired) cached data if present and `!force_refresh`. Stage 2: fetch live, cache on success. Stage 3: on live failure, fall back to stale cache and log a warning. Stage 4: return an offline/degraded response. Copy this pattern directly from `discovery/client.rs`; do not invent a new abstraction.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/discovery/client.rs` (lines 231–300)
- Also: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` (lines 85–130)

**MetadataStore for SQLite persistence**: The `MetadataStore` struct wraps `Arc<Mutex<Connection>>` and exposes `with_conn` / `with_sqlite_conn` helpers. All SQLite access goes through these methods. New tables are added via sequential `migrate_N_to_M` functions in `migrations.rs`, incrementing `PRAGMA user_version`. Current schema is at version 13.

- DB wrapper: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`
- DB open / configure: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/db.rs`
- Migration runner: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`

**OnceLock for singleton HTTP clients**: Long-lived `reqwest::Client` instances are stored in a `static OnceLock<reqwest::Client>` field within the module that uses them — not a shared global. Each module that owns HTTP calls has its own client with its own timeout and user-agent. Copy the pattern; do not share a single client across modules.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` (lines 26–190)

**Tauri events for long-running processes — two variants**:

_Variant A (log-file polling, used by install/update)_: Backend spawns an async task that polls a log file and emits one event per new line. Frontend subscribes before `invoke`. See `commands/update.rs` lines 73–170.

_Variant B (direct stdout/stderr streaming, used by prefix_deps)_: Backend takes `stdout`/`stderr` handles from the child process, wraps each in a `BufReader`, and streams lines directly via `app.emit`. Each stream is its own `spawn`-ed task. A third task waits on `child.wait()` and emits the completion event. See `commands/prefix_deps.rs` lines 234–320.

For ProtonUp download progress, Variant B (direct streaming) is preferred since `libprotonup` exposes async progress. Tauri event names use **kebab-case**: `protonup-install-progress`, `protonup-install-complete`.

- Backend (log-file): `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/update.rs` (lines 73–170)
- Backend (stream): `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/prefix_deps.rs` (lines 234–320)
- Frontend listener: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useUpdateGame.ts` (lines 228–263)

**Tauri managed state**: State objects are constructed once in `lib.rs`, then registered via `.manage(...)` on the `tauri::Builder`. Commands that need state receive `tauri::State<'_, T>` as a parameter. For cancellable/in-progress operations, a dedicated `*ProcessState` struct with `Mutex<Option<u32>>` PID is the pattern.

- State setup: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` (lines 25–203)
- Process PID state: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/update.rs` (lines 13–22)

**`libprotonup` already in Cargo.toml**: The `libprotonup = "0.11.0"` crate is already declared as a dependency in `crosshook-core/Cargo.toml`. The integration should consume it directly rather than shelling out to `protonup-qt` or re-implementing download logic.

- Cargo.toml: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/Cargo.toml`

---

## Code Conventions

**Rust naming**: All public functions, fields, and modules use `snake_case`. Type names (`struct`, `enum`) use `PascalCase`. Constants use `SCREAMING_SNAKE_CASE`. Module files are named after their domain in snake_case (e.g., `protonup/`, `version_store.rs`).

**`#[serde(default)]` on every field**: All struct fields that cross the IPC boundary carry `#[serde(default)]`. This prevents frontend deserialization failures when new optional fields are added and eliminates the need for schema migrations on new optional settings fields.

**`snake_case` Tauri command names**: Tauri commands are registered and invoked with `snake_case` names matching the Rust function name. Frontend `invoke('list_proton_installs', ...)` matches `pub fn list_proton_installs(...)`.

**`pub(crate)` for internal helpers**: Functions used only within the crate use `pub(crate)` visibility. Only items needed by `src-tauri` or tests are `pub`.

**React and TypeScript naming (frontend)**: Components use `PascalCase`. Hooks use `camelCase` with `use` prefix. CSS classes follow BEM-like `crosshook-*` naming (e.g., `crosshook-protonup-panel`). CSS variables belong in `src/crosshook-native/src/styles/variables.css`. Any new `overflow-y: auto` scroll container must be added to the `SCROLLABLE` selector in `src/crosshook-native/src/hooks/useScrollEnhance.ts` or scroll enhancement will target a parent container instead.

**TypeScript types mirror Rust structs with `snake_case` keys**: Frontend type files (e.g., `src/types/update.ts`) mirror the Serde-serialized Rust structs. Fields stay `snake_case` (not camelCase). Validation error string literals in the `.ts` files include a `/** Keep in sync with ... */` comment directing maintainers to the Rust `.message()` implementation.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/update.ts`
- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/proton.ts`

**One command file per domain**: Each `src-tauri/src/commands/` file corresponds to one `crosshook-core` module (e.g., `commands/install.rs` ↔ `core/install/`). A new `commands/protonup.rs` file should be created. It must be declared in `commands/mod.rs` and all its functions registered in `lib.rs`'s `invoke_handler!`.

- Commands mod: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/mod.rs`
- lib.rs handler list: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` (lines 207–322)

**Commit prefixes**: Changes to files under `docs/plans`, `docs/research`, or `docs/internal` must use `docs(internal): ...` prefix to stay out of the CHANGELOG. Other non-user-facing churn uses `chore(...)`.

---

## Error Handling

**Enum-based typed errors with `Display` from `.message()`**: Errors are never untyped strings in the core layer. Each domain owns a `*Error` and `*ValidationError` enum. Both implement `Display` by delegating to `self.message()`, which uses a `match` exhaustively covering every variant.

**`.map_err(|e| e.to_string())` at the command boundary**: The only place errors become `String` is in the `#[tauri::command]` function's `Result<T, String>` return. This keeps the Rust type system intact through the core layer.

**Graceful degradation for MetadataStore**: `MetadataStore::disabled()` returns a no-op store when SQLite is unavailable. `with_conn` returns `Ok(T::default())` when the store is disabled. Features using the metadata store must not hard-fail if it is unavailable.

- Pattern: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` (lines 97–115)
- Init: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` (lines 46–48)

**`tracing::warn!` for non-fatal background errors**: Background operations (cache persistence, log streaming, metadata reconciliation) use `tracing::warn!` with structured fields (`%error`, named fields) instead of propagating errors to the user. Frontend-visible errors come only from the direct `invoke` result.

**Frontend normalizes errors to string**: Hooks use a `normalizeErrorMessage(error: unknown): string` helper (pattern: `error instanceof Error ? error.message : String(error)`) for all `catch` blocks.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useUpdateGame.ts` (lines 59–61)

**Guard IPC nested objects in the frontend**: Profiles loaded over IPC may be sparse (legacy TOML). Do not dereference nested fields like `profile.runtime.proton_path` without first confirming the parent object exists. The lessons log flags this as a past crash source.

---

## Testing Approach

**`#[cfg(test)] mod tests { ... }` co-located with source**: Every non-trivial `.rs` file has an inline test module. Tests import via `use super::*;`. No separate `tests/` directory is used in the core crate.

**`MetadataStore::open_in_memory()` for all Rust tests**: Tests that exercise MetadataStore use the in-memory variant. Never open a real on-disk database in tests.

- API: `MetadataStore::open_in_memory()` in `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`

**`tempfile::tempdir()` for filesystem tests**: Tests that touch the filesystem create a temporary directory with `tempdir().expect("temp dir")`. Paths within the temp dir are constructed programmatically. The `tempfile` crate is in `[dev-dependencies]`.

**Tokio runtime built inline for async-blocking tests**: Tests that exercise functions requiring a Tokio runtime build one inline: `tokio::runtime::Builder::new_current_thread().enable_all().build()`. The test then calls `runtime.block_on(async { spawn_blocking(|| ...).await })`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/service.rs` (lines 455–506)

**Helper fixtures for valid requests**: Each test module defines a `valid_request(temp_dir: &Path) -> *Request` factory that creates a minimally valid request against the temp dir. Tests then mutate individual fields to exercise validation edge cases.

**Pattern matching assertions over `.is_err()` booleans**: Tests use `assert!(matches!(result, Err(SomeError::SpecificVariant)))` rather than just `assert!(result.is_err())`. This verifies the exact error variant is returned.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/update/service.rs` (lines 186–332)

**No `mockall` — trait-free DI via function parameters**: This codebase does not use `mockall`. Testability is achieved by accepting dependencies (e.g., `MetadataStore`, paths, config) as function parameters. Use `MetadataStore::open_in_memory()` and `tempdir()` fixtures instead of mocking.

**No frontend test framework**: There is no configured frontend test framework. UI behavior is verified via dev/build scripts (`./scripts/dev-native.sh`, `./scripts/build-native.sh`), not automated tests.

**Run Rust tests with**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

---

## Patterns to Follow for protonup-integration

1. **Create `crosshook-core/src/protonup/` module** with `mod.rs`, `models.rs`, and `service.rs`. Declare the module in `lib.rs`.

2. **Define `ProtonUpRequest` / `ProtonUpResult` / `ProtonUpError`** following the Request/Result/Error triple pattern. Include `#[serde(default)]` on all fields and `#[serde(rename_all = "snake_case")]` on the error enum.

3. **Use `libprotonup` in `service.rs`** rather than shelling out. The crate is already in `Cargo.toml`. Wrap its async API with `tokio::runtime::Handle::try_current()?.block_on(...)` or expose the function as async and use `spawn_blocking` from the command layer.

4. **Cache the available-versions list in MetadataStore** using the `external_cache_entries` table (same table the ProtonDB and discovery clients use) with a `protonup:available-versions` cache key and a TTL. Add a `migrate_13_to_14` function for any new dedicated tables. Classify installed-version records as SQLite metadata (not TOML settings, not runtime-only).

5. **Follow the 3-stage cache→live→stale-fallback pattern** from `discovery/client.rs` lines 231–300. Do not invent a new abstraction.

6. **Create `src-tauri/src/commands/protonup.rs`** with `#[tauri::command]` wrappers that call core functions and `.map_err(|e| e.to_string())`. Register all commands in `lib.rs`'s `invoke_handler!`.

7. **For download progress**, emit Tauri events using Variant B (direct stdout/stderr streaming from `prefix_deps.rs` lines 234–320). Use kebab-case event names: `protonup-install-progress`, `protonup-install-complete`.

8. **Create `src/types/protonup.ts`** mirroring the Rust `ProtonUpRequest`, `ProtonUpResult`, and error enums with `snake_case` field names. Include a `/** Keep in sync with ProtonUpError::message() in crosshook-core protonup/models.rs */` comment on the validation messages const.

9. **Create `src/hooks/useProtonUpInstalls.ts`** following the `useProtonInstalls.ts` pattern: `useState` + `useEffect` + `invoke` + `reload` callback. For install/download operations, follow `useUpdateGame.ts`: listen before invoke, `unlistenRef`, stage machine (`idle` / `preparing` / `installing` / `complete` / `failed`), `canStart` / `isRunning` derived values.

10. **Validate the installed Proton/Wine path exists and is executable** after download, using the same pattern as `validate_proton_path` in `install/service.rs`.

11. **Guard nested profile fields on the frontend** — when reading `profile.runtime.*` for pre-populating a ProtonUp install form, check that `profile.runtime` exists before dereferencing.

12. **If adding a scroll container**, register the selector in `src/crosshook-native/src/hooks/useScrollEnhance.ts`'s `SCROLLABLE` const. Add `overscroll-behavior: contain` on inner scroll containers.
