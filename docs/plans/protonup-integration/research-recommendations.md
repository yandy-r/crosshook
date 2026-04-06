## Executive Summary

Integrating ProtonUp into CrossHook is feasible and strategically valuable, but should be shipped as a staged capability rather than a single all-at-once feature. The core recommendation is to deliver user trust first (visibility + suggestions), then controlled installs, then deeper automation. This sequence minimizes risk to profile launch reliability, keeps `crosshook-core` as the source of business logic, and avoids introducing fragile behavior into the Tauri layer.

The storage boundary proposed in issue #70 is directionally correct and should be retained with minor refinements:

- Keep available-version catalogs in SQLite `external_cache_entries` with explicit TTL and age metadata surfaced to UI.
- Keep installed Proton enumeration runtime-only from filesystem scans (no DB persistence).
- Keep user preference defaults in `settings.toml`.
- Keep install progress/runtime status ephemeral and stream-oriented (no persistence).

No-go trigger at feature level: if integration introduces launch blocking, long-running UI freezes, or non-recoverable install failure states, rollout should pause until corrected.

### Recommended Implementation Strategy

Use a core-first, adapter-based architecture:

- Implement all ProtonUp orchestration in `crosshook-core` behind a small service boundary (for example `protonup` module with `list_available`, `list_installed`, `install`, `recommend_for_profile`).
- Keep Tauri commands thin wrappers around `crosshook-core` functions with `snake_case` IPC names and Serde-safe DTOs.
- Treat external tool execution (`protonup-rs`/compatible CLI) as a replaceable adapter with strict timeout, structured error mapping, and cancellation support.

Recommended capability sequence:

- Capability 1: Discovery and recommendation read-paths (no install side effects).
- Capability 2: Guided install action for explicit user-selected version.
- Capability 3: Smart recommendation ranking (community requirement + local state + user default preference).

Operational design details:

- Cache key convention: `protonup:available_versions:{provider}:{channel}` with TTL and `fetched_at`.
- Stale fallback: serve cached catalog when offline/fetch fails, and label as stale in UI.
- Installed version detection: enumerate Steam `compatibilitytools.d` locations each request; avoid background daemons in initial rollout.
- Install execution: spawn subprocess via Rust with streaming stdout/stderr parsing into typed progress events; never block launch workflows.

Go/No-Go criteria for this strategy:

- Go when ProtonUp integration is isolated from launch path failures and all external command errors map to actionable user-facing states.
- No-go if recommendations require network call at render time (must be cache-backed/on-demand) or if install logic is tightly coupled to frontend state.

## Phased Rollout Suggestion

Phase 0 - Technical Spike and Contract Freeze

- Scope: validate ProtonUp command compatibility, output formats, install target paths, and error taxonomy across supported Linux environments.
- Deliverables: command contract doc, DTO schema, and failure matrix.
- Go criteria:
  - At least one supported ProtonUp execution path validated end-to-end on Linux desktop.
  - Deterministic parser for list/install outputs with test fixtures.
  - Confirmed non-blocking behavior under timeout/cancel scenarios.
- No-go criteria:
  - Output format instability without robust parsing fallback.
  - Install destination ambiguity that can break Steam tool discovery.

Phase 1 - Read-Only Visibility + Recommendations

- Scope: list installed versions, fetch/cache available versions, and show recommendation badges based on community profile requirements and local availability.
- Deliverables: core APIs + Tauri commands + UI indicators.
- Go criteria:
  - UI remains functional offline, showing installed versions and stale cache where applicable.
  - Recommendation output explains "why" (required version, installed status, suggested action).
  - Zero regressions in launch flow and profile loading.
- No-go criteria:
  - Any launch path dependency on live network fetch.
  - Recommendation ambiguity causing users to install incorrect major compatibility lines.

Phase 2 - Explicit User-Triggered Install

- Scope: allow user to install a selected version; provide progress, cancellation, and post-install verification.
- Deliverables: install command pipeline, progress events, and success/failure reconciliation.
- Go criteria:
  - Install is cancel-safe and leaves app in recoverable state.
  - Post-install filesystem verification confirms version is discoverable.
  - Error handling distinguishes permission/network/tool-missing failures.
- No-go criteria:
  - Partial installs reported as success.
  - UI thread stalls or command deadlocks under long-running installs.

Phase 3 - Recommendation Quality + UX Hardening

- Scope: smarter ranking and preference-aware defaults, better fallback messaging, optional one-click install from recommendation.
- Deliverables: ranking model, telemetry-lite counters (local only), UX refinements.
- Go criteria:
  - Recommendation acceptance rate improves in manual validation sessions.
  - No increase in failed launch incidents linked to Proton selection changes.
- No-go criteria:
  - User confusion from over-automation or silent preference overrides.

## Quick Wins

- Ship read-only installed-version enumeration first to provide immediate user value without external dependency risk.
- Add "tool missing" diagnostics early with concrete install guidance to reduce support burden.
- Reuse existing cache infrastructure (`external_cache_entries`) for available-version catalogs, with explicit stale-age UI badge.
- Add a simple recommendation rule: "required by profile and already installed" > "required by profile and available" > "user default."
- Provide manual refresh action for available versions instead of aggressive auto-refresh polling.

## Future Enhancements

- Multi-provider support abstraction (GE-Proton, Wine-GE, future custom channels) without changing IPC contract.
- Background prefetch job with jitter/backoff when app is idle (opt-in setting).
- Integrity verification (checksums/signatures) before marking install as successful.
- Optional batch install workflow for users importing multiple community profiles.
- Advanced recommendation signals: distro compatibility hints, historical success per game profile (local metadata only).

### Risk Mitigations

Linux desktop + filesystem variability:

- Mitigate by probing known Steam compatibility paths and handling per-user permission differences explicitly.
- Never assume a single Steam root; treat path discovery as layered and fail-soft.

Tauri IPC and UI responsiveness:

- Keep long-running install work in Rust async/subprocess layer with streamed events; UI consumes progress only.
- Enforce command timeouts and cancellation tokens to prevent hanging IPC calls.

Rust process orchestration reliability:

- Use strongly typed command result enums (`Success`, `RecoverableFailure`, `FatalFailure`, `Cancelled`) to avoid stringly-typed error branching.
- Validate all external command inputs; avoid shell interpolation patterns; pass args as structured vectors.

Cache correctness and offline behavior:

- Enforce TTL + fetched timestamp and display cache age to avoid stale-data surprises.
- If network fetch fails, return cached results plus stale flag; never block launch or profile edit workflows.

Product trust and safety:

- Keep install action explicit (no silent auto-installs in initial releases).
- Surface clear post-install verification status before suggesting profile relaunch.

## Decision Checklist

Product decisions required:

- Confirm whether recommendation UX should prioritize "minimum compatible" vs "latest compatible" when both satisfy profile requirements.
- Decide if one-click install from recommendation card is in-scope for initial install phase.
- Define acceptable stale-cache age threshold messaging (for example "older than 7 days").
- Confirm whether "tool missing" should show distro-specific instructions or generic guidance only.

Engineering decisions required:

- Select primary ProtonUp invocation path and fallback behavior when multiple binaries/paths exist.
- Finalize cache TTL and refresh policy for available-version catalog.
- Decide install progress protocol shape for IPC events (granular logs vs coarse state transitions).
- Confirm command timeout defaults and cancellation semantics for long installs.
- Define post-install verification contract (directory existence only vs deeper metadata validation).

Release-governance decisions:

- Decide minimum supported Linux environments for Phase 2 install support.
- Confirm telemetry boundaries (if any counters are added, keep local/offline and non-identifying).
- Approve rollback plan: disable install actions while preserving read-only recommendations if instability appears.
