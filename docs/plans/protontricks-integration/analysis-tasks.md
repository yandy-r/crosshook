# Protontricks Integration: Task Structure Analysis

## Executive Summary

The protontricks-integration feature is well-scoped and cleanly decomposed into 5 phases in the feature spec. The codebase has strong, consistent patterns (store structs, thin IPC commands, hook wrappers, sequential SQLite migrations) that each phase can leverage directly. The primary structural insight is that Phases 1–3 are backend-only with clear internal boundaries, Phase 4 is a narrow surgical integration into the existing health system, and Phase 5 is the largest phase but can be split into two independent parallel tracks (Tauri IPC layer vs. React frontend components). The feature introduces no new crates, no new external dependencies, and the most complex piece (the install runner) has a clear testability boundary via the `ProtontricksRunner` trait.

---

## Recommended Phase Structure

The 5-phase breakdown from the feature spec is the correct high-level structure. The recommended refinement is to split Phase 5 into two sub-tracks that can proceed in parallel once the IPC command signatures are agreed on.

```
Phase 1: Foundation           [~3 tasks, no blockers, internal parallelism]
  ↓
Phase 2: Storage              [~2 tasks, blocked by Phase 1 for types]
  ↓
Phase 3: Install Runner       [~4 tasks, blocked by Phase 2]
  ↓
Phase 4: Health Integration   [~2 tasks, blocked by Phase 3]
  ↓
Phase 5a: IPC Layer           [~2 tasks, blocked by Phase 4]
Phase 5b: React Frontend      [~4 tasks, can start after IPC signatures locked even before 5a ships]
```

### Why This Ordering Is Load-Bearing

- **Phase 1 must precede Phase 2**: `DependencyState`, `BinaryInvocation`, and `PrefixDepsError` Rust types (defined in `prefix_deps/mod.rs` or a `models.rs` sub-module) are imported by the store layer.
- **Phase 2 must precede Phase 3**: The runner calls `upsert_dependency_state()` and reads installed state from SQLite; the store module must exist first.
- **Phase 3 must precede Phase 4**: Health integration reads `DependencyState` rows via the store; the store and enums must be stable.
- **Phase 4 before Phase 5a**: The `check_prefix_dependencies` IPC command calls the runner which was finalized in Phase 3; `get_dependency_status` queries the store. Trying to implement the IPC layer before the runner and store are ready leads to placeholder code that must be torn out.
- **Phase 5b can start in parallel with 5a** once the 4 IPC command signatures (names, request structs, response structs) are finalized and committed — the React hooks only call `invoke()` with known argument shapes; they do not depend on the Rust implementation being complete.

---

## Task Granularity Recommendations

The 1–3 files-per-task constraint maps cleanly to this feature. Recommended task boundaries:

### Phase 1 Tasks (3 tasks)

**Task 1.1 — Binary detection module skeleton**
Files: `crosshook-core/src/prefix_deps/mod.rs` (new), `crosshook-core/src/lib.rs` (add `pub mod prefix_deps;`)
Work: `resolve_winetricks_path()`, `resolve_protontricks_path()` using the `resolve_umu_run_path()` PATH walk pattern from `launch/runtime_helpers.rs:302`. `BinaryInvocation` and `PrefixDepsError` types. Unit tests using `ScopedCommandSearchPath` test pattern.

**Task 1.2 — TOML schema extension**
Files: `crosshook-core/src/profile/models.rs`, `crosshook-core/src/profile/community_schema.rs`
Work: Add `required_protontricks: Vec<String>` to `TrainerSection` with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`. Add `extra_protontricks: Vec<String>` to `LocalOverrideTrainerSection`. Round-trip and backward-compat TOML tests. No behavior — pure data schema.

**Task 1.3 — Settings extension + onboarding check**
Files: `crosshook-core/src/settings/mod.rs`, `crosshook-core/src/onboarding/readiness.rs`
Work: Add `protontricks_binary_path: String` and `auto_install_prefix_deps: bool` to `AppSettingsData` with `#[serde(default)]`. Add winetricks binary check (Check 6) to `check_system_readiness()` with `Info` severity. Unit test for settings serde default and onboarding check behavior.

Tasks 1.1, 1.2, and 1.3 are **fully independent** and can be developed in parallel.

### Phase 2 Tasks (2 tasks)

**Task 2.1 — SQLite migration v14 → v15**
Files: `crosshook-core/src/metadata/migrations.rs`
Work: Add `migrate_14_to_15()` creating `prefix_dependency_state` table with composite unique index. Add test using `open_in_memory()` that verifies table and indices exist post-migration.

**Task 2.2 — Prefix deps store module**
Files: `crosshook-core/src/metadata/prefix_deps_store.rs` (new), `crosshook-core/src/metadata/mod.rs`
Work: `upsert_dependency_state()`, `load_dependency_states()`, `load_dependency_state_for_package()`, `clear_stale_states()` functions. Add `pub mod prefix_deps_store;` to `metadata/mod.rs`. Tests using `open_in_memory()` for all CRUD paths.

Task 2.1 should land before 2.2 (the store functions reference the table schema) but they can be developed concurrently since the DDL is known.

### Phase 3 Tasks (4 tasks)

**Task 3.1 — Validation module**
Files: `crosshook-core/src/prefix_deps/validation.rs` (new)
Work: `validate_protontricks_verbs()` — structural regex `^[a-z0-9][a-z0-9_\-]{0,63}$`, reject `-`-prefixed strings, max 50 verbs per batch, known-verb advisory set. Unit tests covering injection attempts, edge cases, max-verb boundary.

**Task 3.2 — ProtontricksRunner trait + RealRunner**
Files: `crosshook-core/src/prefix_deps/runner.rs` (new)
Work: `ProtontricksRunner` trait with `check_installed()` and `install_packages()` methods. `RealRunner` implementation: Command construction (`env_clear` → `apply_host_environment` → `WINEPREFIX` → `--` separator → verbs), `attach_log_stdio`, 300-second timeout, prefix initialization guard (`pfx/` subdirectory check), DISPLAY/WAYLAND_DISPLAY guard. `FakeRunner` for tests. Unit tests using `FakeRunner`.

**Task 3.3 — Global install lock**
Files: `crosshook-core/src/prefix_deps/mod.rs` (extend), or as a separate `lock.rs` sub-module if preferred
Work: `PrefixDepsInstallLock` (async `Mutex<Option<String>>` keyed on prefix path) and `AlreadyInstalling` error path. Tests for lock acquisition and rejection.

**Task 3.4 — Integration test: runner + store round-trip**
Files: `crosshook-core/src/prefix_deps/` (test module)
Work: End-to-end test using `FakeRunner` + in-memory SQLite: check_installed → upsert state → install → upsert state → verify SQLite state. This validates the Phase 2 + Phase 3 integration without a real winetricks binary.

Tasks 3.1 and 3.2 can be developed in parallel. Task 3.3 can proceed in parallel with 3.1 and 3.2. Task 3.4 requires 3.1, 3.2, and 3.3 to be complete.

### Phase 4 Tasks (2 tasks)

**Task 4.1 — Health report extension**
Files: `crosshook-core/src/profile/health.rs`
Work: Add `dependency_issues: Vec<HealthIssue>` to `ProfileHealthReport`. Add helper `build_dependency_health_issues()` that reads `DependencyState` rows synchronously and returns `HealthIssue` entries (amber `Warning` for missing/failed, `Info` for installed). Add shared-prefix detection with `Info` severity warning.

**Task 4.2 — Health enrichment integration**
Files: `src-tauri/src/commands/health.rs`
Work: Extend `build_enriched_health_summary()` (the batch health scan called at startup and on-demand) to call the new dependency health helper. The call must be a synchronous SQLite read only — no subprocess spawning in health scan path.

### Phase 5a Tasks (2 tasks)

**Task 5a.1 — IPC command module**
Files: `src-tauri/src/commands/prefix_deps.rs` (new), `src-tauri/src/commands/mod.rs`
Work: 4 Tauri commands: `detect_protontricks_binary`, `check_prefix_dependencies`, `install_prefix_dependency`, `get_dependency_status`. `PrefixDepsInstallState` struct (wrapping the global install lock). Thin delegation to `crosshook-core`. Validate inputs, extract state with `State<'_>`, serialize errors to `String`.

**Task 5a.2 — Tauri registration**
Files: `src-tauri/src/lib.rs`
Work: Register `PrefixDepsInstallState::new()` via `.manage()`. Add 4 commands to `invoke_handler!` macro. Verify build compiles and handler names match frontend `invoke()` call names.

### Phase 5b Tasks (4 tasks)

**Task 5b.1 — TypeScript types + hook**
Files: `src/crosshook-native/src/types/profile.ts`, `src/crosshook-native/src/types/settings.ts`, `src/crosshook-native/src/hooks/usePrefixDeps.ts` (new)
Work: Add `required_protontricks?: string[]` to `GameProfile`. Add `protontricks_binary_path` and `auto_install_prefix_deps` to `AppSettingsData`. `usePrefixDeps` hook: wraps `invoke('get_dependency_status')`, owns loading/error state, cleanup flag pattern matching `useProtonInstalls.ts`.

**Task 5b.2 — DependencyStatusBadge component**
Files: `src/crosshook-native/src/components/PrefixDepsPanel.tsx` (new, or scoped component file)
Work: `DependencyStatusBadge` with own `DepStatus` type — do NOT extend `HealthBadge`. Status chip reusing `crosshook-status-chip` CSS class. `PrefixDepsPanel` scaffold: package list, status chips, [Install] / [Skip] banner. Indeterminate progress bar and ConsoleDrawer integration for `prefix-dep-log` events.

**Task 5b.3 — Install flow + confirmation modal**
Files: `src/crosshook-native/src/components/PrefixDepsPanel.tsx` (extend), plus modal component
Work: Install confirmation dialog (package list, human-readable labels, slow-install warnings). Pre-launch gate modal with [Install + Launch] / [Skip and Launch] / [Cancel]. Per-package [Retry] for `install_failed` state. Global install-in-progress disable of all install buttons.

**Task 5b.4 — Settings UI + import preview**
Files: `src/crosshook-native/src/components/SettingsPanel.tsx`, `src/crosshook-native/src/hooks/useScrollEnhance.ts`
Work: Settings binary path fields (text input + Browse + live validation indicator + Flatpak help note) for winetricks/protontricks. `auto_install_prefix_deps` toggle. Community import preview extension: `required_prefix_deps` section with trust disclosure. Register any new `overflow-y: auto` container in `SCROLLABLE` selector in `useScrollEnhance.ts`.

Tasks 5b.1–5b.4 have a soft dependency order (types before hook, hook before components) but 5b.1 and 5b.2 can overlap once type shapes are agreed on.

---

## Dependency Analysis

```
1.1 (binary detection)  ─┐
1.2 (TOML schema)        ├──→  2.1 (migration) ──→ 2.2 (store) ──→ 3.1 (validation) ─┐
1.3 (settings+onboard)  ─┘                                          3.2 (runner)      ├──→ 3.4 (integration test)
                                                                    3.3 (install lock) ─┘
                                                                         ↓
                                                                  4.1 (health model)
                                                                         ↓
                                                                  4.2 (health enrichment)
                                                                         ↓
                                                         5a.1 (IPC commands)  ←→  5b.1 (TS types + hook)
                                                                  ↓                      ↓
                                                         5a.2 (register)          5b.2 (DependencyStatusBadge)
                                                                                         ↓
                                                                                  5b.3 (install flow + modals)
                                                                                         ↓
                                                                                  5b.4 (Settings UI + import)
```

**Critical path**: 1.1 → 2.2 → 3.2 → 4.2 → 5a.1 → 5a.2. This is approximately 6 sequential steps. Everything else is off the critical path and can be parallelized.

**Phase 5a/5b synchronization point**: 5b.1 (TypeScript types) can be written from the IPC command signatures in the feature spec without waiting for 5a.1 to merge. The synchronization point is 5b.2 onward, which needs `check_prefix_dependencies` and `install_prefix_dependency` to be callable.

---

## File-to-Task Mapping

| File                                               | Task       | Type                                            |
| -------------------------------------------------- | ---------- | ----------------------------------------------- |
| `crosshook-core/src/lib.rs`                        | 1.1        | Modify                                          |
| `crosshook-core/src/prefix_deps/mod.rs`            | 1.1, 3.3   | Create                                          |
| `crosshook-core/src/prefix_deps/validation.rs`     | 3.1        | Create                                          |
| `crosshook-core/src/prefix_deps/runner.rs`         | 3.2        | Create                                          |
| `crosshook-core/src/prefix_deps/store.rs`          | 2.2        | Create (or use `metadata/prefix_deps_store.rs`) |
| `crosshook-core/src/profile/models.rs`             | 1.2        | Modify                                          |
| `crosshook-core/src/profile/community_schema.rs`   | 1.2        | Modify                                          |
| `crosshook-core/src/profile/exchange.rs`           | 5b.4       | Modify (import preview extension)               |
| `crosshook-core/src/profile/health.rs`             | 4.1        | Modify                                          |
| `crosshook-core/src/settings/mod.rs`               | 1.3        | Modify                                          |
| `crosshook-core/src/onboarding/readiness.rs`       | 1.3        | Modify                                          |
| `crosshook-core/src/metadata/migrations.rs`        | 2.1        | Modify                                          |
| `crosshook-core/src/metadata/mod.rs`               | 2.2        | Modify                                          |
| `crosshook-core/src/metadata/prefix_deps_store.rs` | 2.2        | Create                                          |
| `src-tauri/src/commands/prefix_deps.rs`            | 5a.1       | Create                                          |
| `src-tauri/src/commands/mod.rs`                    | 5a.1       | Modify                                          |
| `src-tauri/src/lib.rs`                             | 5a.2       | Modify                                          |
| `src-tauri/src/commands/health.rs`                 | 4.2        | Modify                                          |
| `src/types/profile.ts`                             | 5b.1       | Modify                                          |
| `src/types/settings.ts`                            | 5b.1       | Modify                                          |
| `src/hooks/usePrefixDeps.ts`                       | 5b.1       | Create                                          |
| `src/components/PrefixDepsPanel.tsx`               | 5b.2, 5b.3 | Create                                          |
| `src/components/SettingsPanel.tsx`                 | 5b.4       | Modify                                          |
| `src/hooks/useScrollEnhance.ts`                    | 5b.4       | Modify (if new scrollable container added)      |

**Note on store placement**: The feature spec lists `prefix_deps/store.rs` as one option and `metadata/prefix_deps_store.rs` as another. The `metadata/` directory is the correct home — all other `*_store.rs` files (health_store, offline_store, cache_store, etc.) live there. Place it at `metadata/prefix_deps_store.rs` and keep `prefix_deps/mod.rs` focused on detection and public re-exports.

---

## Optimization Opportunities

### Parallelism Wins

1. **Phase 1 is fully parallelizable**: All 3 tasks touch different files with no shared module. Assign to separate implementers or run in sequence with no ordering constraint.

2. **Phase 3, tasks 3.1–3.3 are parallelizable**: Validation, runner, and lock are independent modules. They share only the `PrefixDepsError` type from Phase 1.

3. **Phase 5a and 5b.1 can start in parallel**: TypeScript type mirrors and the `usePrefixDeps` hook stub can be written from the IPC command signatures alone.

### Quick Win Sequencing

If delivering incrementally, the highest-value early deliverables with the least blast radius are:

1. **Task 1.2 (TOML schema)** — zero behavioral change; existing profiles are unaffected by the new `#[serde(default)]` fields. Can ship independently.
2. **Task 2.1 (migration)** — isolated SQLite change. Ships alongside or after 1.2 without UI change.
3. **Task 1.3 (settings + onboarding)** — adds an informational winetricks check to the onboarding panel. Low-risk, visible to users.

### Test Coverage Strategy

Each phase has a natural test boundary:

- Phase 1: TOML round-trip tests (no binary needed), PATH walk unit tests using `ScopedCommandSearchPath`
- Phase 2: In-memory SQLite tests using `open_in_memory()` from `metadata/db.rs`
- Phase 3: `FakeRunner` tests (no real winetricks binary in CI)
- Phase 4: Unit tests for health issue generation given mock `DependencyState` inputs
- Phase 5: No configured frontend test framework — rely on dev/build scripts for UI behavior

---

## Implementation Strategy Recommendations

### 1. Lock IPC Signatures Before Phase 5b

Before starting Task 5b.1, ensure the 4 IPC command names and their request/response struct field names are finalized and documented. The TypeScript types are mirrors of the Rust structs — renaming a field in Rust after 5b work has started requires a coordinated change across `prefix_deps.rs` (Rust), the TS type file, and every component that destructures it.

### 2. Store Module Placement

Put all SQLite functions in `metadata/prefix_deps_store.rs`, not in `prefix_deps/store.rs`. This keeps the `prefix_deps` module focused on behavior (detection, running, validation) and lets the `metadata` module own all persistence — consistent with `health_store.rs`, `offline_store.rs`, `cache_store.rs`, etc.

### 3. Global Lock Scope

The feature spec says "global async Mutex" for install locking. The `UpdateProcessState` pattern in `src-tauri/src/commands/update.rs` uses a `Mutex<Option<u32>>` holding the child PID. For prefix deps, the lock should hold a `String` (the prefix_path being installed) rather than a PID, since the PID is not needed for the "already installing" check and knowing which prefix is active is useful for the UI disable logic.

### 4. Health Integration Must Be Read-Only

Task 4.2 touches `build_enriched_health_summary()` which runs at startup. The dependency health enrichment **must** be a synchronous SQLite read — no `check_prefix_dependencies` subprocess spawning in this path. The spec states this explicitly, but it is worth enforcing at the task level: if a reviewer sees `spawn` or `Command` in health.rs, reject it.

### 5. Phase 5 UI: Scrollable Container Registration

If `PrefixDepsPanel` uses `overflow-y: auto` (likely), Task 5b.2 must add the new container's selector to `SCROLLABLE` in `useScrollEnhance.ts`. This is a gotcha that the CLAUDE.md project instructions flag explicitly. Track it as a checklist item in the 5b.2 task definition.

### 6. Community Import Preview (exchange.rs)

Task 5b.4 mentions adding `required_prefix_deps` to `CommunityImportPreview`. This requires modifying `crosshook-core/src/profile/exchange.rs`. This is a Rust-side change (the preview struct), not just a React change. The task definition for 5b.4 should include `exchange.rs` as a Rust file to modify, even though 5b.4 is primarily a frontend task.

### 7. Decisions from Feature Spec

The feature spec lists 4 open decisions. All 4 should be resolved before implementation begins:

- **Field name** (`required_protontricks` vs `required_wine_deps`): Spec recommends keeping `required_protontricks` — this is the correct call given community familiarity.
- **Allowlist**: Structural regex as hard gate, static known-verb set for advisory — resolve before Task 3.1.
- **`user_skipped` reset**: Per-package action — resolve before Task 5b.3.
- **TTL**: 24 hours — resolve before Task 2.2 (the store's staleness check logic depends on this).
