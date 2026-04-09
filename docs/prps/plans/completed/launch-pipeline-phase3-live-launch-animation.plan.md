# Plan: Launch Pipeline Visualization — Phase 3 (Live Launch Animation)

## Summary

Overlay the existing Tier 1/Tier 2 pipeline status with live launch phase feedback so the pipeline
shows which step is currently running and which steps have completed. Phase 3 stays frontend-only:
it reuses the existing `LaunchPhase`, `helperLogPath`, and guidance copy that already flow through
`LaunchPanel`, and adds no new IPC commands, backend types, persistence, or dependencies.

The implementation should keep the current pipeline as the single visual source of truth in the
runner stack. When the user launches the game or trainer, node status transitions should reflect the
active phase immediately, preserve preview/config-derived detail when idle, and fall back cleanly to
the existing non-live pipeline once the session returns to `Idle`.

## User Story

As a **quick launcher or profile configurator**, I want the launch pipeline to animate through the
current launch step so that I can tell at a glance what CrossHook is doing without interpreting the
separate runner text.

## Problem → Solution

**Current state**: `LaunchPipeline` already renders Tier 1/Tier 2 node status, but
`derivePipelineNodes()` ignores its `phase` argument and the component always treats the first
problem node or the final launch node as the current step. The CSS file includes `active` /
`complete` selectors, but they are never driven by real launch-phase state.

**Desired state**: `derivePipelineNodes()` overlays runtime phase onto the existing node array,
mapping `GameLaunching`, `WaitingForTrainer`, `TrainerLaunching`, and `SessionActive` to concrete
node states. `LaunchPipeline` renders those overlays consistently, including a waiting-specific tone
for the trainer handoff, while `LaunchPanel` continues to show `helperLogPath` and
`launchGuidanceText` below the pipeline without adding a second competing phase indicator.

## Metadata

- **Complexity**: Medium
- **Source PRD**: `docs/prps/prds/launch-pipeline-visualization.prd.md`
- **PRD Phase**: Phase 3 — Tier 3 Live Launch Animation
- **Estimated Files**: 4

---

## UX Design

### Before

```text
[Game] [Wine Prefix] [Proton] [Trainer] [Optimizations] [Launch]
 Ready    Ready         Ready    Ready      3 env vars        Command ready

Log: /path/to/log
Ready to launch the game — The game starts first. The trainer is launched in the second step.
```

### After

```text
Game pulse -> Game done / Trainer waiting -> Trainer pulse -> Launch pulse

[Game] [Wine Prefix] [Proton] [Trainer] [Optimizations] [Launch]
 Running   Ready         Ready    Waiting     3 env vars        Command ready

Log: /path/to/log
Launching the game through Proton. / Game launch is ready. Start the trainer when ready.
```

### Interaction Changes

| Touchpoint                  | Before                                      | After                                                        | Notes                                  |
| --------------------------- | ------------------------------------------- | ------------------------------------------------------------ | -------------------------------------- |
| Launch pipeline             | Static readiness / preview status only      | Live phase overlay updates the active node during launch     | No click targets added                 |
| Waiting-for-trainer handoff | Only status copy changes below the pipeline | Game node shows complete and trainer node shows waiting tone | Preserves two-step launch mental model |
| Session-active feedback     | Text says session is active                 | Launch node becomes the active summary node                  | Avoids duplicate phase widgets         |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                      | Lines                             | Why                                                                             |
| -------------- | --------------------------------------------------------- | --------------------------------- | ------------------------------------------------------------------------------- |
| P0 (critical)  | `src/crosshook-native/src/utils/derivePipelineNodes.ts`   | 1-208                             | Current Tier 1/Tier 2 derivation and the unused `phase` input live here         |
| P0 (critical)  | `src/crosshook-native/src/components/LaunchPipeline.tsx`  | 1-71                              | Current render contract, status labels, and `aria-current` logic                |
| P0 (critical)  | `src/crosshook-native/src/hooks/useLaunchState.ts`        | 51-104, 260-304, 306-381, 425-531 | Source of `LaunchPhase`, helper log path, and guidance text transitions         |
| P1 (important) | `src/crosshook-native/src/types/launch.ts`                | 160-183                           | `PipelineNodeId`, `PipelineNodeStatus`, and `PipelineNode` contract             |
| P1 (important) | `src/crosshook-native/src/styles/launch-pipeline.css`     | 1-252                             | Existing status selectors, motion tokens, breakpoints, and connector styling    |
| P1 (important) | `src/crosshook-native/src/components/LaunchPanel.tsx`     | 899-908                           | Runner-stack integration that must stay intact                                  |
| P2 (reference) | `src/crosshook-native/src/lib/mocks/handlers/launch.ts`   | 103-184, 211-260                  | Browser-mode launch and preview fixtures used for manual validation             |
| P2 (reference) | `src/crosshook-native/src/context/LaunchStateContext.tsx` | 18-45                             | Shows how `LaunchPipeline` gets state through the shared launch session context |

## External Documentation

| Topic             | Source      | Key Takeaway                                                               |
| ----------------- | ----------- | -------------------------------------------------------------------------- |
| External research | None needed | Phase 3 uses existing React, CSS, and app-local launch state patterns only |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION

```ts
// SOURCE: src/crosshook-native/src/components/LaunchPipeline.tsx:15-29
const STATUS_ICON: Record<PipelineNodeStatus, string> = { configured: '✓', active: '●' };
const STATUS_LABEL: Record<PipelineNodeStatus, string> = { configured: 'Ready', active: 'Running' };
```

Keep type names in `PascalCase`, helper maps in `UPPER_SNAKE_CASE`, and DOM classes in
`crosshook-launch-pipeline__*` BEM form.

### TYPE_CONTRACT

```ts
// SOURCE: src/crosshook-native/src/types/launch.ts:160-183
export type PipelineNodeStatus = 'configured' | 'not-configured' | 'error' | 'active' | 'complete';
export interface PipelineNode {
  id: PipelineNodeId;
  status: PipelineNodeStatus;
}
```

Prefer extending the existing pipeline DTO with a small optional field for live-tone metadata rather
than introducing a parallel data structure.

### STATE_FLOW

```ts
// SOURCE: src/crosshook-native/src/hooks/useLaunchState.ts:64-93
case 'game-start': return { ...state, phase: LaunchPhase.GameLaunching };
case 'game-success': return { phase: action.nextPhase, helperLogPath: action.helperLogPath };
case 'trainer-start': return { ...state, phase: LaunchPhase.TrainerLaunching };
case 'trainer-success': return { phase: LaunchPhase.SessionActive, helperLogPath: action.helperLogPath };
```

Treat `useLaunchState` as the sole source of live phase truth. Do not invent new panel-local launch
state for the pipeline.

### ERROR_HANDLING

```ts
// SOURCE: src/crosshook-native/src/utils/derivePipelineNodes.ts:155-177
if (fatalIssue) return { id, label, status: 'error', detail: fatalIssue.message };
if (!isTier2Resolved(id, preview)) return { id, label, status: 'not-configured', detail: 'Not configured' };
return { id, label, status: 'configured', detail };
```

Live overlays must respect existing error / not-configured precedence when the phase is `Idle`, and
should only override the specific nodes required by the active phase.

### CSS_STATE_SELECTOR

```css
/* SOURCE: src/crosshook-native/src/styles/launch-pipeline.css:118-143 */
.crosshook-launch-pipeline__node[data-status='active'] .crosshook-launch-pipeline__node-indicator {
  animation: crosshook-pulse 2s ease-in-out infinite;
}
@keyframes crosshook-pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.45;
  }
}
```

Keep animation CSS driven by `data-*` attributes and shared motion tokens instead of inline styles.

### CURRENT_STEP_SELECTION

```ts
// SOURCE: src/crosshook-native/src/components/LaunchPipeline.tsx:36-42
const firstIssueIdx = nodes.findIndex(
  (n) => n.id !== 'launch' && (n.status === 'not-configured' || n.status === 'error')
);
const launchIndex = nodes.findIndex((n) => n.id === 'launch');
const currentStepIndex = firstIssueIdx >= 0 ? firstIssueIdx : launchIndex >= 0 ? launchIndex : 0;
```

Replace this fallback with active-phase-first selection: live active/waiting node, then first issue,
then launch node.

### MOCK_VALIDATION_AND_EVENTS

```ts
// SOURCE: src/crosshook-native/src/lib/mocks/handlers/launch.ts:121-124
setTimeout(() => {
  emitMockEvent('launch-complete', { code: 0, signal: null });
}, completeDelay);
```

Rely on the existing mock launch flow for browser-only validation; do not add a second mock event
channel just for the pipeline.

### TEST_CONSTRAINT

```json
// SOURCE: src/crosshook-native/package.json:7-15
"build": "tsc && vite build",
"test:smoke": "playwright test"
```

There is no existing frontend unit-test runner in this package, so validation must be build-based
plus manual/browser smoke coverage unless the implementation explicitly introduces a higher-value
test surface.

---

## Files to Change

| File                                                     | Action | Justification                                                                                        |
| -------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/types/launch.ts`               | UPDATE | Extend the pipeline node contract with a minimal live-tone field for the waiting state               |
| `src/crosshook-native/src/utils/derivePipelineNodes.ts`  | UPDATE | Consume `phase`, overlay live node status, and keep Tier 1/Tier 2 fallback intact                    |
| `src/crosshook-native/src/components/LaunchPipeline.tsx` | UPDATE | Render live-phase-first current step selection and expose any new tone/status metadata to the DOM    |
| `src/crosshook-native/src/styles/launch-pipeline.css`    | UPDATE | Add waiting-specific visual treatment, complete-connector styling, and reduced-motion-safe animation |

## Storage / Persistence

No new persisted data is required for Phase 3.

| Datum                        | Classification | Notes                                                                                  |
| ---------------------------- | -------------- | -------------------------------------------------------------------------------------- |
| Live pipeline node overlay   | Runtime-only   | Derived from existing `LaunchPhase` + node status on each render                       |
| Waiting/active tone metadata | Runtime-only   | UI-only helper field; never written to TOML or SQLite                                  |
| Helper log path display      | Runtime-only   | Already provided by `useLaunchState`; Phase 3 only keeps it visible below the pipeline |

### Persistence & Usability

- **Migration / backward compatibility**: No new settings, schemas, or IPC payload migrations.
- **Offline behavior**: Unchanged; the live overlay only depends on in-memory launch state.
- **Degraded behavior**: When `phase === Idle`, the pipeline reverts to existing config/preview-derived status.
- **User visibility / editability**: Read-only visualization; users still control launch behavior from the existing form and action buttons.

## NOT Building

- New Tauri commands, backend events, or schema changes for pipeline progress.
- Per-node interactivity, navigation, tooltips, or popovers; those remain Phase 4 candidates.
- Preview staleness indicators or helper-log relocation changes beyond preserving the current placement below the pipeline.
- New standalone phase indicator widgets outside the existing runner stack.

---

## Step-by-Step Tasks

### Task 1: Extend the pipeline node contract for live-tone metadata

- **ACTION**: Update `src/crosshook-native/src/types/launch.ts`.
- **IMPLEMENT**: Add a minimal optional field such as `tone?: 'default' | 'waiting'` to `PipelineNode`
  so the trainer handoff can render amber while keeping `PipelineNodeStatus` limited to the existing
  union. Export the tone type next to `PipelineNodeStatus`.
- **MIRROR**: `TYPE_CONTRACT`, `NAMING_CONVENTION`
- **IMPORTS**: Type-only exports in `src/crosshook-native/src/types/launch.ts`
- **GOTCHA**: Do not add `'waiting'` to `PipelineNodeStatus`; that would force avoidable refactors
  across status-label maps and CSS selectors that already understand `active` / `complete`.
- **VALIDATE**: `npm run build` succeeds with no type errors from `LaunchPipeline.tsx` or
  `derivePipelineNodes.ts`.

### Task 2: Refactor `derivePipelineNodes()` into base-state + live-overlay phases

- **ACTION**: Update `src/crosshook-native/src/utils/derivePipelineNodes.ts`.
- **IMPLEMENT**: Keep the current Tier 1/Tier 2 node construction as the base layer, then add a
  small `applyPhaseOverlay(nodes, method, phase)` helper that returns a new array. Map
  `GameLaunching` to `game=active`, `WaitingForTrainer` to `game=complete` plus `trainer=active`
  with `tone='waiting'`, `TrainerLaunching` to `game=complete` plus `trainer=active`,
  `SessionActive` to `launch=active` with prior live steps marked `complete`, and `Idle` to the
  untouched base nodes.
- **MIRROR**: `STATE_FLOW`, `ERROR_HANDLING`
- **IMPORTS**: `LaunchPhase`, `PipelineNode`, `PipelineNodeId`, optional new tone type
- **GOTCHA**: Preserve immutability. Build the base nodes first, then overlay by returning copied
  node objects; never mutate the array generated by Tier 1/Tier 2 logic in place.
- **VALIDATE**: Confirm phase-to-node mapping is exhaustive for `LaunchPhase` and does not change
  idle behavior.

### Task 3: Keep live overlays method-aware and sequence-correct

- **ACTION**: Finalize the phase mapping rules inside `src/crosshook-native/src/utils/derivePipelineNodes.ts`.
- **IMPLEMENT**: Encode the overlay rules around actual method-specific node sets. `native` must
  skip trainer/waiting behavior entirely, `steam_applaunch` and `proton_run` must both preserve the
  trainer handoff, and completed states should only be assigned to nodes the user has already passed
  in that flow.
- **MIRROR**: `STATE_FLOW`, `CURRENT_STEP_SELECTION`
- **IMPORTS**: Existing `ResolvedLaunchMethod` and node-id definitions
- **GOTCHA**: `useLaunchState` never emits `WaitingForTrainer` for native launches, but the overlay
  logic should still fail safe if that impossible state is ever encountered.
- **VALIDATE**: Verify each method produces sensible nodes for `Idle`, `GameLaunching`,
  `WaitingForTrainer`, `TrainerLaunching`, and `SessionActive`.

### Task 4: Update `LaunchPipeline` to prioritize live phase in rendering and accessibility

- **ACTION**: Update `src/crosshook-native/src/components/LaunchPipeline.tsx`.
- **IMPLEMENT**: Read the new node metadata, emit `data-tone` for waiting-specific styling, and
  compute `aria-current` from the first active node before falling back to the existing first-issue /
  launch-node behavior. Keep `statusText = node.detail || STATUS_LABEL[node.status]` so live overlay
  detail text doubles as the visible label.
- **MIRROR**: `NAMING_CONVENTION`, `CURRENT_STEP_SELECTION`
- **IMPORTS**: Optional new tone type from `../types/launch`
- **GOTCHA**: Do not duplicate phase derivation in the component. Rendering should consume the node
  model, not reinterpret `LaunchPhase` directly.
- **VALIDATE**: Confirm the runner stack still renders `helperLogPath` and `launchGuidanceText`
  beneath the pipeline without any component API changes to `LaunchPanel`.

### Task 5: Add waiting, active, and complete presentation rules in CSS

- **ACTION**: Update `src/crosshook-native/src/styles/launch-pipeline.css`.
- **IMPLEMENT**: Keep the existing `data-status` selectors, add a waiting-specific variant such as
  `[data-status='active'][data-tone='waiting']`, and style completed connectors so the sequence reads
  visually from left to right while remaining consistent with the current color tokens. Reuse
  `--crosshook-transition-fast`, `--crosshook-color-warning`, `--crosshook-color-success`, and add a
  `prefers-reduced-motion` override if the new waiting/active pulse differs from the current one.
- **MIRROR**: `CSS_STATE_SELECTOR`
- **IMPORTS**: `src/crosshook-native/src/styles/variables.css` color and motion tokens only
- **GOTCHA**: Keep layout breakpoints unchanged unless a visual regression forces a minimal tweak;
  Phase 3 should not reopen the responsive layout work already done in phases 1/2.
- **VALIDATE**: Inspect both normal motion and reduced-motion behavior and ensure the compact / vertical breakpoints still render legibly.

### Task 6: Validate the live overlay in both browser mocks and real Tauri flow

- **ACTION**: Run the phase-specific verification pass after the code changes land.
- **IMPLEMENT**: Use browser dev mode to exercise mock `launch_game` and `launch_trainer` flows and
  confirm the pipeline transitions through running / waiting / complete states. Then re-run the same
  checks in full Tauri dev mode to verify the real event timing matches the mock assumptions.
- **MIRROR**: `MOCK_VALIDATION_AND_EVENTS`, `TEST_CONSTRAINT`
- **IMPORTS**: None
- **GOTCHA**: `launch-complete` does not currently change `phase`, so validation should focus on the
  reducer-driven phase transitions, not the terminal mock event.
- **VALIDATE**: Capture one Proton/Steam two-step launch path and one native launch path before
  marking the phase complete.

---

## Testing Strategy

### Unit Tests

| Test                                            | Input                                                                     | Expected Output                                             | Edge Case? |
| ----------------------------------------------- | ------------------------------------------------------------------------- | ----------------------------------------------------------- | ---------- |
| Phase overlay helper returns base nodes on idle | Existing Tier 1/Tier 2 node array + `LaunchPhase.Idle`                    | Unchanged node statuses and detail                          | Yes        |
| Two-step flow marks trainer as waiting          | `proton_run` or `steam_applaunch` nodes + `LaunchPhase.WaitingForTrainer` | `game=complete`, `trainer=active`, `trainer.tone='waiting'` | Yes        |
| Native flow skips trainer overlay               | `native` nodes + each non-idle phase                                      | No trainer references introduced                            | Yes        |
| Session-active summary targets launch node      | Ready node array + `LaunchPhase.SessionActive`                            | `launch=active`, prior runtime nodes `complete`             | No         |

### Edge Cases Checklist

- [ ] Idle state after a successful preview still shows preview-derived detail text.
- [ ] Native launch never renders waiting/trainer states.
- [ ] Waiting-for-trainer state is visually distinct from active-running state.
- [ ] Existing error / not-configured states reappear correctly after reset or idle fallback.
- [ ] Compact and vertical breakpoints still render the live states legibly.
- [ ] Reduced-motion users do not get forced pulse animation.

---

## Validation Commands

### Static Analysis

```bash
npm run build
```

EXPECT: Zero TypeScript or Vite build errors from the updated pipeline types, utility, component, and CSS imports.

### Unit Tests

```bash
# No dedicated frontend unit-test runner is configured in this package today.
# Prefer targeted implementation-time checks or smoke/manual validation over introducing low-value test scaffolding.
npm run build
```

EXPECT: Build passes and the phase overlay logic remains type-safe.

### Full Test Suite

```bash
npm run test:smoke
```

EXPECT: Existing Playwright smoke coverage remains green if the suite is available in the current environment.

### Browser Validation

```bash
./scripts/dev-native.sh --browser
./scripts/dev-native.sh
```

EXPECT: Browser dev mode reproduces the live phase sequence with mock handlers, and full Tauri dev mode reproduces the same sequence with real launch-state transitions.

### Manual Validation

- [ ] Open the Launch page with a `proton_run` profile that already has a healthy preview.
- [ ] Click `Launch Game` and verify the Game node becomes active immediately.
- [ ] Wait for the handoff and verify Game becomes complete while Trainer enters a waiting state.
- [ ] Click `Launch Trainer` and verify Trainer becomes active, then Launch becomes active when the session is live.
- [ ] Repeat with a `steam_applaunch` profile and confirm the same two-step behavior.
- [ ] Repeat with a `native` profile and confirm no trainer/waiting state appears.
- [ ] Confirm `Log: ...` and the existing guidance line remain visible below the pipeline throughout the flow.
- [ ] Toggle reduced-motion at the OS/browser level and verify pulse animation stops or degrades gracefully.

---

## Acceptance Criteria

- [ ] `derivePipelineNodes()` consumes `phase` and overlays live status for `GameLaunching`,
      `WaitingForTrainer`, `TrainerLaunching`, `SessionActive`, and `Idle`.
- [ ] Waiting-for-trainer is visually distinct without widening `PipelineNodeStatus`.
- [ ] `LaunchPipeline` marks the live node as `aria-current="step"` before falling back to issue-based selection.
- [ ] `helperLogPath` and `launchGuidanceText` remain in the runner stack below the pipeline.
- [ ] No new IPC commands, backend event shapes, persisted data, or dependencies are introduced.

## Completion Checklist

- [ ] Code follows the existing pipeline BEM naming and `data-status` selector pattern.
- [ ] Live overlay logic stays centralized in `derivePipelineNodes()`.
- [ ] Type additions are minimal and runtime-only.
- [ ] CSS uses existing tokens from `variables.css`.
- [ ] Manual validation was performed in browser mock mode and real Tauri mode.
- [ ] No unnecessary scope additions from Phase 4 were pulled into this phase.
- [ ] Plan remains self-contained for a single implementation pass.

## Risks

| Risk                                                         | Likelihood | Impact | Mitigation                                                                                      |
| ------------------------------------------------------------ | ---------- | ------ | ----------------------------------------------------------------------------------------------- |
| Waiting state gets modeled as a new status instead of a tone | Medium     | Medium | Keep `waiting` as optional tone metadata layered on top of `active`                             |
| Live overlay hides real validation errors after launch/reset | Medium     | High   | Apply phase overlay only for non-idle phases and preserve the base node array for idle fallback |
| Mock timing diverges from real Tauri event timing            | Medium     | Medium | Validate both `./scripts/dev-native.sh --browser` and full `./scripts/dev-native.sh`            |
| New animation causes accessibility or Steam Deck regressions | Low        | Medium | Reuse existing breakpoints, add reduced-motion handling, and keep the visual delta small        |

## Notes

- `LaunchPanel` already places `helperLogPath` and `launchGuidanceText` below the pipeline; Phase 3
  should preserve that integration rather than redesign it.
- `launch-complete` is currently a terminal event with no reducer phase change. That is acceptable
  for this phase because the live overlay is driven by the reducer's existing `LaunchPhase`, not by
  the completion event itself.
- If implementation reveals that the waiting state cannot be expressed cleanly with a tone field,
  stop and reassess before widening the shared pipeline status contract.
