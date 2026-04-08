# PR Review: #176 — feat(frontend): extract IPC into hooks for launch dep gate and profile verify

**Reviewed**: 2026-04-08
**Author**: yandy-r
**Branch**: `feat/issue-174-hook-extraction` → `main`
**Head SHA**: `43a30d2905843fc0bc27c10dd3b6b55a89163ebe`
**Decision**: **APPROVE** (operator override: `--approve`)
**Closes**: #174

## Summary

Clean, narrowly-scoped refactor that restores the IPC-agnostic component invariant called out in `.cursorrules` / `AGENTS.md`. `LaunchPage` and `ProfileActions` no longer import `callCommand`; all prefix-dependency gate, Gamescope probe, and acknowledge-version-change IPC now lives behind two hooks. Error semantics are preserved via a structured `AcknowledgeVersionChangeOutcome` discriminated union, and both `LaunchPanel` and `ProfileActions` were updated atomically so the broader contract change is self-consistent. No behavioral drift observed; static checks clean.

## Findings

### CRITICAL

None.

### HIGH

None.

### MEDIUM

None.

### LOW

**LOW-1 — `useLaunchPrefixDependencyGate.checkGamescope` errors are swallowed silently** (`src/crosshook-native/src/hooks/useLaunchPrefixDependencyGate.ts:17-24`)

The `try { ... } catch {}` block intentionally preserves the prior `isGamescopeRunning` value on IPC failure, which matches the original call-site behavior (`.catch(() => {})`) exactly, so this is not a regression. However, CLAUDE.md's "throw errors early and often" rule would normally favor surfacing the error (or at least `console.warn`) so misconfigurations are visible during development. Non-blocking; preserved parity with the pre-PR behavior and is documented by the inline comment.

**Addressed**: IPC failures now log `console.warn('check_gamescope_session failed; …', error)` while still preserving prior session state.

**LOW-2 — Hook returns `checkGamescope` but no consumer invokes it** (`src/crosshook-native/src/hooks/useLaunchPrefixDependencyGate.ts:11,48`)

`checkGamescope` is exposed on `UseLaunchPrefixDependencyGateResult` but the only call is the `useEffect` inside the hook itself. Exporting it keeps the hook flexible (e.g. a future manual-refresh button), but it is currently dead weight from the consumer's perspective. Fine to keep as forward-looking API; delete if you prefer strict "build-what's-needed" minimalism.

**Addressed**: `checkGamescope` removed from the public hook result; the probe runs only inside the hook’s `useEffect`.

**LOW-3 — `handleMarkVerified` short-circuit no longer resets busy state (correctness note)** (`src/crosshook-native/src/components/ProfileActions.tsx:97-111`)

The early-return guard `if (!selectedProfile.trim()) return;` runs *before* `acknowledgeVersionChange`, so busy state is never flipped — correct. I only call it out because it's a subtle behavioral addition not present in the pre-PR component (where there was no empty-name guard at all). Worth confirming this doesn't mask a UX edge case where the button was previously clickable against an empty selection. Given `showMarkVerified` requires `versionStatus != null` (which itself requires a resolved profile entry), this is effectively unreachable — safe.

**Addressed**: Inline comment documents that `showMarkVerified` makes the empty-name path effectively unreachable.

**LOW-4 — Two components now carry near-identical outcome-handling blocks** (`src/crosshook-native/src/components/LaunchPanel.tsx:639-654` and `src/crosshook-native/src/components/ProfileActions.tsx:97-111`)

`handleMarkAsVerified` / `handleMarkVerified` duplicate the same 12-line outcome switch (busy → return, acknowledge stage → alert, revalidate stage → alert). If a third consumer appears, promote this to a `handleAcknowledgeOutcome(outcome)` helper co-located with the hook. Premature to extract for only two call-sites — flagging for future maintainers.

**Addressed**: Shared `presentAcknowledgeVersionChangeOutcome` exported from `useAcknowledgeVersionChange.ts` and used by both components.

## Validation Results

| Check               | Result           | Command                                                                                          |
| ------------------- | ---------------- | ------------------------------------------------------------------------------------------------ |
| Type check          | Pass             | `cd src/crosshook-native && npx tsc --noEmit` (exit 0)                                           |
| Lint                | Skipped          | No lint script defined in `package.json`                                                         |
| Frontend unit tests | Skipped          | No frontend unit test framework configured (plan-documented; smoke-only via Playwright)          |
| Rust unit tests     | Pass             | `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` → 748 + 3 passing |
| Build               | Implicit via tsc | `npm run build` = `tsc && vite build`; tsc portion verified clean                                |
| Playwright smoke    | Not run          | Author's report notes Chromium missing locally; non-blocking per PR notes                        |

Scope guards (from plan acceptance criteria):

```text
rg 'callCommand' src/crosshook-native/src/components/pages/LaunchPage.tsx   → 0 matches
rg 'callCommand' src/crosshook-native/src/components/ProfileActions.tsx      → 0 matches
```

Both component files are now fully IPC-agnostic — acceptance criteria met.

## Category Checklist

| Category               | Status | Notes                                                                                                              |
| ---------------------- | ------ | ------------------------------------------------------------------------------------------------------------------ |
| **Correctness**        | Pass   | Busy-guard preserved via `busyRef`; dep-gate state machine (`depGateInstalling` / event listener) unchanged; `handleBeforeLaunch` early-returns on missing prefix/packages preserved; install-failure catches reset all three dep-gate state slots. |
| **Type Safety**        | Pass   | New discriminated union `AcknowledgeVersionChangeOutcome` is exhaustive; no `any`; hook return type is a named interface; `PrefixDependencyStatus` import moved cleanly into the hook. |
| **Pattern Compliance** | Pass   | Hook naming (`use*`) + `UseXxxResult` interface mirrors `usePrefixDeps`; error-normalization style matches existing hooks; component error-alert style matches the prior inline implementation; `@/lib/ipc` import alias consistent. |
| **Security**           | Pass   | No new IPC commands; no user-supplied strings interpolated into shell; no new auth/secret surface.                 |
| **Performance**        | Pass   | `useCallback`-memoized dep-gate actions; Gamescope probe runs once on mount via `useEffect`; dep-gate effect's prefix-path computation moved into the effect body (re-derived per event, still O(1)). |
| **Completeness**       | Pass   | Atomic update of `LaunchPanel` alongside `useAcknowledgeVersionChange` prevents hook contract drift; plan + report committed under `.claude/PRPs/`; `Closes #174` present in body. |
| **Maintainability**    | Pass   | Single-responsibility hooks; `presentAcknowledgeVersionChangeOutcome` centralizes user-facing handling; Gamescope IPC failure logs a dev warning. |

## Files Reviewed

| File                                                                              | Change     | Notes                                                                                                                   |
| --------------------------------------------------------------------------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/hooks/useLaunchPrefixDependencyGate.ts`                 | Added      | New hook encapsulating `get_dependency_status`, `install_prefix_dependency`, `check_gamescope_session`.                |
| `src/crosshook-native/src/hooks/useAcknowledgeVersionChange.ts`                   | Modified   | Introduces `AcknowledgeVersionChangeOutcome`; replaces silent catch with structured result; busy-guard unchanged.      |
| `src/crosshook-native/src/components/pages/LaunchPage.tsx`                        | Modified   | Removes `callCommand` + `PrefixDependencyStatus` imports; all three IPC call-sites routed via hook; `isGamescopeRunning` now sourced from hook state. |
| `src/crosshook-native/src/components/ProfileActions.tsx`                          | Modified   | Removes direct `callCommand`/`useState`; `busy` is now sourced from hook; added empty-profile-name guard (effectively unreachable given `showMarkVerified`). |
| `src/crosshook-native/src/components/LaunchPanel.tsx`                             | Modified   | Adopts new outcome contract; alert/revalidate branches surface errors that were previously silenced (behavioral *improvement*). |
| `.claude/PRPs/plans/completed/issue-174-hook-extraction.plan.md`                  | Added      | Implementation plan; archived into `completed/`.                                                                        |
| `.claude/PRPs/reports/issue-174-hook-extraction-report.md`                        | Added      | Post-implementation report — transparently flags smoke-test gap for reviewer follow-up.                                |

## Notable Observations

- **Behavioral improvement in `LaunchPanel`**: the previous `useAcknowledgeVersionChange` swallowed all errors with a bare `catch {}`. The new discriminated-outcome shape causes `LaunchPanel.handleMarkAsVerified` to surface acknowledge/revalidate failures to the user via `window.alert`, matching the semantics `ProfileActions` already had. This is a low-key UX upgrade, not a regression.
- **Atomic migration** of both `LaunchPanel` and `ProfileActions` alongside the hook signature change means no mixed-contract risk on `main` — good refactor hygiene.
- **Scope discipline**: Gamescope session probe was *not* in the original issue #174 scope but was pulled into the hook here; the PR title and report note this explicitly, and it's a logical fit since `check_gamescope_session` was the only remaining direct `callCommand` in `LaunchPage`.

## Decision Rationale

Zero CRITICAL, zero HIGH. LOW items are either intentional-parity choices or code-smell notes for future iteration. Static validation passes. Plan acceptance criteria met (both component files scanned clean). Operator supplied `--approve` and the findings support that decision independently.

**Suggested follow-ups (non-blocking)**:

1. Before merge: run `./scripts/dev-native.sh` (native) to exercise the dep-gate + Gamescope probe paths end-to-end, per the author's report Next Steps.
2. If `npx playwright install` is available in CI, re-enable smoke in a follow-up.
3. LOW-4 outcome helper: implemented as `presentAcknowledgeVersionChangeOutcome` in `useAcknowledgeVersionChange.ts`.
