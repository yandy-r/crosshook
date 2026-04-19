# PR Review #353 — test(coverage): Add tests to achieve 60% line coverage on critical surfaces

**Reviewed**: 2026-04-19
**Mode**: PR
**Author**: app/anthropic-code-agent
**Branch**: claude/add-tests-for-critical-surfaces → main
**Head SHA**: 095ee60dce6b6a3f5257d8567a2cb02d0868e53d
**Decision**: REQUEST CHANGES

## Summary

The PR delivers solid, working unit tests for the `src/lib/{ipc,events,runtime}.ts` trio (~96% line coverage on that slice) and wires Vitest coverage config per PRD §4.6. However, it ships a 60% coverage threshold that `vitest run --coverage` cannot satisfy (measured 10%), quietly narrows the PRD-defined critical surface by excluding `src/hooks/{install,profile}/**`, and lands an internal tracking artifact at the repo root where CLAUDE.md forbids it. Acceptance criterion #1 from issue #286 (`vitest run --coverage` passes with the gate enabled) is explicitly not met.

## Findings

### CRITICAL

- **[F001]** `src/crosshook-native/vitest.config.ts:41-46` — Coverage `thresholds.{lines,functions,branches,statements}: 60` is enabled but measured coverage on the configured `include` globs is ~10%. `npm run test:coverage` now fails locally with `ERROR: Coverage for lines (10.18%) does not meet global threshold (60%)` (and the same for functions/statements/branches). Issue #286 acceptance criterion #1 — "`vitest run --coverage` passes with the gate enabled on critical globs" — is not met. `docs/TESTING.md:20` advertises `npm run test:coverage` as a supported workflow; this PR breaks it. CI does not currently invoke coverage, so the red signal is local-only, but shipping an intentionally-failing gate is a regression whether or not CI enforces it yet.
  - **Status**: Open
  - **Category**: Completeness
  - **Suggested fix**: Either (a) deliver enough hook + page tests in this PR to actually reach 60%, or (b) split the work — keep the `src/lib/*` tests here, drop the 60% threshold (or set it to the currently-achievable value) until the hook/page coverage lands, and open a follow-up issue for ratcheting the gate up. The `PHASE4_COVERAGE_TRACKING.md` doc (see F002) already pre-acknowledges the gate fails; that is not an acceptable end state for merge.

### HIGH

- **[F002]** `PHASE4_COVERAGE_TRACKING.md:1` — Internal tracking doc committed at repo root. `CLAUDE.md` → _MUST / MUST NOT_: internal docs live under `docs/plans/`, `docs/research/`, or `docs/internal/` and use the `docs(internal): …` commit prefix. PRD §4.6 explicitly prescribes **"a single uncovered critical surfaces tracking issue with a file checklist"** — a GitHub issue, not a markdown file. The doc's own header admits: _"Automated issue creation blocked by API permissions. Issue body prepared in /tmp/tracking-issue-body.md for manual creation…"_ — confirming it is a placeholder for an issue the author could not create.
  - **Status**: Open
  - **Category**: Pattern Compliance
  - **Suggested fix**: Remove `PHASE4_COVERAGE_TRACKING.md` from the repo root. Open a single GitHub tracking issue (labels: `type:feature`, `area:build`, `priority:medium`, `source:prd`, `feat:frontend-test-framework`, `tracking`, `phase:4`) using the checklist in this document as its body. If a longer in-tree artifact is still wanted, move it to `docs/plans/frontend-test-framework/phase4-coverage-gaps.md` and use a `docs(internal): …` commit.

- **[F003]** `src/crosshook-native/vitest.config.ts:36-40` — Coverage `exclude` list adds `src/hooks/install/**` and `src/hooks/profile/**` with only the inline comment _"Hook utilities/subdirectories"_. PRD §4.6 lists `src/hooks/**` as fully gated, and its non-goals defer **only** `useProfile.ts`. `src/hooks/profile/**` contains first-class hooks — `useProfileCrud.ts` (14 KB), `useProfileHistory.ts`, `useProfileLaunchAutosave.ts` (18 KB), `useProfileLaunchAutosaveEffects.ts` (13 KB) — none of which are sanctioned exemptions in the PRD. This silently narrows the PRD-defined critical surface.
  - **Status**: Open
  - **Category**: Pattern Compliance
  - **Suggested fix**: Remove `src/hooks/install/**` and `src/hooks/profile/**` from the `exclude` list. If genuine deferral is needed (e.g., treating the `useProfile*` cluster as part of the deferred `useProfile` monolith), amend the PRD first or link an issue that records that decision, and reference it in the config comment. The current comment does not justify shrinking the gated surface.

### MEDIUM

- **[F004]** `src/crosshook-native/src/lib/__tests__/events.test.ts:100-117` — `"should delegate to Tauri listen API"` is a no-op test. Its only assertion is `expect(runtime.isTauri()).toBe(true)`, which the `beforeEach` already guarantees. The inline comment — _"we can't easily test the dynamic import without more complex mocking"_ — openly admits the Tauri branch is not actually verified. The test inflates coverage on `events.ts` without proving behavior.
  - **Status**: Open
  - **Category**: Correctness
  - **Suggested fix**: Mirror the `ipc.test.ts` Tauri pattern — isolate the test with `vi.resetModules()`, register `vi.doMock('@/lib/runtime', …)` + `vi.doMock('@tauri-apps/api/event', …)` before `await import('../events')`, call `subscribeEvent(...)` and assert `expect(mockListen).toHaveBeenCalledWith('tauri-test', handler)`. Alternatively, delete the test.

- **[F005]** `src/crosshook-native/src/lib/__tests__/runtime.test.ts:17,30,38,47,64,68` — Six `{} as any` casts on `global.window`. `CLAUDE.md` → _Type Safety_: _"Never use `any`-equivalent escape hatches… without documented justification."_ Biome surfaces all six as `lint/suspicious/noExplicitAny` warnings (`npx @biomejs/biome ci src/` → 7 new warnings introduced by this PR). CI does not fail on Biome warnings, but these are still policy violations.
  - **Status**: Open
  - **Category**: Type Safety
  - **Suggested fix**: Extract a typed helper, e.g. `const asTauriWindow = (w: object): Window & typeof globalThis => w as unknown as Window & typeof globalThis;` and use `global.window = asTauriWindow({ __TAURI_INTERNALS__: {} });`. Or declare a local interface `interface TauriWindow extends Window { __TAURI_INTERNALS__?: unknown }` and cast through that.

- **[F006]** `src/crosshook-native/src/lib/__tests__/runtime.test.ts:1` — `beforeEach` is imported but never used (`lint/correctness/noUnusedImports`, Biome-fixable).
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: `import { afterEach, describe, expect, it } from 'vitest';`

- **[F007]** Commits `dcba45a feat(test): Add tests for critical surfaces (Phase 4 - src/lib coverage)` and `352ac0e feat(test): Add comprehensive tests for src/lib (ipc, events, runtime)` — both use `feat(test):`. `CLAUDE.md` → _Commits / changelog_ and user-global rules: `feat` is reserved for user-facing features; the correct Conventional-Commits type for tests is `test(scope): …`. `git-cliff` will mis-categorize these as features in the generated `CHANGELOG.md`. The PR title itself (`test(coverage): …`) is correct — the individual commits are not.
  - **Status**: Open
  - **Category**: Pattern Compliance
  - **Suggested fix**: Rewrite both commits (rebase or squash-merge using the PR title) so they land as `test(coverage): …` or `test(lib): …`.

- **[F008]** `PHASE4_COVERAGE_TRACKING.md:21-24,31-33` — Uses emoji (`✅`, `🚧`) in a committed doc. `CLAUDE.md` best practices flag _"Emoji usage in code/comments"_; user-global rules state _"Only use emojis if the user explicitly requests it."_
  - **Status**: Open
  - **Category**: Pattern Compliance
  - **Suggested fix**: Drop the emojis. If the doc survives F002's resolution (moved to `docs/plans/…` or opened as an issue), replace `✅ Completed` with `Completed` and `🚧 In Progress` with `In Progress`.

### LOW

- **[F009]** `src/crosshook-native/src/lib/__tests__/ipc.test.ts:28-48` — Relies on ordered side effects: `beforeEach(() => vi.resetModules())` clears the module cache but does not unregister `vi.doMock(...)` factories from the previous test; those persist until explicit `vi.doUnmock(...)` or end of file. Today the second test re-issues its own `doMock` calls so it works, but the pattern is brittle and diverges from `docs/TESTING.md` which recommends `@tauri-apps/api/mocks`'s `mockIPC` helper for this adapter branch.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Add `afterEach(() => { vi.doUnmock('@/lib/runtime'); vi.doUnmock('@tauri-apps/api/core'); vi.doUnmock('@/lib/ipc.dev'); })`. Consider swapping the ad-hoc `doMock` of `@tauri-apps/api/core` for `mockIPC` from `@tauri-apps/api/mocks` to match the TESTING.md canonical pattern.

- **[F010]** `src/crosshook-native/src/lib/__tests__/runtime.test.ts:15-17,46-47` — Each test overwrites `global.window = { … }`, which replaces the happy-dom-provided `window` wholesale. No current test touches `document` / `matchMedia` etc., so nothing breaks today, but any future assertion or imported side effect that expects a real `window` will break silently.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Patch just the field under test — e.g., `Object.defineProperty(window, '__TAURI_INTERNALS__', { value: {}, configurable: true });` — and restore in `afterEach`. Or introduce a `withTauriWindow(cb)` helper.

- **[F011]** `.gitignore:52-53` — Adds `src/crosshook-native/coverage/` at the repo root. The workspace already carries its own `.gitignore`; workspace-scoped ignores typically live there. Trivial.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Optionally move `coverage/` to `src/crosshook-native/.gitignore`. Leaving as-is is also acceptable since other `src/crosshook-native/*` entries already sit in the root ignore file.

## Validation Results

| Check                              | Result                                                                      |
| ---------------------------------- | --------------------------------------------------------------------------- |
| Type check (`tsc --noEmit`)        | Pass                                                                        |
| Lint (`biome ci src/`)             | Pass (exit 0) — 7 new warnings from this PR (F005, F006)                    |
| Tests (`npm test`)                 | Pass — 9 files, 36/36 tests                                                 |
| Coverage (`npm run test:coverage`) | **Fail** — lines 10.18% / functions 9.45% / branches 9.48% vs 60% threshold |
| Build                              | Skipped — test-only PR                                                      |

## Files Reviewed

- `.gitignore` (Modified)
- `PHASE4_COVERAGE_TRACKING.md` (Added)
- `src/crosshook-native/src/lib/__tests__/events.test.ts` (Added)
- `src/crosshook-native/src/lib/__tests__/ipc.test.ts` (Modified)
- `src/crosshook-native/src/lib/__tests__/runtime.test.ts` (Added)
- `src/crosshook-native/vitest.config.ts` (Modified)
