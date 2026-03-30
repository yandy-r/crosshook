# Pattern Research: trainer-version-correlation

## Architectural Patterns

**MetadataStore Facade with Module Delegation**: `MetadataStore` (struct) is a thread-safe facade (`Arc<Mutex<Connection>>`) that delegates to private per-concern module functions. New per-table stores (e.g., a version correlation store) should follow this — add a module under `metadata/`, add free functions there, and expose them via public methods on `MetadataStore` using the `with_conn` / `with_conn_mut` helpers.

- Pattern: `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:79-115`
- Example module: `src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs`

**Fail-Soft Metadata Access**: All MetadataStore methods return `Ok(T::default())` when the store is unavailable, never propagating errors to callers. Any new version-correlation queries must follow the same fail-soft pattern used in the health enrichment flow.

- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:79-115`
- `src/crosshook-native/src-tauri/src/commands/health.rs:92-99`

**Enrichment Layer in Tauri Commands**: Enrichment with metadata (failure trends, launcher drift, etc.) happens in the Tauri command handler, not in core. Core provides focused query functions; the command layer assembles composite response structs. Trainer version correlation enrichment should follow the same pattern as `EnrichedProfileHealthReport`.

- `src/crosshook-native/src-tauri/src/commands/health.rs:26-41`

**Batch Prefetch Pattern**: Before iterating over profiles, prefetch all metadata into `HashMap`s in a single pass, then do O(1) lookups per profile. Avoid N+1 queries.

- `src/crosshook-native/src-tauri/src/commands/health.rs:81-156`

**Community Version Data is Already Stored**: `game_version`, `trainer_version`, and `proton_version` are already present in `CommunityProfileMetadata`, stored in the `community_profiles` SQLite table, and exposed in `CommunityProfileRow`. Trainer version correlation can query these existing columns without a migration.

- `src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs:22-42`
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs:244-264`
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs:155-172` (migration 3→4)

**SHA-256 Hashing Already Available**: `sha2::{Digest, Sha256}` is already an active dependency used in `metadata/profile_sync.rs`. Trainer file content hashing for change detection follows the same pattern — no new crate needed.

- `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs:5-6`

**A6 Bounds Gap in Version Fields**: `check_a6_bounds()` in `community_index.rs` validates `game_name`, `trainer_name`, `description`, `author`, and `platform_tags`, but `game_version`, `trainer_version`, and `proton_version` are currently **unbounded**. Any new version correlation indexing should add `MAX_VERSION_BYTES = 256` bounds checks for these fields alongside the existing constants.

- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs:9-14`

**Launch Outcome Hook Point**: `record_launch_finished()` in `metadata/launch_history.rs` is called after every launch and determines `LaunchOutcome::Succeeded` via `FailureMode::CleanExit`. This is the correct hook point for recording a successful launch against a trainer version snapshot.

- `src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs:56-119`
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs:103-121` (`LaunchOutcome` enum)

## Code Conventions

**Rust Naming**:

- Module files: `snake_case` (e.g., `version_correlation.rs`)
- Functions: `snake_case` verbs (e.g., `query_matching_community_profiles`, `list_version_matches`)
- Structs: `PascalCase` (e.g., `VersionMatchRow`, `VersionCorrelationResult`)
- Tauri commands: `snake_case` matching frontend `invoke()` call name (e.g., `get_trainer_version_matches`)

**Serde Conventions**:

- All IPC-crossing types: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- Enums: `#[serde(rename_all = "snake_case")]`
- Optional fields that omit when empty: `#[serde(default, skip_serializing_if = "Vec::is_empty")]`
- `Option<String>` columns use a `nullable_text()` helper before insertion
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs:305-311`

**Error Propagation in Tauri Commands**: Commands return `Result<T, String>`. The canonical helper used throughout is:

```rust
fn map_error(error: impl ToString) -> String { error.to_string() }
```

And used as `.map_err(map_error)`. Defined locally per command module, not shared.

- `src/crosshook-native/src-tauri/src/commands/community.rs:12-14`

**TypeScript Types Mirror Rust Serde**: TS interfaces use `snake_case` fields to match serde output. Union string types for enums. Types in `src/types/*.ts`, re-exported from `src/types/index.ts`.

- `src/crosshook-native/src/types/health.ts`
- `src/crosshook-native/src/types/profile.ts`

## Error Handling

**Structured Error Enums**: Each layer defines its own error enum with context-carrying variants:

```rust
MetadataStoreError::Database { action: &'static str, source: SqlError }
MetadataStoreError::Io { action: &'static str, path: PathBuf, source: std::io::Error }
```

The `action` string is a past-tense description useful for diagnostics (e.g., `"query version correlation matches"`).

- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs:8-63`

**Non-Fatal Metadata Failures Use `tracing::warn!`**: When metadata enrichment fails, use `tracing::warn!(%e, ...)` and continue, never propagate to the user. The same pattern applies to any version correlation lookups.

- `src/crosshook-native/src-tauri/src/commands/community.rs:108-119`
- `src/crosshook-native/src-tauri/src/commands/health.rs:238-252`

**Frontend Error Normalization**:

```ts
function normalizeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
```

Used in all hook `catch` handlers. Pattern in `src/crosshook-native/src/hooks/useProfileHealth.ts:112-114`.

## Testing Approach

**Real In-Memory Database for Store Tests**: Use `MetadataStore::open_in_memory()` (not mocks) to test query functions. See `src/crosshook-native/crates/crosshook-core/src/metadata/` — all store modules are integration-tested against real SQLite.

- `src/crosshook-native/crates/crosshook-core/src/metadata/db.rs:53-62`

**Tempfile for Filesystem Tests**: Tests that exercise file scanning or path checks use `tempfile::tempdir()`.

- `src/crosshook-native/crates/crosshook-core/src/profile/health.rs:533-545`

**IPC Contract Tests via Type Casts**: The community command module has a test that casts each command function to its expected signature to verify the IPC contract at compile time.

- `src/crosshook-native/src-tauri/src/commands/community.rs:244-286`

**Test Naming Convention**: `{scenario}_{expected_behavior}`, e.g., `healthy_profile_reports_healthy_status`, `missing_game_exe_reports_stale`.

- `src/crosshook-native/crates/crosshook-core/src/profile/health.rs:599-677`

**Tests Inline with Source**: All tests are in `#[cfg(test)] mod tests {}` at the bottom of the source file, never in separate test files.

## Patterns to Follow

**Adding a New Query to MetadataStore**:

1. Add free functions in a new or existing module under `metadata/` (e.g., `metadata/version_correlation.rs`)
2. Import the module in `metadata/mod.rs` as `mod version_correlation;`
3. Add a public delegating method on `MetadataStore` using `with_conn`
4. Keep the method fail-soft (returns `Ok(Vec::new())` or `Ok(None)` when unavailable)

**Adding a New Migration** (if new columns/tables needed):

- Schema is currently at **version 8** — next migration is `migrate_8_to_9`
- Add `migrate_8_to_9(conn)` function in `metadata/migrations.rs`
- Add an `if version < 9` guard block in `run_migrations`
- Use `ALTER TABLE ... ADD COLUMN` for additive changes (cheapest)
- Use CREATE TABLE new / INSERT / DROP / RENAME pattern for structural changes
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`

**Adding a New Tauri Command**:

1. Add command function in the appropriate `src-tauri/src/commands/*.rs` file
2. Register it in `src-tauri/src/commands/mod.rs` in `tauri::generate_handler![]`
3. Add a type-cast IPC contract test alongside it
4. Mirror the return type as a TypeScript interface in `src/types/`
5. Use `metadata_store.is_available()` guard — never block launch on metadata unavailability

**Recommended module name**: `metadata/version_store.rs` (matches `health_store.rs` naming convention exactly)

**Version Correlation Query Shape** (following community_index pattern):

```rust
pub fn query_community_profiles_for_trainer_version(
    conn: &Connection,
    trainer_name: &str,
    game_name: Option<&str>,
) -> Result<Vec<CommunityProfileRow>, MetadataStoreError>
```

Query `community_profiles` table (already has `trainer_version`, `game_version`, `trainer_name` columns). No new table needed for basic correlation.

**KISS: No Semver Parser Needed**: Steam `buildid` values are integers. Version matching is `!=` equality, not range comparison. No semver crate, no parsing logic.

**KISS: Extend Existing Hook, Don't Create a New One**: For v1, add `version_record?: VersionCorrelationRecord` to `EnrichedProfileHealthReport` rather than creating a standalone `useVersionCorrelation` hook. Version data surfaces in the Health Dashboard and LaunchPage warning banner — the same surfaces already driven by `useProfileHealth`.

**Frontend Hook Pattern** (following useProfileHealth):

```ts
type HookStatus = 'idle' | 'loading' | 'loaded' | 'error';
type State = { status: HookStatus; data: T | null; error: string | null };
// useReducer for state machine, useCallback for invoke wrappers, useMemo for derived maps
```
