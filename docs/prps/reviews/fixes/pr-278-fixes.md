# Fix Report: pr-278-review

**Source**: `docs/prps/reviews/pr-278-review.md`
**Applied**: 2026-04-17T04:00:13+00:00
**Mode**: Parallel sub-agents (2 batches, max width 6)
**Severity threshold**: LOW

## Summary

- **Total findings in source**: 18
- **Already processed before this run**:
  - Fixed: 4
  - Failed: 0
- **Eligible this run**: 14
- **Applied this run**:
  - Fixed: 14
  - Failed: 0
- **Skipped this run**:
  - Below severity threshold: 0
  - No suggested fix: 0
  - Missing file: 0

## Fixes Applied

| ID   | Severity | File                                                                     | Line | Status | Notes                                                                                                          |
| ---- | -------- | ------------------------------------------------------------------------ | ---- | ------ | -------------------------------------------------------------------------------------------------------------- |
| F005 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs` | 1    | Fixed  | Extracted loader/map concerns into `capability_loader.rs`; `capability.rs` now focused on capability models/derivation. |
| F006 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs` | 390  | Fixed  | Reused cached `umu_run.resolved_path` before falling back to live `resolve_umu_run_path()` probe.            |
| F007 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs` | 405  | Fixed  | Stopped mapping guidance description into install alternatives (`alternatives` now empty unless explicitly provided). |
| F008 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/onboarding/details.rs`    | 202  | Fixed  | Capped child pipe reads with `take(VERSION_OUTPUT_CAP)` to prevent unbounded buffering.                       |
| F009 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`  | 1    | Fixed  | Split distro-detection and install-advice helpers into dedicated modules; reduced `readiness.rs` size.        |
| F010 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`  | 171  | Fixed  | Removed hardcoded per-distro fallback command matrix in favor of catalog-authoritative behavior + generic fallback. |
| F011 | MEDIUM   | `src/crosshook-native/src/components/host-readiness/HostToolCard.tsx`     | 153  | Fixed  | Removed production `console.error` logging from action error paths.                                            |
| F012 | MEDIUM   | `src/crosshook-native/src/hooks/useHostReadiness.ts`                      | 125  | Fixed  | Committed live snapshot before optional cache enrichment so cache IPC failures cannot discard fresh data.      |
| F013 | MEDIUM   | `src/crosshook-native/src/hooks/useLaunchPrefixDependencyGate.ts`         | 29   | Fixed  | Removed non-actionable `console.warn` in recoverable failure path.                                             |
| F014 | LOW      | `src/crosshook-native/crates/crosshook-core/src/onboarding/details.rs`    | 143  | Fixed  | Shared bounded pipe-drain fix with F008 covers defense-in-depth EOF/drain concern.                            |
| F015 | LOW      | `src/crosshook-native/src/components/host-readiness/HostToolCard.tsx`     | 253  | Fixed  | Added `disabled={isProbingDetails}` to the Details button while probes are running.                           |
| F016 | LOW      | `src/crosshook-native/src/components/host-readiness/HostToolDashboardHandoff.tsx` | 12   | Fixed  | Replaced two inline styles with BEM classes backed by `host-tool-dashboard.css`.                              |
| F017 | LOW      | `src/crosshook-native/src/components/host-readiness/HostToolMetricsHero.tsx` | 17   | Fixed  | Replaced index-based skeleton keys with explicit stable skeleton elements.                                     |
| F018 | LOW      | `src/crosshook-native/src/hooks/useHostReadiness.ts`                      | 7    | Fixed  | Hook now uses scoped `useRef` bootstrap flag (module-level persistent mutable flag no longer in use).         |

## Files Changed

- `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs` (Fixed F005, F006, F007)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/capability_loader.rs` (Fixed F005)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/details.rs` (Fixed F008, F014)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/distro.rs` (Fixed F009)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/install_advice.rs` (Fixed F009, F010)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/mod.rs` (Fixed F005, F009)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs` (Fixed F009, F010)
- `src/crosshook-native/src/components/host-readiness/HostToolCard.tsx` (Fixed F011, F015)
- `src/crosshook-native/src/components/host-readiness/HostToolDashboardHandoff.tsx` (Fixed F016)
- `src/crosshook-native/src/components/host-readiness/HostToolMetricsHero.tsx` (Fixed F017)
- `src/crosshook-native/src/hooks/useHostReadiness.ts` (Fixed F012, F018)
- `src/crosshook-native/src/hooks/useLaunchPrefixDependencyGate.ts` (Fixed F013)
- `src/crosshook-native/src/styles/host-tool-dashboard.css` (Fixed F016)

## Failed Fixes

None.

## Validation Results

| Check      | Result | Notes |
| ---------- | ------ | ----- |
| Type check | Pass   | `npx tsc --noEmit` |
| Tests      | Fail   | `npm test` is not defined in `src/crosshook-native/package.json` (`Missing script: \"test\"`). |
| Rust tests | Pass   | `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` |

## Next Steps

- Re-run `/code-review 278` to confirm all findings stay resolved against current branch head
- Add or define a canonical frontend test command if `npm test` should be part of automated validation
- Run `/git-workflow` to commit the fixes when satisfied
