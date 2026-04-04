# Context Analysis: protontricks-integration

## Executive Summary

This feature adds WINE prefix dependency management (vcrun2019, dotnet48, etc.) to CrossHook by integrating winetricks as the primary tool and protontricks as an optional secondary. The implementation is a new `prefix_deps` module in `crosshook-core` with 4 files, a SQLite migration v14→v15, TOML schema extensions on existing structs, 4 new Tauri IPC commands, and a `PrefixDepsPanel` React component — all following established in-tree patterns without new crates.

---

## Pre-Implementation Decisions (Resolve Before Phase 1)

These 4 open decisions from the feature spec must be resolved before any code is written:

1. **Field name**: Keep `required_protontricks` (recommended — community familiarity; winetricks verbs are the canonical naming regardless of tool)
2. **Verb allowlist approach**: Structural regex `^[a-z0-9][a-z0-9_\-]{0,63}$` as hard gate + static known-verb set for advisory — resolve before Task 3.1
3. **`user_skipped` reset**: Per-package action (recommended) — resolve before Task 5b.3
4. **TTL value**: 24 hours (recommended, matches health check staleness model) — resolve before Task 2.2 (store staleness logic depends on this)

---

## Architecture Context

- **System Structure**: Three-layer architecture — `crosshook-core` (all business logic), `src-tauri/src/commands/` (thin IPC wrappers, no business logic), React frontend (hooks wrapping `invoke()`). New `prefix_deps` module mirrors `install/`, `launch/`, `profile/` directory-with-`mod.rs` structure. SQLite store functions go in `metadata/prefix_deps_store.rs` (not `prefix_deps/store.rs`) — consistent with `health_store.rs`, `offline_store.rs`, `cache_store.rs`.
- **Data Flow**: Frontend `invoke()` → `src-tauri/commands/prefix_deps.rs` → `crosshook_core::prefix_deps::*` → `tokio::process::Command` (winetricks) + `MetadataStore` (SQLite). Long-running install ops stream `prefix-dep-log` events via `app.emit()`; completion emits `prefix-dep-complete`. Short-lived check ops return synchronously after SQLite read or 30s timeout.
- **Integration Points**: 9 existing files modified + 1 new Rust file in `metadata/` + 4 new files in `prefix_deps/` + 1 new Tauri command file + 3 new frontend files. SQLite schema bumped to v15. TypeScript type mirrors updated in `src/types/`.

---

## Critical Files Reference

### Must Read Before Any Code

- `docs/plans/protontricks-integration/feature-spec.md`: Authoritative spec — all data models, 4 IPC command contracts, 14 business rules, security table, 5-phase plan
- `docs/plans/protontricks-integration/research-security.md`: 7 CRITICAL + 14 WARNING findings — must all be addressed before ship
- `docs/plans/protontricks-integration/research-practices.md`: Exact reusable file inventory with line numbers — prevents reimplementing existing utilities
- `docs/plans/protontricks-integration/analysis-tasks.md`: Full 17-task breakdown with dependency graph, file-to-task mapping, and critical path

### Core Patterns to Reuse

- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`: `resolve_umu_run_path()` at line 302 (PATH walk binary detection), `apply_host_environment()`, `is_executable_file()`
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: `attach_log_stdio()` — canonical subprocess spawn + log stdio pattern
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Sequential `if version < N` migration runner; add `migrate_14_to_15()` here
- `src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs`: Template for new `metadata/prefix_deps_store.rs` — `upsert_*`/`load_*` taking bare `&Connection`
- `src/crosshook-native/crates/crosshook-core/src/community/taps.rs`: `validate_branch_name` — security template for CLI arg validation (mirrors required `validate_protontricks_verbs()`)
- `src/crosshook-native/src-tauri/src/commands/update.rs`: `Mutex<Option<u32>>` concurrent-install lock pattern; for prefix deps, lock should hold `String` (prefix_path being installed) rather than PID
- `src/crosshook-native/src-tauri/src/commands/launch.rs`: Async spawn + `app.emit()` streaming pattern; log streaming at line 350

### Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/lib.rs`: Add `pub mod prefix_deps;` (Task 1.1)
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: Add `required_protontricks: Vec<String>` to `TrainerSection`, `extra_protontricks: Vec<String>` to `LocalOverrideTrainerSection` (Task 1.2)
- `src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs`: Add `required_protontricks` to community manifest (Task 1.2)
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: Add `protontricks_binary_path: String` + `auto_install_prefix_deps: bool` to `AppSettingsData` (Task 1.3)
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Add `migrate_14_to_15()` (Task 2.1)
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: Add `pub mod prefix_deps_store;` and expose store methods (Task 2.2)
- `src/crosshook-native/crates/crosshook-core/src/profile/health.rs`: Extend `ProfileHealthReport` with `dependency_issues`; synchronous SQLite reads only — no subprocess in health path (Task 4.1)
- `src/crosshook-native/src-tauri/src/commands/health.rs`: Extend `build_enriched_health_summary()` with dependency enrichment (Task 4.2)
- `src/crosshook-native/src-tauri/src/lib.rs`: Register 4 commands + manage `PrefixDepsInstallState` (Task 5a.2)
- `src/crosshook-native/src/hooks/useScrollEnhance.ts`: Add `PrefixDepsPanel` scroll container to `SCROLLABLE` selector — required by CLAUDE.md scroll rule (Task 5b.4)
- `src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs`: Add `required_prefix_deps` to `CommunityImportPreview` — Rust-side change, part of Task 5b.4

---

## Patterns to Follow

- **Thin IPC**: `#[tauri::command]` fns → `spawn_blocking` → `core_fn().map_err(|e| e.to_string())`. Zero business logic in `src-tauri`. See `commands/install.rs`.
- **Store Placement**: New store goes in `metadata/prefix_deps_store.rs` (not `prefix_deps/store.rs`). Functions take bare `&Connection`, return `Result<T, MetadataStoreError>`. Dispatched via `MetadataStore::with_conn()`.
- **Binary Detection**: Copy `resolve_umu_run_path()` signature — iterate `env::split_paths()`, check `is_executable_file()`. Discovery order: settings override → winetricks on PATH → protontricks on PATH.
- **Process Execution**: `Command::new(binary)` + individual `.arg()` per package (never `.args([joined])`) + `env_clear()` + `apply_host_environment()` + `WINEPREFIX` env + `cmd.arg("--")` before verbs + `kill_on_drop(true)`. Winetricks needs POSIX env — do NOT skip `apply_host_environment()`.
- **SQLite Migration**: `if version < 15 { migrate_14_to_15(conn)?; pragma_update(None, "user_version", 15); }` — additive-only (new tables only).
- **Serde Backward Compat**: All new TOML fields use `#[serde(default, skip_serializing_if = "Vec::is_empty")]`. No schema version bump required for profile files.
- **Error Enum**: Custom enum variants (not `anyhow`) — `anyhow` is not in `crosshook-core` deps. Match `MetadataStoreError` structure: `Database { action: &'static str, source }`, etc.
- **Concurrent Lock**: `Mutex<Option<String>>` holding the active prefix_path (not PID). Managed via Tauri state. Prevents concurrent winetricks runs. UI disables all install buttons when lock is held.
- **React Hook**: `usePrefixDeps.ts` with `let active = true; return () => { active = false; }` cleanup. Mirrors `useProtonInstalls.ts`.
- **Event Streaming**: `app.emit("prefix-dep-log", line)` + `app.emit("prefix-dep-complete", result)` from background task. Frontend listens via `listen()` from `@tauri-apps/api/event`.

---

## Cross-Cutting Concerns

- **Security (CRITICAL — all 7 must be addressed before Phase 5)**: `Command::arg()` per-verb (never shell); `cmd.arg("--")` separator before verbs; `validate_protontricks_verbs()` regex `^[a-z0-9][a-z0-9_\-]{0,63}$` + reject `-`-prefixed strings + max 50 verbs; same validation for manual UI input (S-22); raw subprocess output NEVER reaches UI (S-27) — stderr to `tracing` only, stdout filtered before emit; `BinaryNotFound` error must not leak filesystem paths through IPC (S-25); `PackageDependencyState.last_error` must be sanitized before IPC return (S-24).
- **Health path must be read-only**: `build_enriched_health_summary()` runs at startup — the dependency enrichment in Task 4.2 must be synchronous SQLite reads ONLY. No `Command` or `spawn` in health path. Reject any PR that adds subprocess spawning to `health.rs`.
- **Concurrent install prevention**: Global `PrefixDepsInstallState` mutex holding active prefix_path; UI disables all install buttons; `AlreadyInstalling` error variant.
- **Prefix initialization guard**: Check `pfx/` subdirectory exists before invoking winetricks; surface remediation message "Launch once to initialize prefix."
- **Community trust disclosure**: Import preview must show `required_protontricks` list with explicit trust notice before completing import (BR-12). Requires `exchange.rs` Rust change in Task 5b.4.
- **Scroll jank prevention**: `PrefixDepsPanel` scroll container must be registered in `useScrollEnhance.ts` SCROLLABLE selector (CLAUDE.md rule — tracked in Task 5b.4 checklist).
- **Timeout enforcement**: 30s for `check_prefix_dependencies`; 300s for `install_prefix_dependency`. Use `tokio::time::timeout`.
- **TTL gating**: 24h TTL on cached check results; after expiry state reverts to `unknown`. Force re-check available via [Check Now].

---

## Phase Structure and Parallelization

Phase ordering is a hard dependency chain — not organizational preference:

```
Phase 1 (Foundation) ──→ Phase 2 (Storage) ──→ Phase 3 (Runner) ──→ Phase 4 (Health) ──→ Phase 5a (IPC)
  [3 tasks, fully          [2 tasks]              [4 tasks]             [2 tasks]             [2 tasks]
   parallelizable                                                                          ↕ parallel after
   internally]                                                                          Phase 5a signatures
                                                                                          Phase 5b (UI/React)
                                                                                            [4 tasks]
```

**Critical path**: Task 1.1 → Task 2.2 → Task 3.2 → Task 4.2 → Task 5a.1 → Task 5a.2 (6 sequential steps).

**Parallelism within phases:**

- Phase 1: Tasks 1.1, 1.2, 1.3 touch different files — fully independent, all 3 can run simultaneously
- Phase 3: Tasks 3.1 (validation), 3.2 (runner), 3.3 (lock) are independent modules sharing only `PrefixDepsError` from Phase 1
- Phase 5a/5b split: Task 5b.1 (TypeScript types + hook stub) can be written from IPC signatures in `feature-spec.md` without waiting for 5a.1 to merge; synchronization point is Task 5b.2 onward (needs callable commands)

**Full 17-task breakdown**: See `docs/plans/protontricks-integration/analysis-tasks.md`

---

## Implementation Constraints

- **No new crates**: All dependencies (`tokio`, `rusqlite`, `serde`, `toml`, `tracing`) already in `crosshook-core/Cargo.toml`.
- **winetricks-direct is primary**: CrossHook stores prefix paths; protontricks' Steam App ID resolution is redundant. Use `WINEPREFIX=<path> winetricks -q -- <verbs>`. Protontricks is user-configured secondary only.
- **No shell interpolation — ever**: `Command::new()` + `.arg()` per argument (S-01, S-02, S-19 CRITICAL).
- **Full POSIX env required for winetricks**: Unlike Proton commands, must call `apply_host_environment()` after `env_clear()`. No exception.
- **Soft-block only**: Missing deps → amber `HealthStatus::Stale`, not launch-blocking (BR-6). Users can skip and launch.
- **No community schema version bump**: `#[serde(default)]` on `required_protontricks` — old clients silently ignore (BR-11).
- **`anyhow` not available in `crosshook-core`**: Custom error enums only.
- **No cancellation in v1**: Unsafe mid-install; show slow-install warning in pre-confirmation dialog instead.
- **Static verb allowlist for v1**: Structural regex hard gate + static known-verb advisory. No dynamic `winetricks list` query.
- **Store placement is `metadata/`**: `metadata/prefix_deps_store.rs` — not `prefix_deps/store.rs`. Keeps `prefix_deps` module focused on behavior; `metadata` owns all persistence.

---

## Key Recommendations

- **Resolve 4 open decisions first**: Field name, allowlist approach, `user_skipped` reset, TTL — all must be settled before Phase 1 begins.
- **Lock IPC signatures before Phase 5b**: Once `prefix_deps.rs` command names and request/response struct field names are finalized (in or before Task 5a.1), do not rename. TypeScript mirrors require coordinated changes across Rust, TS types, and every component that destructures them.
- **Security review gate before Phase 5**: Confirm all 7 CRITICAL findings (S-01, S-02, S-03, S-06, S-19, S-22, S-27) are addressed in Phase 3 `runner.rs` + `validation.rs` before any IPC/UI work begins.
- **Phase 2 (Storage) is the highest-priority early deliverable**: Migration + store API unblocks all downstream parallel work. Define `PrefixDependencyStateRow` struct and store function signatures first.
- **Write `FakeRunner` in Phase 3**: Enables full unit tests without system winetricks. Mirrors `ScopedCommandSearchPath` pattern in `launch/test_support.rs`.
- **`DependencyStatusBadge` is a new component**: Do NOT extend `HealthBadge`. Own `DepStatus` type, reuse `crosshook-status-chip` CSS class.
- **Prefix path key**: Store `STEAM_COMPAT_DATA_PATH` (parent of `pfx/`) in `prefix_dependency_state.prefix_path`. Use `resolve_proton_paths()` to derive consistently.
- **`exchange.rs` is a Rust file in Task 5b.4**: Community import preview trust disclosure requires a Rust-side struct change in `profile/exchange.rs`, not just React work. Plan for Rust compilation in that task.
