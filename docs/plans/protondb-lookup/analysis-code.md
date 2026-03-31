# Code Analysis: protondb-lookup

## Executive Summary

The current codebase already has strong precedents for every layer this feature needs except the HTTP client itself. The key code decision is to avoid overloading existing compatibility types: `CompatibilityRating` is tuned for CrossHook’s community profile experience, while issue `#53` needs exact ProtonDB tiers and a new lookup DTO. Recommendation application should reuse existing profile mutation paths (`launch.custom_env_vars` and copyable Steam launch options) instead of inventing a raw “store arbitrary launch string” model.

## Existing Code Structure

### Related Components

- /src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: cache/store facade and metadata query surface
- /src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs: read/write TTL cache implementation
- /src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs: legacy compatibility metadata surface
- /src/crosshook-native/src-tauri/src/commands/version.rs: typed command + metadata-state precedent
- /src/crosshook-native/src/components/ProfileFormSections.tsx: primary mount point for new advisory UI
- /src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx: copyable launch-options pattern
- /src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx: editable env-var table and conflict expectations

### File Organization Pattern

Related backend features live in feature-local modules under `crosshook-core/src/<feature>/`, then get exported through `crosshook-core/src/lib.rs`. Tauri commands each live in `src-tauri/src/commands/<feature>.rs` and are registered through both `commands/mod.rs` and the big `generate_handler!` block in `src-tauri/src/lib.rs`. Frontend contracts live under `src/types/`, hooks under `src/hooks/`, and UI surfaces under `src/components/`.

## Implementation Patterns

### Pattern: Thin Tauri Command

**Description**: `src-tauri` should translate arguments/state and call backend helpers; it should not own feature logic.  
**Example**: See `/src/crosshook-native/src-tauri/src/commands/version.rs`  
**Apply to**: the new `protondb_lookup` command

### Pattern: Shared Cache Helper Reuse

**Description**: remote/cache-like feature state is persisted through `MetadataStore`, which already enforces payload caps and TTL semantics.  
**Example**: See `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`  
**Apply to**: ProtonDB summary and normalized recommendation snapshots

### Pattern: Component + Composition

**Description**: larger editor features are introduced as their own component, then mounted inside the form/page that owns the workflow.  
**Example**: See `/src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx`  
**Apply to**: `ProtonDbLookupCard`

### Pattern: Typed Invoke Hook

**Description**: the frontend hides `invoke()` behind a typed hook with stable state names and cancellation-safe effects.  
**Example**: See `/src/crosshook-native/src/hooks/useProfileHealth.ts`  
**Apply to**: `useProtonDbLookup`

## Integration Points

### Files to Create

- /src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs: backend module entry point
- /src/crosshook-native/crates/crosshook-core/src/protondb/models.rs: exact-tier and normalized DTO contracts
- /src/crosshook-native/crates/crosshook-core/src/protondb/client.rs: remote fetch + cache read-through
- /src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs: recommendation normalization and safe parsing
- /src/crosshook-native/src-tauri/src/commands/protondb.rs: IPC bridge
- /src/crosshook-native/src/types/protondb.ts: frontend DTO mirror
- /src/crosshook-native/src/hooks/useProtonDbLookup.ts: lookup-state hook
- /src/crosshook-native/src/components/ProtonDbLookupCard.tsx: editor UI

### Files to Modify

- /src/crosshook-native/crates/crosshook-core/Cargo.toml: add HTTP client dependency
- /src/crosshook-native/crates/crosshook-core/src/lib.rs: export the new module
- /src/crosshook-native/src-tauri/src/commands/mod.rs: register command module
- /src/crosshook-native/src-tauri/src/lib.rs: register command handler
- /src/crosshook-native/src/components/ProfileFormSections.tsx: mount and wire the card
- /src/crosshook-native/src/components/pages/ProfilesPage.tsx: pass selected-profile context
- /src/crosshook-native/src/styles/theme.css: exact-tier styling
- /src/crosshook-native/src/types/index.ts: export the new types

## Code Conventions

### Naming

- Rust: `snake_case` modules and command names, Serde-friendly structs, no `any`
- TypeScript: `PascalCase` components, `camelCase` hooks/functions, exported interfaces in `src/types/`

### Error Handling

- Use project `Result` types in `crosshook-core`, then collapse to `String` only at the Tauri boundary.
- Use soft advisory UI states for remote fetch errors rather than throwing them into the profile form’s generic error surface.

### Testing

- Add Rust tests around parsing, cache fallback, and safe suggestion normalization.
- Run `cargo test` for `crosshook-core`, `cargo test --no-run` for `crosshook-native`, and `tsc --noEmit` for the frontend.

## Dependencies and Services

### Available Utilities

- `MetadataStore`: remote snapshot persistence with payload size guardrails
- `copyToClipboard`: reusable frontend copy action helper through `SteamLaunchOptionsPanel`
- `onUpdateProfile`: existing safe profile mutation path already threaded through `ProfileFormSections`

### Required Dependencies

- `reqwest` (recommended) in `crosshook-core` for HTTP/TLS/JSON/timeout support

## Gotchas and Warnings

- `CompatibilityRating` cannot represent `gold`, `silver`, `bronze`, or `borked`; do not force it to do so.
- ProtonDB’s richer report feed is not documented and not keyed by Steam App ID in the observed live route.
- CrossHook has no raw profile field for arbitrary launch strings, so unsupported ProtonDB launch options cannot be auto-applied safely.
- `theme.css` currently only styles `unknown`, `broken`, `partial`, `working`, and `platinum`; exact ProtonDB tiers need dedicated styling.
- Any apply flow that touches `launch.custom_env_vars` must preserve user intent on key collisions.

## Reuse and Modularity Guidance

- **Reuse First**: keep using `external_cache_entries` until a real limitation forces a schema change.
- **Keep Feature-Local**: build `crosshook-core::protondb` as a focused module, not a generalized “remote advisory” framework.
- **Build vs. Depend**: add one HTTP dependency instead of relocating the feature into the frontend or shelling out.

## Task-Specific Guidance

- **For backend tasks**: mirror the thin-command and cache helper patterns already used by version/steam features.
- **For UI tasks**: reuse the existing panel/copy/action patterns rather than creating a bespoke dialog or page.
- **For recommendation tasks**: only auto-map whitelisted env-var suggestions into existing profile fields; leave everything else informational.
