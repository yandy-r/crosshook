# PR #25 Review: feat(launcher): implement launcher lifecycle management

**PR**: #25 (`feat/launcher-delete` -> `main`)
**Date**: 2026-03-24
**Scope**: +6,957 / -22 across 34 files (source: +1,811, docs/plans: +5,146)
**Closes**: #24

## Overview

This PR implements launcher lifecycle management for CrossHook's exported `.sh` scripts and `.desktop` entries. A new `launcher_store.rs` module (1,007 lines) handles check/delete/rename/list/orphan-detection for launcher files. The frontend gains status badges, delete confirmations, stale notifications, and an orphan management section in Settings.

Five specialized review agents analyzed the changes in parallel:

| Agent                 | Focus                                              |
| --------------------- | -------------------------------------------------- |
| Code Reviewer         | CLAUDE.md compliance, bugs, security, code quality |
| Silent Failure Hunter | Swallowed errors, missing logging, race conditions |
| Type Design Analyzer  | Type encapsulation, invariants, Rust/TS alignment  |
| Test Analyzer         | Coverage gaps, edge cases, test quality            |
| Comment Analyzer      | Comment accuracy, staleness, documentation         |

---

## Critical Issues (4 found)

### C1. IPC argument mismatch: `confirmDelete` calls `check_launcher_exists` with wrong parameters

**Status**: Open
**Agents**: Code Reviewer (98%), Silent Failure Hunter, Comment Analyzer
**File**: `src/crosshook-native/src/hooks/useProfile.ts:393-395`

The `confirmDelete` callback invokes:

```typescript
const launcherInfo = await invoke<LauncherInfo>('check_launcher_exists', {
  profileName: trimmed,
});
```

The Tauri command (`src-tauri/src/commands/export.rs:20-34`) expects **five** parameters: `displayName`, `steamAppId`, `trainerPath`, `targetHomePath`, `steamClientInstallPath`. The parameter name `profileName` does not match any of these. Tauri's IPC deserialization will always fail, and the empty catch block at line 401 silently swallows the error.

**Impact**: The entire "show launcher files in the delete confirmation dialog" feature is non-functional. The user never sees the warning that launcher files will be removed when deleting a profile. The cascade delete still happens server-side, but users are never informed.

**Fix**: Load the full profile and pass correct parameters matching the backend signature, or create a simplified backend command that accepts a profile name.

---

### C2. IPC argument mismatch: `handleDelete` in SettingsPanel calls `delete_launcher` with wrong parameters

**Status**: Open
**Agents**: Code Reviewer (97%), Comment Analyzer
**File**: `src/crosshook-native/src/components/SettingsPanel.tsx:201-205`

```typescript
await invoke<string[]>('delete_launcher', {
  launcherSlug: slug,
  targetHomePath,
  steamClientInstallPath,
});
```

The Tauri command expects `displayName`, `steamAppId`, `trainerPath`, `targetHomePath`, `steamClientInstallPath`. There is no `launcherSlug` parameter. This call will always fail at IPC deserialization.

**Impact**: The "Manage Launchers" section's per-launcher delete button in Settings is completely broken. Clicking "Confirm" always produces an error.

**Fix**: Either change the backend command to accept a `launcher_slug` parameter (more ergonomic since `list_launchers` only returns slugs), or pass the parameters the current command expects.

---

### C3. `profile_rename` does not cascade to `rename_launcher_files`

**Status**: Open
**Agent**: Type Design Analyzer
**File**: `src/crosshook-native/src-tauri/src/commands/profile.rs:44-51`

The `profile_delete` command correctly cascades to `delete_launcher_for_profile`. However, `profile_rename` only renames the TOML file -- it does **not** call `rename_launcher_files`. CLAUDE.md states: _"profile deletion and renaming cascade to launcher cleanup via Tauri commands"_, but the rename cascade is not wired up.

**Impact**: Renaming a profile leaves orphaned launcher files with the old slug and display name. The `LauncherRenameResult` type and `rename_launcher_files` function exist but are dead code from the Tauri command perspective.

**Fix**: Add launcher rename cascade to `profile_rename`, mirroring the pattern in `profile_delete`.

---

### C4. Misplaced doc comment on `find_orphaned_launchers`

**Status**: Open
**Agent**: Comment Analyzer
**File**: `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs:413-415`

The doc comment's first two lines describe `extract_display_name_from_desktop` (line 428), not `find_orphaned_launchers`. Only the third line is accurate.

```rust
/// Extracts the display name from a `.desktop` file by reading the `Name=` line
/// and stripping the ` - Trainer` suffix.
/// Returns launchers that don't match any known profile slug.
```

**Fix**: Replace with a comment that describes only `find_orphaned_launchers`.

---

## Important Issues (12 found)

### I1. Empty catch blocks mask real bugs

**Status**: Open
**Agents**: Silent Failure Hunter, Code Reviewer
**Files**: `useProfile.ts:401-403`, `LauncherExport.tsx:186-188`

Two empty catch blocks silently swallow errors:

- `confirmDelete` catch (line 401): Hides the C1 IPC mismatch. Comment says "Backend command not available" but the actual failure is a permanent parameter mismatch.
- `refreshLauncherStatus` catch (line 186): Sets status to `null` on any error, hiding permission errors, I/O failures, and IPC issues. The status indicator simply disappears.

Per CLAUDE.md: _"ALWAYS throw errors early and often. Do not use fallbacks."_

**Fix**: At minimum, log errors to console. Better: distinguish expected failures from unexpected ones.

---

### I2. Duplicate `LauncherInfo` type in `useProfile.ts` diverges from canonical type

**Status**: Open
**Agents**: Code Reviewer (88%), Comment Analyzer
**File**: `src/crosshook-native/src/hooks/useProfile.ts:5-11`

The hook defines its own `LauncherInfo` missing `display_name` and `launcher_slug` fields that exist in `types/launcher.ts`. Both are exported. Components import inconsistently from different sources.

**Fix**: Remove the duplicate and import from `../types`.

---

### I3. TypeScript `LauncherDeleteResult` missing skip reason fields

**Status**: Open
**Agents**: Code Reviewer (82%), Type Design Analyzer, Comment Analyzer
**File**: `src/crosshook-native/src/types/launcher.ts:11-16`

The Rust struct includes `script_skipped_reason: Option<String>` and `desktop_entry_skipped_reason: Option<String>`, but the TypeScript type omits both. The frontend cannot tell users _why_ a deletion was skipped (watermark missing, symlink, etc.).

**Fix**: Add `script_skipped_reason?: string | null` and `desktop_entry_skipped_reason?: string | null`.

---

### I4. `rename_launcher_files` silently ignores old file deletion errors

**Status**: Open
**Agents**: Code Reviewer (81%), Silent Failure Hunter
**File**: `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs:312-317`

```rust
let _ = remove_file_if_exists(&old_script_path);
```

The `let _ =` pattern discards the `Result`. If old files cannot be deleted, the rename reports success while leaving orphaned old files on disk. The user sees duplicate launchers with no indication.

**Fix**: Add warning fields to `LauncherRenameResult` (e.g., `old_file_cleanup_warnings: Vec<String>`) or at minimum log via `tracing::warn!`.

---

### I5. `rename_launcher_files` skips watermark verification before deleting old files

**Status**: Open
**Agents**: Code Reviewer (80%), Silent Failure Hunter
**File**: `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs:310-318`

`delete_launcher_files` verifies watermarks before every delete. `rename_launcher_files` calls `remove_file_if_exists` directly without watermark checks. This inconsistency means the rename path could delete a non-CrossHook file that the delete path would refuse to touch.

**Fix**: Apply `verify_crosshook_file` before deleting old files during rename, consistent with the delete path.

---

### I6. `delete_launcher_for_profile` called with empty strings for home/steam paths

**Status**: Open
**Agents**: Silent Failure Hunter, Test Analyzer
**File**: `src/crosshook-native/src-tauri/src/commands/profile.rs:34`

```rust
crosshook_core::export::delete_launcher_for_profile(&profile, "", "")
```

The cascade delete passes empty strings for both path arguments. `resolve_target_home_path` falls back to `$HOME` env var, which works on standard Linux but silently no-ops in non-standard environments (containers, Flatpak, etc.).

**Fix**: Extract paths from the profile's Steam settings or from app settings.

---

### I7. `is_stale` always `false` in `list_launchers` but computed in `check_launcher_exists`

**Status**: Open
**Agents**: Type Design Analyzer, Comment Analyzer
**File**: `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs:406`

`list_launchers` hard-codes `is_stale: false` for every entry. Consumers of `LauncherInfo` from different sources get inconsistent semantics. The doc comment does not mention this limitation.

**Fix**: Either compute staleness in `list_launchers` (via a shared constructor), or document the limitation in both the doc comment and the `LauncherInfo` struct docs.

---

### I8. `list_launchers` silently returns empty Vec on directory read errors

**Status**: Open
**Agent**: Silent Failure Hunter
**File**: `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs:350-353`

```rust
Err(_) => return Vec::new(),
```

Any error reading the launchers directory (permissions, I/O) returns an empty list. Only `NotFound` is expected; other errors represent real filesystem problems.

**Fix**: Distinguish `NotFound` from other errors. Log non-`NotFound` at `tracing::warn!`.

---

### I9. Stale detection uses wrong default on read failure

**Status**: Open
**Agent**: Silent Failure Hunter
**File**: `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs:152-154`

```rust
.unwrap_or(false)
```

If the `.desktop` file exists but cannot be read (permissions, encoding), the launcher is reported as "not stale" rather than "unknown." The safe default should be `true` (assume stale, prompt re-export) since failing to verify freshness should not be treated as confirmation of freshness.

**Fix**: Change to `unwrap_or(true)` or return an error state.

---

### I10. `delete_launcher` result not inspected by frontend

**Status**: Open
**Agent**: Silent Failure Hunter
**File**: `src/crosshook-native/src/components/LauncherExport.tsx:364-371`

The `delete_launcher` invoke call discards the `LauncherDeleteResult` and always shows "Launcher deleted." If watermark verification skipped both files, the user still sees success.

**Fix**: Inspect the result. Show a warning with skip reasons when files were not actually deleted.

---

### I11. `verify_crosshook_file` doc comment incomplete about NotFound behavior

**Status**: Open
**Agent**: Comment Analyzer
**File**: `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs:445-449`

The doc says `Ok(None)` means "safe to delete" but the function also returns `Ok(None)` for non-existent files. This ambiguity could confuse future maintainers.

**Fix**: Add: "If the file does not exist, returns `Ok(None)` -- the caller handles the no-op case."

---

### I12. No doc comments on Tauri command functions in `commands/export.rs`

**Status**: Open
**Agent**: Comment Analyzer
**File**: `src/crosshook-native/src-tauri/src/commands/export.rs`

Seven Tauri command functions have zero doc comments. For IPC boundary functions that define the frontend-backend contract, this is a missed opportunity. Tauri's `camelCase` parameter conversion is not documented anywhere.

**Fix**: Add `///` doc comments to at least the public-facing commands.

---

## Test Coverage Gaps (6 found)

### T1. `find_orphaned_launchers` has zero tests — Criticality: 9/10

**Status**: Open
**Agent**: Test Analyzer
**File**: `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs:416-426`

No test verifies the orphan detection logic. If the slug-matching filter were accidentally inverted, orphans would be silently misreported.

---

### T2. Staleness detection (`is_stale`) never asserted — Criticality: 8/10

**Status**: Open
**Agent**: Test Analyzer

The `check_when_both_files_exist` test uses placeholder content (no `Name=` line), so `is_stale` defaults to `false` and the detection path is never exercised.

---

### T3. `profile_delete` cascade untested — Criticality: 8/10

**Status**: Open
**Agent**: Test Analyzer
**File**: `src/crosshook-native/src-tauri/src/commands/profile.rs:28-42`

The empty-string home path arguments, native method skip, and error swallowing behavior are all untested. The cascade may silently no-op in production.

---

### T4. Rename partial-state untested — Criticality: 7/10

**Status**: Open
**Agent**: Test Analyzer

Only the both-files-exist case is tested. Partial state (script exists, desktop doesn't, or vice versa) is realistic but untested.

---

### T5. `rename` overwriting an existing profile — Criticality: 6/10

**Status**: Open
**Agent**: Test Analyzer
**File**: `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs:138`

`fs::rename` silently overwrites a file at `new_path`. No test confirms whether this is intended or should error.

---

### T6. `extract_display_name_from_desktop` edge cases — Criticality: 6/10

**Status**: Open
**Agent**: Test Analyzer

Only the `Name=Foo - Trainer` case is indirectly tested. The "no suffix" and "no Name= line" branches are never exercised.

---

## Type Design Ratings

| Type                   | Encapsulation | Invariant Expression | Usefulness | Enforcement |
| ---------------------- | :-----------: | :------------------: | :--------: | :---------: |
| `LauncherInfo`         |     2/10      |         3/10         |    6/10    |    3/10     |
| `LauncherDeleteResult` |     2/10      |         4/10         |    7/10    |    3/10     |
| `LauncherRenameResult` |     2/10      |         4/10         |    6/10    |    3/10     |
| `LauncherStoreError`   |     7/10      |         6/10         |    6/10    |    7/10     |
| TS mirror types        |     4/10      |         3/10         |    5/10    |    2/10     |

Key type design recommendations:

- Remove `Default` derive on result types; use `Option<LauncherInfo>` for "not found" cases
- Replace `bool` + `Option<String>` pairs in `LauncherDeleteResult` with a per-artifact enum (`Deleted | Skipped(String) | NotPresent`)
- `HomePathResolutionFailed` error variant appears unused (dead code)

---

## Suggestions (5 found)

### S1. Test name references stale C# heritage

**Status**: Open
**File**: `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs:572`

`desktop_exec_escaping_matches_csharp_rules` references C# — a language no longer in the project. Rename to `desktop_exec_escaping_follows_freedesktop_spec`.

### S2. `list_launchers` iterator silently skips failed directory entries

**Status**: Open
**File**: `launcher_store.rs:357` — `.flatten()` discards `Err` variants from `DirEntry` reads.

### S3. No logging when cascade delete skipped for native profiles

**Status**: Open
**File**: `profile.rs:32` — Silent skip with no `tracing::debug!`.

### S4. `rename_launcher_files` doc comment doesn't mention same-slug case

**Status**: Open
**File**: `launcher_store.rs:232-234` — Should note that when slug is unchanged, files are rewritten in place without deletion.

### S5. `check_launcher_exists` Tauri command never returns an error

**Status**: Open
**File**: `export.rs:20-34` — Returns `LauncherInfo` directly (not `Result`), so filesystem errors are silently degraded.

---

## Strengths

All five agents highlighted positive aspects:

1. **Watermark verification** is well-designed and thoroughly tested. Three distinct tests cover the critical safety gate (watermarked file, missing watermark, symlink rejection).

2. **Write-then-delete rename strategy** is well-motivated — both `.sh` and `.desktop` embed plaintext display names that must be regenerated, not just moved.

3. **Filesystem test isolation** — all Rust tests use `tempfile::tempdir()` with proper cleanup. Test helpers are well-factored with `///` doc comments.

4. **Consistent filesystem assertions** — tests check both return values AND filesystem state, catching the class of bugs where a function reports success without performing the operation.

5. **`pub(crate)` visibility** for shared helpers strikes the right balance — `launcher_store` imports helpers from `launcher.rs` without exposing them publicly.

6. **15 new passing tests** with good coverage of the core happy paths, watermark safety, symlink rejection, rename with and without slug changes, and profile rename operations.

---

## Recommended Action

### Before merge (Critical)

1. Fix C1 + C2: Correct IPC parameter mismatches in `useProfile.ts` and `SettingsPanel.tsx`
2. Fix C3: Wire `rename_launcher_files` into `profile_rename` Tauri command
3. Fix C4: Correct the misplaced doc comment on `find_orphaned_launchers`

### Before merge (Important)

4. Fix I1: Replace empty catch blocks with error logging
5. Fix I2: Remove duplicate `LauncherInfo` type from `useProfile.ts`
6. Fix I3: Add missing skip reason fields to TypeScript `LauncherDeleteResult`

### Soon after merge

7. Add tests for T1 (orphan detection) and T2 (staleness)
8. Address I4-I5 (rename error handling and watermark consistency)
9. Address I10 (inspect delete result in frontend)

### Nice to have

10. Type design improvements (remove `Default`, enum-based delete outcomes)
11. Remaining suggestions (S1-S5)
