# PR Review #408 — refactor: prefix_health.rs into smaller modules

**Reviewed**: 2026-04-20
**Mode**: PR
**Author**: app/openai-code-agent
**Branch**: codex/refactor-split-prefix-health-file → main
**Decision**: APPROVE

## Summary

Clean, surgical split of the 625-line `storage/prefix_health.rs` into a directory module with 9 cohesive submodules plus `mod.rs`. Logic is byte-for-byte equivalent to the original; no behavioral changes. All 1123 `crosshook-core` tests pass, clippy/rustfmt are clean, the host-gateway contract is preserved, and every caller in `src-tauri` still resolves through the unchanged `crosshook_core::storage::*` re-export surface.

## Findings

### CRITICAL

_None._

### HIGH

_None._

### MEDIUM

_None._

### LOW

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/storage/prefix_health/mod.rs:1` — New submodules have no module-level rustdoc describing their scope (types, constants, utils, discovery, scan, cleanup, disk, staged_trainers, references).
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Add a one-line `//!` doc comment at the top of each file (e.g., `//! Filesystem scanning for candidate wine prefixes.` in `discovery.rs`). Optional; the module names already carry most of the intent.

## Validation Results

| Check      | Result                                                                                    |
| ---------- | ----------------------------------------------------------------------------------------- |
| Type check | Pass (`cargo clippy -p crosshook-core --all-targets`)                                     |
| Lint       | Pass (`cargo clippy -D warnings`, `cargo fmt --check`, `./scripts/check-host-gateway.sh`) |
| Tests      | Pass (`cargo test -p crosshook-core` — 1123/1123)                                         |
| Build      | Pass (included in clippy check)                                                           |

## Files Reviewed

- `src/crosshook-native/crates/crosshook-core/src/storage/prefix_health.rs` (Deleted, 625 lines)
- `src/crosshook-native/crates/crosshook-core/src/storage/prefix_health/mod.rs` (Added, 20 lines — re-export surface)
- `src/crosshook-native/crates/crosshook-core/src/storage/prefix_health/constants.rs` (Added, 2 lines — internal `pub(super)` path constants)
- `src/crosshook-native/crates/crosshook-core/src/storage/prefix_health/types.rs` (Added, 79 lines — public DTOs + `DEFAULT_*` constants)
- `src/crosshook-native/crates/crosshook-core/src/storage/prefix_health/utils.rs` (Added, 54 lines — shared fs helpers)
- `src/crosshook-native/crates/crosshook-core/src/storage/prefix_health/discovery.rs` (Added, 99 lines — candidate-prefix discovery)
- `src/crosshook-native/crates/crosshook-core/src/storage/prefix_health/disk.rs` (Added, 48 lines — `check_low_disk_warning`)
- `src/crosshook-native/crates/crosshook-core/src/storage/prefix_health/references.rs` (Added, 45 lines — `collect_profile_prefix_references`)
- `src/crosshook-native/crates/crosshook-core/src/storage/prefix_health/scan.rs` (Added, 92 lines — `scan_prefix_storage` orchestrator)
- `src/crosshook-native/crates/crosshook-core/src/storage/prefix_health/staged_trainers.rs` (Added, 64 lines — `staged_trainers_health`)
- `src/crosshook-native/crates/crosshook-core/src/storage/prefix_health/cleanup.rs` (Added, 194 lines — `cleanup_prefix_storage` + orphan/stale helpers)

## Review Notes

**Behavior preservation verified:**

- All public symbols re-exported by `storage/mod.rs` (16 items) remain reachable through `prefix_health/mod.rs`.
- All three external callers (`src-tauri/src/commands/storage.rs`, `src-tauri/src/commands/launch/warnings.rs`, `src-tauri/src/lib.rs`) import via `crosshook_core::storage::*` and compile unchanged.
- Path-safety invariants in `cleanup_stale_staged_trainer` (canonical staged root must start with canonical prefix, canonical target must start with canonical staged root, symlink refusal) are preserved verbatim.
- Orphan-prefix guards (`has_crosshook_managed_marker`, `drive_c` presence check, canonicalization round-trip) are preserved.

**File-size compliance (CLAUDE.md 500-line soft cap):** largest new file is `cleanup.rs` at 194 lines. All submodules comfortably within the cap.

**Single responsibility:** each submodule has one clear concern — constants, shared types, fs utilities, discovery, scan orchestration, cleanup, disk-space check, staged-trainer health, profile references. Visibility is tight (`pub(super)` for siblings-only helpers, `pub` only on the re-exported surface).

**Acceptance criteria from the child issue:**

- [x] Preserve public APIs — unchanged.
- [x] Every resulting file ≤500 lines — max 194.
- [x] `cargo test -p crosshook-core` passes — 1123/1123.
- [x] `./scripts/lint.sh`-equivalent checks (clippy/fmt/host-gateway) pass.
- [x] Links back to umbrella issue #290 — PR body references the child issue, which links #290.
