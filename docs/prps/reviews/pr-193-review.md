# Code Review: PR #193 — feat(ui): add live launch pipeline phase 3 overlay

**PR**: [yandy-r/crosshook#193](https://github.com/yandy-r/crosshook/pull/193)
**Branch**: `feat/launch-pipeline-phase3-live-launch-animation` -> `main`
**Author**: yandy-r
**Reviewed**: 2026-04-09
**Mode**: Parallel (3 specialized reviewers: correctness, security, quality)

## Summary

Phase 3 of the launch pipeline visualization. Maps `LaunchPhase` onto existing Tier 1/2 pipeline
nodes via `applyPhaseOverlay()` with an optional `PipelineNodeTone` for the trainer handoff waiting
state. Updates `LaunchPipeline` to prefer `aria-current` on the active node and emit `data-tone`.
Extends CSS with waiting pulse animation, complete-step connectors, and `prefers-reduced-motion`
overrides. Also fixes Playwright smoke failures: `useFocusTrap` defers focus restore via
`queueMicrotask` and skips when another modal remains mounted; the collection assign menu restores
focus to the library card root after close.

**Scope**: 12 files, +257/-35 (code ~192 lines, docs ~65 lines)

## Validation Results

| Check | Status |
|-------|--------|
| `npx tsc --noEmit` | Pass (clean) |
| `npm run build` | Pass (per PR description) |
| `npm run test:smoke` | Pass (13/13, per PR description) |
| `cargo test -p crosshook-core` | N/A (no backend changes) |

## Decision: APPROVE

No critical or high-severity issues. Three medium findings are all maintainability/design concerns,
not runtime defects. The phase overlay logic is correct, types are properly narrowed (no implicit
`any`), all five `LaunchPhase` values are handled, and the focus restoration fix is sound. The CSS
additions are accessible (reduced-motion) and compositor-friendly (opacity-only animations).

No security issues found — all data originates from local typed objects, JSX renders text content
(no `dangerouslySetInnerHTML`), and DOM queries use literal selectors.

---

## Findings

### Finding 1 — Dead `'default'` variant in `PipelineNodeTone`

- **File**: `src/crosshook-native/src/types/launch.ts:177`
- **Severity**: medium
- **Category**: Type Safety / Pattern Compliance
- **Status**: Fixed
- **Description**: `PipelineNodeTone = 'default' | 'waiting'` includes `'default'` but it is never
  assigned anywhere in the codebase. Every call site either sets `tone: 'waiting'` or
  `tone: undefined`. Since `tone` is already optional (`tone?: PipelineNodeTone`), the absence of
  tone is naturally expressed as `undefined` — making `'default'` a phantom state with no producer,
  consumer, or CSS rule.

  The guard in `LaunchPipeline.tsx:61` (`node.tone === 'waiting' ? 'waiting' : undefined`) exists
  only to suppress `'default'` from leaking as a `data-tone` attribute. The established precedent
  (`LaunchAutoSaveStatusTone` in the same file) only enumerates variants with actual visual meaning.

- **Suggested fix**: Remove `'default'` from the union:
  ```ts
  export type PipelineNodeTone = 'waiting';
  ```
  Then simplify the JSX to `data-tone={node.tone}` (React does not render `undefined` attributes).

### Finding 2 — Dead `WaitingForTrainer` native branch; no exhaustive switch guard

- **File**: `src/crosshook-native/src/utils/derivePipelineNodes.ts:77-88`, `72-108`
- **Severity**: medium
- **Category**: Correctness / Maintainability
- **Status**: Fixed
- **Description**: Two related issues in `applyPhaseOverlay`:

  (a) The `WaitingForTrainer` branch handles `method === 'native'` (lines 78-80), but per
  `useLaunchState.ts` (`isTwoStepLaunch = method !== 'native'`), `WaitingForTrainer` is only
  dispatched for non-native methods. The native sub-case is dead code. More importantly, the
  `else if (twoStepTrainerFlow)` guard means a future fourth method that is neither `native` nor in
  `twoStepTrainerFlow` would silently fall through with no overlay — a fragile no-op.

  (b) The `switch` has no `default` case. Since `applyPhaseOverlay` returns `nodes` unconditionally
  after the switch, a new `LaunchPhase` enum variant would silently return the unmodified shallow
  copy with no compile-time error. A `default: phase satisfies never` guard would catch this.

  Additionally, the trainer-existence check uses `twoStepTrainerFlow` (method name comparison) in
  `WaitingForTrainer` but `ids.includes('trainer')` (data-driven) in `TrainerLaunching`. The
  inconsistency could diverge if a new method is added.

- **Suggested fix**:
  - Replace `else if (twoStepTrainerFlow)` with `else` (since `native` is already handled)
  - Use `ids.includes('trainer')` uniformly instead of `twoStepTrainerFlow`
  - Add `default: phase satisfies never` to the switch

### Finding 3 — Uncancellable microtask in `useFocusTrap` cleanup

- **File**: `src/crosshook-native/src/hooks/useFocusTrap.ts:217-227`
- **Severity**: medium
- **Category**: Performance / Robustness
- **Status**: Fixed
- **Description**: The cleanup function queues a `queueMicrotask()` callback for deferred focus
  restore, but there is no cancellation path if the component unmounts before the microtask runs.
  The `requestAnimationFrame` in the setup path (line 192) is correctly cancelled via
  `cancelAnimationFrame(frame)` in cleanup, but the microtask has no equivalent.

  In practice the window is narrow (microtasks resolve in the same event-loop turn), but in React
  Strict Mode double-invoke or `flushSync` scenarios, the callback could fire on a stale tree. The
  `document.querySelector('[data-crosshook-focus-root="modal"]')` sentinel is also fragile — if a
  Suspense boundary delays the new modal's render, the sentinel will be absent and focus will
  incorrectly restore.

- **Suggested fix**: Use a ref-backed flag:
  ```ts
  const microtaskSuppressRef = useRef(false);
  // In effect setup: microtaskSuppressRef.current = false;
  // In cleanup: microtaskSuppressRef.current = true;
  // In microtask: if (microtaskSuppressRef.current) return;
  ```

### Finding 4 — `complete` CSS rules split across non-contiguous locations

- **File**: `src/crosshook-native/src/styles/launch-pipeline.css:99` and `159-165`
- **Severity**: low
- **Category**: Maintainability
- **Status**: Fixed
- **Description**: The `::after` connector rule for `[data-status='complete']` (line 99) is placed
  between `configured` and `not-configured`, while the indicator and label rules appear at line
  159-165 after the `active` block. All other statuses keep their three sub-selectors contiguous.

- **Suggested fix**: Consolidate all `complete` rules into one block after the `active` section.

### Finding 5 — `@keyframes` ordering and `@media` placement

- **File**: `src/crosshook-native/src/styles/launch-pipeline.css:153`, `168`, `179`
- **Severity**: low
- **Category**: Maintainability
- **Status**: Fixed
- **Description**: `crosshook-pulse-waiting` (line 153) is defined before `crosshook-pulse` (line
  179), despite `crosshook-pulse` being referenced first (line 127). The `@media
  (prefers-reduced-motion)` block (line 168) falls between the two `@keyframes` blocks, fragmenting
  the animation section.

- **Suggested fix**: Group as: status rules -> all `@keyframes` -> `@media (prefers-reduced-motion)`.

### Finding 6 — Magic string `'Waiting'` in overlay detail

- **File**: `src/crosshook-native/src/utils/derivePipelineNodes.ts:86`
- **Severity**: low
- **Category**: Maintainability
- **Status**: Fixed
- **Description**: `detail: 'Waiting'` is the only free-form display string injected by the overlay
  logic. All other detail values come from resolved data. This sits alongside
  `STATUS_LABEL['active'] = 'Running'` as display copy but lives in the utility rather than the
  label map.

- **Suggested fix**: Either promote to a constant or accept as a one-off contextual override.

### Finding 7 — `returnFocusTo` vs `restoreFocusTo` naming inconsistency

- **File**: `LibraryCard.tsx:17`, `LibraryPage.tsx:58`, `CollectionAssignMenu.tsx:21`
- **Severity**: low
- **Category**: Maintainability
- **Status**: Fixed
- **Description**: The callback parameter and state field use `returnFocusTo` throughout the
  LibraryCard -> LibraryGrid -> LibraryPage chain, while `CollectionAssignMenu` exposes the prop as
  `restoreFocusTo`. The rename at the boundary adds friction when tracing the data flow.

- **Suggested fix**: Standardize on `restoreFocusTo` throughout (the public prop name).

### Finding 8 — `HTMLElement` stored in React state

- **File**: `src/crosshook-native/src/components/pages/LibraryPage.tsx:54-59`
- **Severity**: low
- **Category**: Performance / Pattern Compliance
- **Status**: Fixed
- **Description**: `returnFocusTo: HTMLElement | null` is held in `useState`. React state is
  designed for serializable values that drive rendering. A DOM node in state means React holds a
  hard reference outside the normal ref mechanism. The lifecycle is short (open -> close) and the
  element remains mounted in the grid, so no memory leak occurs. The `isConnected` check in
  `handleClose` handles stale elements correctly.

- **Suggested fix**: Consider a `useRef` for the DOM element to keep it outside the reconciler, or
  add an inline comment explaining the design choice.

### Finding 9 — `data-tone` guard is a tautology given current types

- **File**: `src/crosshook-native/src/components/LaunchPipeline.tsx:61`
- **Severity**: low
- **Category**: Maintainability
- **Status**: Fixed
- **Description**: `data-tone={node.tone === 'waiting' ? 'waiting' : undefined}` tests for
  `'waiting'` then emits `'waiting'`. This guard only exists to prevent the phantom `'default'`
  value (Finding 1) from rendering. Once Finding 1 is resolved, this simplifies to
  `data-tone={node.tone}`.

- **Suggested fix**: Resolve Finding 1 first, then simplify to `data-tone={node.tone}`.

### Finding 10 — Three `findIndex` calls outside `useMemo`

- **File**: `src/crosshook-native/src/components/LaunchPipeline.tsx:36-40`
- **Severity**: low
- **Category**: Performance
- **Status**: Open
- **Description**: Three `findIndex` calls run on every render after the `useMemo` that produces
  `nodes`. For arrays of 3-6 nodes this is entirely negligible (sub-microsecond per search). Could
  be folded into a single pass inside `useMemo` if the pipeline ever grows, but not worth the
  complexity today.

- **Suggested fix**: Leave as-is. Note for future reference only.

---

## Summary Table

| # | Severity | Category | File | Status |
|---|----------|----------|------|--------|
| 1 | medium | Type Safety | `types/launch.ts:177` | Fixed |
| 2 | medium | Correctness | `derivePipelineNodes.ts:77-108` | Fixed |
| 3 | medium | Performance | `useFocusTrap.ts:217-227` | Fixed |
| 4 | low | Maintainability | `launch-pipeline.css:99,159` | Fixed |
| 5 | low | Maintainability | `launch-pipeline.css:153,168,179` | Fixed |
| 6 | low | Maintainability | `derivePipelineNodes.ts:86` | Fixed |
| 7 | low | Maintainability | `LibraryCard/Page/AssignMenu` | Fixed |
| 8 | low | Pattern | `LibraryPage.tsx:54-59` | Fixed |
| 9 | low | Maintainability | `LaunchPipeline.tsx:61` | Fixed |
| 10 | low | Performance | `LaunchPipeline.tsx:36-40` | Open |

**Critical: 0 | High: 0 | Medium: 3 | Low: 7**
