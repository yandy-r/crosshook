# PR Review #391 — refactor: split health.rs into smaller modules

**Reviewed**: 2026-04-19
**Mode**: PR
**Author**: app/openai-code-agent
**Branch**: codex/refactor-split-health-module → main
**Decision**: APPROVE

## Worktree Setup

- **Parent**: `~/.claude-worktrees/crosshook-pr-391/` (branch: `codex/refactor-split-health-module`)
- **Children** (per severity; created by `/ycc:review-fix --worktree`):
  - LOW → `~/.claude-worktrees/crosshook-pr-391-low/` (branch: `feat/pr-391-low`)

## Summary

Clean, behavior-preserving split of `src-tauri/src/commands/health.rs` (800 lines) into a `health/` module with 8 focused submodules (all ≤150 lines), matching the pattern established by prior refactors (#387 settings, #388 capability, #389 profile migration). All Tauri IPC commands, Serde boundary types, and the internal `build_enriched_health_summary` surface are preserved. `cargo clippy -D warnings`, 1097 unit/integration tests, the `crosshook-native` Tauri build, and `./scripts/check-host-gateway.sh` all pass. Only low-severity polish nits.

## Findings

### CRITICAL

_None._

### HIGH

_None._

### MEDIUM

_None._

### LOW

- **[F001]** `src/crosshook-native/src-tauri/src/commands/health/enrich.rs:75` — The refactor drops the load-bearing comment `// Inject version mismatch as a Warning health issue (BR-6: Warning, not Error)` that lived above the version-mismatch injection site in the original `enrich_profile`. BR-6 is a business-rule reference — dropping it loses institutional context that explains _why_ this path injects a `Warning` (not an `Error`).
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Re-add the BR-6 rationale comment in `enrich.rs` above the `if let Some(ref status) = version_status` block. Keep the "why" only; drop the pure section-header comments (`// Inject prefix dependency health issues`, etc.) that were already near-duplicates.
- **[F002]** `src/crosshook-native/src-tauri/src/commands/health/batch.rs:99-103` — `HealthStatus::Healthy`/`Stale`/`Broken` are fully-qualified as `crosshook_core::profile::health::HealthStatus::Healthy` (repeated three times in-line), whereas the original `health.rs` imported `HealthStatus` via `use crosshook_core::profile::health::{…, HealthStatus, …}`. The fully-qualified form is verbose and inconsistent with the rest of the file's import style.
  - **Status**: Open
  - **Category**: Pattern Compliance
  - **Suggested fix**: Add `HealthStatus` to the existing `use crosshook_core::profile::health::{…}` import at the top of `batch.rs` and drop the `crosshook_core::profile::health::` prefix from the three match arms.
- **[F003]** `src/crosshook-native/src-tauri/src/commands/health/types.rs:126` — `sanitize_issues` uses a fully-qualified `crate::commands::shared::sanitize_display_path(&issue.path)` call, whereas the original imported it with `use super::shared::sanitize_display_path;`. Minor style regression — fully-qualified paths are less readable and inconsistent with neighboring files.
  - **Status**: Open
  - **Category**: Pattern Compliance
  - **Suggested fix**: Add `use crate::commands::shared::sanitize_display_path;` at the top of `types.rs` and call `sanitize_display_path(&issue.path)`.
- **[F004]** `src/crosshook-native/src-tauri/src/commands/health/mod.rs:17` — `#[allow(unused_imports)]` is suppressing a warning on the `pub use types::{…}` re-export of `ProfileHealthMetadata`, `OfflineReadinessBrief`, `EnrichedProfileHealthReport`, `EnrichedHealthSummary`, `CachedHealthSnapshot`, `CachedOfflineReadinessSnapshot`. A quick `grep` confirms these types are only used inside `commands::health` — nothing outside the module imports them. Silencing the lint hides whether the re-export is actually needed.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Either (a) drop the `pub use types::{…}` block and the `#[allow(unused_imports)]` attribute — types remain visible to submodules via `super::types::…` — or (b) keep the re-export to preserve the original `commands::health::ProfileHealthMetadata` external path (useful for future external consumers) but document _why_ the re-export is defensive with a one-line comment, e.g. `// Re-exported to preserve the pre-split external surface; types are currently module-internal.` The former is more honest with the current codebase; the latter is friendlier to downstream callers.

## Validation Results

| Check      | Result                                                                       |
| ---------- | ---------------------------------------------------------------------------- |
| Type check | Pass (via `cargo clippy`)                                                    |
| Lint       | Pass (`cargo clippy -p crosshook-core -p crosshook-native -- -D warnings`)   |
| Tests      | Pass (`cargo test -p crosshook-core` — 1097 unit + integration tests passed) |
| Build      | Pass (`cargo build -p crosshook-native`)                                     |

Additional project-specific check:

| Check                             | Result |
| --------------------------------- | ------ |
| `./scripts/check-host-gateway.sh` | Pass   |
| `cargo fmt --all -- --check`      | Pass   |

## Files Reviewed

- `src/crosshook-native/src-tauri/src/commands/health.rs` (Deleted)
- `src/crosshook-native/src-tauri/src/commands/health/mod.rs` (Added, 23 lines)
- `src/crosshook-native/src-tauri/src/commands/health/types.rs` (Added, 137 lines)
- `src/crosshook-native/src-tauri/src/commands/health/single.rs` (Added, 127 lines)
- `src/crosshook-native/src-tauri/src/commands/health/batch.rs` (Added, 142 lines)
- `src/crosshook-native/src-tauri/src/commands/health/enrich.rs` (Added, 144 lines)
- `src/crosshook-native/src-tauri/src/commands/health/prefetch.rs` (Added, 132 lines)
- `src/crosshook-native/src-tauri/src/commands/health/snapshots.rs` (Added, 35 lines)
- `src/crosshook-native/src-tauri/src/commands/health/steam.rs` (Added, 102 lines)

## Verification Notes

- Diffed `main:src/crosshook-native/src-tauri/src/commands/health.rs` against the concatenation of the 8 new files — all logic, control flow, error handling, fail-soft branches, SQL key ordering, and tracing calls are preserved verbatim. Only textual differences are (1) the three dropped comments, (2) `enrich_profile` signature taking `mut report` directly instead of rebinding, (3) cross-module visibility adjustments (`pub(super)`), and (4) the two fully-qualified call sites flagged above.
- Confirmed all four `#[tauri::command]` entry points (`batch_validate_profiles`, `get_profile_health`, `get_cached_health_snapshots`, `get_cached_offline_readiness_snapshots`) and the internal `build_enriched_health_summary` are re-exported from `health/mod.rs` and unchanged at the call sites in `src-tauri/src/lib.rs:257,455-458`.
- No new persisted data introduced; no SQLite/TOML/runtime boundary changes — issue acceptance criteria satisfied.
- Largest new file is `enrich.rs` at 144 lines — well under the 500-line soft cap.
