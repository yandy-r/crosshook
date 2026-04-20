# PR Review #392 — refactor(export): split diagnostics module

**Reviewed**: 2026-04-19T22:30:00-04:00
**Mode**: PR
**Author**: app/openai-code-agent
**Branch**: codex/refactor-diagnostics-into-modules → main
**Head SHA**: f49a28a11c45a10f5c7dd19258e8b731f3122fed
**Decision**: APPROVE

## Summary

This is a clean, line-preserving refactor of the 793-line `export/diagnostics.rs` into a `diagnostics/` directory with six cohesive submodules plus `mod.rs` (347 lines). Behavior is byte-identical — each collector function was moved verbatim (only the `fn` visibility changed to `pub(super)`), tests were distributed to live alongside the code they exercise, and the crate-root re-exports in `export/mod.rs` continue to expose `DiagnosticBundleOptions`, `DiagnosticBundleResult`, `DiagnosticBundleSummary`, `DiagnosticBundleError`, and `export_diagnostic_bundle` at the same path. External consumers in `crosshook-cli` and `src-tauri` resolve without changes. All 12 `export::diagnostics::*` tests pass, full `-p crosshook-core` suite passes (1097/1097), `cargo clippy --all-targets -- -D warnings` is clean, `cargo fmt --check` is clean, and `./scripts/check-host-gateway.sh` passes. Every new file is well under the 500-line soft cap.

## Findings

### CRITICAL

_None._

### HIGH

_None._

### MEDIUM

_None._

### LOW

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/export/diagnostics/mod.rs:72` — `DiagnosticBundleError::ProfileStore(String)` is still defined but never constructed anywhere in the crate; the only assertion in `diagnostic_bundle_error_display_formats_correctly` keeps it from tripping dead-code warnings. This is pre-existing (the variant was already unused on `main`), so not a PR blocker, but the refactor is a natural moment to either wire it up (the `ProfileStore::list()` branch at `profiles.rs:10` currently swallows the error) or delete the variant and its display/error impl arm. Leaving the variant in place keeps a dead discriminant in the public-facing error type.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Either (a) map `store.list()` errors in `collect_profiles` to `DiagnosticBundleError::ProfileStore(err.to_string())` and bubble up, or (b) remove the variant and its `Display`/`Error::source` arms plus the test assertion that constructs it. Scope this to a follow-up PR to keep this refactor strictly mechanical.

- **[F002]** `src/crosshook-native/crates/crosshook-core/src/export/diagnostics/health.rs:1-8` and `steam_diagnostics.rs:1-43` — Both submodules have zero direct tests. The original single-file layout had no dedicated tests for `collect_health_summary` or `collect_steam_diagnostics` either (they were covered only transitively via `export_diagnostic_bundle_produces_valid_archive`), so this is also pre-existing. With the code now in its own file, adding a two-line smoke test (`collect_health_summary` on an empty `ProfileStore`, `collect_steam_diagnostics` on a machine with no Steam) would be cheap and make the module boundaries observable.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Add minimal smoke tests in a follow-up. Not worth holding this PR.

### Positive observations

- Acceptance criteria from the parent issue are all met: public API preserved (`pub use` list in `export/mod.rs:7-10` unchanged by the diff), every file ≤ 500 lines (largest is `mod.rs` at 347), `cargo test -p crosshook-core` and `./scripts/lint.sh`-equivalents pass, and the PR links to umbrella #290 via "Part of" conventions.
- Submodule boundaries are clean and single-responsibility: `system_info` (proc/GPU), `crosshook_info` (app metadata), `profiles` (TOML dump + redaction), `logs` (rotated + launch tails), `steam_diagnostics` (Steam/Proton discovery), `health` (profile health JSON). `mod.rs` keeps only the orchestrator, the error type, the tar helpers, and `redact_home_paths` (shared across `crosshook_info` and `profiles` via `super::redact_home_paths`).
- Visibility discipline is correct: collector fns are `pub(super)` rather than `pub`, so they cannot be accessed outside the `diagnostics` module — the narrow public surface is preserved.
- Constants and `#[allow(clippy::vec_init_then_push)]` were moved with their owning functions, not floated into `mod.rs`.
- Tests were moved alongside their code, not left in `mod.rs`, which is the right call for maintainability.

## Validation Results

| Check                                                    | Result | Notes                                                                                                                                                                                                                                                  |
| -------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `cargo fmt --check`                                      | Pass   |                                                                                                                                                                                                                                                        |
| `cargo clippy -D warnings` (crosshook-core, all-targets) | Pass   |                                                                                                                                                                                                                                                        |
| `cargo test -p crosshook-core` (full suite)              | Pass   | 1097/1097. One flake seen on first run in `collect_profiles_with_redaction_replaces_home_paths`; 3 subsequent runs all green. The test body is byte-identical to `main` (pre-existing `HOME` env-var race across parallel tests), not a PR regression. |
| `cargo test -p crosshook-core --lib export::diagnostics` | Pass   | 12/12.                                                                                                                                                                                                                                                 |
| `./scripts/check-host-gateway.sh`                        | Pass   | ADR-0001 gateway contract preserved (`system_info.rs` keeps the `platform::is_flatpak()` → `platform::host_std_command("lspci")` branch).                                                                                                              |

## Files Reviewed

- `src/crosshook-native/crates/crosshook-core/src/export/diagnostics.rs` (Deleted, 793 lines)
- `src/crosshook-native/crates/crosshook-core/src/export/diagnostics/mod.rs` (Added, 347 lines)
- `src/crosshook-native/crates/crosshook-core/src/export/diagnostics/crosshook_info.rs` (Added, 52 lines)
- `src/crosshook-native/crates/crosshook-core/src/export/diagnostics/health.rs` (Added, 8 lines)
- `src/crosshook-native/crates/crosshook-core/src/export/diagnostics/logs.rs` (Added, 165 lines)
- `src/crosshook-native/crates/crosshook-core/src/export/diagnostics/profiles.rs` (Added, 94 lines)
- `src/crosshook-native/crates/crosshook-core/src/export/diagnostics/steam_diagnostics.rs` (Added, 43 lines)
- `src/crosshook-native/crates/crosshook-core/src/export/diagnostics/system_info.rs` (Added, 121 lines)
