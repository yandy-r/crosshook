# Implementation Report: Flatpak Per-App Isolation & First-Run Migration (Phase 4)

## Summary

Retired the Phase 1 unconditional `override_xdg_for_flatpak_host_access()` call
as the default Flatpak behavior and replaced it with:

1. **Per-app isolation by default** — sandbox XDG paths are left intact.
2. **First-run migration** from the host AppImage tree: config copied verbatim,
   data copied selectively (metadata DB trio + community + media + launchers;
   prefixes/artifacts/cache/logs/runtime-helpers skipped).
3. **Opt-in shared mode** via `CROSSHOOK_FLATPAK_HOST_XDG=1` (kept for power
   users; undocumented on Flathub).
4. **Host-prefix-root override** — wine prefixes stay on host
   (`$HOME/.local/share/crosshook/prefixes/`) regardless of sandbox XDG.
5. **One-time UI toast** via a `flatpak-migration-complete` Tauri event, with
   `sessionStorage` dedup.

Unblocks Flathub submission (#206). Closes #212. Part of #210 (Phase 4 tracker).

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual                                                                                        |
| ------------- | ---------------- | --------------------------------------------------------------------------------------------- |
| Complexity    | Large (11 tasks) | Large, matched                                                                                |
| Files Changed | ~15              | 24 (plan budget ~15 new/updated; extras are script-cleanup drift from main and test fixtures) |
| Tests added   | ~17 unit + 4 e2e | 27 unit (flatpak_migration) + 7 fs_util + 4 integration + 6 frontend = 44                     |

## Tasks Completed

| #   | Task                                               | Status   | Notes                                                                                                       |
| --- | -------------------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------------- |
| 1.1 | Module skeleton + error enum + manifest            | Complete |                                                                                                             |
| 1.2 | Extract `copy_dir_recursive` to `fs_util`          | Complete | All pre-existing `app_id_migration` tests still green                                                       |
| 2.1 | First-run detector                                 | Complete | 7 unit tests                                                                                                |
| 2.2 | Selective copier with staged rename                | Complete | EXDEV fallback preserved                                                                                    |
| 2.3 | Host-prefix-root resolver                          | Complete | Relocated `EnvSink` to `platform/env.rs`; widened `platform::tests::common` to `pub(crate)` for test access |
| 3.1 | `flatpak_migration::run()` orchestrator            | Complete | `run_for_roots` exposed as `#[doc(hidden)] pub` for integration-test access                                 |
| 4.1 | Wire migration into `src-tauri/src/lib.rs` startup | Complete | `FLATPAK_MIGRATION_OUTCOME` static stash + Tauri `setup`-closure emit                                       |
| 4.2 | Prefix-root override in install/service + adhoc    | Complete | 4 adhoc tests + 2 install-service tests                                                                     |
| 5.1 | End-to-end integration test                        | Complete | 4 fixture scenarios                                                                                         |
| 5.2 | Frontend toast handler + sessionStorage dedup      | Complete | Uses project's custom DOM-toast pattern (no external toast lib)                                             |
| 5.3 | Documentation + ADR + PRD + packaging              | Complete | **ADR numbered 0004** (0003 already claimed by proton-download-manager)                                     |

## Validation Results

| Level           | Status | Notes                                                                               |
| --------------- | ------ | ----------------------------------------------------------------------------------- |
| Static Analysis | Pass   | `cargo clippy --workspace -- -D warnings` clean; biome green on all touched files   |
| Unit Tests      | Pass   | 1164 `crosshook-core` lib tests (incl. 27 flatpak_migration + 7 fs_util)            |
| Build           | Pass   | `cargo build --workspace` green                                                     |
| Integration     | Pass   | 4 `flatpak_migration_integration` tests pass                                        |
| Edge Cases      | Pass   | Idempotency, host-missing, partial-host, skip-list all covered                      |
| Host-gateway    | Pass   | `./scripts/check-host-gateway.sh` green                                             |
| Frontend        | Pass   | 42 Vitest tests (incl. 6 new `useFlatpakMigrationToast`), `npm run typecheck` clean |

## Files Changed

| File                                                                                    | Action | Notes                                                                           |
| --------------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/mod.rs`               | CREATE | Facade + `run()` + `#[doc(hidden)] pub fn run_for_roots`                        |
| `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/types.rs`             | CREATE | `FlatpakMigrationError`, `MigrationOutcome`, include/skip manifest              |
| `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/detector.rs`          | CREATE | `needs_first_run`, `host_config_dir`, `host_data_dir`                           |
| `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/copier.rs`            | CREATE | `copy_tree_or_rollback` + `copy_data_subtrees` (EXDEV-safe)                     |
| `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/prefix_root.rs`       | CREATE | `host_prefix_root`, `is_isolation_mode_active`, `host_prefix_root_with`         |
| `src/crosshook-native/crates/crosshook-core/src/fs_util.rs`                             | CREATE | Shared `copy_dir_recursive`, `copy_symlink`, `dir_is_empty`                     |
| `src/crosshook-native/crates/crosshook-core/src/platform/env.rs`                        | CREATE | Relocated `EnvSink` trait + `SystemEnv`                                         |
| `src/crosshook-native/crates/crosshook-core/tests/flatpak_migration_integration.rs`     | CREATE | E2E integration tests                                                           |
| `src/crosshook-native/crates/crosshook-core/src/lib.rs`                                 | UPDATE | `+pub mod flatpak_migration; +mod fs_util;`                                     |
| `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs`                    | UPDATE | Re-uses `fs_util` helpers; behavior unchanged                                   |
| `src/crosshook-native/crates/crosshook-core/src/platform/mod.rs`                        | UPDATE | Added `env` submodule + re-export; `tests` → `pub(crate)`                       |
| `src/crosshook-native/crates/crosshook-core/src/platform/xdg.rs`                        | UPDATE | Imports `EnvSink` from `super::env`                                             |
| `src/crosshook-native/crates/crosshook-core/src/platform/tests/mod.rs`                  | UPDATE | `common` → `pub(crate)`                                                         |
| `src/crosshook-native/crates/crosshook-core/src/platform/tests/common.rs`               | UPDATE | Visibilities widened to `pub(crate)` for cross-module test access               |
| `src/crosshook-native/crates/crosshook-core/src/install/service.rs`                     | UPDATE | `resolve_prefix_root_with(host_override)` seam                                  |
| `src/crosshook-native/crates/crosshook-core/src/run_executable/service/adhoc_prefix.rs` | UPDATE | `*_with(host_override, …)` seams for the adhoc path                             |
| `src/crosshook-native/src-tauri/src/lib.rs`                                             | UPDATE | Gated startup; `FLATPAK_MIGRATION_OUTCOME` stash; setup-closure emit            |
| `src/crosshook-native/src/hooks/useFlatpakMigrationToast.ts`                            | CREATE | Event subscription + sessionStorage dedup                                       |
| `src/crosshook-native/src/hooks/__tests__/useFlatpakMigrationToast.test.ts`             | CREATE | 6 Vitest cases                                                                  |
| `src/crosshook-native/src/App.tsx`                                                      | UPDATE | Mounts `useFlatpakMigrationToast()` + renders toast element                     |
| `docs/architecture/adr-0004-flatpak-per-app-isolation.md`                               | CREATE | ADR for the isolation contract                                                  |
| `docs/prps/prds/flatpak-distribution.prd.md`                                            | UPDATE | §10.3 → "in-progress"; references ADR-0004                                      |
| `packaging/flatpak/README.md`                                                           | UPDATE | "Shared mode (advanced users)" section with opt-in env override                 |
| `AGENTS.md`                                                                             | UPDATE | SQLite Metadata DB section → per-app isolation note + host-prefix-root override |

## Deviations from Plan

- **ADR numbering**: Plan called for `adr-0003-flatpak-per-app-isolation.md`.
  `adr-0003-proton-download-manager.md` already exists on main, so the new ADR
  was created as **`adr-0004-flatpak-per-app-isolation.md`**. All cross-references
  (PRD §10.3, packaging README, AGENTS.md) point at ADR-0004.
- **`run_for_roots` visibility**: Plan's mirror pattern (`app_id_migration.rs:247-276`)
  uses `#[cfg(test)] pub(crate) fn`. That would NOT be reachable from an integration
  test under `crates/crosshook-core/tests/`. Task 3.1 used `#[doc(hidden)] pub fn`
  (always compiled, hidden from rustdoc) to expose it cross-crate for Task 5.1.
- **`EnvSink` relocation scope**: Task 2.3 widened `platform::tests::common` and
  its items to `pub(crate)` so `flatpak_migration::prefix_root::tests` could reach
  `FakeEnv`. This was anticipated by the plan's Gotcha note.
- **Toast primitive**: Plan speculated `sonner` or a custom `useToast()`. The repo
  actually uses a project-specific DOM-toast pattern (e.g. `collectionDescriptionToast`).
  `useFlatpakMigrationToast` returns `{ importCount, dismiss }` and `App.tsx`
  renders the toast element with `crosshook-toast--flatpak-migration`.

## Issues Encountered

- **Expected merge conflict** on `crosshook-core/src/lib.rs` between Task 1.1
  (`pub mod flatpak_migration;`) and Task 1.2 (`mod fs_util;`) — both inserted
  at the alphabetically-adjacent slot. Resolved manually by keeping both lines.
- **Pre-existing biome warnings** in `src/lib/__tests__/runtime.test.ts` (unused
  `MockWindow` interface) surface when running `./scripts/lint.sh` but are
  unrelated to this plan's changes (file untouched on this branch).
- **Local main drift**: local `main` moved forward by one commit
  (`dd9dd0f fix(scripts): skip generated paths in tooling`) after this branch
  was based at `8328aa7`. The scripts-cleanup diff will auto-resolve on rebase
  before PR.

## Tests Written

| Test File                                                                          | Tests    | Coverage                                                     |
| ---------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------ |
| `crates/crosshook-core/src/fs_util.rs` (inline)                                    | 7 tests  | `copy_dir_recursive`, `copy_symlink`, `dir_is_empty`         |
| `crates/crosshook-core/src/flatpak_migration/detector.rs` (inline)                 | 7 tests  | `needs_first_run`, `host_{config,data}_dir`                  |
| `crates/crosshook-core/src/flatpak_migration/copier.rs` (inline)                   | 5 tests  | `copy_tree_or_rollback`, `copy_data_subtrees`                |
| `crates/crosshook-core/src/flatpak_migration/prefix_root.rs` (inline)              | 11 tests | `host_prefix_root_with`, `is_isolation_mode_active`          |
| `crates/crosshook-core/src/flatpak_migration/mod.rs` (inline)                      | 3 tests  | `run_for_roots` full-import + idempotent + partial           |
| `crates/crosshook-core/src/install/service.rs` (inline, added)                     | 2 tests  | `resolve_prefix_root_with` override + fallthrough            |
| `crates/crosshook-core/src/run_executable/service/adhoc_prefix.rs` (inline, added) | 4 tests  | throwaway detection + default-path override                  |
| `crates/crosshook-core/tests/flatpak_migration_integration.rs`                     | 4 tests  | Full fixture: include/skip matrix, idempotency, partial host |
| `crosshook-native/src/hooks/__tests__/useFlatpakMigrationToast.test.ts`            | 6 tests  | Event dedup, unmount cleanup, no-op path                     |

## Next Steps

- [ ] Rebase onto latest `main` to absorb `dd9dd0f` (scripts cleanup) before opening the PR.
- [ ] Manual Flatpak validation matrix (plan §"Manual Validation") — run on a
      real Flatpak install against an existing AppImage data tree.
- [ ] Code review via `/ycc:code-review` or `/review`.
- [ ] Create PR via `/ycc:prp-pr` referencing `Closes #212`, `Part of #210`,
      `Unblocks #206`.

## Worktree Summary

| Path                                            | Branch                 | Status |
| ----------------------------------------------- | ---------------------- | ------ |
| ~/.claude-worktrees/crosshook-flatpak-isolation | feat/flatpak-isolation | parent |

Cleanup after merge/push:

```bash
git worktree remove ~/.claude-worktrees/crosshook-flatpak-isolation
```
