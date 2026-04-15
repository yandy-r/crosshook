# PR Review #266 — feat(launch): enable umu-launcher by default for non-Steam launches (Phase 4)

**Reviewed**: 2026-04-15T13:32:18-04:00
**Mode**: PR
**Author**: yandy-r
**Branch**: feat/umu-migration-phase-4-auto-default → main
**Decision**: REQUEST CHANGES

## Summary

Validation passed, but the exported-launcher Phase 4 path is not runtime-equivalent to the in-app umu launch path. Generated trainer scripts can ignore an explicit Proton compatibility opt-out and, when they do take the umu branch, they omit the env contract that the real umu launch builders require.

## Findings

### CRITICAL

### HIGH

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs:537` — Exported launcher generation now always prefers `umu-run` when it is present on the target host, but the export request model still carries no `umu_preference`. A profile or global setting pinned to `UmuPreference::Proton` will therefore export a script that silently flips back to `umu-run`, which breaks the documented “Proton is the compatibility escape hatch” contract.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Thread the effective `umu_preference` through `SteamExternalLauncherExportRequest` and make `build_exec_line()` honor it: `Proton` should emit direct-Proton exec only, while `Auto` and `Umu` can keep the umu branch logic.

- **[F002]** `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs:557` — The new exported `umu-run` branch does not set the env contract that the real umu builders rely on (`GAMEID`, `PROTON_VERB`, `PROTONPATH`). In-app umu launches populate those before execing umu, but the generated script does not, so exported launchers can resolve the wrong Proton or lose the intended trainer `runinprefix` behavior.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Emit the same umu-specific env setup into the generated script before the `umu-run` branch, and extend the export request with any additional fields needed to compute it correctly.

### MEDIUM

### LOW

## Validation Results

| Check      | Result |
| ---------- | ------ |
| Type check | Pass   |
| Lint       | Pass   |
| Tests      | Pass   |
| Build      | Pass   |

## Files Reviewed

- `docs/prps/plans/completed/umu-migration-phase-4-auto-default.plan.md` (Added)
- `docs/prps/prds/umu-launcher-migration.prd.md` (Modified)
- `docs/prps/reports/umu-migration-phase-4-auto-default-report.md` (Modified)
- `src/crosshook-native/Cargo.lock` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` (Modified)
- `src/crosshook-native/src/components/SettingsPanel.tsx` (Modified)
- `src/crosshook-native/src/types/settings.ts` (Modified)
