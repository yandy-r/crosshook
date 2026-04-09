# PR #191 Review — Launch Pipeline Stepper Phase 1

**PR**: [#191](https://github.com/yandy-r/crosshook/pull/191) — "feat(ui): add launch pipeline
stepper phase 1" **Head SHA**: `4cd3568b5fb92c2fa4955305b559f139c77b4c52` **Base**: `main`
**Author**: @yandy-r **Reviewer**: `/ycc:code-review --parallel` (3 agents: correctness, security,
quality) **Date**: 2026-04-09 **Decision**: **APPROVE WITH SUGGESTIONS**

---

## Summary

Phase 1 of the launch pipeline visualization adds a horizontal CSS stepper to the Launch page,
replacing the old runner indicator row. The implementation introduces `derivePipelineNodes()` as a
pure helper to derive Tier-1 (config-only) node status from `GameProfile`, a `LaunchPipeline`
component with responsive CSS and accessibility support, shared `PipelineNode` /
`PipelineNodeStatus` types, and integration into `LaunchPanel` / `LaunchPage` with a new required
`profile` prop.

**Validation status**:

| Check                          | Result                                        |
| ------------------------------ | --------------------------------------------- |
| `cargo test -p crosshook-core` | Pass (all tests green)                        |
| `npx tsc --noEmit`             | Pass (clean)                                  |
| Manual smoke                   | Not executed (PR recommends browser dev mode) |

**Why APPROVE WITH SUGGESTIONS**: No security issues. No critical or high-severity bugs. The
implementation is clean, well-structured, and follows project conventions. Three medium-severity
findings should be addressed before or shortly after merge: (1) the `optimizations` node incorrectly
shows "Not configured" when zero optimizations are enabled — which is a valid state — cascading to
make the `launch` node also appear unconfigured for otherwise-complete profiles; (2) orphaned CSS in
`theme.css` from the removed runner indicator; (3) the `steam` node unconditionally returns
`'configured'` without validating Steam config fields. Six additional minor/nitpick findings are
noted for Phase 2 cleanup.

---

## Findings

### F001 — `optimizations` node shows `not-configured` for a valid empty selection

- **Severity**: MEDIUM
- **Category**: Correctness
- **File**: `src/crosshook-native/src/utils/derivePipelineNodes.ts:75`
- **Status**: Fixed

**Description**:
`profile.launch.optimizations.enabled_option_ids.length > 0 ? 'configured' : 'not-configured'`
treats an empty optimization list as misconfigured. However, having zero optimizations enabled is a
deliberate and valid choice. This causes a cascade: the `launch` node checks `allPriorConfigured`
(line 30), which will be `false` when `optimizations` is `'not-configured'`, so a fully-valid
profile (game path, prefix, proton, trainer all set, zero optimizations) shows both the
Optimizations and Launch nodes as "Not configured."

**Suggested fix**: Return `'configured'` unconditionally for the `optimizations` node in Phase 1,
since an empty selection is valid:

```ts
case 'optimizations':
  return 'configured'; // Empty selection is valid; optimizations are optional
```

---

### F002 — Orphaned CSS for the removed runner indicator widget

- **Severity**: MEDIUM
- **Category**: Dead Code
- **File**: `src/crosshook-native/src/styles/theme.css` (lines ~3370–3422, ~4348–4352)
- **Status**: Fixed

**Description**: The PR removes the runner indicator DOM (`crosshook-launch-panel__indicator`,
`__indicator-dot`, `__indicator-row`, `__runner-primary-row`, `__status[data-phase]`) from
`LaunchPanel.tsx`, but ~15 CSS rules in `theme.css` targeting these removed elements remain. This
adds dead bundle weight and will confuse future developers tracing styles.

The `@keyframes crosshook-pulse` (theme.css:3419–3422) is still referenced by
`launch-pipeline.css:122` for the `active` status animation — it cannot be removed but should be
co-located with its only remaining consumer.

**Suggested fix**: Remove orphaned CSS blocks from `theme.css`. Move `@keyframes crosshook-pulse`
into `launch-pipeline.css`.

---

### F003 — `steam` node unconditionally returns `'configured'`

- **Severity**: MEDIUM
- **Category**: Correctness
- **File**: `src/crosshook-native/src/utils/derivePipelineNodes.ts:71`
- **Status**: Fixed

**Description**: The `steam` case in `tier1Status` returns
`method === 'steam_applaunch' ? 'configured' : 'not-configured'`. Since the `steam` node only
appears in the `steam_applaunch` pipeline (per `METHOD_NODE_IDS`), the `'not-configured'` branch is
unreachable. More importantly, the Steam node shows "Ready" regardless of whether
`profile.steam.app_id` or related Steam fields are populated.

**Suggested fix**: Check actual Steam config fields:

```ts
case 'steam':
  return profile.steam.app_id.trim() !== '' ? 'configured' : 'not-configured';
```

---

### F004 — `ResolvedLaunchMethod` defined in two locations

- **Severity**: MINOR
- **Category**: Type Safety / Maintainability
- **Files**: `src/crosshook-native/src/types/launch.ts:139`,
  `src/crosshook-native/src/utils/launch.ts:4`
- **Status**: Fixed

**Description**: `ResolvedLaunchMethod` is independently defined as `Exclude<LaunchMethod, ''>` in
`types/launch.ts` and as `Exclude<GameProfile['launch']['method'], ''>` in `utils/launch.ts`. Both
expand to the same union today but could silently drift. The PR's new files import from
`utils/launch`, while `OnboardingWizard.tsx` imports from `types`. This is a pre-existing issue
deepened by this PR.

**Suggested fix**: Remove the definition from one location and re-export from the other:

```ts
// types/launch.ts — replace local definition with:
export type { ResolvedLaunchMethod } from '../utils/launch';
```

---

### F005 — `tier1Status` accepts untyped `string` for node ID

- **Severity**: MINOR
- **Category**: Maintainability / Type Safety
- **File**: `src/crosshook-native/src/utils/derivePipelineNodes.ts:59`
- **Status**: Fixed

**Description**: `tier1Status(nodeId: string, ...)` takes a bare `string`. The `default` branch
silently returns `'not-configured'`, so a typo in a node ID would cause a subtly wrong status with
no compile-time or runtime error.

**Suggested fix**: Extract a `PipelineNodeId` union type:

```ts
type PipelineNodeId = 'game' | 'wine-prefix' | 'proton' | 'steam' | 'trainer' | 'optimizations' | 'launch';
const METHOD_NODE_IDS: Record<ResolvedLaunchMethod, readonly PipelineNodeId[]> = { ... };
function tier1Status(nodeId: PipelineNodeId, ...): ... { ... }
```

---

### F006 — No memoization on `derivePipelineNodes()` call

- **Severity**: MINOR
- **Category**: Performance
- **File**: `src/crosshook-native/src/components/LaunchPipeline.tsx:31`
- **Status**: Fixed

**Description**: `derivePipelineNodes()` is called on every render without memoization. It produces
a new array reference each time, causing React to re-allocate child elements even when inputs
haven't changed. The function itself is cheap (O(n) over ≤6 nodes), but `profile` is a new object
reference on every `LaunchPage` render, so `useMemo` alone won't fully solve this without also
stabilizing the profile reference upstream.

**Suggested fix**: Add `useMemo` to self-document intent and make a future stable-reference fix
effective immediately:

```tsx
const nodes = useMemo(
  () => derivePipelineNodes(method, profile, preview, phase),
  [method, profile, preview, phase]
);
```

---

### F007 — `crosshook-pulse` keyframe defined externally in `theme.css`

- **Severity**: MINOR
- **Category**: Maintainability
- **File**: `src/crosshook-native/src/styles/launch-pipeline.css:122` → `theme.css:3419`
- **Status**: Fixed

**Description**: `launch-pipeline.css` references the `crosshook-pulse` animation but the
`@keyframes` rule lives in `theme.css`. This creates implicit coupling — if `launch-pipeline.css`
were ever loaded in isolation (Storybook, test renderer), the animation would silently fail.

**Suggested fix**: Co-locate the keyframe in `launch-pipeline.css` (overlap with F002 — moving the
keyframe is part of the orphaned CSS cleanup).

---

### F008 — `display: none` on status text at ≤1023px — accessible but undocumented

- **Severity**: NITPICK
- **Category**: Accessibility
- **File**: `src/crosshook-native/src/styles/launch-pipeline.css:184`
- **Status**: Fixed

**Description**: The `.crosshook-launch-pipeline__node-status` is hidden with `display: none` at
compact breakpoints. This is safe because the status is duplicated in the `aria-label` on each
`<li>`. However, a future developer might not realize why `display: none` is acceptable here and
might try to "fix" it.

**Suggested fix**: Add a brief comment: `/* safe: status is announced via aria-label on <li> */`.

---

### F009 — `currentStepIndex` fallback to `0` on invariant violation

- **Severity**: NITPICK
- **Category**: Correctness
- **File**: `src/crosshook-native/src/components/LaunchPipeline.tsx:35`
- **Status**: Fixed

**Description**: `launchIndex >= 0 ? launchIndex : 0` — the fallback to `0` can never legitimately
occur since every pipeline always ends with a `'launch'` node. If this invariant were ever violated,
`aria-current="step"` would silently land on the first node with no error.

**Suggested fix**: No change required. A defensive comment noting the invariant is sufficient.

---

## Security Assessment

**No security issues found.** All three reviewers independently confirmed:

- Node labels (`NODE_DEFS`) are compile-time constants, never user-derived. React's JSX escaping
  handles rendering safely.
- Profile paths (`executable_path`, `trainer.path`, `prefix_path`) are consumed only as boolean
  checks (`.trim() !== ''`), never rendered into the DOM.
- `data-status` values are constrained to the `PipelineNodeStatus` union type. Profile data cannot
  influence these values.

---

## Performance Assessment

**No high-severity performance issues.** Minor notes:

- `derivePipelineNodes()` is O(n) for n≤6 — negligible cost per call.
- `crosshook-pulse` animates only `opacity` — GPU-composited, no layout or paint impact.
- `color-mix(in srgb, ...)` is supported in WebKitGTK 4.1+ (Tauri v2 target).
- Connector `::after` transitions `background-color` and `opacity` — paint only, 5 elements at 2px
  height, 140ms duration. No jank risk.

---

## Validation Results

| Level        | Status | Notes                                             |
| ------------ | ------ | ------------------------------------------------- |
| Type Check   | Pass   | `npx tsc --noEmit` — clean                        |
| Rust Tests   | Pass   | `cargo test -p crosshook-core` — all green        |
| Build        | N/A    | PR body claims pass; not re-verified in review    |
| Manual Smoke | N/A    | PR recommends `./scripts/dev-native.sh --browser` |

---

## Review Verdict

| Severity | Count | Findings               |
| -------- | ----- | ---------------------- |
| CRITICAL | 0     | —                      |
| HIGH     | 0     | —                      |
| MEDIUM   | 3     | F001, F002, F003       |
| MINOR    | 4     | F004, F005, F006, F007 |
| NITPICK  | 2     | F008, F009             |

**Decision: APPROVE WITH SUGGESTIONS**

The implementation is well-structured, accessible, and follows project conventions. No security or
critical correctness issues. The three MEDIUM findings (F001 optimizations false negative, F002
orphaned CSS, F003 steam node always configured) are recommended before merge but none are blocking.
MINOR and NITPICK findings are suitable for Phase 2 follow-up.
