# Pattern Research: protonup-integration

## Architectural Patterns

**Core-first domain services**: business logic is implemented in `crosshook-core`, then exposed through thin IPC wrappers.

- Example: `/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`
- Example: `/src/crosshook-native/src-tauri/src/commands/protondb.rs`

**Cache-first with stale fallback**: remote lookups first check cache, then fetch live, then fallback to stale data.

- Example: `/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`
- Example: `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`

**Hook-wrapped `invoke()` contract**: frontend pages consume typed custom hooks instead of invoking Tauri commands inline.

- Example: `/src/crosshook-native/src/hooks/useProtonInstalls.ts`
- Example: `/src/crosshook-native/src/hooks/useProtonDbSuggestions.ts`

## Code Conventions

- Rust modules use snake_case paths and explicit `mod.rs` wiring.
- Tauri commands are `#[tauri::command]` functions named in snake_case and return serializable DTOs/errors as strings at boundary.
- TypeScript uses PascalCase components and camelCase hooks/functions.
- IPC types mirror Serde camelCase fields in TS interfaces.

## Error Handling

- Core layer uses `Result<T, E>` with domain errors and contextual mapping.
- IPC layer converts domain errors to user-safe string errors.
- UI layer distinguishes actionable failures (dependency/path/network/checksum) from advisory states.

Reference examples:

- `/src/crosshook-native/src-tauri/src/commands/protondb.rs`
- `/src/crosshook-native/crates/crosshook-core/src/install/service.rs`

## Testing Approach

- Rust unit tests live near implementation modules.
- Metadata tests favor in-memory SQLite and deterministic fixtures.
- Hook/component behavior is validated through existing build/dev verification flow (no dedicated frontend test framework currently configured).

Reference examples:

- `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`
- `/src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`

## Patterns to Follow

- Reuse Steam/runtime discovery from `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs` instead of creating a second install inventory system.
- Reuse cache namespace + TTL model through `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`.
- Preserve non-blocking advisory matching semantics in UI and keep hard launch blocking tied only to invalid configured runtime paths.
- Keep provider integration behind an adapter so CLI/library implementation can change without command or UI contract churn.
