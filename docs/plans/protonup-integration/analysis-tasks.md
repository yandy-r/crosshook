# Task Structure Analysis: protonup-integration

## Executive Summary

The feature should be split into foundation contracts, core orchestration, and UI integration/hardening phases so high-risk security and path-validation work lands early. Backend and frontend can run in parallel once DTOs and command contracts are stable. Task granularity should keep each task focused on 1-3 files to reduce merge conflicts and improve review clarity.

## Recommended Phase Structure

### Phase 1: Contracts and Foundations

**Purpose**: establish DTOs, module skeleton, and cache/recommendation contracts.  
**Suggested Tasks**:

- Define Rust/TS DTOs and settings fields.
- Create protonup core module skeleton and service interface.
- Add recommendation matching pure logic and tests.
- Define Tauri command signatures and registration plan.
  **Parallelization**: 2-3 tasks can run in parallel once DTO ownership is clear.

### Phase 2: Core Provider and IPC

**Purpose**: implement catalog + install orchestration with guarded execution.  
**Suggested Tasks**:

- Implement provider adapter and catalog cache flow.
- Implement install orchestration with path + checksum guardrails.
- Add Tauri command handlers and error mapping.
- Add backend tests for cache/offline/install error paths.
  **Dependencies**: Phase 1 contracts and module boundaries.
  **Parallelization**: provider implementation and command wiring can run parallel after interface lock.

### Phase 3: Frontend UX Integration

**Purpose**: expose recommendation/install UX while preserving launch continuity.  
**Suggested Tasks**:

- Build `useProtonUp` hook and integrate with existing install list refresh.
- Add recommendation and resolve actions in `ProfilesPage`.
- Add compatibility-page install affordances and stale/offline states.
- Add UI guardrails for progress/recovery and advisory messaging.
  **Dependencies**: Phase 2 command availability.

### Phase 4: Hardening and Final Validation

**Purpose**: reduce regression risk and finalize implementation readiness.  
**Suggested Tasks**:

- Add targeted tests for matching edge cases and install outcomes.
- Verify no launch-path regression for already-valid runtime configs.
- Validate docs and update troubleshooting notes.
  **Dependencies**: Phase 2 and 3 complete.

## Task Granularity Recommendations

### Appropriate Task Sizes

- “Add protonup domain DTOs and settings flags” (2-3 files).
- “Implement provider catalog cache retrieval” (2 files).
- “Wire Tauri command registration and wrappers” (2 files).
- “Integrate ProfilesPage recommendation card” (1-2 files).

### Tasks to Split

- “Implement all backend and UI install logic” should be split into provider, IPC, and UI tasks.
- “Full validation and test suite” should be split into core tests and UI behavior verification.

### Tasks to Combine

- DTO definition and command signature setup can be combined if owned by one contributor.

## Dependency Analysis

### Independent Tasks (Can Run in Parallel)

- Task: Recommendation matching pure functions - File(s): `crosshook-core/src/protonup/*`.
- Task: Settings/type surface updates - File(s): `settings/mod.rs`, `src/types/settings.ts`.
- Task: UX copy/state design prep - File(s): `ProfilesPage.tsx`, `CompatibilityPage.tsx`.

### Sequential Dependencies

- Provider adapter contract must complete before command handlers to avoid API churn.
- Command handlers must complete before frontend hook integration for end-to-end wiring.
- Install guardrails (path + checksum) should complete before enabling one-click install UI.

### Potential Bottlenecks

- Shared command registration file: `/src/crosshook-native/src-tauri/src/lib.rs`.
- Shared UI pages: `/src/crosshook-native/src/components/pages/ProfilesPage.tsx`.

## File-to-Task Mapping

### Files to Create

| File                                                                  | Suggested Task                                   | Phase | Dependencies |
| --------------------------------------------------------------------- | ------------------------------------------------ | ----- | ------------ |
| `/src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs`     | Create protonup core domain types/interfaces     | 1     | none         |
| `/src/crosshook-native/crates/crosshook-core/src/protonup/service.rs` | Implement catalog/install/recommendation service | 2     | Phase 1      |
| `/src/crosshook-native/src-tauri/src/commands/protonup.rs`            | Add protonup IPC wrappers                        | 2     | Phase 1      |
| `/src/crosshook-native/src/hooks/useProtonUp.ts`                      | Add frontend hook for protonup flows             | 3     | Phase 2      |
| `/src/crosshook-native/src/types/protonup.ts`                         | Add TS IPC DTO definitions                       | 1     | none         |

### Files to Modify

| File                                                               | Suggested Task                            | Phase | Dependencies |
| ------------------------------------------------------------------ | ----------------------------------------- | ----- | ------------ |
| `/src/crosshook-native/crates/crosshook-core/src/lib.rs`           | Export protonup module                    | 1     | none         |
| `/src/crosshook-native/src-tauri/src/lib.rs`                       | Register protonup commands                | 2     | command file |
| `/src/crosshook-native/src/hooks/useProtonInstalls.ts`             | Refresh after install completion          | 3     | Phase 2      |
| `/src/crosshook-native/src/components/pages/ProfilesPage.tsx`      | Add recommendation + resolve actions      | 3     | Phase 2      |
| `/src/crosshook-native/src/components/pages/CompatibilityPage.tsx` | Add install controls and stale/offline UX | 3     | Phase 2      |
| `/src/crosshook-native/src/types/settings.ts`                      | Add preference/path settings fields       | 1     | none         |

## Optimization Opportunities

### Maximize Parallelism

- Run TS type/settings updates in parallel with core matching logic.
- Run frontend page work in parallel once hook contract is fixed.

### Minimize Risk

- Isolate provider install side-effects behind explicit guardrails before exposing UI triggers.
- Keep initial provider scope to GE-Proton per confirmed decision.

## Security-Critical Sequencing

- Path validation and checksum verification must be implemented before install commands are enabled in UI.
- Input validation and command argument sanitization should ship with first command implementation.

## Reuse Recommendations

- Extend `steam/proton.rs` and `cache_store.rs` instead of introducing duplicate discovery/cache subsystems.
- Keep provider adapter feature-local until multi-provider needs justify shared abstractions.

## Implementation Strategy Recommendations

- Prefer bottom-up for core module and command contracts, then top-down for UI integration.
- Write unit tests with core matching/cache logic and extend coverage as install side-effects are added.
- Gate UI install affordances on backend command readiness and explicit error category handling.
