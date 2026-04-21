# Fix Report: pr-412-review

**Source**: `docs/prps/reviews/pr-412-review.md`
**Applied**: 2026-04-20 (two passes — MEDIUM then LOW)
**Mode**: Parallel sub-agents (MEDIUM: 1 batch, width 10 · LOW: 1 batch, width 4)
**Severity threshold**: LOW (cumulative across both passes)

## Summary

- **Total findings in source**: 22 (0 CRITICAL, 0 HIGH, 16 MEDIUM, 6 LOW)
- **Already processed before these runs**:
  - Fixed: 3 (F002, F004, F005 — landed before this audit pass)
- **Applied across both passes**:
  - Fixed: 18 (13 MEDIUM + 5 LOW)
  - Failed: 1 (F017 — false positive)
- **Skipped**: 0
- **Final state in source review**: 21 Fixed · 1 Failed · 0 Open

## Fixes Applied — MEDIUM pass

| ID   | Severity | File                                                           | Line    | Status | Notes                                                                                                              |
| ---- | -------- | -------------------------------------------------------------- | ------- | ------ | ------------------------------------------------------------------------------------------------------------------ |
| F008 | MEDIUM   | `crates/crosshook-core/src/flatpak_migration/copier.rs`        | 182–191 | Fixed  | Split `dir_is_empty` Err arm: `NotFound` proceeds; other errors push `FlatpakMigrationError::Io` and continue.     |
| F001 | MEDIUM   | `crates/crosshook-core/src/flatpak_migration/copier.rs`        | 110–128 | Fixed  | EXDEV fallback now copies to `<dst>.migrating2` sibling and renames into `dst`. Same "never partial" invariant.    |
| F009 | MEDIUM   | `crates/crosshook-core/src/flatpak_migration/copier.rs`        | 70–80   | Fixed  | Replaced `stage.exists()` with `stage.symlink_metadata().is_ok()`; same fix landed in `app_id_migration.rs`.       |
| F016 | MEDIUM   | `crates/crosshook-core/tests/flatpak_migration_integration.rs` | +       | Fixed  | Added `wal_trio_partial_failure_rolls_back_and_migration_continues`.                                               |
| F015 | MEDIUM   | `crates/crosshook-core/tests/flatpak_migration_integration.rs` | +       | Fixed  | Added `dangling_symlink_in_include_subtree_is_preserved` — pins "preserve the dangle" contract.                    |
| F003 | MEDIUM   | `crates/crosshook-core/src/flatpak_migration/detector.rs`      | 8,13,18 | Fixed  | Removed three stale `#[allow(dead_code)]` attributes.                                                              |
| F006 | MEDIUM   | `src-tauri/src/lib.rs`                                         | 348     | Fixed  | Replaced `.lock().ok().and_then(…)` with `.lock().unwrap_or_else(\|e\| e.into_inner()).take()`.                    |
| F007 | MEDIUM   | `crates/crosshook-core/src/flatpak_migration/mod.rs`           | 87–91   | Fixed  | Removed dead fallback; now `expect("host_data_dir always has a parent (.local/share)")` with invariant comment.    |
| F010 | MEDIUM   | `crates/crosshook-core/src/fs_util.rs`                         | 41–61   | Fixed  | Reject absolute or `..`-traversing symlink targets with `tracing::warn!` + `InvalidInput` Io error.                |
| F011 | MEDIUM   | `crates/crosshook-core/src/platform/xdg.rs`                    | 27–33   | Fixed  | Doc updated to reflect Phase 4 landed: function is opt-in shared-mode, not the future replacement.                 |
| F012 | MEDIUM   | `docs/architecture/adr-0004-flatpak-per-app-isolation.md`      | 123     | Fixed  | Path updated to `docs/prps/plans/completed/flatpak-isolation.plan.md`.                                             |
| F013 | MEDIUM   | `docs/prps/prds/flatpak-distribution.prd.md`                   | 444     | Fixed  | Path + status label ("in-progress" → "complete").                                                                  |
| F14  | MEDIUM   | `src/App.tsx` + `styles/theme.css` + profiles toast sites      | mixed   | Fixed  | Introduced `crosshook-status-toast` shared base; migration toast + rename toasts now share it; CSS selector moved. |

## Fixes Applied — LOW pass

| ID   | Severity | File                                                    | Line     | Status | Notes                                                                                                                                       |
| ---- | -------- | ------------------------------------------------------- | -------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------- |
| F018 | LOW      | `crates/crosshook-core/src/flatpak_migration/copier.rs` | 100–116  | Fixed  | `dst.exists()` → `dst.symlink_metadata().is_ok()`; `remove_dir` error now wrapped with "dst became non-empty after staging completed" note. |
| F019 | LOW      | `crates/crosshook-core/src/flatpak_migration/mod.rs`    | 4–5      | Fixed  | Module doc clarified to "before any store's `BaseDirs::new()` call".                                                                        |
| F020 | LOW      | `src/hooks/useFlatpakMigrationToast.ts`                 | 4        | Fixed  | Added one-line comment above the session key describing the `sessionStorage`-over-`localStorage` intent.                                    |
| F021 | LOW      | `crates/crosshook-core/src/flatpak_migration/mod.rs`    | 3        | Fixed  | Removed the stale "(Task 4.1)" parenthetical.                                                                                               |
| F022 | LOW      | `src-tauri/src/lib.rs`                                  | 120, 132 | Fixed  | `std::env::set_var` + `std::env::remove_var` wrapped in `unsafe { ... }` with matching `// SAFETY:` comments (Rust 2024 forward-compat).    |

## Failed Fixes

### F017 — `src/crosshook-native/src/hooks/useFlatpakMigrationToast.ts:39–44`

**Severity**: LOW
**Category**: Maintainability
**Description (from review)**: Inside `if (sessionStorage.getItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY) === '1') return;` there is a subsequent `sessionStorage.setItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY, '1');` — the `set` inside the already-set branch is a no-op.
**Suggested fix (from review)**: Remove the redundant `setItem` inside the "already set" guard.
**Blocker**: The `setItem` is NOT inside the early-return branch. Lines 40–41 are:

```ts
if (sessionStorage.getItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY) === '1') return;
sessionStorage.setItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY, '1');
```

Line 40 is a single-statement `if (…) return;`. Line 41 executes only on the key-NOT-set path — it is the dedup mark. Removing it would break the "show once per session" contract: the key would never be set by the event handler, and the toast would re-surface on every qualifying event until the user explicitly dismissed it.

**Root cause**: The finding misread the single-line `if (…) return;` as a block with both statements inside the guard. The `setItem` is structurally the else-branch of the condition, not nested under it.

**Recommendation**: Mark F017 as a false-positive in a follow-up review note. No code change is warranted. Verified against both the current HEAD and the review HEAD (`865e265`).

## Files Changed

Source files:

- `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/copier.rs` (F001, F008, F009, F018)
- `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/detector.rs` (F003)
- `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/mod.rs` (F007, F019, F021)
- `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs` (F009 follow-on)
- `src/crosshook-native/crates/crosshook-core/src/fs_util.rs` (F010)
- `src/crosshook-native/crates/crosshook-core/src/platform/xdg.rs` (F011)
- `src/crosshook-native/crates/crosshook-core/tests/flatpak_migration_integration.rs` (F015, F016)
- `src/crosshook-native/src-tauri/src/lib.rs` (F006, F022)
- `src/crosshook-native/src/App.tsx` (F014)
- `src/crosshook-native/src/styles/theme.css` (F014)
- `src/crosshook-native/src/components/pages/profiles/ProfilesOverlays.tsx` (F014)
- `src/crosshook-native/src/components/pages/profiles/ProfilesHero.tsx` (F014)
- `src/crosshook-native/src/hooks/useFlatpakMigrationToast.ts` (F020)

Docs:

- `docs/architecture/adr-0004-flatpak-per-app-isolation.md` (F012)
- `docs/prps/prds/flatpak-distribution.prd.md` (F013)

Review artifact (Status updates only):

- `docs/prps/reviews/pr-412-review.md` — 18 × Open → Fixed, 1 × Open → Failed

## Validation Results — final (after LOW pass)

| Check                          | Result                                                      |
| ------------------------------ | ----------------------------------------------------------- |
| `cargo check --workspace`      | Pass (from `src/crosshook-native/`)                         |
| `cargo test -p crosshook-core` | Pass — 1193 tests passed, 0 failed                          |
| `npm run typecheck`            | Pass (`tsc --noEmit && tsc -p tsconfig.test.json --noEmit`) |
| `npm test` (Vitest)            | Pass — 42/42 across 10 files                                |

## Next Steps

- Re-run `/ycc:code-review 412` to verify the 18 applied fixes actually resolved the flagged issues and that no regressions slipped in.
- Run `/ycc:git-workflow` to commit the changes (this run used `--no-worktree`; nothing is staged).
- Note F017 as a known false-positive in the eventual review-reply / PR comment so it doesn't get re-opened.
