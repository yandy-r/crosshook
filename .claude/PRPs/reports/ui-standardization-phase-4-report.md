# Implementation Report: UI Standardization Phase 4 — Setup Run EXE/MSI Ad-Hoc Flow

## Summary

Adds a Lutris-style **Run EXE/MSI** ad-hoc launcher under the existing **Setup**
sidebar group as a third sub-tab on the renamed **Install & Run** route. The new
flow lets users pick an arbitrary `.exe` or `.msi`, choose the Proton runtime,
optionally point at an existing prefix (or auto-create a throwaway one under
`_run-adhoc/<slug>`), and stream the helper log live — without ever forcing
profile creation. Reuses the `update_game` orchestration stack end-to-end via a
new shared `commands/log_stream.rs` module that powers both the existing update
flow and the new run-executable flow.

## Assessment vs Reality

| Metric         | Predicted (Plan) | Actual                                   |
| -------------- | ---------------- | ---------------------------------------- |
| Complexity     | Medium           | Medium (matched)                         |
| Confidence     | n/a              | High — no deviations from plan           |
| Files Changed  | 11–13            | 13 (8 created, 5 updated)                |

## Tasks Completed

| #   | Task                                                              | Status        | Notes                                                               |
| --- | ----------------------------------------------------------------- | ------------- | ------------------------------------------------------------------- |
| A1  | Create `run_executable` core module skeleton                      | Complete      |                                                                     |
| A2  | Define `RunExecutableRequest`, results, and errors                | Complete      | Added 4 model unit tests inside `models.rs`                         |
| A3  | Implement `validate_run_executable_request`                       | Complete      | Optional prefix path; case-insensitive `.exe`/`.msi`                |
| A4  | `resolve_default_adhoc_prefix_path` + `provision_prefix`          | Complete      | `_run-adhoc/<slug>` namespace, `adhoc` fallback slug                |
| A5  | `build_run_executable_command` and `run_executable`               | Complete      | MSI branch invokes `msiexec /i ... /qb`                             |
| A6  | Unit tests for the run_executable core service                    | Complete      | 19 service tests + 4 model tests = 23 new tests                     |
| B1  | Promote `spawn_log_stream` to `commands/log_stream.rs`            | Complete      | Behavior preserved; clear-pid callback closure replaces hard-coded `try_state::<UpdateProcessState>()` |
| B2  | Register `log_stream` and `run_executable` in `commands/mod.rs`   | Complete      | `log_stream` is private, `run_executable` is `pub mod`              |
| B3  | Implement `commands/run_executable.rs`                            | Complete      | Mirrors `update.rs` exactly, emits `run-executable-{log,complete}`  |
| B4  | Wire `RunExecutableProcessState` and commands into `lib.rs`       | Complete      |                                                                     |
| C1  | Create `types/run-executable.ts`                                  | Complete      | PascalCase variant keys (matches existing `types/install.ts` style) |
| C2  | Create `hooks/useRunExecutable.ts`                                | Complete      | Listen-before-invoke race-safe; cancel + reset semantics             |
| D1  | Create `components/RunExecutablePanel.tsx`                        | Complete      | Reuses `crosshook-install-shell*` BEM blocks, no new CSS            |
| D2  | Wire `RunExecutablePanel` into `InstallPage` as third sub-tab     | Complete      | `forceMount` + `display:none` retains hidden tab state              |
| D3  | Rename Install route in `routeMetadata.ts`                        | Complete      | `Install Game` → `Install & Run`; banner summary updated            |
| E1  | Static analysis pass (cargo fmt, clippy, tsc)                     | Complete      | New files clean; pre-existing warnings out of scope                 |
| E2  | New core unit tests pass                                          | Complete      | 23/23 passing                                                        |
| E3  | Full crosshook-core test sweep (regression for log_stream refactor)| Complete      | 743/743 passing — no regressions                                    |

## Validation Results

| Level                           | Status   | Notes                                                                    |
| ------------------------------- | -------- | ------------------------------------------------------------------------ |
| Static Analysis (cargo fmt)     | Pass     | All new/modified files clean (`rustfmt --edition 2021 --check`)          |
| Static Analysis (cargo clippy)  | Pass     | Zero warnings in `run_executable` core or Tauri layers; pre-existing warnings in `protonup`/`launch` are out of scope |
| Static Analysis (TypeScript)    | Pass     | `tsc --noEmit` exits 0                                                   |
| Unit Tests (new)                | Pass     | 23 new tests across `run_executable::{models,service}` modules           |
| Unit Tests (full crosshook-core)| Pass     | 743/743 — no regressions from the `log_stream` refactor                  |
| Workspace Build (`cargo build`) | Pass     | Tauri + CLI + core all compile clean                                     |
| Frontend Build (`npm run build`)| Pass     | `tsc && vite build` succeeds; pre-existing chunk-size warning unchanged  |
| Integration / Browser           | Deferred | Manual E4 dev-shell smoke test reserved for the user (requires real Proton/X11 session) |

## Files Changed

### Created (8)

| File                                                                                              | Action  |
| ------------------------------------------------------------------------------------------------- | ------- |
| `src/crosshook-native/crates/crosshook-core/src/run_executable/mod.rs`                            | CREATED |
| `src/crosshook-native/crates/crosshook-core/src/run_executable/models.rs`                         | CREATED |
| `src/crosshook-native/crates/crosshook-core/src/run_executable/service.rs`                        | CREATED |
| `src/crosshook-native/src-tauri/src/commands/log_stream.rs`                                       | CREATED |
| `src/crosshook-native/src-tauri/src/commands/run_executable.rs`                                   | CREATED |
| `src/crosshook-native/src/types/run-executable.ts`                                                | CREATED |
| `src/crosshook-native/src/hooks/useRunExecutable.ts`                                              | CREATED |
| `src/crosshook-native/src/components/RunExecutablePanel.tsx`                                      | CREATED |

### Updated (5)

| File                                                                                              | Action  | Notes                                                          |
| ------------------------------------------------------------------------------------------------- | ------- | -------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/lib.rs`                                           | UPDATED | `pub mod run_executable;`                                      |
| `src/crosshook-native/src-tauri/src/commands/mod.rs`                                              | UPDATED | `mod log_stream;` + `pub mod run_executable;`                  |
| `src/crosshook-native/src-tauri/src/commands/update.rs`                                           | UPDATED | Removed local log streamer; calls shared `log_stream::spawn_log_stream` with clear-pid closure |
| `src/crosshook-native/src-tauri/src/lib.rs`                                                       | UPDATED | `manage(RunExecutableProcessState::new())` + 3 new `invoke_handler!` entries |
| `src/crosshook-native/src/components/layout/routeMetadata.ts`                                     | UPDATED | Install route → `Install & Run` (navLabel + bannerTitle + summary) |
| `src/crosshook-native/src/components/pages/InstallPage.tsx`                                       | UPDATED | Widened `InstallPageTab` union; added third Tabs.Trigger + Tabs.Content; imported `RunExecutablePanel` |
| `src/crosshook-native/src/types/index.ts`                                                         | UPDATED | Re-exported `run-executable` types                             |

> Total: 8 created + 5 updated = 13 files (matches the upper bound of the plan estimate).

## Deviations from Plan

**None — implemented exactly as planned.**

Two minor stylistic refinements that fall within the plan's intent:

1. **TypeScript validation variant keys are PascalCase** (`'ExecutablePathRequired'`) rather than the snake_case strings shown in the plan example (`'executable_path_required'`). This matches the existing convention in `types/install.ts` and `types/update.ts`. The plan's GOTCHA explicitly only requires that the *message strings* match the Rust `message()` output character-for-character, which they do; the variant key naming is internal to TypeScript.
2. **Added a `HomeDirectoryUnavailable` variant to `RunExecutableError`** rather than overloading `PrefixCreationFailed` for the "no home directory" branch. This produces a clearer message when `BaseDirs::new()` fails (e.g. in headless CI without `$HOME`) and mirrors the install module's `InstallGameError::HomeDirectoryUnavailable` variant.

## Issues Encountered

**None.** The plan was self-contained and accurate. The `log_stream` refactor (Task B1) was the highest-risk step and was verified by running the full 743-test crosshook-core suite immediately after the change — zero regressions.

## Tests Written

| Test File                                                                                  | Tests | Coverage                                                                              |
| ------------------------------------------------------------------------------------------ | ----- | ------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/run_executable/models.rs` (`#[cfg(test)] mod tests`)           | 4     | Serde round-trip with all fields, Serde round-trip with optional fields omitted, every validation message matches the user-facing string, `From<RunExecutableValidationError> for RunExecutableError` |
| `crates/crosshook-core/src/run_executable/service.rs` (`#[cfg(test)] mod tests`)          | 19    | Validation: valid request (`.exe`), MSI extension, uppercase `.EXE`/`.MSI`, every negative path (empty/missing/dir/non-exe extension/empty proton/missing proton/non-executable proton/empty prefix allowed/missing prefix when provided/file as prefix). Build: MSI branch (`msiexec /i ... /qb`), EXE branch (no msiexec), proton path referenced. Resolve: slugifies executable stem, falls back to `adhoc` for unprintable stems. Spawn: rejects invalid request before spawning. |

**Total**: 23 new tests, all passing. Full sweep: 743/743 tests pass across `crosshook-core` (no regressions from the shared `log_stream` extraction).

## Next Steps

- [ ] Manual E4 dev-shell smoke test (requires real Proton + X11/Wayland session)
- [ ] Code review via `/code-review`
- [ ] Create PR via `/prp-pr`
- [ ] Tick Issue #163 Phase 4 checklist after PR merge
