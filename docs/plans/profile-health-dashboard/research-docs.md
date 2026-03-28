# Documentation Research: Profile Health Dashboard (v2)

**Date**: 2026-03-28
**Scope**: Documentation catalog for the profile-health-dashboard feature — second pass accounting for the SQLite3 metadata layer (PRs 89-91).

---

## Overview

This report catalogs all documentation and configuration files relevant to implementing the profile health dashboard. The feature spec has been revised to v2 to integrate the SQLite `MetadataStore` (PRs 89-91). Seven research files have been updated in a second pass and are the authoritative reference for implementation. The key architectural shift in v2 is a two-layer design: `profile/health.rs` (pure filesystem, no MetadataStore dependency) composed with `commands/health.rs` (enrichment + fail-soft MetadataStore queries). Zero new Rust dependencies are required.

---

## Architecture Docs

### Feature Planning Documents

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-health-dashboard/feature-spec.md` — **REQUIRED READING** (v2). Primary spec: business rules, tri-state classification, two-layer architecture, data models (Rust structs + TypeScript interfaces), API contracts, edge cases, success criteria, phase breakdown (A → D). 47 KB — read in full before any implementation.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-health-dashboard/research-technical.md` — **REQUIRED READING** (v2). Validation logic pseudocode, `check_file_path`/`check_directory_path`/`check_executable_path` helper patterns, `health_snapshots` SQLite schema (migration v6), `health_store.rs` implementation, `batch_validate()` implementation, `derive_status()` logic, and detailed `commands/health.rs` structure.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-health-dashboard/research-practices.md` — **REQUIRED READING** (v2). Reusable code inventory with exact file:line references, module boundary rationale (why `health.rs` belongs in `profile/`, not a top-level module), KISS assessment, testability patterns using `tempfile::tempdir()` + `MetadataStore::open_in_memory()`, interface design.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-health-dashboard/research-security.md` — **REQUIRED READING** (v2). Security findings: 0 critical, 3 warnings (W-1 CSP at `tauri.conf.json:23`, W-2 path sanitization, W-3 diagnostic bundle), 4 new SQLite-specific findings (N-1 through N-4). Secure coding guidelines (12 rules). Cross-reference table against SQLite3 spec findings W1-W8.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-health-dashboard/research-recommendations.md` — **REQUIRED READING** (v2). Synthesized architectural decisions from all 6 research streams. Key decision table, risk register, critical correctness requirements (`derive_steam_client_install_path()` move), open questions.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-health-dashboard/parallel-plan.md` — **REQUIRED READING**. Concrete implementation plan with specific file paths and task ordering. Phase S (security pre-ship), Phase A (core), Phase B (polish), Phase C (startup), Phase D (persistence). The critical path: S2 → A5 → A7 → A9.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-health-dashboard/analysis-tasks.md` — IMPORTANT. Phase/task breakdown with dependency graph, blocking relationships, and parallelization strategy.

### SQLite3 Addition Planning Documents (Cross-Referenced)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/sqlite3-addition/feature-spec.md` — **REQUIRED READING** for MetadataStore integration. The SQLite3 layer spec: schema for `profiles`, `launchers`, `launch_operations`, `community_taps`, `collections`; security findings W1-W8, A1-A6; `MetadataStore` public API; fail-soft behavior rules.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/sqlite3-addition/research-security.md` — Security findings for the SQLite layer (W1-W8, A1-A6) with direct applicability to health dashboard. W2 (path sanitization), W4 (SQL injection via `format!()`), W6 (SQLite-sourced paths in fs ops) are the most relevant.

### Other Health Dashboard Research Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-health-dashboard/research-business.md` — Business analysis (v2): user stories US-1 through US-11, health vs. launch validation boundary table, method-aware validation matrix, notification rules, remediation text guidance.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-health-dashboard/research-ux.md` — UX research (v2): component hierarchy, metadata-enriched workflows, badge overlays (`↑3x` failure chip, `✦` drift badge), gamepad navigation requirements, community-import context note, "unconfigured" profile soft-rendering.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-health-dashboard/research-external.md` — External API/library research (v2): Tokio API reference for `spawn_blocking`, phase boundary summary (Phase A/B uses zero new MetadataStore methods; Phase D adds migration v6), §6 Phase D forward spec for `health_snapshots` persistence.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-health-dashboard/shared.md` — Shared context used across research agents (feature description, codebase context, teammate roles).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-health-dashboard/analysis-code.md` — Code analysis from first pass (pre-SQLite); superseded by v2 research files but still useful for understanding the `validate_all()` reuse rationale.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-health-dashboard/analysis-context.md` — Context analysis from first pass; superseded by v2.

---

## API Docs

### Core Rust Files Referenced by Research

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/request.rs` — `validate_all()` (line ~444), `ValidationError` enum + `.issue()` + `.help()`, private path helpers `require_directory()` (line ~700), `require_executable_file()` (line ~721), `is_executable_file()` (line ~742), `ValidationSeverity` (line ~143), `LaunchValidationIssue` (line ~151). These must be promoted to `pub(crate)`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — `GameProfile` struct with all path fields (`game.executable_path`, `trainer.path`, `steam.compatdata_path`, `steam.proton_path`, `runtime.prefix_path`, `runtime.proton_path`); `resolve_launch_method()` for method-aware dispatch.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs` — `ProfileStore::list()` (line ~136), `ProfileStore::load()` (line ~100), `ProfileStore::with_base_path()` (line ~96) — the test-injection constructor.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — `MetadataStore` implementation: `open_in_memory()` (line 44), `disabled()` (line 48), `with_conn()` fail-soft wrapper (line ~67), `query_last_success_per_profile()` (line 401), `query_failure_trends(days)` (line 437). These are the only MetadataStore methods called in Phase A/B — no new methods needed.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` — `DriftState` enum (line ~122): `Unknown/Aligned/Missing/Moved/Stale`; `FailureTrendRow` struct (line ~278): `{profile_name, successes, failures, failure_modes}`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/shared.rs` — `sanitize_display_path()` (line 20) — already moved here and imported from `launch.rs`. The health command must use this for all path strings in IPC responses.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs` — `sanitize_diagnostic_report()` (line ~372), `sanitize_display_path` import pattern (line 21). Reference for how path sanitization is applied before IPC.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs` — Existing pattern for commands that accept both `State<'_, ProfileStore>` and `State<'_, MetadataStore>`. Health commands follow this exact pattern.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs` — `LauncherInfo.is_stale` (line ~42) — the staleness pattern to mirror for `ProfileHealthInfo`.

### TypeScript / Frontend Files Referenced

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useLaunchState.ts` — `useReducer` + typed actions state machine pattern (line ~46) — the template for `useProfileHealth`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/launch.ts` — `LaunchFeedback` discriminated union (line ~43), `LaunchValidationSeverity` — pattern for health type design.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/CompatibilityViewer.tsx` — `crosshook-status-chip crosshook-compatibility-badge--{rating}` CSS pattern (line ~76) — reuse for `HealthBadge`.

---

## Development Guides

### Project-Level Configuration

- `/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md` — Project conventions: Rust naming (`snake_case`), TypeScript strict mode, Tauri IPC pattern, commit message rules (`git-cliff` changelog generation), build commands.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/tauri.conf.json` — **Security action required** (W-1): `"csp": null` at line 23. Must change to `"default-src 'self'; script-src 'self'"` as Phase S.1. Also shows `devUrl: "http://localhost:5173"` which may need `'unsafe-eval'` in dev CSP.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/capabilities/default.json` — Tauri capabilities: `core:default` + `dialog:default` only. No new capabilities required for the health feature (`std::fs::metadata()` does not need a Tauri plugin capability).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/Cargo.toml` — Workspace root (3 members: `crosshook-core`, `crosshook-cli`, `src-tauri`).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/Cargo.toml` — Core crate dependencies. Confirms: `rusqlite = { version = "0.38", features = ["bundled"] }`, `uuid = { version = "1", features = ["v4", "serde"] }`, `chrono = "0.4"`, `tempfile = "3"` (dev-dep), `tokio = { version = "1", features = ["fs", "process", "rt", "sync"] }`. Zero new dependencies needed for health feature.

### External Documentation (Live Links from Research)

- [Tauri v2 State Management](https://v2.tauri.app/develop/state-management/) — `app.manage(Mutex<T>)` pattern for optional health cache
- [Tauri v2 Calling Frontend](https://v2.tauri.app/develop/calling-frontend/) — `AppHandle::emit()` for startup background health scan event
- [SQLite WAL](https://www.sqlite.org/wal.html) — WAL journaling mode used by `metadata.db`; relevant for understanding the existing DB setup
- [rusqlite docs](https://docs.rs/rusqlite/latest/rusqlite/) — Query API reference for any new `MetadataStore` methods in Phase D

---

## Must-Read Documents

Ordered by priority for implementation start:

| Priority | Document                                                          | Why Required                                                                                          |
| -------- | ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| 1        | `docs/plans/profile-health-dashboard/feature-spec.md`             | Complete v2 spec — data models, business rules, two-layer architecture, phase structure               |
| 2        | `docs/plans/profile-health-dashboard/parallel-plan.md`            | Exact implementation task list with file paths, blocking dependencies, critical path                  |
| 3        | `docs/plans/profile-health-dashboard/research-technical.md`       | Validation logic pseudocode, SQLite schema (migration v6), `health_store.rs` implementation           |
| 4        | `docs/plans/profile-health-dashboard/research-practices.md`       | Reusable code inventory (exact file:line references), module boundary rationale, testability patterns |
| 5        | `docs/plans/profile-health-dashboard/research-security.md`        | 3 warnings + 4 new SQLite findings that affect implementation decisions                               |
| 6        | `docs/plans/sqlite3-addition/feature-spec.md`                     | MetadataStore API, fail-soft rules, SQLite security findings W1-W8                                    |
| 7        | `docs/plans/profile-health-dashboard/research-recommendations.md` | Synthesized architectural decisions, risk register, open questions                                    |
| 8        | `docs/plans/profile-health-dashboard/research-business.md`        | User stories, notification rules, method-aware validation matrix                                      |
| 9        | `CLAUDE.md`                                                       | Project conventions (commit style, IPC naming, type strictness)                                       |
| 10       | `docs/plans/profile-health-dashboard/research-ux.md`              | Badge overlays, gamepad navigation, metadata-enriched UI workflows                                    |

---

## Documentation Gaps

1. **`lib.rs` startup event pattern not explicitly documented** — `parallel-plan.md` references `lib.rs` lines ~46-56 for the `auto-load-profile` emit pattern that the startup health scan should mirror. No dedicated doc covers this. Read `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` directly before implementing Phase C.1.

2. **`commands/health.rs` command naming not finalized** — `research-security.md` uses `profile_health_check_all` and `profile_health_check(name)` while `feature-spec.md` uses `batch_validate_profiles` and `get_profile_health`. The `parallel-plan.md` uses `batch_validate_profiles` / `get_profile_health`. Treat `parallel-plan.md` naming as authoritative for Tauri command names (it is the most recent task-oriented document).

3. **`derive_steam_client_install_path()` relocation not yet implemented** — `research-recommendations.md` identifies this as a critical correctness requirement: the function must move from `src-tauri/src/commands/profile.rs` into `crosshook-core` before `check_profile_health()` can handle `steam_applaunch` profiles. No doc describes the move in detail — read the current implementation before starting A2.

4. **`ProfilesPage.tsx` path** — `parallel-plan.md` references `src/components/pages/ProfilesPage.tsx` while `CLAUDE.md` architecture shows `src/components/` without a `pages/` subdirectory. Verify the actual path before starting A9 integration.

5. **Phase D `health_snapshots` migration version** — `research-technical.md` specifies migration `5 to 6` for `health_snapshots`. Verify the current `user_version` in `migrations.rs` before implementing Phase D to confirm it is still v5 and no other migration has been added since PRs 89-91.
