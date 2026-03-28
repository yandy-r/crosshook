# Documentation Research: sqlite3-addition

## Overview

This document catalogs all documentation, configuration, and code references relevant to implementing the SQLite metadata layer in CrossHook. The feature adds `rusqlite` 0.39.0 (bundled SQLite 3.51.3) as a secondary local store in `crosshook-core`, keeping TOML profiles canonical and adding durable identity, relationship, history, and cache tables. All research documents in `docs/plans/sqlite3-addition/` have been reviewed and their key content is summarized below.

---

## Feature Research Documents

All eight files in `docs/plans/sqlite3-addition/` are complete and cross-referenced.

| File                          | What It Covers                                                                                                                                                                                                                                                                                                                            |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `feature-spec.md`             | **Master spec** — executive summary, business rules, Phase 1/2/3 schemas, success criteria, edge case table, architecture diagram. Start here.                                                                                                                                                                                            |
| `research-architecture.md`    | **Codebase architecture analysis** — verified file-level inventory of every Rust file relevant to the implementation, with per-file notes on what changes and what stays untouched.                                                                                                                                                       |
| `research-integration.md`     | **IPC contract and type mapping** — all 30 Tauri commands with exact Rust signatures (`invoke_handler` in `lib.rs:70-113`), verified type-to-table mapping, filesystem paths confirmed, security constraints.                                                                                                                             |
| `research-technical.md`       | **Implementation blueprint** — verified module inventory, full 14-table schema with column-level detail, type-to-table mapping, integration points verified against actual code, file-level impact (create/modify lists), open decisions with resolutions, sync/async architecture, AppImage constraints, Tauri state management pattern. |
| `research-recommendations.md` | **Phased rollout and technology decisions** — Phase 1/2/3 task breakdown with time estimates, technology choice table (rusqlite vs. sqlx vs. diesel), reusable codebase patterns table, required new shared utilities (7 items, priority ordered), business rule resolutions (RF-1 through RF-5).                                         |
| `research-practices.md`       | **Existing patterns and KISS assessment** — reusable code table with exact file:line references, module boundary design, KISS table (what to cut from Phase 1), build-vs-depend table, public API surface design, testability patterns (in-memory SQLite for unit tests, tempfile for integration).                                       |
| `research-external.md`        | **SQLite and rusqlite API reference** — rusqlite 0.39.0 feature flags (all 46), PRAGMA reference table with URLs, WAL behavior and sidecar files, UPSERT syntax, FTS5, JSONB, migration pattern with `rusqlite_migration`, connection management, constraints and gotchas list.                                                           |
| `research-security.md`        | **Security findings** — W1–W8 (must address) and A1–A6 (advisory), data sensitivity table per SQLite table, SQL injection pattern, file permissions requirement (`0600`), dependency CVE table (CVE-2025-3277, CVE-2025-6965 etc.), supply chain posture.                                                                                 |
| `research-business.md`        | **Business rules and workflows** — user stories, 16 business rules with edge cases, full workflow sequences (profile create/rename/duplicate/delete, launcher export, drift detection, community sync), domain model entities, state transition diagrams, risk factor resolutions.                                                        |
| `research-ux.md`              | **UX design** — competitive analysis (Steam, Heroic, Lutris, Playnite), interaction patterns for rename/drift/history/community flows, gamepad navigation requirements, status chip taxonomy, accessibility checklist, 9 UX risk mitigations.                                                                                             |

---

## Project Guidelines

### CLAUDE.md (root)

`/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md`

Key conventions directly affecting this implementation:

- **Error handling**: throw errors early; use `Result<T, E>`; no silent fallbacks
- **Type safety**: no `any`; derive proper types from existing code
- **Rust conventions**: `snake_case`, `mod.rs` directories, `anyhow` or custom error enums, `#[derive(Serialize, Deserialize)]` for IPC-crossing types
- **Commit messages**: conventional commits required; `feat(metadata): ...`, `fix(metadata): ...`, `chore(release): ...` for non-user-facing work; titles appear in CHANGELOG via `git-cliff`
- **Testing**: run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` — the only automated test suite
- **Build commands**: `./scripts/build-native.sh` for AppImage, `./scripts/build-native.sh --binary-only` for quick binary check

### Global CLAUDE.md

`/home/yandy/.claude/CLAUDE.md`

- Plan mode for non-trivial tasks
- Subagents liberally; keep main context clean
- Never mark task complete without proving it works
- Fail-fast: catch issues at development time

---

## Configuration Files

### Rust Workspace

`src/crosshook-native/Cargo.toml`

- Members: `crates/crosshook-core`, `crates/crosshook-cli`, `src-tauri`
- `resolver = "2"`; workspace version `0.2.3`

### crosshook-core Cargo.toml

`src/crosshook-native/crates/crosshook-core/Cargo.toml`

Current dependencies (no SQLite yet):

```toml
chrono = "0.4"
directories = "5"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
tokio = { version = "1", features = ["fs", "process", "rt", "sync"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt", "time"] }

[dev-dependencies]
tempfile = "3"
```

**What to add for Phase 1:**

```toml
rusqlite = { version = "0.39", features = ["bundled"] }  # bundled required: system SQLite on SteamOS may be pre-3.51.3 (WAL race + CVEs)
uuid     = { version = "1",    features = ["v4", "serde"] }
```

### Tauri Configuration

`src/crosshook-native/src-tauri/tauri.conf.json`

- Target: `appimage` only (no .deb, no .rpm)
- Window: 1280×800, dark theme, single `main` window
- Resources include `../runtime-helpers/*.sh`
- CSP: null (disabled)

### Pull Request Template

`.github/pull_request_template.md`

PR checklist includes:

- `./scripts/build-native.sh --binary-only` must pass
- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` must pass
- Conditional checks for `src/launch/`, `src/profile/`, `src/steam/`, `src/components/`

**The new `metadata` module will need a conditional check added to the PR template.**

### GitHub Workflows

`.github/workflows/release.yml` — builds and publishes AppImage on tag push; validates `CHANGELOG.md` section before publishing

### Issue Templates

`.github/ISSUE_TEMPLATE/` — YAML form templates for bug, feature, compatibility reports; blank issues disabled

---

## Code Documentation

### Module Root

`src/crosshook-native/crates/crosshook-core/src/lib.rs`

Current exports (new `metadata` module to be added as peer):

```rust
pub mod community;   // taps.rs, index.rs
pub mod export;      // launcher.rs, launcher_store.rs
pub mod install;     // discovery.rs, models.rs, service.rs
pub mod launch;      // request.rs, script_runner.rs, diagnostics/, ...
pub mod logging;     // structured logging via tracing
pub mod profile;     // models.rs, toml_store.rs, community_schema.rs, exchange.rs, legacy.rs
pub mod settings;    // mod.rs (SettingsStore), recent.rs (RecentFilesStore)
pub mod steam;       // discovery, libraries, manifest, proton, vdf, auto_populate, diagnostics, models
pub mod update;      // models.rs, service.rs
```

### Key Source Files for Pattern Reference

| File                                                  | Why It Matters                                                                                                                                                            |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-core/src/profile/toml_store.rs`        | **Primary pattern template** — `ProfileStore::try_new()`, `with_base_path()`, `validate_name()` (line ~300), error enum, `rename()`/`delete()`/`duplicate()` return types |
| `src/crosshook-core/src/settings/mod.rs`              | `SettingsStore::try_new()` pattern; `BaseDirs::config_dir()` usage                                                                                                        |
| `src/crosshook-core/src/settings/recent.rs`           | `RecentFilesStore::try_new()` using `BaseDirs::data_local_dir()` — **confirms `metadata.db` belongs here too**                                                            |
| `src/crosshook-core/src/logging.rs`                   | `Arc<Mutex<RotatingLogState>>` pattern — **the `MetadataStore` connection model**                                                                                         |
| `src/crosshook-core/src/community/taps.rs`            | `CommunityTapStore`, `CommunityTapSubscription`, `CommunityTapSyncResult` with `head_commit`; custom error enum with `Io { action, path, source }` variant                |
| `src/crosshook-core/src/export/launcher_store.rs`     | `LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult`, `sanitize_launcher_slug()`, `derive_launcher_paths()` — all map to the `launchers` table                  |
| `src/crosshook-core/src/launch/request.rs`            | `LaunchRequest` struct — **missing `profile_name` field** (must be added before Phase 2 `record_launch_started()`)                                                        |
| `src/crosshook-core/src/launch/diagnostics/models.rs` | `DiagnosticReport`, `ExitCodeInfo`, `PatternMatch`, `ActionableSuggestion`, `FailureMode` — all map to `launch_diagnostics` table                                         |
| `src-tauri/src/lib.rs`                                | Tauri state `.manage()` setup; where `MetadataStore` will be added                                                                                                        |
| `src-tauri/src/commands/profile.rs`                   | `profile_rename`, `profile_delete`, `profile_duplicate` — where metadata sync hooks go                                                                                    |
| `src-tauri/src/commands/launch.rs`                    | `launch_game`, `launch_trainer`, `stream_log_lines`, `sanitize_display_path()` (private, line ~301) — **must be promoted to `commands/shared.rs`**                        |
| `src-tauri/src/commands/export.rs`                    | `export_launchers`, `check_launcher_exists`, `delete_launcher` — where launcher metadata sync goes                                                                        |
| `src-tauri/src/commands/community.rs`                 | `community_sync` — where `sync_tap_index()` call goes after `sync_many()`                                                                                                 |
| `src-tauri/src/startup.rs`                            | Auto-load profile on startup; where bootstrap `sync_profiles_from_store()` goes                                                                                           |

---

## Must-Read Documents (Prioritized)

For someone implementing the SQLite metadata layer, read in this order:

1. **`docs/plans/sqlite3-addition/feature-spec.md`** — Start here. Master spec with authority matrix, Phase 1 schema tables, business rules, edge cases, success criteria.

2. **`docs/plans/sqlite3-addition/research-technical.md`** — Implementation blueprint. All integration points verified against actual code. Files to create/modify with exact paths. Open decisions resolved.

3. **`docs/plans/sqlite3-addition/research-recommendations.md`** — Technology decisions (rusqlite vs alternatives), phased rollout with time estimates, required new utilities (priority-ordered), business rule resolutions (RF-1–RF-5).

4. **`CLAUDE.md`** (repo root) — Project conventions (commit messages, build commands, Rust style, test commands).

5. **`docs/plans/sqlite3-addition/research-practices.md`** — Existing reusable patterns with file:line references. KISS table. Module boundary design. Public API surface for `MetadataStore`. Testability patterns.

6. **`docs/plans/sqlite3-addition/research-security.md`** — W1–W8 security findings (file permissions, path sanitization, payload limits, SQL injection, symlink check). CVE table for SQLite dependency.

7. **`docs/plans/sqlite3-addition/research-architecture.md`** — File-level inventory of every Rust file touched by the implementation. Cross-references which files are modified vs. left untouched.

8. **`docs/plans/sqlite3-addition/research-integration.md`** — Complete IPC contract: all 30 Tauri commands with Rust signatures. Verified type-to-SQLite-table mappings. All filesystem paths confirmed. Key security constraints summarized.

9. **`src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`** — Read before writing any `MetadataStore` code. The `Store` pattern to follow exactly.

10. **`docs/plans/sqlite3-addition/research-external.md`** — rusqlite API reference, PRAGMA guide, connection setup patterns, UPSERT syntax, WAL gotchas.

11. **`docs/plans/sqlite3-addition/research-business.md`** — Business rules and workflow sequences. Covers all cascade behaviors.

12. **`docs/plans/sqlite3-addition/research-ux.md`** — UX requirements for drift display, collections, history panels, gamepad navigation, error message taxonomy.

---

## Documentation Gaps

The following are absent or incomplete and could affect implementation:

| Gap                                                                            | Impact               | Notes                                                                                                                                                                                                            |
| ------------------------------------------------------------------------------ | -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No `src-tauri/src/commands/mod.rs` documentation                               | Medium               | New `pub mod metadata;` must be added; no doc comments exist for the commands module yet                                                                                                                         |
| New metadata commands must be registered in `invoke_handler` (`lib.rs:70-113`) | **Hard requirement** | `#[tauri::command]` annotation alone is not sufficient — a command is not callable from the frontend unless explicitly listed in `invoke_handler`. See `research-integration.md` for the full registration list. |
| No documented test pattern for async Tauri commands                            | Medium               | `launch_game`/`launch_trainer` are async; `spawn_blocking` bridge for rusqlite writes has no existing example in the codebase                                                                                    |
| `LaunchRequest` missing `profile_name` field                                   | **Phase 2 blocker**  | Identified in research-practices and research-recommendations; must be resolved before `record_launch_started()` can link launch events to profile IDs                                                           |
| `sanitize_display_path()` is private in `commands/launch.rs`                   | **Phase 1 blocker**  | Must be promoted to `commands/shared.rs` before any metadata IPC commands return path strings                                                                                                                    |
| No migration rollback strategy documented                                      | Low                  | Hand-rolled migrations with `PRAGMA user_version` have no down-migration support; accept this limitation for Phase 1                                                                                             |
| No Dependabot or CI check for rusqlite version                                 | Low                  | Security research (A1) recommends tracking; not yet configured                                                                                                                                                   |
| PR template missing metadata module checklist item                             | Low                  | Should be added when metadata module lands                                                                                                                                                                       |
