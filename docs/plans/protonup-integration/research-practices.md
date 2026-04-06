# Practices Research: protonup-integration

## Executive Summary

The best implementation strategy is to extend existing Proton discovery, metadata caching, and hook-based IPC patterns rather than creating parallel subsystems. Most reusable value is already present in `steam/proton.rs`, `metadata/cache_store.rs`, and existing Tauri/frontend command-hook contracts. The key modularity risk is over-abstracting provider behavior too early; begin with a tight adapter interface and keep workflow-specific logic feature-local until multiple providers are proven.

## Existing Reusable Code

| Module/Utility | Location | Purpose | How to Reuse for This Feature |
| -------------- | -------- | ------- | ----------------------------- |
| Proton install discovery | `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs` | discover local compatibility tools | source of truth for installed runtimes post-install |
| Steam root detection | `/src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs` | discover candidate Steam roots | validate and resolve allowed install destinations |
| Cache store API | `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` | TTL cache in `external_cache_entries` | store/retrieve available provider catalogs |
| Async IPC pattern | `/src/crosshook-native/src-tauri/src/commands/protondb.rs` | safe async command boundary pattern | model new `protonup_*` commands |
| Proton install hook pattern | `/src/crosshook-native/src/hooks/useProtonInstalls.ts` | typed `invoke()` wrapper with refresh UX | extend for install/candidate listing hooks |

## Modularity Findings

- Add a dedicated `crosshook-core` protonup module with three local responsibilities: catalog, install orchestration, and recommendation matching.
- Keep provider adapter internal to that module; expose only stable domain DTOs and service methods upward.
- Keep UI state handling in hooks/components and avoid mixing provider execution logic into frontend.

## KISS Assessment

- Start with one provider family (GE-Proton) and advisory matching; avoid broad multi-provider policy engines in v1.
- Avoid persisting detailed install job histories until a clear product need exists; runtime status + logs are sufficient initially.
- Avoid introducing new generalized “runtime platform framework” abstractions before a second real integration demands them.

## Build vs. Depend Decisions

- Prefer depending on existing `metadata/cache_store` and `steam/*` modules rather than building new cache/discovery subsystems.
- Use a provider adapter that can support `libprotonup` or CLI without hard-binding all architecture to one implementation.
- Defer new dependencies unless they materially reduce risk/complexity versus existing project patterns.

## Testability Notes

- Keep matching/recommendation functions pure and deterministic for straightforward unit tests.
- Isolate provider execution behind trait-like boundaries to enable fixture-based tests and error-path coverage.
- Use in-memory metadata store patterns already used in core for cache behavior and stale/fresh fallback tests.
