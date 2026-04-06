# Context Analysis: protonup-integration

## Executive Summary

The feature introduces an in-app path for listing and installing Proton variants while preserving CrossHook’s existing launch validation contract. Implementation should add a focused ProtonUp service in `crosshook-core`, keep Tauri handlers thin and snake_case, and extend existing Proton hooks/pages for UX. The rollout should prioritize read-only recommendation visibility before install side effects to reduce regression risk.

## Architecture Context

- **System Structure**: Core logic in `crosshook-core`; IPC wrappers in `src-tauri`; typed React hooks/pages in frontend.
- **Data Flow**: frontend action -> Tauri command -> core provider/cache/discovery service -> DTO response -> UI state.
- **Integration Points**: `steam/proton.rs` for installed inventory, `metadata/cache_store.rs` for catalogs, `community_schema.rs` for advisory version context.

## Critical Files Reference

- `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`: canonical installed-runtime scan and normalization.
- `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: version catalog cache substrate.
- `/src/crosshook-native/src-tauri/src/lib.rs`: command registration and IPC exposure boundary.
- `/src/crosshook-native/src/hooks/useProtonInstalls.ts`: existing hook model to extend for ProtonUp workflows.
- `/src/crosshook-native/src/components/pages/ProfilesPage.tsx`: primary runtime selection and recommendation touchpoint.

## Patterns to Follow

- **Core-first services**: new feature logic should live in core modules, not in command files.
- **Cache-live-stale behavior**: use cache as first-class UX source with explicit stale indicators.
- **Typed hook wrapping**: page components should not make raw `invoke()` calls for ProtonUp operations.

## Cross-Cutting Concerns

- Preserve non-blocking launch behavior where local runtime remains valid.
- Maintain clear distinction between advisory suggestions and hard validation failures.
- Keep UX responsive during long-running install operations.

## Security Constraints

- Enforce path root checks and no shell interpolation in provider execution path.
- Require checksum verification before marking installs successful.
- Treat community `proton_version` metadata as untrusted advisory input.

## Reuse Opportunities

- `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`: extend discovery refresh, do not duplicate scan logic.
- `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: reuse cache API and key TTL handling.
- Keep provider abstraction local to new protonup module; avoid premature global runtime framework.

## Parallelization Opportunities

- Core catalog/matching service work can run in parallel with frontend recommendation UI wiring after DTO contracts are defined.
- Security guardrails and provider adapter contract should land before broad install UX integration.

## Implementation Constraints

- Must keep Tauri command names snake_case and Serde-compatible DTOs.
- Must classify persistence by TOML vs SQLite vs runtime-only.
- Should avoid new DB migration for v1 unless install-history persistence is required.

## Key Recommendations

- Phase sequence: discovery/suggestions -> explicit install pipeline -> hardening/tests.
- Set provider scope to accepted decision (GE-Proton first).
- Keep command contracts stable so backend provider implementation can evolve safely.
