---
pr: 352
title: 'docs(tests): Update documentation for testing pyramid and patterns'
head_ref: codex/update-testing-documentation
head_oid: 4db106047c5099175137c25cec546edb9db4e918
base_ref: main
reviewer: claude-code (/ycc:code-review)
reviewed_at: 2026-04-19
decision: APPROVE
---

# PR #352 — Code Review

Docs-only PR. Adds `docs/TESTING.md`, a new `src/crosshook-native/README.md`, updates the test-commands section in `AGENTS.md`/`CLAUDE.md`, and rewrites the Vitest/Playwright split in `src/crosshook-native/tests/README.md`. No source code or package manifest changes; no runtime surface area affected.

## Validation

| Check               | Command                                                                                                                                   | Result                                                                                                                                                                                                                           |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Markdown formatting | `npx prettier --check` on all five touched files (in PR worktree)                                                                         | PASS                                                                                                                                                                                                                             |
| Path verification   | `git ls-tree` at `4db1060` for every path referenced in the new doc                                                                       | All referenced files exist                                                                                                                                                                                                       |
| Exported symbols    | Read `src/test/render.tsx`, `src/test/setup.ts`, `src/test/fixtures.ts`, `src/lib/mocks/index.ts`                                         | `mockCallCommand`, `configureMockHandlers`, `renderWithMocks`, `resetMockHandlers`, `triggerIntersection`, `makeLibraryCardData`, `makeReadinessResult`, `makeProfileDraft`, `resetMockEnvironment`, `registerMocks` all present |
| Example fidelity    | Compared the three snippets in TESTING.md against `useOnboarding.test.ts`, `LibraryCard.test.tsx`, `OnboardingWizard.test.tsx` at PR head | Snippets are faithful distillations — imports, helpers, patterns match                                                                                                                                                           |
| Scripts referenced  | Read `src/crosshook-native/package.json` at PR head                                                                                       | `test`, `test:watch`, `test:coverage`, `test:smoke`, `test:smoke:update`, `test:smoke:install`, `typecheck` all present                                                                                                          |

Skipped Vitest/Playwright runs — docs-only PR with no code delta and no test-fixture changes.

## Findings

### F001 — `mockIPC` section cites a non-matching example test — MEDIUM — Open

**File:** `docs/TESTING.md:198-203`

The paragraph reads:

> `mockIPC` (tauri API): reserve for testing `src/lib/ipc.ts` itself (the adapter). Example:
> `src/lib/__tests__/ipc.test.ts` uses `vi.doMock('@tauri-apps/api/mocks', ...)` to prove the
> adapter calls `@tauri-apps/api/core` when `isTauri()` is true.

The actual `src/lib/__tests__/ipc.test.ts` at HEAD (`4db1060`) does **not** use `mockIPC` and does **not** touch `@tauri-apps/api/mocks`. It mocks `@/lib/runtime` + `@/lib/ipc.dev` and proves the **inverse** branch — that `callCommand` routes to `runMockCommand` (the webdev bridge) when `isTauri() === false && isBrowserDevUi() === true`. Two factual errors: wrong modules cited, and the branch direction is inverted ("`isTauri()` is true" vs. the actual "`isTauri()` is false").

This matters because the PRD acceptance criterion is "an agent following the doc can add a new hook/IPC test correctly on first attempt." An agent trying to test the Tauri path by copying this example will find no equivalent in-repo reference to crib from.

**Suggested fix:** either (a) rewrite the paragraph to describe the real test — "proves the browser-dev branch routes through `@/lib/ipc.dev`" — and keep the `mockIPC` sentence as forward guidance decoupled from the example, or (b) drop the example pointer entirely and leave just the "reserve `mockIPC` for adapter-layer tests" rule until such a test exists in-repo.

### F002 — Pitfall section slightly conflates reset helpers — LOW — Open

**File:** `docs/TESTING.md:211-213`

> `renderWithMocks` + `configureMockHandlers` call `resetMockEnvironment()` and `resetMockHandlers()` so every test starts clean.

Reading `src/test/render.tsx`: both `renderWithMocks` and `configureMockHandlers` call `createHandlerMap`, which calls `resetMockEnvironment()` only. `resetMockHandlers()` is invoked by `afterEach` in `src/test/setup.ts` — not by `renderWithMocks`/`configureMockHandlers` themselves. The net effect the sentence describes ("every test starts clean") is correct because the `afterEach` hook closes the loop, but the attribution is inaccurate.

**Suggested fix:** "`renderWithMocks` and `configureMockHandlers` call `resetMockEnvironment()` to re-seed; `src/test/setup.ts` calls `resetMockHandlers()` from `afterEach` to drop the active handler map between tests."

### F003 — Command block loses comment alignment in AGENTS.md / CLAUDE.md — NIT — Open

**Files:** `AGENTS.md:110-116`, `CLAUDE.md:103-109`

The preceding lines in the same fenced block all have aligned `# …` trailing comments. The three new entries `npm run test:smoke:update`, `npm run test:smoke:install`, and `npm run typecheck` are added without comments, which breaks the visual pattern inside the same block.

**Suggested fix:** add brief comments (`# update Playwright snapshots`, `# install the Playwright browser`, `# tsc --noEmit (app + tests)`). Tiny, non-blocking.

### Pattern compliance notes (no action required)

- New `docs/TESTING.md` (186 lines) is within the soft 500-line cap and is topical — trophy + three patterns + pitfalls. Well-scoped.
- New `src/crosshook-native/README.md` (20 lines) is deliberately minimal; it hands the testing deep-dive off to `docs/TESTING.md` and the smoke-suite README, which is the right layering.
- Referenced helper paths (`src/test/setup.ts`, `src/test/render.tsx`, `src/test/fixtures.ts`, `src/lib/mocks/*`) and builder names (`makeLibraryCardData`, `makeReadinessResult`, `makeProfileDraft`) all resolve at HEAD.
- Commit hygiene follows repo conventions: `docs:` prefix on the user-visible doc commit, `chore:` on the lint-autofix commit. No `docs(internal):` required since these are user-facing docs.
- Synthetic-data policy is respected throughout the snippets (`Synthetic Quest`, `/mock/media/...`, no real game names).

## Decision

**APPROVE.**

This PR delivers on the Phase 3 PRD acceptance criteria: a single-entry testing doc with the trophy layout, three canonical patterns distilled from real in-repo tests, IPC-mocking guidance, and the fixture/toggle/reset pitfalls. The three findings above are polish — F001 is the one worth fixing before merge because it risks sending future agents down a dead end when they try to write a Tauri-path adapter test. F002 and F003 are optional cleanups.
