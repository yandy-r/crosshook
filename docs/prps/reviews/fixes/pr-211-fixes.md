# Fix Report: pr-211-review

- **Source**: `docs/prps/reviews/pr-211-review.md`
- **Applied**: 2026-04-11
- **Mode**: Parallel (1 batch, max width 2 — one review-fixer per same-file group)
- **Severity threshold**: MEDIUM (lowered from default HIGH because no eligible HIGH findings remained — finding [8] is Accepted/deferred as Phase 1 scope)
- **Run command**: `/ycc:review-fix 211 --parallel --severity MEDIUM` (equivalent)

## Summary

- **Total findings in source**: 16
- **Already processed before this run**:
  - Accepted/Acknowledged/Resolved (non-Open): 4 ([8] Accepted, [14] Acknowledged, [15] Acknowledged, [16] Resolved)
- **Eligible this run**: 4
- **Applied this run**:
  - Fixed: 4 ([2], [4], [5], [6])
  - Failed: 0
- **Skipped this run**:
  - Below severity threshold (LOW): 8 ([1], [3], [7], [9], [10], [11], [12], [13])
  - Not Open status (Accepted/Acknowledged/Resolved): 4

## Fixes Applied

| ID  | Severity | File                                                              | Line(s) | Status | Notes                                                                |
| --- | -------- | ----------------------------------------------------------------- | ------- | ------ | -------------------------------------------------------------------- |
| [2] | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/platform.rs`      | 102-120 | Fixed  | Same-file group with [5]; 2 new tests                                |
| [5] | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/platform.rs`      | 73-82   | Fixed  | Same-file group with [2]; option (b) `host_command_with_env`; 2 new tests |
| [4] | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs` | 99-115  | Fixed  | Same-file group with [6]; staged-rename pattern; 1 new test          |
| [6] | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs` | 69, 186 | Fixed  | Same-file group with [4]; typed error enum (hand-rolled, no new dep) |

## Files Changed

- `src/crosshook-native/crates/crosshook-core/src/platform.rs` — finding [2] (`HOST_XDG_*_HOME` preference + `EnvSink::get` seam + 2 new tests) and finding [5] (`host_command_with_env` helper with `--env=KEY=VALUE` threading in Flatpak mode + 2 new tests + strong doc warning on bare `host_command`)
- `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs` — finding [6] (`AppIdMigrationError { Io, DestinationNotEmpty }` enum replacing `String` errors; hand-rolled `Display` + `std::error::Error` impls since `thiserror` is not a crate dep; `DestinationNotEmpty` is now a surfaced error instead of a silent skip) and finding [4] (staged-rename `{new_root_name}.migrating` sibling path so partial copies are never visible in `new_root`; + 1 new test for partial-failure recovery)

## Detailed Per-Finding Notes

### [2] `HOST_XDG_*_HOME` preference — `platform.rs:102-120`

The previous implementation derived every `XDG_*` value from `$HOME`, silently ignoring `HOST_XDG_CONFIG_HOME` / `HOST_XDG_DATA_HOME` / `HOST_XDG_CACHE_HOME` that Flatpak exposes specifically to carry the host's real (customized) XDG values. The fix introduces a read seam on the existing `EnvSink` trait (`fn get(&self, key: &str) -> Option<OsString>`) so tests can inject `HOST_XDG_*_HOME` without touching the real process env. A new `host_xdg_or_default` helper reads `HOST_XDG_*_HOME` via the sink, falls back to `<home>/<default_rel>` when absent. Two new tests cover the happy-path customized-layout scenario and the all-three-host-vars-set scenario.

**Why this matters**: any user with a customized XDG layout (e.g. `XDG_CONFIG_HOME=/data/configs`) previously had their AppImage and Flatpak installs writing to **different** paths, contradicting the documented "share data on disk" invariant.

### [5] `host_command_with_env` helper — `platform.rs:73-82`

Option (b) from the review was chosen (the reviewer's recommendation): a new `host_command_with_env(program: &str, envs: &BTreeMap<String, String>) -> Command` helper that threads each env entry as `--env=KEY=VALUE` args **before** the program name when in Flatpak, and uses `.envs()` otherwise. The bare `host_command()` function now has a strongly-worded doc comment warning that `.env()` / `.envs()` calls are silently dropped by `flatpak-spawn --host`, pointing Phase 3 Proton/Wine callers at the helper.

**Why this matters**: Phase 3 will migrate Proton/Wine/Steam launch sites to `host_command()`. Every one of those launches depends on `STEAM_COMPAT_*`, `PROTON_*`, `WINEPREFIX`, `MANGOHUD_CONFIG`, `GAMESCOPE_*` env vars. Without the helper, naive callers using `.env()` would see games launch with wrong prefixes, broken ProtonDB integration, broken overlays — and the bugs would be blamed on Proton rather than this scaffold.

### [4] Staged-rename for cross-device migration — `app_id_migration.rs:99-115` (now `:152-203`)

The previous fallback was `copy_dir_recursive(old_root, new_root)?` then `fs::remove_dir_all(old_root)?`. A mid-copy failure (ENOSPC, permission, interrupt) left `new_root` partially populated; on the next startup, the `new_root.exists() && !dir_is_empty(new_root)` pre-check silently skipped the migration and the user's data became permanently split.

The fix stages the copy to a sibling `{new_root_name}.migrating` path (same parent = same filesystem, so the subsequent `fs::rename` is atomic/cheap), and only after the full copy succeeds does the rename atomically publish `new_root`. On any mid-copy failure, best-effort `remove_dir_all(&stage)` cleans up and the original `old_root` is untouched. **Invariant restored**: `new_root` non-empty ⇒ migration succeeded.

A new test `staged_rename_partial_failure_recovery` pre-creates a stale stage directory simulating a prior interrupted run and asserts that the migration succeeds on the next attempt.

### [6] Typed errors for `migrate_one_app_id_root` — `app_id_migration.rs:69, 186`

Replaced `-> Result<(), String>` with `-> Result<(), AppIdMigrationError>` where the new enum has two variants:

```rust
pub enum AppIdMigrationError {
    Io { path: PathBuf, source: std::io::Error },
    DestinationNotEmpty(PathBuf),
}
```

Hand-rolled `impl fmt::Display` and `impl std::error::Error` (with `source()`) — no `thiserror` dependency added, since it is not currently in `crosshook-core/Cargo.toml`. The test helper `migrate_legacy_tauri_app_id_xdg_directories_for_roots` now returns `Vec<AppIdMigrationError>`. All call sites use `.map_err(|source| AppIdMigrationError::Io { path: ..., source })`.

**Semantic change worth noting**: the previous behavior silently returned `Ok(())` when the destination was non-empty (pre-migrated). Now it returns `Err(AppIdMigrationError::DestinationNotEmpty(...))`, which the production call site at `migrate_legacy_tauri_app_id_xdg_directories` surfaces through `tracing::warn!` + `eprintln!` via the existing `%e` formatter. This is **strictly better observability** — previously, pre-migrated destinations were invisible; now they're logged. Two existing tests (`skips_when_destination_non_empty` and `one_root_failure_does_not_stop_others`) had their assertions updated to match the new surfaced-error behavior.

## Validation Results

| Check                                                                            | Result                                    |
| -------------------------------------------------------------------------------- | ----------------------------------------- |
| `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`  | **Pass**                                  |
| `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`   | **Pass — 798 unit + 4 integration tests, 0 failed** |

Pre-fix test count per the PR #211 review body was 793 (16 new). Post-fix is 798, reflecting the 5 new tests added by this run (2 for [2], 2 for [5], 1 for [4]).

## Skipped This Run (below MEDIUM threshold)

The following findings remain `Status: Open` in the source review file and can be processed in a follow-up run with `/ycc:review-fix 211 --severity LOW`:

| ID   | Severity | File                                                                      | Reason                                                                    |
| ---- | -------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| [1]  | LOW      | PR #211 body                                                              | Non-code: requires `gh pr edit` to update the PR description              |
| [3]  | LOW      | `platform.rs:35-60`                                                       | Add `$HOME` preservation note to doc comment                              |
| [7]  | LOW      | `src-tauri/src/lib.rs:83,94`                                              | Wrap pre-existing `set_var`/`remove_var` in `unsafe` + SAFETY comment     |
| [9]  | LOW      | `app_id_migration.rs:149-153`                                             | Doc comment explaining `eprintln!` alongside `tracing::warn!`             |
| [10] | LOW      | `platform.rs:62`                                                          | Test coverage for the `!is_flatpak()` early-return branch                 |
| [11] | LOW      | `app_id_migration.rs:46`                                                  | Symlink-to-dir + broken-symlink test coverage for `copy_dir_recursive`    |
| [12] | LOW      | `scripts/build-flatpak.sh:122-129` + `scripts/build-native.sh:25-43`      | Move `arch_suffix_for_triple` into `scripts/lib/build-paths.sh`           |
| [13] | LOW      | `packaging/flatpak/dev.crosshook.CrossHook.yml:22` + `scripts/build-flatpak.sh:35` | Preflight check for `runtime-version` vs `CROSSHOOK_FLATPAK_RUNTIME_VERSION` drift |

Finding [8] (HIGH) remains `Status: Accepted as Phase 1 scope` and is **not** eligible — its "paper trail" fix requires PR-body edits + manifest YAML comment, and the review explicitly defers its code-level resolution to Phase 4 (Flathub submission).

## Failed Fixes

None.

## Next Steps

- **Re-run `/ycc:code-review 211`** to verify the 4 fixes resolved the findings and to catch any regressions introduced by the staged-rename refactor or the new `EnvSink::get` seam.
- **Optionally run `/ycc:review-fix 211 --severity LOW`** to address the 8 remaining LOW findings (mostly doc comments + test coverage + a small shell-script helper consolidation).
- **Update the PR #211 description** to address finding [1] (stale app-ID references, test count drift, new commit list) and add the "Flathub blockers" subsection from finding [8].
- **Commit** these changes via `/ycc:git-workflow` when satisfied. Suggested commit scope: `fix(flatpak)` or `refactor(crosshook-core)` — the platform.rs and app_id_migration.rs edits are both user-facing correctness improvements.
