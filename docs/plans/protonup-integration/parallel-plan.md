# ProtonUp Integration Implementation Plan

ProtonUp integration should be implemented as a core-first feature that extends existing runtime discovery and cache infrastructure instead of introducing parallel systems. The highest-risk work is provider execution and filesystem writes, so path-validation and integrity guardrails must land before install UX is enabled. IPC contracts should remain minimal and stable, with Tauri as a thin translation layer and React consuming typed hooks. Delivery is staged: read-only recommendations first, explicit install flow second, then hardening and quality improvements.

## Critically Relevant Files and Documentation

- /src/crosshook-native/crates/crosshook-core/src/steam/proton.rs: Installed runtime discovery source of truth.
- /src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs: Steam root resolution for allowed install destinations.
- /src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs: Cache storage for available version catalog payloads.
- /src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: TOML settings surface for ProtonUp defaults/path override.
- /src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs: Community metadata (`proton_version`) input for suggestions.
- /src/crosshook-native/src-tauri/src/commands/steam.rs: Existing proton list command pattern.
- /src/crosshook-native/src-tauri/src/lib.rs: Command registration entry point.
- /src/crosshook-native/src/hooks/useProtonInstalls.ts: Existing hook pattern and refresh flow.
- /src/crosshook-native/src/components/pages/ProfilesPage.tsx: Profile-level recommendation/resolve integration point.
- /src/crosshook-native/src/components/pages/CompatibilityPage.tsx: Compatibility-page install controls and status messaging.
- /docs/plans/protonup-integration/feature-spec.md: Accepted decisions and authoritative phase intent.
- /docs/plans/protonup-integration/shared.md: Consolidated architecture, security, and reuse guidance.

## Persistence and Usability

### Data Classification (authoritative for this plan)

- **TOML settings (user-editable, documented):**
  - Data surfaced via `/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` (for example Proton defaults/path overrides and discovery toggles) is persisted in `settings.toml` through `AppSettingsData`.
  - These values are user-facing preferences and must remain user-editable and documented in settings UX/tooling.

- **SQLite metadata (operational/cache/community context):**
  - Proton catalog payloads and freshness timestamps persisted through `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` are SQLite metadata (`external_cache_entries`), not user-edited settings.
  - Community metadata modeled in `/src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs` (including `proton_version`) is treated as metadata context when indexed/stored; it is not a direct user-editable preference surface.

- **Runtime-only state (ephemeral):**
  - Runtime discovery outputs from `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs` and `/src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs` are computed at runtime from filesystem state and command inputs.
  - Hook/UI state from `/src/crosshook-native/src/hooks/useProtonInstalls.ts` is in-memory React state (`installs`, `error`, reload counters) and should remain ephemeral.
  - Cache semantics still apply to remote catalogs via SQLite cache entries; runtime discovery lists themselves are not persisted by default.

### Migration and Backward Compatibility Strategy

- **No-schema-change path (preferred for v1):** use existing `external_cache_entries` and existing community metadata flow; no migration required.
- **If new SQLite tables/columns become necessary:** add a forward-only migration in `/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`, increment `user_version`, keep old rows readable, and provide safe defaults for newly introduced fields.
- **Backward compatibility contract:** missing/newly-added settings keys in `AppSettingsData` must deserialize with defaults and preserve prior behavior; migration must not break existing profile launch flows.

### Offline Expectations and Caching Policy

- Catalog lookups should follow cache-first/live-refresh/stale-fallback semantics through `cache_store.rs` with explicit TTL and age visibility.
- Offline or fetch-failure scenarios must preserve runtime discovery and local profile operations.
- UI should surface stale/offline status explicitly instead of implying hard failure.

### Degraded and Failure Fallbacks

- **Command layer (`/src/crosshook-native/src-tauri/src/commands/steam.rs` pattern):**
  - `list_proton_installs` should continue returning discovered installs (possibly empty) without introducing launch-blocking behavior.
  - Diagnostics belong in tracing/logging, not hard UI crashes.

- **Profile page (`/src/crosshook-native/src/components/pages/ProfilesPage.tsx`):**
  - If runtime discovery fails, show actionable error state and fallback to empty install list while preserving other profile editing actions.
  - Advisory recommendation mismatches must not become implicit hard blocks.

- **Compatibility page (`/src/crosshook-native/src/components/pages/CompatibilityPage.tsx`):**
  - When catalog/community feeds are unavailable, show empty/degraded messaging and retain navigation/read-only compatibility context.
  - Prefer explicit “offline/stale” messaging over silent omission.

- **Hook layer (`/src/crosshook-native/src/hooks/useProtonInstalls.ts` and future ProtonUp hook):**
  - Normalize command errors into UI-safe messages, keep state reset deterministic, and avoid stale in-memory state leaks across reloads.

### User Visibility and Editability

- Settings from `settings/mod.rs` are user-visible and editable; any new ProtonUp setting must be represented as a documented preference, not hidden metadata.
- SQLite cache/community metadata is internal operational data; expose status (fresh/stale/offline/source) but not raw row editing in normal UX.
- Steam command affordances and React hooks should expose clear user actions (refresh/retry/select/install) while preserving safety rails (explicit install action, integrity checks, and non-blocking launch when local runtime is valid).

## Implementation Plan

### Phase 1: Foundations and Contracts

#### Task 1.1: Define ProtonUp Core and TS DTO Contracts Depends on [none]

**READ THESE BEFORE TASK**

- /docs/plans/protonup-integration/feature-spec.md
- /src/crosshook-native/crates/crosshook-core/src/settings/mod.rs
- /src/crosshook-native/src/types/settings.ts

**Instructions**

Files to Create

- /src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs
- /src/crosshook-native/src/types/protonup.ts

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/lib.rs
- /src/crosshook-native/src/types/settings.ts

Define the minimal shared DTOs and service interface for ProtonUp catalog/list/install/suggestion workflows. Keep DTO names and field casing compatible with Serde and TypeScript usage. Add optional settings fields for ProtonUp behavior/path overrides without changing existing defaults, and include backward-compat checks confirming omitted keys preserve prior behavior.

#### Task 1.2: Implement Pure Recommendation Matching Logic Depends on [1.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/crates/crosshook-core/src/steam/proton.rs
- /src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs
- /docs/plans/protonup-integration/research-business.md

**Instructions**

Files to Create

- /src/crosshook-native/crates/crosshook-core/src/protonup/matching.rs

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs

Add pure, testable match-status logic that compares community runtime requirements to installed runtimes and returns advisory statuses. Keep the result contract explicit for UI rendering (`matched`, `missing`, `unknown`), and avoid side effects or provider calls in this module.

#### Task 1.3: Add ProtonUp Command Contracts and Registration Stubs Depends on [1.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src-tauri/src/commands/steam.rs
- /src/crosshook-native/src-tauri/src/lib.rs
- /docs/plans/protonup-integration/research-technical.md

**Instructions**

Files to Create

- /src/crosshook-native/src-tauri/src/commands/protonup.rs

Files to Modify

- /src/crosshook-native/src-tauri/src/lib.rs

Add snake_case command skeletons and register them in the Tauri command handler list. Keep handlers thin and defer business logic into core service calls. Ensure signatures and DTOs are stable before downstream UI wiring begins.

### Phase 2: Core Catalog and Install Orchestration

#### Task 2.1: Implement Provider Catalog Retrieval with Cache Fallback Depends on [1.1, 1.3]

**READ THESE BEFORE TASK**

- /src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs
- /src/crosshook-native/crates/crosshook-core/src/protondb/client.rs
- /docs/plans/protonup-integration/research-external.md

**Instructions**

Files to Create

- /src/crosshook-native/crates/crosshook-core/src/protonup/catalog.rs

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs
- /src/crosshook-native/src-tauri/src/commands/protonup.rs

Implement catalog listing with cache-live-stale behavior using `external_cache_entries`. Normalize provider release metadata into stable DTOs and expose stale/offline state for UI transparency. Keep provider scope aligned with accepted decision (GE-Proton first).

#### Task 2.2: Implement Install Execution with Security Guardrails Depends on [1.1, 1.3]

**READ THESE BEFORE TASK**

- /src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs
- /src/crosshook-native/crates/crosshook-core/src/steam/proton.rs
- /docs/plans/protonup-integration/research-security.md

**Instructions**

Files to Create

- /src/crosshook-native/crates/crosshook-core/src/protonup/install.rs

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs
- /src/crosshook-native/src-tauri/src/commands/protonup.rs

Implement explicit install orchestration with destination-root validation, structured process invocation, and checksum-required success criteria. Map failures into actionable categories (`dependency_missing`, `permission_denied`, `checksum_failed`, `network_error`) and keep launch-path side effects isolated. Define required timeout, cancellation, and progress-state semantics in the command response/event contract before enabling UI install triggers.

#### Task 2.3: Wire Suggestion and Status Commands to Core Services Depends on [1.2, 1.3, 2.1, 2.2]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src-tauri/src/commands/steam.rs
- /docs/plans/protonup-integration/research-technical.md
- /src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs

**Instructions**

Files to Modify

- /src/crosshook-native/src-tauri/src/commands/protonup.rs

Connect command handlers to core catalog/matching services and ensure responses clearly differentiate advisory mismatch from launch-blocking conditions. Keep command latency predictable and avoid blocking the Tauri main thread for long-running install operations.

### Phase 3: Frontend Integration and UX Flow

#### Task 3.1: Build useProtonUp Hook and Integrate Refresh Flow Depends on [2.1, 2.2, 2.3]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/hooks/useProtonInstalls.ts
- /src/crosshook-native/src/types/settings.ts
- /docs/plans/protonup-integration/analysis-code.md

**Instructions**

Files to Create

- /src/crosshook-native/src/hooks/useProtonUp.ts

Files to Modify

- /src/crosshook-native/src/hooks/useProtonInstalls.ts

Create a typed hook for listing available versions, triggering installs, and returning advisory status metadata. Integrate post-install refresh so installed runtime lists update reliably after successful operations, and ensure stale/offline cache metadata is propagated unchanged from command payloads to page consumers.

#### Task 3.2: Add Profile-Context Recommendation and Resolve Actions Depends on [3.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/components/pages/ProfilesPage.tsx
- /docs/plans/protonup-integration/research-ux.md
- /docs/plans/protonup-integration/research-business.md

**Instructions**

Files to Modify

- /src/crosshook-native/src/components/pages/ProfilesPage.tsx

Render advisory recommendation state near runtime selection and add explicit “resolve/install” actions. Preserve non-blocking behavior by keeping continue paths available when local runtime remains valid.

#### Task 3.3: Add Compatibility Page Install Controls and Degraded-State Messaging Depends on [3.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/components/pages/CompatibilityPage.tsx
- /docs/plans/protonup-integration/research-ux.md
- /docs/plans/protonup-integration/research-recommendations.md

**Instructions**

Files to Modify

- /src/crosshook-native/src/components/pages/CompatibilityPage.tsx

Add install controls, cache-age/stale indicators, and offline/dependency-missing messages that map directly to backend error categories. Keep copy explicit about source, destination, and integrity outcomes. Confirm this page reads cache freshness from hook payloads instead of recomputing client-side.

### Phase 4: Hardening, Tests, and Final Validation

#### Task 4.1: Add Core Tests for Matching, Cache, and Install Failure Paths Depends on [2.1, 2.2, 2.3]

**READ THESE BEFORE TASK**

- /src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs
- /src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs
- /docs/plans/protonup-integration/research-security.md

**Instructions**

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/protonup/matching.rs
- /src/crosshook-native/crates/crosshook-core/src/protonup/catalog.rs
- /src/crosshook-native/crates/crosshook-core/src/protonup/install.rs

Add focused unit tests for match status edge cases, cache stale/fresh behavior, and install error mapping. Verify checksum and path-validation guardrails are covered by tests before release.

#### Task 4.2: Verify End-to-End Contracts and Launch-Safety Rules Depends on [3.2, 3.3, 4.1]

**READ THESE BEFORE TASK**

- /docs/plans/protonup-integration/feature-spec.md
- /docs/plans/protonup-integration/shared.md
- /docs/plans/protonup-integration/research-security.md

**Instructions**

Files to Modify

- /src/crosshook-native/src/components/pages/ProfilesPage.tsx
- /src/crosshook-native/src/components/pages/CompatibilityPage.tsx

Run final contract and behavior validation focused on production behavior: command names, payload type parity, decision compliance, and advisory-vs-blocking semantics. Required pass criteria: each protonup command returns expected success/error envelopes, stale/offline catalog states render correctly in both UI entry points, install timeout/cancel/progress semantics are honored, and valid-local-runtime launches remain non-blocking. Patch any remaining UI state or messaging gaps that violate accepted launch-safety rules.

#### Task 4.3: Reconcile Spec and Plan Documentation After Verification Depends on [4.2]

**READ THESE BEFORE TASK**

- /docs/plans/protonup-integration/feature-spec.md
- /docs/plans/protonup-integration/shared.md
- /docs/plans/protonup-integration/parallel-plan.md

**Instructions**

Files to Modify

- /docs/plans/protonup-integration/feature-spec.md
- /docs/plans/protonup-integration/parallel-plan.md

Update planning docs only where implementation realities differ from the accepted decisions or phase assumptions. Preserve the confirmed decisions section and keep changes limited to concrete contract or sequencing corrections discovered during Task 4.2. Explicitly document whether `version_snapshots` remains deferred/optional in v1 to avoid ambiguity in future implementation work.

## Advice

- Provider adapter boundaries should be finalized before deep UI integration to avoid churn across commands/hooks.
- Keep install side effects behind explicit user actions; avoid automatic installs from recommendation state.
- Reuse `steam/proton.rs` for post-install refresh and avoid hidden drift between installed inventory implementations.
- Treat stale/offline catalog state as first-class UX signals, not generic errors.
- If command timeouts/cancellation are introduced, document and test them before enabling one-click install flows broadly.
