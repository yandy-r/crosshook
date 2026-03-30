# Documentation Research: Trainer-Version Correlation

**Researcher**: docs-researcher
**Date**: 2026-03-29
**Scope**: All documentation relevant to implementing trainer-version-correlation

---

## Overview

The trainer-version-correlation feature has an unusually complete documentation set already in place. Seven research documents plus a synthesized feature spec live in `docs/plans/trainer-version-correlation/`. All six required implementation docs (business, technical, security, practices, UX, external APIs) are present. The only genuine documentation gap is `research-technical.md`, which exceeds a single read window and must be read in segments. No external API documentation is required — this feature is entirely local filesystem I/O with zero new dependencies.

---

## Relevant Files

### Feature Specification (Start Here)

- `docs/plans/trainer-version-correlation/feature-spec.md` — **Master spec**: architecture diagram, SQLite schema for `version_snapshots` (migration 8→9), full API contract (4 Tauri commands), files to create/modify, phasing (4 phases), security requirements, resolved decisions, and research cross-references. Read this before any other file.

### Research Documents (All in `docs/plans/trainer-version-correlation/`)

- `research-technical.md` — Architecture decisions, SQLite schema alternatives, Tauri command shapes, system integration, TOCTOU analysis. **NOTE: File exceeds single read window; read in 2 segments with `offset` parameter.**
- `research-business.md` — Business rules BR-1 through BR-20, domain model (VersionSnapshot, VersionMismatchWarning), state machine, edge cases, and 6 key integration constraints (including: `steam.app_id` is NOT in the metadata `profiles` table — must be passed explicitly from Tauri command layer).
- `research-security.md` — 0 CRITICAL, 3 WARNING (W1-W3), 8 ADVISORY security findings. W1/W2/W3 are hard-stop prerequisites before shipping. W3 is an architectural constraint: community data must NEVER drive behavioral outcomes.
- `research-practices.md` — Table of 14 reusable code locations with exact file:line references. KISS assessment rejecting semver, version history tables, and filesystem watchers for v1. Module boundaries and testability patterns.
- `research-recommendations.md` — Phasing strategy (Phase 1+2 as MVP), alternative approaches A/B/C with effort estimates, risk table, full task breakdown with parallelization opportunities.
- `research-ux.md` — Three-layer warning system, competitive analysis (WeMod/Vortex/Heroic/ProtonDB), five version states, button labels, Steam Deck gamepad navigation requirements.
- `research-external.md` — Steam ACF manifest format (VDF key semantics, `buildid`/`StateFlags`/`LastUpdated`), trainer version source analysis (no standard format), Rust crate survey (notify, semver, pelite — all deferred for v1), code examples for manifest extension.

### Project Architecture Docs

- `CLAUDE.md` / `AGENTS.md` — Code conventions (snake_case Rust, PascalCase React, Tauri IPC patterns), workspace structure, build commands, commit message rules.
- `README.md` — High-level feature list; confirms health dashboard and Steam discovery are already shipped.
- `docs/features/steam-proton-trainer-launch.doc.md` — Launch method semantics, Steam manifest discovery search paths (including Flatpak), health dashboard integration, auto-populate mechanics.
- `docs/features/profile-duplication.doc.md` — Confirms profiles follow `profile_id` UUID semantics (not filename), and that SQLite metadata is separate from TOML — relevant for FK cascade behavior in `version_snapshots`.

### Supporting Context Docs

- `docs/research/additional-features/deep-research-report.md` — Original feature prioritization: 7/8 research perspectives recommended trainer-version correlation as the highest-value P1 feature.
- `tasks/todo.md` — Historical implementation log; shows patterns for how tasks are structured and verified in this repo.
- `tasks/lessons.md` — Accumulated repo lessons; relevant pattern: "In stateful diagnostic UI, do not map 'no fresh scan result yet' to an error-like state such as NotFound" — directly applicable to `version_untracked` vs. `version_mismatch` distinction (BR-4).

---

## Architectural Patterns

- **Tauri IPC shape**: `#[tauri::command]` functions accepting `State<'_>` for `ProfileStore`/`MetadataStore`, returning `Result<T, String>`. New commands go in `src-tauri/src/commands/version.rs`, registered in `src-tauri/src/lib.rs`. Pattern in: `commands/health.rs:60`.
- **SQLite metadata store pattern**: `upsert` / `load` / `lookup` triad following `metadata/health_store.rs` exactly. New module: `metadata/version_store.rs`.
- **Migration ladder**: Sequential `user_version` PRAGMA upgrades in `metadata/migrations.rs`. New function: `migrate_8_to_9()` adding `version_snapshots` table.
- **Health pipeline enrichment**: `BatchMetadataPrefetch` in `commands/health.rs` adds version fields; `ProfileHealthMetadata` in TypeScript extended rather than duplicated.
- **Pure function + I/O separation**: Mismatch logic extracted to `compute_correlation_status()` pure function (no I/O) for testability. Pattern: `resolve_launch_method()` in `profile/models.rs`.
- **Fail-soft DB access**: All new version DB calls wrapped in `metadata_store.is_available()` guard. DB failure must never block launch.
- **Community data trust boundary**: Community `game_version`/`trainer_version` are display-only — never control warnings or launch decisions. This is W3 from `research-security.md`.
- **Version check timing**: On-demand via startup reconciliation (`startup.rs`) and health dashboard — NOT in the synchronous launch path (Steam Deck SD card latency concern).

---

## Gotchas & Edge Cases

- **`steam.app_id` not in SQLite `profiles` table** — Only `game_name` and `launch_method` are promoted to metadata. The Tauri command layer must pass `steam.app_id` explicitly to `upsert_version_snapshot()` as a parameter (Option B from `research-business.md` open questions). Do not attempt a JOIN.
- **`version_untracked` ≠ `version_mismatch`** — Profiles with no baseline show `status = 'untracked'`. This is intentional and must NOT show a warning badge. Only `status = 'game_updated'` / `trainer_changed` / `both_changed` trigger warnings. See BR-4.
- **Multi-row history table** — `feature-spec.md` chose multi-row history (departed from `health_snapshots` single-row pattern). Mismatch detection queries: `WHERE profile_id = ? ORDER BY checked_at DESC LIMIT 1`. Requires per-profile row pruning on INSERT (A7 security advisory).
- **`StateFlags != 4`** — Steam auto-update in progress. Return `status: "unknown"` with `update_in_progress: true` instead of mismatch to avoid false alerts during updates. StateFlags 4 = fully installed, 1026 = update in progress.
- **Non-Steam profiles** — `native`/`proton_run` without `steam.app_id` → `status = 'untracked'` silently. Trainer SHA-256 hash still works for all launch methods but is NOT surfaced as a "version" to users.
- **`research-technical.md` file size** — This file exceeds the 10,000-token read limit. Must be read in two segments: lines 1-150 (architecture/schema), lines 150+ (integration/testing).
- **`parse_manifest_full()` vs. extending `parse_manifest()`** — Resolved decision: add `parse_manifest_full()` alongside existing function. Do NOT modify `parse_manifest()` signature (would break current callers). See feature-spec.md Decisions section.
- **Community `game_buildid` not in `CommunityProfileMetadata`** — The external researcher originally recommended adding it; the feature-spec resolved against it. Community version data stays as display-only free-text strings. The SQLite `steam_build_id` is local machine state only.
- **`pinned_commit` git injection gap (W2)** — Pre-existing bug in `community/taps.rs:checkout_pinned_commit()`. Must be fixed before shipping. Validate hex-only, 7-64 chars before passing to git subprocess.
- **`check_a6_bounds()` gap (W1)** — `game_version` and `trainer_version` are unbounded in `metadata/community_index.rs`. Must add `MAX_VERSION_BYTES = 256` check before version correlation reads from those columns.

---

## Required vs. Nice-to-Have Documents

### REQUIRED READING (Implementation-blocking)

| Document                                                       | Why Required                                                                                  |
| -------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `docs/plans/trainer-version-correlation/feature-spec.md`       | Master spec: exact files to create/modify, schema, API surface, decisions                     |
| `docs/plans/trainer-version-correlation/research-business.md`  | BR-1 through BR-20 business rules; domain model state machine; integration constraints        |
| `docs/plans/trainer-version-correlation/research-security.md`  | W1/W2/W3 are mandatory pre-ship fixes; A8 (DB failure must not block launch) is architectural |
| `docs/plans/trainer-version-correlation/research-practices.md` | Exact reuse targets (file:line), KISS scope limits, module boundaries                         |
| `CLAUDE.md` / `AGENTS.md`                                      | Code conventions, workspace structure, Tauri IPC patterns                                     |

### NICE-TO-HAVE (Context, Reference)

| Document                                                             | What It Provides                                                                      |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `docs/plans/trainer-version-correlation/research-technical.md`       | Deeper architecture alternatives and integration edge cases (read after feature-spec) |
| `docs/plans/trainer-version-correlation/research-ux.md`              | Three-layer warning design, button labels, Steam Deck gamepad requirements            |
| `docs/plans/trainer-version-correlation/research-recommendations.md` | Phasing strategy, risk table, parallelization opportunities                           |
| `docs/plans/trainer-version-correlation/research-external.md`        | Steam ACF format reference, VDF key semantics, code examples for manifest parsing     |
| `docs/features/steam-proton-trainer-launch.doc.md`                   | Steam discovery paths, Flatpak paths, health dashboard integration model              |
| `docs/features/profile-duplication.doc.md`                           | UUID profile identity, SQLite FK cascade behavior                                     |
| `tasks/lessons.md`                                                   | Repo-specific patterns; "untracked ≠ error" lesson directly applicable                |

### Team Research Output (Created During This Session)

- `docs/plans/trainer-version-correlation/research-architecture.md` — System overview, component map with exact file:line references for all touched files, full data flow diagram showing launch-success path → version store → startup scan → health enrichment path, and integration points.
- `docs/plans/trainer-version-correlation/research-integration.md` — Complete Tauri IPC endpoint table (existing + new commands), full `version_snapshots` SQL schema, existing table schemas (`profiles`, etc.) with FK relationships, and the four-command API surface with exact signatures.
- `docs/plans/trainer-version-correlation/research-patterns.md` — Exact file:line references for every reuse pattern, serde conventions (`nullable_text()` helper, `#[serde(rename_all = "snake_case")]`), `map_error()` Tauri error helper, and the `record_launch_finished()` hook location at `metadata/launch_history.rs:56-119`.

### Inline Code Documentation (Critical Comments)

Architecture-researcher surfaced these inline documentation points that aren't in any written doc:

- `metadata/community_index.rs:9` — comment "A6 string length bounds (advisory security finding)" marks the `check_a6_bounds()` function that needs the W1 extension (add `game_version`/`trainer_version` 256-byte bounds).
- `metadata/mod.rs:79-115` — `with_conn()` / `with_conn_mut()` docstrings explain the fail-soft `Arc<Mutex<Connection>>` pattern. This is the mandatory access path for all new version store operations (implements A8: DB failure must not block launch).
- `profile/models.rs` — `effective_profile()` and `storage_profile()` docstrings explain the local-override vs. portable export split. Version data must NOT appear in `storage_profile()` output — it is local machine state, not part of the portable profile contract.
- `metadata/launch_history.rs:56-119` — `record_launch_finished()` is the primary hook point for `upsert_version_snapshot()` on `LaunchOutcome::Succeeded` (BR-1). `LaunchOutcome` enum at `metadata/models.rs:103-121`.

---

## Documentation Gaps

1. **`research-technical.md` read constraint** — File exceeds single-read context limit. Not a gap per se, but implementers must read it in two segments. No workaround needed; `offset` + `limit` parameters handle this.
2. **No Tauri IPC reference doc** — There is no standalone Tauri command API doc. Implementers must read `commands/health.rs` and `commands/launch.rs` directly as the authoritative patterns.
3. **No frontend hook architecture doc** — No written guide to the `useReducer`+`useCallback`+`AbortController` hook pattern. Read `src/hooks/useProfileHealth.ts:116` directly.
4. **No test pattern doc** — No written testing guide. Patterns: `steam/manifest.rs:219-346` for manifest unit tests; `metadata/db.rs:open_in_memory()` for SQLite tests.
5. **`version_snapshots` status values not documented as an enum** — The six status strings (`'untracked'`, `'matched'`, `'game_updated'`, `'trainer_changed'`, `'both_changed'`, `'unknown'`) are defined in `feature-spec.md` but have no corresponding type documentation analogous to `DriftState` or `LaunchOutcome` in `metadata/models.rs`. Implementers should add a `VersionCorrelationStatus` enum with `as_str()`/`impl FromStr` alongside the new store (patterns-researcher confirmed this pattern).
6. **`health_snapshots` vs. `version_snapshots` update semantics not called out** — `health_snapshots` uses single-row upsert (`INSERT OR REPLACE`); `version_snapshots` uses multi-row insert with per-profile pruning. These look structurally similar but behave differently. Any architecture doc or code comment should call this out explicitly to prevent copy-paste errors from `health_store.rs`.
7. **No community schema version doc** — `CommunityProfileMetadata` schema versioning is implicit. If Phase 4 adds `game_buildid`, that requires a schema version bump not currently documented.
8. **Quickstart not updated** — `docs/getting-started/quickstart.md` does not mention version correlation (expected — feature not shipped yet). Will need updating after Phase 2 ships.

---

## External Documentation

- [Steam ACF Format Overview](https://github.com/leovp/steamfiles/blob/master/docs/acf_overview.rst) — VDF key-value format, `buildid` semantics
- [Valve KeyValues format](https://developer.valvesoftware.com/wiki/KeyValues) — Official VDF spec
- [notify crate docs](https://docs.rs/notify/latest/notify/) — Filesystem watching (v2 consideration only)
- [WeMod Version Guard (Medium)](https://medium.com/wemod/version-guard-781d5e152a13) — Competitive pattern; "Launch Anyway" UX lesson
