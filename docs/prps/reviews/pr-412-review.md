# PR Review #412 — feat(flatpak): per-app isolation with first-run host migration

**Reviewed**: 2026-04-20
**Mode**: PR
**Author**: yandy-r (Yandy Ramirez)
**Branch**: `feat/flatpak-isolation` → `main`
**Head**: `865e265`
**Decision**: APPROVE (with comments)

## Summary

Solid, well-scoped implementation of the Phase 4 Flatpak isolation model: per-app sandbox by default, one-way first-run migration from the host AppImage tree, opt-in shared mode retained, and Wine prefixes pinned to host. Validation is clean (clippy, 1164+ Rust tests, 42 Vitest tests, typecheck, host-gateway all green). Findings are concentrated in migration edge cases (EXDEV partial-writes, symlink handling), small doc/maintenance drift (stale ADR/PRD plan paths, stale Phase 4 future-tense comments, leftover `#[allow]` suppressions), and a UX rough edge in the toast count. None block merge.

## Findings

### CRITICAL

- None

### HIGH

- None

### MEDIUM

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/copier.rs:113` — EXDEV cross-filesystem fallback writes directly to `dst` with no rollback guarantee. If `copy_dir_recursive(&stage, dst)` is interrupted (SIGKILL, power loss), `dst` is left partially populated. On the next launch, `copy_data_subtrees`' idempotency check at line ~187 sees a non-empty `dst` and permanently skips this subtree. Host data is untouched so this is not data loss, but the user's sandbox is silently stuck with a partial subtree.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: In the EXDEV branch, copy to a second sibling stage (`<dst>.migrating2`) then rename that into `dst`. Preserves the same "never partial" invariant as the primary path. [correctness, security]

- **[F002]** `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/copier.rs:165` and `src/crosshook-native/src/hooks/useFlatpakMigrationToast.ts:36` — `copy_data_subtrees` returns a single `imported: Vec<&'static str>` that mixes `DATA_INCLUDE_SUBTREES` and `DATA_INCLUDE_FILES` entries. `MigrationOutcome::imported_subtrees` (the field name implies directories only) therefore contains `.db`, `.db-wal`, and `.db-shm` alongside `community`/`media`/`launchers`. The frontend toast computes `importCount = imported_subtrees.length + (imported_config ? 1 : 0)`, so a full migration shows "7 items imported" where a user would reasonably expect 5 (config + 3 subtrees + 1 metadata DB).
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Either (a) count the WAL trio as one logical item in the toast computation (e.g. emit a separate `imported_metadata_db: bool` and drop the three DB file paths from `imported_subtrees`), or (b) rename the field to `imported_items` and adjust the toast string so the count corresponds to something meaningful to the user.
  - **Resolution**: `copy_data_subtrees` now treats the SQLite WAL trio atomically: idempotency skips all three if any dst exists, copy rolls back any trio members written so far on failure, and the trio reports as a single representative entry (`crosshook/metadata.db`) in `imported`. Toast count is now honest (config + 3 subtrees + 1 DB = 5).

- **[F003]** `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/detector.rs:8`, `:13`, `:18` — `#[allow(dead_code)]` on `host_config_dir`, `host_data_dir`, `needs_first_run`. All three are actively called from `flatpak_migration/mod.rs:71–83` (via `detector::host_config_dir`, `detector::host_data_dir`, `detector::needs_first_run`). The suppressions are stale and would mask a real future dead-code warning if these items stop being used.
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Remove all three `#[allow(dead_code)]` attributes.

- **[F004]** `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/mod.rs:18–19` — Stale `#[allow(unused_imports)] // consumed by tasks 4.1 and 4.2` above `pub(crate) use prefix_root::{host_prefix_root_with, is_isolation_mode_active};`. Tasks 4.1 and 4.2 are complete. The `allow` should be removed; if removing it triggers a warning because the re-export is truly unused outside `prefix_root.rs`, gate the re-export with `#[cfg(test)]` or drop it entirely.
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Remove the `#[allow(unused_imports)]` attribute and the inline task-reference comment; if the lint fires, remove or `#[cfg(test)]`-gate the re-export.
  - **Resolution**: Dropped the stale `#[allow(unused_imports)]` and the `tasks 4.1 and 4.2` comment. `host_prefix_root_with` was genuinely unused outside `prefix_root.rs` (its tests access it via `super`) — removed from the re-export. `is_isolation_mode_active` is now used internally by the new `is_host_xdg_opt_in()` public wrapper (see F005).

- **[F005]** `src/crosshook-native/src-tauri/src/lib.rs:31–42` — `flatpak_host_xdg_opt_in()` duplicates the parsing logic of `crosshook_core::flatpak_migration::prefix_root::is_isolation_mode_active` (the comment now explicitly acknowledges this "Parity with …"). Per CLAUDE.md's thin-Tauri convention, business logic should live in `crosshook-core` and future changes to accepted env-var values will now need to be made in two places.
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Expose a `pub fn` wrapper in `crosshook_core::flatpak_migration` (e.g. `pub fn flatpak_host_xdg_opt_in() -> bool { !prefix_root::is_isolation_mode_active(&SystemEnv) }`) and delete the local duplicate in `src-tauri/src/lib.rs`.
  - **Resolution**: Added `pub fn crosshook_core::flatpak_migration::is_host_xdg_opt_in() -> bool` as an inverse of `is_isolation_mode_active(&SystemEnv)`. Deleted the local `flatpak_host_xdg_opt_in` helper in `src-tauri/src/lib.rs` and the accompanying `OsStr` import; startup gate now calls the crate entry point directly.

- **[F006]** `src/crosshook-native/src-tauri/src/lib.rs:352` — `FLATPAK_MIGRATION_OUTCOME.get().and_then(|slot| slot.lock().ok().and_then(…))` silently discards `PoisonError`, so a poisoned mutex drops the toast event. The migration runs single-threaded before the Tauri builder, so poisoning is theoretically impossible today — but the pattern is inconsistent with the codebase's usual `lock().unwrap_or_else(|e| e.into_inner())` recovery (e.g. in `platform/tests/common.rs`) and will hide a future regression if the migration is ever moved onto a thread.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Replace `.lock().ok()` with `.lock().unwrap_or_else(|e| e.into_inner())` to extract the inner value across poison boundaries. Matches the rest of the codebase's mutex recovery.

- **[F007]** `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/mod.rs:74–78` — `host_data_parent = host_data.parent().map(Path::to_path_buf).unwrap_or_else(|| host_data.clone())` is dead-branch defensive code. `detector::host_data_dir` always returns `<home>/.local/share/crosshook`, which has a parent even when `home == "/"`. The fallback is unreachable under any non-degenerate input.
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Remove the `unwrap_or_else` branch and either `expect("host_data_dir always has a parent")` with an invariant comment, or adjust the call so the parent is constructed directly.

- **[F008]** `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/copier.rs:182–194` — The `match dir_is_empty(&dst)` arm `Ok(true) | Err(_) => { /* proceed */ }` swallows _any_ error from the destination probe. If `dst` exists but is unreadable (e.g. permission-denied subdirectory), the loop falls through to `copy_tree_or_rollback` and the user-facing error loses the original permission context — you end up reporting a failure against the staging path rather than the real root cause on `dst`.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Pattern-match the error kind explicitly: treat `ErrorKind::NotFound` as "proceed" and route other errors into the `errors` vec with `FlatpakMigrationError::Io { path: dst.clone(), source }` before `continue`-ing.

- **[F009]** `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/copier.rs:74` — `stage.exists()` follows symlinks. If the staging path is a pre-existing symlink to somewhere else, the existence check reports `true` and `fs::remove_dir_all(&stage)` is called. Rust 1.73+ `remove_dir_all` safely refuses to follow a top-level symlink, so data loss at the target does not occur; however, using `symlink_metadata()` makes the intent explicit and robust regardless of future stdlib behavior. Same shape appears in `app_id_migration.rs` around the stage cleanup path.
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Replace `stage.exists()` with `stage.symlink_metadata().is_ok()` (i.e. `lstat`) so the presence check does not traverse symlinks. Apply the same treatment to any companion check in `app_id_migration.rs`.

- **[F010]** `src/crosshook-native/crates/crosshook-core/src/fs_util.rs:32–55` — `copy_symlink` recreates symlinks verbatim (including absolute targets and `..`-traversing relative targets). A malicious or accidental symlink in `~/.config/crosshook/` such as `/run/host/etc/passwd` is faithfully reproduced in the sandbox. Rust I/O will happily follow it once the sandbox reads the migrated file. The Flatpak threat model limits practical impact (attacker already controls the user's own XDG tree), but the migration gives no warning and no filtering.
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: In `copy_symlink`, check the resolved target: if it's absolute or contains `ParentDir` components, either skip it with a `tracing::warn!` or fail the entry with `FlatpakMigrationError::Io` so `copy_data_subtrees` records it in its per-entry error list.

- **[F011]** `src/crosshook-native/crates/crosshook-core/src/platform/xdg.rs:27–33` — Doc comment on `override_xdg_for_flatpak_host_access` is now stale: "Phase 4 (Flathub submission) will replace this with a proper per-app isolation model and a first-run migration — see the tracking issue linked from `docs/prps/prds/flatpak-distribution.prd.md` §10.2." Phase 4 _has_ landed; this function is the opt-in shared-mode hatch, not the future replacement target.
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Rewrite the paragraph to say "As of Phase 4 this function runs only under `CROSSHOOK_FLATPAK_HOST_XDG=1` (opt-in shared mode). The default Flatpak startup path uses per-app isolation + first-run migration — see ADR-0004 and `crosshook_core::flatpak_migration::run()`."

- **[F012]** `docs/architecture/adr-0004-flatpak-per-app-isolation.md:123` — References `docs/prps/plans/flatpak-isolation.plan.md`, but that file was moved to `docs/prps/plans/completed/flatpak-isolation.plan.md` in commit `cdc0549`. The link is broken.
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Update the reference to `docs/prps/plans/completed/flatpak-isolation.plan.md`.

- **[F013]** `docs/prps/prds/flatpak-distribution.prd.md:444` — Same stale path: heading reads `10.3 Phase 4 follow-up — per-app isolation (in-progress — see \`docs/prps/plans/flatpak-isolation.plan.md\`)`. Both the status label and the path are out of date.
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Update the path to `docs/prps/plans/completed/flatpak-isolation.plan.md` and change the status label from "in-progress" to "complete" to match the report's state.

- **[F014]** `src/crosshook-native/src/App.tsx:303` — The flatpak migration toast uses `className="crosshook-rename-toast crosshook-toast--flatpak-migration"`. The base class `crosshook-rename-toast` is scoped to the profile-rename workflow (`ProfilesOverlays`/`ProfilesHero`) and its positioning rules in `theme.css:191` are keyed off `.crosshook-page-scroll-shell--profiles > .crosshook-rename-toast`. Borrowing a component-specific class as a generic base couples this toast's styling to an unrelated workflow's future refactors.
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Introduce a shared base class (e.g. `crosshook-status-toast`) and switch both the rename toast and the migration toast to it. Update CSS rules accordingly.

- **[F015]** `src/crosshook-native/crates/crosshook-core/tests/flatpak_migration_integration.rs` — No test case covers a dangling symlink inside a host include-subtree. `copy_dir_recursive` preserves symlinks; a dangling symlink in `crosshook/community/` (plausible for a partially-deleted tap) has no documented behavior. Either "preserve the dangle in the sandbox" or "skip it with a warning" is fine, but the intent should be pinned down by a test.
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Add one integration case that places a dangling symlink inside a host include-subtree and asserts migration completes without error, then assert the desired end state (symlink present, or absent with a warning logged).

- **[F016]** `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/copier.rs:206–237` — The WAL trio is iterated file-by-file with no transaction. If `.db` is copied but `.db-wal` copy fails, the sandbox gets a journal-less database. SQLite recovers from a missing WAL, so this is not a data-corruption risk per se, but the plan calls out the partial-WAL scenario as a known caveat and there is no test pinning down the behavior.
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Add a test asserting that when `.db` copies succeed and `.db-wal` fails (e.g. simulated via a pre-existing unwritable destination file), the migration reports the error and the subsequently-opened DB is still usable.

### LOW

- **[F017]** `src/crosshook-native/src/hooks/useFlatpakMigrationToast.ts:39–44` — Inside `if (sessionStorage.getItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY) === '1') return;` there is a subsequent `sessionStorage.setItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY, '1');` — the `set` inside the already-set branch is a no-op.
  - **Status**: Failed
  - **Category**: Maintainability
  - **Suggested fix**: Remove the redundant `setItem` inside the "already set" guard.

- **[F018]** `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/copier.rs:104–112` — Between `copy_dir_recursive(src, &stage)` completing and `if dst.exists() { fs::remove_dir(dst) }`, another process could briefly populate `dst`. `fs::remove_dir` then fails with `ENOTEMPTY` and the whole operation returns `Err`. No data loss occurs; the error message ("i/o error at dst") is just less helpful than it could be.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Same `symlink_metadata()` recommendation as F009; optionally surface a clearer error message when the check races.

- **[F019]** `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/mod.rs:5` — Module doc says "before any `BaseDirs::new()` call". The `run()` function at line 44 itself calls `BaseDirs::new()`. The intended meaning is "before any _store's_ `BaseDirs::new()`", i.e. `SettingsStore::try_new`, `ProfileStore::try_new`, etc.
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Clarify to "before any _store's_ `BaseDirs::new()` call" (or equivalent).

- **[F020]** `src/crosshook-native/src/hooks/useFlatpakMigrationToast.ts:4,25,39,41` — Uses `sessionStorage` for dedup, so the toast re-appears after each cold Tauri window restart until the user dismisses it. If the intent is "show once per session, re-show after restart" that's fine — but the choice is undocumented and a future maintainer looking at the behavior may "fix" it by swapping to `localStorage`.
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Add a one-line comment explaining the intent, e.g. `// sessionStorage: resets on window restart so the toast re-surfaces until the user explicitly dismisses it.` If instead the intent is one-time-ever, switch to `localStorage`.

- **[F021]** `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/mod.rs:3` — Module doc comment says "(Task 4.1)". Task identifiers are plan-internal and stop meaning anything once the plan is archived.
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Remove the parenthetical "(Task 4.1)". The surrounding sentence stands on its own.

- **[F022]** `src/crosshook-native/src-tauri/src/lib.rs:133,144` — `std::env::set_var` / `remove_var` called without `unsafe`. Currently safe on Rust 2021 edition (deprecated but not yet `unsafe`), but will require `unsafe {}` once the crate moves to Rust 2024. Adding the `unsafe {}` now, with a "SAFETY: startup, single-threaded, before Tauri builder spawns threads" comment, brings this in line with the pattern used for `override_xdg_for_flatpak_host_access` and removes a silent-tick-tock future migration.
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Wrap both calls in `unsafe { … }` with a short SAFETY note affirming they run before any thread spawns.

## Validation Results

| Check          | Result                                              |
| -------------- | --------------------------------------------------- |
| Type check     | Pass                                                |
| Lint (clippy)  | Pass                                                |
| Lint (biome)   | Pass (touched files)                                |
| Tests (Rust)   | Pass — 1164 + 4 integration + adhoc/install/fs_util |
| Tests (Vitest) | Pass — 42/42                                        |
| Host-gateway   | Pass                                                |
| Build          | Pass (`cargo build --workspace`)                    |

## Files Reviewed

- `AGENTS.md` (Modified)
- `docs/architecture/adr-0004-flatpak-per-app-isolation.md` (Added)
- `docs/prps/plans/completed/flatpak-isolation.plan.md` (Added/moved)
- `docs/prps/prds/flatpak-distribution.prd.md` (Modified)
- `docs/prps/reports/flatpak-isolation-report.md` (Added)
- `packaging/flatpak/README.md` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/copier.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/detector.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/mod.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/prefix_root.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/types.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/fs_util.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/install/service.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/lib.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/platform/env.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/platform/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/platform/tests/common.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/platform/tests/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/platform/xdg.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/run_executable/service/adhoc_prefix.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/tests/flatpak_migration_integration.rs` (Added)
- `src/crosshook-native/src-tauri/src/lib.rs` (Modified)
- `src/crosshook-native/src/App.tsx` (Modified)
- `src/crosshook-native/src/hooks/__tests__/useFlatpakMigrationToast.test.ts` (Added)
- `src/crosshook-native/src/hooks/useFlatpakMigrationToast.ts` (Added)
