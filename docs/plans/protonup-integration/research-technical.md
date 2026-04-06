## Executive Summary

CrossHook already has strong local Proton discovery and path validation in `crosshook-core`, but it lacks install orchestration for missing runtimes. Technical implementation should add a ProtonUp-focused service in `crosshook-core` for release discovery, install execution, and suggestion matching against community metadata. `src-tauri` should expose thin snake_case commands, and the frontend should consume typed hooks that extend existing Proton install listing patterns.

## Architecture Approach

- Add a new domain module under `crosshook-core` (for example `protonup/`) with:
  - release catalog retrieval and caching,
  - installer orchestration through a provider abstraction (CLI or library),
  - suggestion/matching logic between `community_profiles.proton_version` and installed runtimes.
- Reuse existing Steam discovery roots and install scanning in `crosshook-core/src/steam/proton.rs`.
- Keep `src-tauri` thin by adding command wrappers that call core services and return Serde-safe DTOs.
- Keep frontend integration in hooks and page-level UI (settings/profiles/compatibility flows), then refresh installed runtime list after install completion.

### Data Model Implications

- TOML settings:
  - optional default preference for auto-suggest behavior,
  - optional binary path override if CLI execution is chosen.
- SQLite metadata:
  - cache available release list in `external_cache_entries` with TTL and source timestamp,
  - optionally track install operation history if product wants audit/retry analytics.
- Runtime-only:
  - install progress stream,
  - transient job status and cancellation flags.
- Migration impact:
  - no mandatory DB migration if using existing cache table and no new history table.

## API Design Considerations

- Proposed Tauri commands (snake_case):
  - `protonup_list_available_versions`
  - `protonup_install_version`
  - `protonup_get_install_status`
  - `protonup_suggest_for_profile`
- Request/response contracts should include:
  - provider/tool family,
  - version identifier,
  - target install root,
  - normalized match status (`matched`, `missing`, `unknown`, `error`),
  - user-facing error codes for dependency missing, permission denied, network failure, checksum failure.
- Keep command naming aligned with frontend `invoke()` usage and typed wrappers in hooks.

## System Constraints

- Platform scope remains native Linux desktop; feature must not assume Windows execution environments.
- Path safety must enforce writes only inside expected compatibility tool directories.
- Download integrity should verify checksums where available before install finalization.
- Long-running install operations should avoid UI thread blocking and support cancellation.
- Offline mode should not fail local discovery or existing launch validation flows.

## File-Level Impact Preview

- Likely files to create:
  - `src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs`
  - `src/crosshook-native/crates/crosshook-core/src/protonup/service.rs`
  - `src/crosshook-native/src-tauri/src/commands/protonup.rs`
  - `src/crosshook-native/src/hooks/useProtonUp.ts`
  - `src/crosshook-native/src/types/protonup.ts`
- Likely files to modify:
  - `src/crosshook-native/crates/crosshook-core/src/lib.rs`
  - `src/crosshook-native/src-tauri/src/lib.rs`
  - `src/crosshook-native/src/hooks/useProtonInstalls.ts`
  - `src/crosshook-native/src/components/pages/ProfilesPage.tsx`
  - `src/crosshook-native/src/components/pages/CompatibilityPage.tsx`
  - `src/crosshook-native/src/types/settings.ts` (if adding settings flags/paths)
