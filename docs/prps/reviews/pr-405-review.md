# PR Review #405 — refactor: onboarding.ts into smaller modules

**Reviewed**: 2026-04-20
**Mode**: PR
**Author**: app/openai-code-agent
**Branch**: codex/refactor-split-onboarding-ts → main
**Decision**: APPROVE

## Worktree Setup

- **Parent**: ~/.claude-worktrees/crosshook-pr-405/ (branch: codex/refactor-split-onboarding-ts)
- **Children** (per severity; created by /ycc:review-fix --worktree):
  - MEDIUM → ~/.claude-worktrees/crosshook-pr-405-medium/ (branch: feat/pr-405-medium)
  - LOW → ~/.claude-worktrees/crosshook-pr-405-low/ (branch: feat/pr-405-low)

## Summary

Clean, behavior-preserving split of a 678-line mock handler into five small, domain-coherent modules (constants, state, events, readiness, trainer) plus a thin barrel. All handlers, side-effects (eager module-init synthesis), and the reset contract are preserved. Type-check, Biome, `cargo test -p crosshook-core`, and Vitest all pass. The only notable gap is that several non-obvious rationale comments in the original file were dropped during the split.

## Findings

### CRITICAL

_None._

### HIGH

_None._

### MEDIUM

- **[F001]** `src/crosshook-native/src/lib/mocks/handlers/onboarding-events.ts:35` — Refactor lost the "why" comments documenting the onboarding-event synthesis semantics: the original `onboarding.ts:17-26` explained that the 500 ms initial delay exists so `App.tsx` has mounted and called `subscribeEvent()` before the event fans out, that the guard prevents HMR re-imports from re-firing, and that `onboardingSynthesisScheduled` prevents duplicate retry loops when `registerOnboarding()` is invoked after module init already scheduled one. Original `onboarding.ts:559-562` also explained why the eager `maybeSynthesizeOnboardingEvent()` call at module init is required. Without these, a future reader could easily delete the guard, shorten the delay, or remove the eager call and reintroduce the race with `App.tsx` or an HMR double-emit.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Port the two comment blocks from the original file. A good home is `onboarding-events.ts` above `maybeSynthesizeOnboardingEvent` (for the HMR-guard + 500 ms rationale) and above the eager call site in `onboarding.ts:24` (for why synthesis must run at module init, not just from `registerOnboarding`). A one-line `why` comment on `onboardingSynthesisScheduled` in `onboarding-state.ts:5` also helps.

### LOW

- **[F002]** `src/crosshook-native/src/lib/mocks/handlers/onboarding-state.ts:3-7` — Inconsistent state-access idioms. `onboardingDismissed`, `onboardingEventSynthesized`, and `onboardingSynthesisScheduled` are exposed as `export let` (relying on ES-module live bindings from consumers — correct but easy to misread), while `cachedHostReadinessSnapshot` is exposed through `get`/`set` helpers. Mixing the two styles in one module makes it non-obvious which reads are live bindings and which are function calls.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Pick one idiom. Either (a) add `getOnboardingEventSynthesized()` / `getOnboardingSynthesisScheduled()` / `getOnboardingDismissed()` and drop the `export let` re-exports of those variables, so all state reads are explicit getter calls; or (b) expose the snapshot as `export let cachedHostReadinessSnapshot` too and drop its getter. Option (a) is safer because it avoids relying on ESM live-binding semantics across three modules.

- **[F003]** `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts:93` — `export { onboardingDismissed }` is preserved for contract parity, but nothing in the repo imports it (confirmed via `rg onboardingDismissed` — only the state module, the barrel, and this export itself). Carrying an unused live-binding re-export through the barrel costs complexity for no consumer.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Either drop the re-export (and the corresponding `import { onboardingDismissed }` on line 18) as a follow-up cleanup, or add a short comment noting it is part of the documented mock surface even if currently unused. Out of scope for this refactor PR — flag for a follow-up.

## Validation Results

| Check      | Result                                                                                                                                                                |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Type check | Pass                                                                                                                                                                  |
| Lint       | Pass (2 pre-existing Biome warnings in unrelated files: `src/hooks/useAccessibilityEnhancements.ts`, `src/lib/__tests__/runtime.test.ts` — not introduced by this PR) |
| Tests      | Pass (Vitest 36/36; `cargo test -p crosshook-core` 4/4)                                                                                                               |
| Build      | Skipped (cargo check ran via `./scripts/lint.sh`; full AppImage build not exercised — not in scope for a mock-handler refactor)                                       |

## Files Reviewed

- `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts` (Modified — now 93 lines, was 678; barrel)
- `src/crosshook-native/src/lib/mocks/handlers/onboarding-constants.ts` (Added — 221 lines; capability + host-tool fixtures, DTO types, timing constants)
- `src/crosshook-native/src/lib/mocks/handlers/onboarding-events.ts` (Added — 57 lines; `maybeSynthesizeOnboardingEvent`)
- `src/crosshook-native/src/lib/mocks/handlers/onboarding-readiness.ts` (Added — 293 lines; readiness/capability builders, arg parsers, sanitizer)
- `src/crosshook-native/src/lib/mocks/handlers/onboarding-state.ts` (Added — 62 lines; module-local state + reset)
- `src/crosshook-native/src/lib/mocks/handlers/onboarding-trainer.ts` (Added — 45 lines; trainer-guidance payload)

All six files are well under the 500-line soft cap (largest is `onboarding-readiness.ts` at 293). No host-gateway surface is touched; ADR-0001 is unaffected.

## Parity Checks Performed

- All 10 handler names re-registered (`check_readiness`, `check_generalized_readiness`, `probe_host_tool_details`, `get_cached_host_readiness_snapshot`, `get_capabilities`, `dismiss_onboarding`, `dismiss_umu_install_nag`, `dismiss_steam_deck_caveats`, `dismiss_readiness_nag`, `get_trainer_guidance`) — match original.
- `resetOnboardingMockState` → `resetOnboardingState` resets all four state vars and cancels timers — matches original `resetOnboardingMockState`.
- Eager `maybeSynthesizeOnboardingEvent()` call at module init preserved (`onboarding.ts:24`).
- `registerOnboarding` calls `maybeSynthesizeOnboardingEvent()` once — matches original.
- External consumer surface: only `src/lib/mocks/index.ts` imports `registerOnboarding` + `resetOnboardingMockState`; both are still exported from the barrel.
- `MockCapabilityDefinition.category` widened typing from `string` → `Capability['category']` (which resolves to `string`) — intent-revealing, no runtime change.
