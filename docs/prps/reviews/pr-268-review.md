# PR Review #268 — feat(onboarding): Steam Deck caveats, watchdog exe-name fallback, Flathub resolution (Phase 5b)

**Reviewed**: 2026-04-15T21:39:42-04:00
**Mode**: PR
**Author**: yandy-r
**Branch**: feat/umu-phase-5b-issue-followups → main
**Decision**: REQUEST CHANGES

## Summary

Validation passed, but the new Flatpak gamescope watchdog fallback does not actually recover the missing-capture-file case it is meant to fix. The PR also records Faugus-related implementation details in the Phase 5b report that are not present in the shipped code, which makes the PRP artifacts misleading for future work.

## Findings

### CRITICAL

### HIGH

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/launch/watchdog.rs:162` — The new exe-name fallback seeds `resolve_watchdog_target_by_exe_name()` with `observed_gamescope_pid`, but in the Flatpak path that value comes from `child.id()` on the spawned `flatpak-spawn --host ...` wrapper, not from the real host-side gamescope process. The runtime helper already documents why it writes the host PID capture file (`runtime_helpers.rs:299-300`), so when that file is missing the fallback now walks the host `ps` tree from the wrong root and returns no descendants. In practice, issue `#244` still stands down in the exact Flatpak failure mode this PR claims to fix.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Plumb a real host-root PID into the fallback path instead of reusing `child.id()`. For example, persist the host shell/gamescope PID before spawn returns and use that as the host descendant-walk root, or restructure the fallback so it can discover the host compositor PID without depending on the sandbox child PID namespace.

### MEDIUM

- **[F002]** `docs/prps/reports/umu-migration-phase-5b-issue-followups-report.md:5` — The Phase 5b implementation report says this PR added a Faugus Launcher install pointer in `build_umu_install_advice()` and a `~/.var/app/io.github.Faugus.faugus-launcher/.../umu-run` probe in `probe_flatpak_host_umu_candidates()`, but neither code path exists in the reviewed Rust sources. Because this repo uses `docs/prps/` artifacts as planning and review inputs, that false implementation history is a maintainability bug rather than harmless prose drift.
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Either land the Faugus guidance/probe behavior in the Rust implementation, or rewrite the Phase 5b report so it describes only the behavior that actually shipped.

### LOW

## Validation Results

| Check      | Result |
| ---------- | ------ |
| Type check | Pass   |
| Lint       | Pass   |
| Tests      | Pass   |
| Build      | Pass   |

## Files Reviewed

- `docs/prps/plans/completed/umu-migration-phase-5b-issue-followups.plan.md` (Renamed)
- `docs/prps/prds/umu-launcher-migration.prd.md` (Modified)
- `docs/prps/reports/umu-migration-phase-5b-issue-followups-report.md` (Added)
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/watchdog.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/platform.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/onboarding.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/settings.rs` (Modified)
- `src/crosshook-native/src-tauri/src/lib.rs` (Modified)
- `src/crosshook-native/src/components/OnboardingWizard.tsx` (Modified)
- `src/crosshook-native/src/components/wizard/WizardReviewSummary.tsx` (Modified)
- `src/crosshook-native/src/hooks/useOnboarding.ts` (Modified)
- `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts` (Modified)
- `src/crosshook-native/src/lib/toggles.ts` (Modified)
- `src/crosshook-native/src/types/onboarding.ts` (Modified)
- `src/crosshook-native/src/types/settings.ts` (Modified)
