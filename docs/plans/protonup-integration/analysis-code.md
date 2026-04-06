# Code Analysis: protonup-integration

## Executive Summary

Current code already provides key primitives needed for ProtonUp integration: runtime discovery, cache storage, command wiring, and hook-based frontend IPC patterns. New implementation should create a dedicated `protonup` domain under `crosshook-core` and connect it to minimal new command/hook surfaces. This keeps the feature modular and testable while aligning with established repository conventions.

## Existing Code Structure

### Related Components

- `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`: installed runtime detection and compatibility-tool normalization.
- `/src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs`: Steam root discovery used to validate install targets.
- `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: generic cached payload API.
- `/src/crosshook-native/src-tauri/src/commands/steam.rs`: existing proton install listing command.
- `/src/crosshook-native/src/hooks/useProtonInstalls.ts`: typed frontend retrieval + refresh logic.

### File Organization Pattern

Domain logic is grouped under `crosshook-core/src/<domain>/`; Tauri commands are grouped by domain in `src-tauri/src/commands/`; frontend hook and type layers map command payloads before page components render state.

## Implementation Patterns

### Pattern: Cache-first service

**Description**: Services attempt local cache first, then network, with explicit stale/offline signaling.
**Example**: See `/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`.
**Apply to**: Proton version catalog retrieval and stale-state UX.

### Pattern: Thin command wrappers

**Description**: Commands defer business logic and map errors at IPC boundary.
**Example**: See `/src/crosshook-native/src-tauri/src/commands/protondb.rs`.
**Apply to**: `protonup_list_available_versions`, `protonup_install_version`, suggestion check commands.

### Pattern: Hook abstraction over invoke

**Description**: Frontend wraps command calls in hooks and exposes stable state API.
**Example**: See `/src/crosshook-native/src/hooks/useProtonInstalls.ts`.
**Apply to**: New ProtonUp catalog/install hook and UI integration.

## Integration Points

### Files to Create

- `/src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs`: domain types + service interfaces.
- `/src/crosshook-native/crates/crosshook-core/src/protonup/service.rs`: catalog/install/suggestion orchestration.
- `/src/crosshook-native/src-tauri/src/commands/protonup.rs`: new command wrappers.
- `/src/crosshook-native/src/hooks/useProtonUp.ts`: frontend command state management.
- `/src/crosshook-native/src/types/protonup.ts`: TypeScript DTOs for IPC payloads.

### Files to Modify

- `/src/crosshook-native/crates/crosshook-core/src/lib.rs`: export new module.
- `/src/crosshook-native/src-tauri/src/lib.rs`: register protonup commands.
- `/src/crosshook-native/src/hooks/useProtonInstalls.ts`: refresh integration after install completion.
- `/src/crosshook-native/src/components/pages/ProfilesPage.tsx`: recommendation and resolve actions.
- `/src/crosshook-native/src/components/pages/CompatibilityPage.tsx`: install action surface and state messaging.
- `/src/crosshook-native/src/types/settings.ts`: optional preference/path fields.

## Code Conventions

### Naming

- Rust modules/functions: snake_case.
- Tauri commands: snake_case matching frontend invoke names.
- TS hooks/components: camelCase hooks, PascalCase components.

### Error Handling

- Core returns structured `Result`/domain errors.
- Tauri maps to boundary-safe errors.
- Frontend maps categories into actionable UI copy.

### Testing

- Add focused unit tests in new protonup modules for matching, cache behavior, and error mapping.
- Use in-memory metadata patterns for cache tests where possible.

## Dependencies and Services

### Available Utilities

- `MetadataStore` + `cache_store`: existing cache implementation.
- `steam/proton.rs`: runtime discovery utility to reuse.
- Existing command/hook patterns: reduce boilerplate and contract drift.

### Required Dependencies

- Provider dependency decision accepted as hybrid adapter strategy; implementation may use `libprotonup` and/or CLI adapter behind stable interface.

## Gotchas and Warnings

- Do not duplicate installed-runtime scanning in provider module.
- Do not let community metadata mismatch become hard launch block by accident.
- Ensure install destination checks protect against writes outside allowed roots.
- Ensure long-running install operations do not block UI thread or command responsiveness.

## Reuse and Modularity Guidance

- **Reuse First**: `steam/proton.rs`, `cache_store.rs`, and existing hook/command patterns.
- **Keep Feature-Local**: provider implementation details inside `crosshook-core/protonup`.
- **Build vs. Depend**: keep adapter boundary; avoid committing architecture to a single upstream integration strategy.

## Task-Specific Guidance

- **For core logic tasks**: start with pure matching/cache DTO logic before side-effectful install orchestration.
- **For IPC tasks**: define stable payload types first, then wire command handlers.
- **For UI tasks**: wire read-only recommendation states before install-trigger actions.
