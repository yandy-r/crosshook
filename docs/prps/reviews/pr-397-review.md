# PR Review #397 — refactor: LaunchOptimizationsPanel into smaller modules

**Reviewed**: 2026-04-20
**Mode**: PR
**Author**: app/openai-code-agent
**Branch**: codex/refactor-split-launch-optimizations-panel → main
**Decision**: APPROVE

## Summary

Faithful, behavior-preserving split of `LaunchOptimizationsPanel.tsx` (728 → 374 lines) into a thin container plus a dedicated `OptionGroup.tsx` (349 lines) and `utils.ts` (24 lines). All three files land comfortably under the 500-line soft cap, public surface is preserved (default export + `LaunchOptimizationsPanelStatus` re-export), typecheck/test/build/host-gateway/Rust tests all pass.

## Findings

### CRITICAL

_None._

### HIGH

_None._

### MEDIUM

_None._

### LOW

- **[F001]** `src/crosshook-native/src/components/launch-optimizations/OptionGroup.tsx:14-100` — `OptionGroup.tsx` bundles the rendering component together with seven small presentation helpers (`getConflictLabels`, `getMainCaveat`, `formatConflictLabels`, `formatConflictSummary`, `capabilityIdForRequiredBinary`, `toolIdForRequiredBinary`, `getGpuVendorLabel`). At 349 lines the file is still well below the 500-line soft cap, but extracting these pure helpers into a sibling `launch-optimizations/helpers.ts` (mirroring the split already done for `utils.ts`) would give the component file a single rendering responsibility and make the helpers trivially unit-testable. Optional.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Move the seven pure helpers out of `OptionGroup.tsx` into `src/crosshook-native/src/components/launch-optimizations/helpers.ts` and re-import them. No behavior change.

- **[F002]** `src/crosshook-native/src/components/launch-optimizations/utils.ts:1-24` — The three pure utilities (`joinClasses`, `formatCountLabel`, `groupOptions`) and the `GroupedOptions`/`CapabilityId` types are now reusable module-level exports but have no unit tests. CLAUDE.md asks for "tests alongside changes"; this PR is a pure refactor so the status-quo coverage is preserved, but extracting these into a proper module is a natural moment to add a small `utils.test.ts` covering `groupOptions` ordering, empty-group filtering, and `formatCountLabel` pluralization.
  - **Status**: Open
  - **Category**: Completeness
  - **Suggested fix**: Add `src/crosshook-native/src/components/launch-optimizations/__tests__/utils.test.ts` with a couple of table-driven cases for each helper. Non-blocking.

## Validation Results

| Check      | Result                                                                                                                                                       |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Type check | Pass                                                                                                                                                         |
| Lint       | Pass (2 pre-existing warnings in unrelated files: `src/hooks/useAccessibilityEnhancements.ts`, `src/lib/__tests__/runtime.test.ts` — not touched by this PR) |
| Tests      | Pass (Vitest: 9 files, 36 tests; Cargo: `crosshook-core` 4/4)                                                                                                |
| Build      | Pass (`vite build` succeeded; host-gateway check passed)                                                                                                     |

## Files Reviewed

- `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx` (Modified, 728 → 374 lines)
- `src/crosshook-native/src/components/launch-optimizations/OptionGroup.tsx` (Added, 349 lines)
- `src/crosshook-native/src/components/launch-optimizations/utils.ts` (Added, 24 lines)

## Reviewer notes

- **Acceptance criteria met**: public API unchanged (default export `LaunchOptimizationsPanel` + named re-export `LaunchOptimizationsPanelStatus`), every resulting file ≤ 500 lines, `cargo test -p crosshook-core` green, `./scripts/lint.sh` clean for the touched files, Vitest/build green.
- **External consumer audit**: Only `src/crosshook-native/src/components/LaunchSubTabs.tsx` imports `LaunchOptimizationsPanel` (default) and `LaunchOptimizationsPanelStatus` — both still resolve.
- **Helper parity check**: Every helper/type previously internal to the 728-line file is accounted for — `joinClasses`, `formatCountLabel`, `groupOptions`, `GroupedOptions`, `CapabilityId` → exported from `utils.ts`; `getConflictLabels`, `getMainCaveat`, `formatConflictLabels`, `formatConflictSummary`, `capabilityIdForRequiredBinary`, `toolIdForRequiredBinary`, `getGpuVendorLabel`, `OptionGroup` → private to `OptionGroup.tsx`.
- **Small quality-of-life improvements carried over**: props type hoisted to a named `OptionGroupProps` interface with direct signature destructuring (was `function OptionGroup(props: {...}) { const { ... } = props; }`); `commonGroups`/`advancedGroups` now carry an explicit `GroupedOptions[]` annotation. Both are improvements, not regressions.
- **Link to umbrella**: PR description links back to umbrella issue #290 as required.
