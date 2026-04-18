# PRD: Comprehensive Frontend Test Framework

**Issue**: [#282](https://github.com/yandy-r/crosshook/issues/282)
**Status**: Ready for planning
**Date**: 2026-04-17

---

## 1. Problem

CrossHook's frontend has no component/hook/integration test layer. Every UI change is validated by hand in `vite --mode webdev` or full Tauri dev mode. Playwright smoke (`tests/smoke.spec.ts`, `collections.spec.ts`, `pipeline.spec.ts`) covers 9 route sidebars and a launch-pipeline walk-through, but that is route-level sanity — not component behavior, hook async state, IPC-adapter edges, or focused interaction flows. `npx tsc --noEmit` catches type errors; Biome catches lint and static a11y. Between those and the Playwright smoke lies the entire live UI surface of the app — ~250 TS/TSX files — covered only by eyeballing browser dev mode.

This matters more now because:

- The application is advancing toward a marketing and contributor-facing phase. Manual browser/Tauri re-verification will not scale past a solo maintainer.
- AI-agent review/fix loops (e.g., the PR #281 work that surfaced this gap) have no narrow, reliable frontend test command to gate their own changes. They currently ship untested, or force the maintainer back into browser dev mode on every iteration.
- 113–144 Rust `#[tauri::command]` signatures are mirrored by hand in `src/crosshook-native/src/lib/mocks/handlers/*.ts`. Drift is detected by name (advisory-only shell script) but not by shape.

**Hypothesis**: A Vitest + React Testing Library framework backed by the already-existing `registerMocks()` handler registry will replace most manual browser/Tauri re-verification for frontend-only changes, for the solo maintainer and AI-agent contributors. We'll know we're right when frontend PRs land on a single CI-green signal and the Playwright smoke retry rate stays ≤5% once integrated into CI.

This PRD covers the TS/React layer only. Rust command correctness stays in `cargo test`. Full IPC typesafety (tauri-specta, ts-rs, runtime validators) is out-of-scope for v1 and tracked as a follow-up phase.

---

## 2. Users & Personas

| Persona                      | Context                                                                                                                            | Key Need                                                                                                                 |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| **Solo maintainer** (today)  | Owns the entire stack. Edits hooks, components, routes; today runs `npm run dev:browser` or Tauri dev mode after every change.     | Save a file → one command runs a focused suite in under 5 seconds → clear failure signal → merge without firing up Tauri |
| **AI-agent contributor**     | Runs review-fix and implement loops autonomously against the repo. Currently has no frontend test command to gate its own changes. | Mechanically-discoverable conventions, typed helpers, canonical patterns to mirror. Quality > onboarding speed.          |
| **Future human contributor** | Opens a PR with frontend changes. Needs fast local feedback and predictable CI gating.                                             | `npm test` just works. Docs explain pyramid (unit vs. smoke vs. manual Tauri).                                           |

---

## 3. Goals & Success Criteria

### 3.1 Goals

| #   | Goal                                                                                                                                                      | Phase |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ----- |
| G1  | Wire Vitest 4.1.x into `src/crosshook-native/` with a shared `renderWithMocks()` harness                                                                  | 1     |
| G2  | Ship initial suite: 3 hooks + 3 components that prove the framework handles the hard cases                                                                | 1     |
| G3  | CI gates: Vitest runs on every PR in the `typescript` job; Playwright becomes a new `smoke` job                                                           | 2     |
| G4  | Contract drift gate: `scripts/check-mock-coverage.sh` flipped to `exit 1`, wired into CI                                                                  | 2     |
| G5  | Contributor docs — a `docs/TESTING.md` pattern library with canonical hook/component/page test examples                                                   | 2     |
| G6  | Expand coverage to 60% on critical surfaces (`src/hooks/**`, `src/lib/{ipc,events,runtime}.ts`, `src/components/pages/**`). Gaps tracked as GitHub issues | 3     |
| G7  | Follow-up phase: `ts-rs`-based shape contract between Rust commands and TS types (evaluated, not shipped in v1)                                           | 4     |

### 3.2 Success Criteria

| Metric                            | Target                                                                                                   | Measurement                                          |
| --------------------------------- | -------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| Local test loop speed             | Initial 6-test suite runs in <5 s locally (watch mode, single file edit)                                 | `vitest --watch` wall time                           |
| CI critical-path impact           | Vitest adds ≤60 s to existing `typescript` job; Playwright `smoke` job ≤4 min                            | GitHub Actions job duration                          |
| Coverage on critical surfaces     | ≥60% lines on `src/hooks/**`, `src/lib/{ipc,events,runtime}.ts`, `src/components/pages/**`               | `@vitest/coverage-v8` report, gated by Vitest config |
| Drift detection                   | 100% of Rust command renames caught in CI before merge                                                   | `check-mock-coverage.sh` exit code                   |
| Playwright flake rate (in CI)     | ≤5% retry rate over 30 consecutive CI runs                                                               | GitHub Actions retry count                           |
| Manual Tauri re-verification rate | Subjective — does not replace every manual check, but should eliminate frontend-only-change verification | Maintainer self-report per feature                   |

### 3.3 Non-Goals

- **Rust/Tauri backend unit tests** — remain in `cargo test`; out-of-scope
- **Full IPC typesafety via `tauri-specta`** — deferred to Phase 4 evaluation; requires refactoring all `callCommand<T>()` call-sites
- **Runtime response validation with `zod`/`valibot`** — orthogonal concern; may be layered later
- **`ts-rs` type codegen** — tracked as Phase 4; not shipped in v1
- **Visual regression suite with committed screenshot baselines** — Playwright snapshot baselines stay opt-in, not committed
- **Auto-wiring `test:watch` into Tauri dev mode** — out-of-scope
- **Cross-platform test matrix** — Linux-only for now; macOS/Windows testing remains aspirational per `AGENTS.md`
- **Replacing Biome's a11y static rules with `eslint-plugin-jsx-a11y`** — Biome already covers this (`biome.json:42-51`)
- **100% coverage of all 258 TS/TSX files in v1** — gaps tracked as GitHub issues
- **Full `useProfile` (1668-line giant) coverage** — deferred to Phase 3+; its 4 debounced timers, event subscription, and serialized write chain deserve a dedicated pass
- **Vitest browser mode** — jsdom/happy-dom-based tests are enough; browser mode gets evaluated only if a flaky Radix test emerges

---

## 4. Key Decisions

### 4.1 Runner: Vitest 4.1.x

Vitest is the 2026 default for React + Vite projects. Version 4.1 (March 2026) adds explicit Vite 8 support and can reuse the installed Vite (`8.0.5` per `src/crosshook-native/package.json:43`) instead of downloading a parallel copy. Auto-detects GitHub Actions (emits annotations + Job Summary) and AI-agent contexts (suppresses passed-test output). Jest is legacy-only unless React Native is involved.

Configuration lives in a dedicated `vitest.config.ts` (not merged into `vite.config.ts`) so test-only concerns stay isolated. The config must reuse:

- The `@ → src/` path alias from `tsconfig.json:18-22`
- The `define: { __WEB_DEV_MODE__: true }` block from `vite.config.ts:14-16` — otherwise the `src/lib/ipc.ts:5-16` dev branch is unreachable and `callCommand()` throws the fallback error
- `@vitejs/plugin-react` already installed as a devDep

### 4.2 DOM environment: happy-dom

Happy-dom is 2–10× faster on import and test parse than jsdom. The one reason to prefer jsdom (axe-core's known incompatibility with happy-dom's `Node.prototype.isConnected`) does not apply here: a11y testing lives entirely in the Playwright tier via `@axe-core/playwright`, not in the Vitest tier.

### 4.3 Component/hook stack: React Testing Library (current)

- `@testing-library/react@16+` — built on `createRoot`, React-18-concurrent-safe. Exports `renderHook` and `act` directly; the deprecated `@testing-library/react-hooks` package is NOT used.
- `@testing-library/user-event@14+` — realistic interaction sequences. Required for anything keyboard/focus-sensitive (CrossHook has `useGamepadNav`, focus-trapped modals, `ContextMenu`/`Shift+F10`, form flows).
- `@testing-library/jest-dom@6+` — DOM matchers (`toBeInTheDocument`, `toHaveAttribute`, `toHaveFocus`, ...). Registered via `import '@testing-library/jest-dom/vitest'` in a setup file. Improves failure-message legibility and matches the community corpus AI agents are trained on.
- `@testing-library/dom` — explicit dev-dep for lockfile stability (RTL peer).

### 4.4 IPC mocking: boundary-mock via `vi.mock('@/lib/ipc')`

Two canonical patterns exist. We pick the repo-specific one because the repo already has the infrastructure:

**Chosen — Pattern B**: `vi.mock('@/lib/ipc', ...)` at the adapter boundary, dispatching into the existing `registerMocks()` handler map from `src/crosshook-native/src/lib/mocks/index.ts`. The same handlers that power `?fixture=populated` in browser-dev mode drive component tests. Single source of truth.

**Rejected — Pattern A**: `@tauri-apps/api/mocks.mockIPC` — reserved only for thin tests of `src/lib/ipc.ts` itself (to confirm the `isTauri()`-true branch still routes through `@tauri-apps/api/core`).

Shared helper: `renderWithMocks(ui, options)` at `src/crosshook-native/src/test/render.tsx`. Wraps RTL `render()` with:

1. `resetStore()` in `beforeEach`
2. Optional seed fn to pre-populate mock state
3. Optional `fixture: 'populated' | 'empty' | 'error' | 'loading'` selector
4. All necessary React context providers

### 4.5 Accessibility: `@axe-core/playwright` only

Axe runs in the Playwright tier against a Chromium-rendered app where CSS cascade, focus rings, and layout are real. Unit-layer axe is skipped. Rationale:

- `vitest-axe` (the classic) has been unmaintained since October 2022 — effectively abandoned
- The active fork `@chialab/vitest-axe` is a viable alternative, but axe against happy-dom/jsdom false-positives on color contrast and misses layout-dependent violations anyway
- Biome's `a11y` rule group already covers the static layer (`biome.json:42-51`) — no `eslint-plugin-jsx-a11y` needed
- RTL's role/label queries (`getByRole`, `getByLabelText`) enforce accessible selectors as a matter of test hygiene implicitly

Signature axe coverage lives in Playwright for: modals (OnboardingWizard, confirmation dialogs), menus (CollectionAssignMenu), forms (profile-sections), custom interactive controls (LibraryCard).

### 4.6 Coverage: 60% on critical surfaces, gaps tracked as issues

| Surface                                            | Rationale                                                 | 60% gate |
| -------------------------------------------------- | --------------------------------------------------------- | -------- |
| `src/hooks/**`                                     | Async state + IPC wrapping — where regressions hide       | Yes      |
| `src/lib/ipc.ts`, `events.ts`, `runtime.ts`        | The adapter boundary itself                               | Yes      |
| `src/components/pages/**`                          | Route-level assembly — empty/loading/error/success states | Yes      |
| `src/components/**` (non-page)                     | Starts ungated; gaps tracked in issues                    | No (v1)  |
| `src/utils/**`, `src/types/**`, `src/lib/mocks/**` | Utilities, types, mock layer itself                       | No (v1)  |

Tracking: a single "uncovered critical surfaces" tracking issue with a file checklist, rather than per-file issues. Ratcheting up (70 → 80%) handled in Phase 3+.

### 4.7 File layout: co-located `__tests__/`

Tests live next to the code they exercise:

```
src/hooks/__tests__/useCapabilityGate.test.ts
src/hooks/__tests__/useOnboarding.test.ts
src/components/library/__tests__/LibraryCard.test.tsx
src/components/library/__tests__/LibraryGrid.test.tsx
```

Shared helpers centralized under `src/test/`:

```
src/test/setup.ts          # jest-dom registration, WebCrypto polyfill, afterEach resetStore
src/test/render.tsx        # renderWithMocks() helper + provider wrapper
src/test/fixtures.ts       # seed helpers (seedProfilePopulated, seedInstallError, ...)
```

Rationale: AI agents touch one directory per feature. Agent-authored tests live next to the code they exercise — no "which tree?" question. Playwright smoke stays at `src/crosshook-native/tests/` (unchanged).

### 4.8 CI: new `smoke` job in `lint.yml`

Vitest slots into the existing `typescript` job in `.github/workflows/lint.yml:51-73` after `tsc --noEmit`:

```yaml
- name: Run Vitest
  working-directory: src/crosshook-native
  run: npx vitest run --coverage
```

Playwright gets a new parallel job in the same workflow:

```yaml
smoke:
  name: Playwright smoke
  runs-on: ubuntu-latest
  timeout-minutes: 15
  steps:
    - uses: actions/checkout@v4
    - uses: actions/setup-node@v4
      with:
        node-version: '20'
        cache: npm
        cache-dependency-path: src/crosshook-native/package-lock.json
    - name: Install dependencies
      working-directory: src/crosshook-native
      run: npm ci
    - name: Install Playwright + browser deps
      working-directory: src/crosshook-native
      run: npx playwright install --with-deps chromium
    - name: Run smoke suite
      working-directory: src/crosshook-native
      run: npm run test:smoke
```

Rationale:

- Isolation — a Playwright flake must not block Vitest feedback or AI-agent iteration
- Concurrency group `lint-${{ github.ref }}` (`.github/workflows/lint.yml:12-14`) cancels both on force-push
- No browser caching — Playwright's own docs say restore time ≈ fresh download, and `install-deps` cannot be cached
- ~30 s duplicate `npm ci` is acceptable for flake isolation

### 4.9 Contract drift: Tier 1 now, Tier 3 phased

**Tier 1 (v1, Phase 2)** — flip `scripts/check-mock-coverage.sh:134-136` from `exit 0` to `exit 1` on non-empty drift. Wire into CI as a new step in the existing `shell` job (or adjacent). Catches: Rust command renames, adds, deletes. Misses: shape/argument/return-type drift.

**Tier 3 (Phase 4, tracked)** — `ts-rs` adoption. `#[derive(TS)]` + `#[ts(export)]` on every arg/return struct in `crosshook-core` and `src-tauri`. Generated `.ts` types replace `src/types/*.ts` incrementally (ts-rs supports coexistence). `callCommand<T>()` pattern stays — no call-site refactor. Preserves the existing `registerMocks()` harness.

Rejected for v1:

- **Tier 2** (`zod`/`valibot` at IPC boundary) — ~143 validator definitions, orthogonal to the test framework
- **Tier 4** (`tauri-specta`) — still RC after 2 years, known `Vec<u8>` / 10-arg / hot-reload-loop issues, requires replacing every `callCommand<T>()` call-site

### 4.10 Dependency footprint

Seven new devDependencies. Minimum-viable for a modern React test stack:

| Package                       | Version baseline | Why                                                           |
| ----------------------------- | ---------------- | ------------------------------------------------------------- |
| `vitest`                      | `^4.1.4`         | Runner                                                        |
| `@vitest/coverage-v8`         | `^4.1.4`         | Coverage (60% gate)                                           |
| `happy-dom`                   | `^20.9.0`        | DOM env — faster than jsdom, compatible since axe isn't here  |
| `@testing-library/react`      | `^16.3.2`        | Render + query                                                |
| `@testing-library/dom`        | `^10.x`          | Explicit RTL peer for lockfile stability                      |
| `@testing-library/jest-dom`   | `^6.9.1`         | DOM matchers, readability, AI-agent-friendly failure messages |
| `@testing-library/user-event` | `^14.6.1`        | Realistic interaction sequences — keyboard, gamepad, forms    |

Plus `@axe-core/playwright` (Playwright side) — axe-core arrives transitively.

Locked-out: `jsdom`, `vitest-axe` (stale), `@chialab/vitest-axe`, `eslint-plugin-jsx-a11y`, `ts-rs` (v1), `tauri-specta` (v1), `zod`/`valibot` (v1).

---

## 5. Initial Suite Seed

Deliberately sized to prove every hard case, not to hit coverage targets on its own.

### 5.1 Hooks (in order)

| Hook                | File                                  | Why first                                                                                                                                          |
| ------------------- | ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `useCapabilityGate` | `src/hooks/useCapabilityGate.ts:1-59` | Smallest — pure context-derived state + `useMemo`/`useCallback`. Proves the provider-wrapper pattern in `renderWithMocks()` works.                 |
| `useOnboarding`     | `src/hooks/useOnboarding.ts:108-267`  | Canonical IPC + async pattern. 4 `callCommand` calls, stage-progression machine. Exercises error branches and mock-handler round-trip.             |
| `useGamepadNav`     | `src/hooks/useGamepadNav.ts:258-755`  | Hardest case: `requestAnimationFrame` polling, global keydown capture, MutationObserver. Proves fake timers + mocked `navigator.getGamepads` work. |

### 5.2 Components (in order)

| Component          | File                                            | Why first                                                                                                                        |
| ------------------ | ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `LibraryGrid`      | `src/components/library/LibraryGrid.tsx:1-60`   | Textbook empty-state vs populated-state split. No IPC. Smallest component test win.                                              |
| `LibraryCard`      | `src/components/library/LibraryCard.tsx:27-211` | IntersectionObserver, right-click + `Shift+F10` context menu, `useGameCoverArt` IPC. Covers a11y-sensitive keyboard interaction. |
| `OnboardingWizard` | `src/components/OnboardingWizard.tsx`           | Focus trap (`FOCUSABLE_SELECTOR`), stage machine, Escape-to-dismiss, portal-rendered. Highest-value wizard-flow target.          |

### 5.3 Pattern library (in `docs/TESTING.md`)

One canonical example per tier, distilled from the three hooks + three components:

- Canonical hook test (follows `useOnboarding`)
- Canonical component test with user-event (follows `LibraryCard`)
- Canonical mock-driven page test (follows `OnboardingWizard`)
- When to reach for `vi.mock('@/lib/ipc')` vs. seeding `registerMocks()`
- When to prefer `mockIPC` (Tauri official) for `ipc.ts` self-tests

---

## 6. Implementation Phases

<!--
  STATUS: pending | in-progress | complete
  PARALLEL: phases that can run concurrently
  DEPENDS: phases that must complete first
  PRP: link to generated plan file once created
-->

| #   | Phase                                          | Description                                                                                                                 | Status  | Parallel | Depends | PRP Plan |
| --- | ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- | ------- | -------- | ------- | -------- |
| 1   | Framework scaffolding + initial suite          | Wire Vitest + RTL + helpers; ship 3 hooks + 3 components as the proving suite                                               | pending | -        | -       | -        |
| 2   | CI integration + contract gate                 | Add Vitest step to `typescript` job; add `smoke` job for Playwright; flip `check-mock-coverage.sh` to exit 1                | pending | with 3   | 1       | -        |
| 3   | Pattern library + contributor docs             | `docs/TESTING.md` with canonical examples, pyramid guidance, when-to-use-what                                               | pending | with 2   | 1       | -        |
| 4   | Coverage expansion to 60% on critical surfaces | Add tests across `src/hooks/**`, `src/lib/{ipc,events,runtime}.ts`, `src/components/pages/**`. Track gaps as GitHub issues. | pending | -        | 2, 3    | -        |
| 5   | `ts-rs` shape-contract evaluation (follow-up)  | Prototype `#[derive(TS)]` on `crosshook-core` types. Measure refactor cost. Decide ship/defer.                              | pending | -        | 4       | -        |

### Phase 1: Framework scaffolding + initial suite

**Goal**: Prove the framework handles every hard case CrossHook has — async IPC, fake timers, focus traps, context menus, IntersectionObserver, MutationObserver.

**Scope**:

- `vitest.config.ts` with happy-dom environment, `@` alias, `__WEB_DEV_MODE__` define, `setupFiles: ['./src/test/setup.ts']`, coverage provider `v8`
- `src/test/setup.ts` — jest-dom registration, WebCrypto polyfill (`crypto.randomFillSync`), `afterEach(resetStore)`, `afterEach(vi.useRealTimers)`
- `src/test/render.tsx` — `renderWithMocks()` helper with provider wrapper, fixture seeding, and mock-handler dispatch through `vi.mock('@/lib/ipc')`
- `src/test/fixtures.ts` — seed helpers (`seedProfilePopulated`, `seedInstallError`, ...)
- Tests: `useCapabilityGate`, `useOnboarding`, `useGamepadNav`, `LibraryGrid`, `LibraryCard`, `OnboardingWizard` (minimum 2–3 cases each)
- `package.json` scripts: `test`, `test:watch`, `test:coverage`
- Node types — either add `@types/node` via a separate `tsconfig.test.json`, or mirror the ambient `declare const process` pattern from `playwright.config.ts:9`

**Success signal**: `npm test` passes locally in <5 s on watch mode after a single hook edit. All 6 tests green.

### Phase 2: CI integration + contract gate

**Goal**: Frontend PRs gated by a single CI-green signal before merge.

**Scope**:

- Add `Run Vitest` step to `.github/workflows/lint.yml` `typescript` job after `tsc --noEmit`
- Add new `smoke` job to same workflow — `timeout-minutes: 15`, `npx playwright install --with-deps chromium`, `npm run test:smoke`
- Flip `scripts/check-mock-coverage.sh:134-136` to exit 1 on drift
- Wire `check-mock-coverage.sh` into the `shell` job in `.github/workflows/lint.yml`
- Verify CI budget: Vitest ≤60 s added to `typescript` job; Playwright `smoke` ≤4 min

**Success signal**: PR created with a deliberate Rust command rename fails `check-mock-coverage.sh` in CI. PR with a deliberate frontend regression fails Vitest. PR with a deliberate route regression fails Playwright. All three produce legible GitHub annotations.

### Phase 3: Pattern library + contributor docs

**Goal**: AI agents and future human contributors can write tests without reading Vitest docs from scratch.

**Scope**:

- `docs/TESTING.md` — pyramid explanation (Testing Trophy shape), 3 canonical test patterns (hook, component, page), when to use each, common pitfalls
- Quick reference: module-init caching (`?fixture=`, `?delay=`) and how to override per-test
- Quick reference: singleton state bleed in `handlers/profile.ts`, `handlers/collections.ts` — how to reset
- Update `AGENTS.md` and `CLAUDE.md` with the new test commands
- Update `src/crosshook-native/README.md` with the testing section
- Update `src/crosshook-native/tests/README.md` to describe the Vitest/Playwright split

**Success signal**: An AI agent asked to add a test for a new hook produces a correctly-wired test on first attempt using the pattern library.

### Phase 4: Coverage expansion to 60% on critical surfaces

**Goal**: 60% line coverage on gated surfaces; every uncovered critical file tracked in a GitHub issue.

**Scope**:

- Add tests for remaining `src/hooks/**` — `useProfile` deliberately deferred to Phase 3+ (1668 lines, 4 debounced timers, event subscription, serialized write chain deserves its own pass)
- Add tests for `src/lib/ipc.ts` (Tauri-real branch via `mockIPC`), `src/lib/events.ts`, `src/lib/runtime.ts`
- Add tests for `src/components/pages/**` — representative empty/loading/error/success for each page
- Set Vitest coverage gate: `thresholds: { lines: 60 }` scoped via `include`/`exclude` to the critical-surface globs
- Open a single tracking issue with a file checklist for uncovered-but-desired surfaces

**Success signal**: `vitest --coverage` passes with the 60% gate active on the critical surfaces. Gaps are visible and tracked.

### Phase 5: `ts-rs` shape-contract evaluation (follow-up)

**Goal**: Decide whether to adopt `ts-rs` for shape-level contract enforcement between Rust commands and TS types.

**Scope**:

- Prototype: add `ts-rs` to `crosshook-core/Cargo.toml`, `#[derive(TS)] + #[ts(export)]` on 3–5 representative structs, emit to `src/types/generated/`
- Migrate 1 hand-maintained `src/types/*.ts` file to generated equivalent
- Measure: per-struct annotation cost, edge cases (serde rename_all, `Option`, `Vec<u8>`, chrono/uuid), build-step integration
- Decide: full migration vs. incremental-only vs. defer

**Success signal**: A decision document at `docs/prps/specs/ts-rs-evaluation-spec.md` with the data needed to decide.

---

## 7. Risks & Mitigations

| Risk                                                                                                             | Likelihood | Mitigation                                                                                                                                                                           |
| ---------------------------------------------------------------------------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `__WEB_DEV_MODE__` Vite define not reused in `vitest.config.ts` → `callCommand()` throws fallback error in tests | H          | Phase 1 mirrors the `define` block explicitly. Smoke test: a deliberate test that calls `callCommand('any')` must succeed (proving the define is wired)                              |
| Module-init caching of `?fixture=` and `?delay=` toggles (`src/lib/fixture.ts:33`) blocks per-test fixture swap  | M          | Document `vi.mock('@/lib/fixture')` pattern in `docs/TESTING.md`. Provide a `withFixture()` helper in `src/test/fixtures.ts` that wraps the mock                                     |
| Singleton state bleed in module-scope `Set`s/`Map`s in `handlers/profile.ts`, `handlers/collections.ts`          | M          | Phase 1 `setup.ts` calls `resetStore()` in `afterEach`. Handler-local state gets explicit reset exports or `vi.resetModules()` for cases that need them. Documented in `TESTING.md`. |
| Playwright flake rate in CI exceeds 5% after integration                                                         | M          | Start with `retries: 1` (already in `playwright.config.ts:40`). If flake rate is high, mark `smoke` job `continue-on-error: true` temporarily while root-causing                     |
| Vitest v8 coverage regression on inline React components (known in 4.0)                                          | L          | Pin to `^4.1.4` (fix shipped in 4.1 series); add a CI job annotation if coverage numbers look off                                                                                    |
| CI budget exceeded — frontend PRs get slow                                                                       | L          | Measure in Phase 2; if Playwright smoke exceeds 5 min wall time, revisit containerized runner or a `paths:` filter to skip on docs-only PRs                                          |
| Contract gate (`check-mock-coverage.sh`) false-positives block legitimate PRs                                    | L          | Script already has allowlist logic for intentional mock-only commands; extend if needed. Phase 2 includes a dry-run PR to validate before flipping to exit 1                         |
| AI agents write tests that pass but don't meaningfully cover behavior (e.g., trivial snapshot tests)             | M          | Phase 3 pattern library explicitly names anti-patterns (implementation-detail testing, excessive mocking, duplicating E2E). Code review gates catch the rest                         |
| `happy-dom` surface gap hits a Radix primitive (portal, focus management, `getComputedStyle`)                    | L          | Fallback to per-file `@vitest-environment jsdom` override documented in `TESTING.md`. Worst case: switch default to jsdom for the whole suite                                        |

---

## 8. Decisions Log

| Decision            | Choice                                                                         | Alternatives                                     | Rationale                                                                                                       |
| ------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------- |
| Runner              | Vitest 4.1.x                                                                   | Jest 30, Node `node:test`                        | Vite-native; auto-detects CI + AI agents; Jest is legacy-only for React Native                                  |
| DOM env             | happy-dom                                                                      | jsdom                                            | 2–10× faster; axe moved to Playwright so the happy-dom incompatibility is irrelevant                            |
| IPC mocking         | `vi.mock('@/lib/ipc')` → `registerMocks()`                                     | `@tauri-apps/api/mocks.mockIPC` (Pattern A)      | Reuses the existing mock layer that already powers browser-dev mode — single source of truth; AI-agent-friendly |
| a11y location       | Playwright only (`@axe-core/playwright`)                                       | `vitest-axe` in unit tests                       | `vitest-axe` is abandoned; happy-dom/jsdom false-positive on color contrast; Playwright renders real CSS        |
| Coverage scope      | 60% on critical surfaces                                                       | 80% everywhere, or no gate                       | Maintainer directive — "start with 60% on critical areas, expand later, track gaps"                             |
| Critical surfaces   | `src/hooks/**` + `src/lib/{ipc,events,runtime}.ts` + `src/components/pages/**` | All of `src/**`                                  | Where async state + IPC + route assembly lives — highest regression density; rest ungated in v1                 |
| File layout         | Co-located `__tests__/`                                                        | Centralized `src/test/unit/`                     | AI agents touch one directory per feature; tests live next to the code they exercise                            |
| CI shape            | New `smoke` job in `lint.yml`; Vitest in `typescript` job                      | Single job, or separate workflow file            | Isolates Playwright flakes from Vitest feedback; shares concurrency group; avoids split-workflow complexity     |
| Contract drift (v1) | Tier 1: flip `check-mock-coverage.sh` to exit 1                                | Tier 2 zod, Tier 3 ts-rs, Tier 4 tauri-specta    | Catches rename/add/delete drift immediately; ts-rs evaluated in Phase 5 as the next honest step                 |
| Dep footprint       | 7 devDeps + `@axe-core/playwright`                                             | 9 (with unit-layer axe) or 5 (maximally minimal) | Quality > minimalism per maintainer directive; jest-dom + user-event are not worth skipping for ~700 KB         |
| `useProfile` in v1  | Deferred to Phase 3+                                                           | Include in initial suite                         | 1668 lines, 4 debounced timers, event subscription — needs a dedicated pass, not a rushed first test            |

---

## 9. Open Questions

- [ ] **Coverage enforcement scoping** — should the 60% gate run on the whole suite with `include`/`exclude` globs, or as a separate `vitest run --coverage` invocation with narrower config? Decide during Phase 4.
- [ ] **Playwright smoke retries in CI** — current `playwright.config.ts:40` uses `retries: CI ? 1 : 0`. Should this move to 2 for CI stability, or stay at 1 and force root-cause fixes? Measure in Phase 2.
- [ ] **`useProfile` test strategy** — 1668 lines, 4 debounced timers, event subscription, serialized write chain. Is the right unit a single mega-test file or split by concern (auto-save / event / validation / rollback)? Decide when Phase 3+ starts.
- [ ] **Coverage ratchet** — v1 is 60%. What's the cadence for raising it (70 → 80%)? Per-phase or per-release?
- [ ] **Frontend-only PR fast-lane** — should Vitest be the _only_ required check for PRs that touch only `src/crosshook-native/src/**`, skipping the Rust/shell jobs via `paths:` filters? Revisit once CI budget is measured.
- [ ] **Tauri E2E with WebDriver** — the repo's Playwright smoke runs against the Vite dev server (Chromium), not against real Tauri (WebKitGTK). Does this gap warrant a `tauri-driver`-based E2E track eventually? Out-of-scope for v1, but worth tracking.

---

## 10. Research Summary

**Market / technical findings**:

- Vitest 4.1 (March 2026) is the React + Vite default in 2026, with explicit Vite 8 support, auto-detection of GitHub Actions and AI-agent contexts ([Vitest 4.1 blog](https://vitest.dev/blog/vitest-4-1.html))
- `@testing-library/react-hooks` is deprecated — `renderHook` + `act` are re-exported from `@testing-library/react` directly ([RTL docs](https://testing-library.com/docs/react-testing-library/api/#renderhook))
- `vitest-axe` has been unmaintained since October 2022 — active alternatives are `@chialab/vitest-axe` and `@sa11y/vitest`. Not chosen; a11y moves entirely to Playwright.
- `tauri-specta` is still RC 2 years in, with known `Vec<u8>` / 10-arg / hot-reload-loop issues; `ts-rs` is the lower-friction shape-contract route and leaves `callCommand<T>()` untouched
- Testing Trophy (Kent C. Dodds, re-endorsed in 2026) recommends heavy integration, thin unit, thin E2E — aligns with CrossHook's shape
- Playwright's own CI docs say _don't cache browsers_ — cache restore time ≈ fresh download, and `install-deps` is not cacheable

**Codebase findings**:

- `src/crosshook-native/src/lib/ipc.ts:7-17` — single `callCommand<T>()` function is the entire IPC boundary. Mocking this one symbol covers ~99% of hook/component test cases.
- `src/lib/mocks/index.ts:33-61` — `registerMocks()` returns a pure `Map<string, Handler>` with no Vite-runtime dependencies. Directly importable in Vitest. Covers 113–144 commands across 14 domains.
- `src/lib/mocks/store.ts:21-39` — `resetStore()` already exists. Tests hook directly into it.
- 143 Rust `#[tauri::command]` functions across 26 files (via `scripts/check-mock-coverage.sh:67-93`). Currently matched on name only — zero shape-level enforcement.
- `scripts/check-mock-coverage.sh:134-136` — already detects name drift, but exits 0 regardless. 10-minute PR to convert to CI gate.
- `playwright.config.ts:38-41` — `fullyParallel: false, workers: 1` to avoid singleton bleed. Vitest parallel worker threads sidestep this because each worker has its own module graph.

**Flipped decisions during grounding**:

| Topic          | Initial instinct                   | After research                                                                |
| -------------- | ---------------------------------- | ----------------------------------------------------------------------------- |
| DOM env        | jsdom (needed for unit-layer axe)  | **happy-dom** (axe moved to Playwright → incompatibility irrelevant)          |
| a11y location  | "signature axe tests in Vitest"    | **all axe in Playwright** via `@axe-core/playwright`                          |
| Contract drift | not discussed initially            | **Tier 1 now (gate the script), Tier 3 phased** (`ts-rs`, not `tauri-specta`) |
| CI shape       | "add step to `typescript` job"     | **new `smoke` job in `lint.yml`** (isolate Playwright flakes)                 |
| Dep count      | 9 (with `vitest-axe` + `axe-core`) | **7** (+ `@axe-core/playwright` on Playwright side)                           |

---

_Generated: 2026-04-17_
_Status: DRAFT — ready for Phase 1 planning_
