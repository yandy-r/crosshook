# Launch Pipeline Visualization

> **Source**: [GitHub issue #74](https://github.com/yandy-r/crosshook/issues/74) — promoted from P3
> (2/8 support) to active scope **Status**: DRAFT — needs validation **Generated**: 2026-04-09

---

## Problem Statement

CrossHook's launch configuration is presented as a form with disconnected sections (profile fields,
sub-tabs for optimizations/env/gamescope, runner indicator). There is no visual representation of
the **sequential relationship** between launch steps — users must mentally reconstruct what happens
at launch time from scattered form fields. The runner indicator area
(`"Proton runner selected" + Idle`) communicates method and phase but not **configuration
completeness** or the **launch chain** that their profile actually produces.

For users configuring complex Proton + Trainer + Optimization stacks, this disconnect means they
cannot tell at a glance whether their launch chain is complete, what will actually execute, or where
a misconfiguration lies.

## Evidence

- **GitHub issue #74** explicitly identifies the "disconnected sections" pain and proposes a
  directed chain visualization.
- **Codebase confirms the gap**: No pipeline/chain/flow/step visualization component exists —
  searches for `crosshook-pipeline`, `crosshook-step`, `crosshook-chain`, `crosshook-flow` CSS
  classes returned zero results.
- **The data already exists**: `build_launch_preview()`
  (`crates/crosshook-core/src/launch/preview.rs:267-414`) computes all pipeline steps and returns
  per-section results via `LaunchPreview`. The `LaunchPhase` enum models the runtime execution flow.
  The visualization just needs to surface this data visually.
- **Existing runner indicator is underutilized**: The `crosshook-launch-panel__runner-stack` div
  shows method + phase as plain text. This real estate can communicate far more with a visual chain.
- **Market gap**: None of the four major Linux game launchers (Lutris, Heroic, Bottles, GameHub)
  visualize their launch pipeline. All use flat tab-based or list-based settings. This is a
  **first-in-category differentiator**.
- **Research context**: Deep research report
  (`docs/research/additional-features/deep-research-report.md`) scored pipeline visualization at 2/8
  support, High effort, Medium readiness. Promoted because most higher-priority issues are now
  implemented.

## Proposed Solution

Build a **read-only pipeline visualization** that replaces the current runner indicator area in
`LaunchPanel`. The pipeline renders as a CSS-only horizontal stepper showing connected nodes — one
per logical launch step — that **adapts to the active launch method** (different methods show
different node sets). Each node displays its configuration status (configured / not configured /
error) derived from existing profile and preview data.

The pipeline operates in three progressive fidelity modes:

1. **Config-derived** (default): Node status derived from `GameProfile` field presence checks.
   Available immediately, no IPC call needed.
2. **Preview-derived**: After user clicks "Preview", node status upgrades to reflect the resolved
   `LaunchPreview` output (file existence checks, directive resolution, validation results).
3. **Live launch**: After user clicks "Launch", nodes **animate to reflect runtime phase** — the
   active node pulses during its execution phase (`GameLaunching`, `WaitingForTrainer`,
   `TrainerLaunching`, `SessionActive`).

### Pipeline nodes per launch method

| Method            | Nodes (left to right)                                                                   |
| ----------------- | --------------------------------------------------------------------------------------- |
| `proton_run`      | Game &rarr; Wine Prefix &rarr; Proton &rarr; Trainer &rarr; Optimizations &rarr; Launch |
| `steam_applaunch` | Game &rarr; Steam &rarr; Trainer &rarr; Optimizations &rarr; Launch                     |
| `native`          | Game &rarr; Trainer &rarr; Launch                                                       |

The Trainer node is **always shown** — when no trainer is configured, it displays "Not configured"
status. This communicates the capability exists and keeps the chain length consistent for a given
method.

### Node status states

| Status             | Visual                                       | When                                      |
| ------------------ | -------------------------------------------- | ----------------------------------------- |
| **Configured**     | Green indicator + checkmark + label          | Step has all required data                |
| **Not configured** | Gray indicator + dash + "Not configured"     | Step's fields are empty / not applicable  |
| **Error**          | Red indicator + X + error hint               | Validation found a fatal or warning issue |
| **Active**         | Blue indicator + pulse animation + "Running" | Step is currently executing during launch |
| **Complete**       | Green indicator + checkmark (solid)          | Step completed successfully during launch |

### Why this approach

- **vs. a new interactive control surface (toggles per step)**: The existing toggles
  (`launch_trainer_only`, `launch_game_only`) and sub-tab panels are the right control surfaces. The
  pipeline is a **status communicator**, not a settings editor. Adding toggles per node would create
  redundant control paths and increase complexity significantly.
- **vs. enhancing the Preview modal only**: The Preview modal is a deep-dive view opened on demand.
  The pipeline should be **always-visible** context in the launch panel — the point is to
  communicate chain status at a glance without requiring a modal.
- **vs. a node-based workflow editor**: Over-engineered for a linear pipeline. The launch chain is
  always sequential (no branching, no fan-out). A simple stepper is the right abstraction.
- **vs. vertical pipeline layout**: Horizontal is more space-efficient for 3-6 nodes and matches the
  left-to-right "flow" mental model of a launch sequence. Vertical is used only as a responsive
  fallback on small screens.

## Key Hypothesis

We believe a **read-only pipeline visualization** that replaces the runner indicator area will give
users **immediate visual confidence** in their launch chain's completeness. We'll know we're right
when users report that they can **identify misconfigured steps at a glance** instead of hunting
through form fields, and when the configuration-related questions/issues on GitHub decrease.

## What We're NOT Building

| Out of Scope                                      | Why                                                                                                                                                                                                                          |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Per-step toggles / interactive controls**       | v1 is read-only. Toggles create redundant control paths with existing launch settings.                                                                                                                                       |
| **Real-time streaming of backend pipeline steps** | The backend has no step-by-step progress callback. Each `build_launch_preview()` step either succeeds or produces a partial result. Adding streaming would require significant Rust refactoring for purely cosmetic benefit. |
| **New IPC commands or backend types**             | All data needed is already in `LaunchPreview`, `GameProfile`, and `LaunchPhase`.                                                                                                                                             |
| **Pipeline visualization in the Preview modal**   | v2 — the modal already has structured sections. Adding a second pipeline view inside it would be redundant with the always-visible panel pipeline.                                                                           |
| **Animated connector lines / particle effects**   | CSS transitions on node state changes are in scope; decorative animation is not.                                                                                                                                             |
| **Custom node ordering or hiding**                | Nodes are determined by launch method, not user preference.                                                                                                                                                                  |
| **DAG / branching visualization**                 | The launch pipeline is strictly linear. No fan-out or conditional branching needed.                                                                                                                                          |

## Success Metrics

| Metric                                                                                               | Target                                                                     | How Measured                                         |
| ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ---------------------------------------------------- |
| **Comprehension** — users can identify misconfigured steps without opening Preview                   | Qualitative: positive feedback on GitHub/Discord within 4 weeks            | GitHub Discussions + issue reports                   |
| **Runner indicator parity** — pipeline conveys at least as much info as the current runner indicator | Method + phase + validation status all visible at a glance                 | Manual verification against current indicator        |
| **Zero regression** — existing launch functionality unaffected                                       | All existing launch flows (game, trainer, preview, reset) work identically | Manual testing + `cargo test -p crosshook-core`      |
| **Performance** — no perceptible render delay                                                        | Pipeline render < 16ms (single frame)                                      | Browser DevTools profiling in dev mode               |
| **Steam Deck** — usable at 1280x800                                                                  | All nodes visible and status readable without horizontal scrolling         | Manual testing on Steam Deck or at 1280x800 viewport |

## Open Questions

- [ ] **Exact breakpoint for horizontal-to-compact layout**: The Steam Deck viewport is 1280x800.
      With sidebar and padding, the usable content width is ~900-1000px. At 6 nodes, each gets
      ~150-166px. This may work horizontally with compact labels, or may need a condensed icon-only
      mode. Needs visual prototype to decide — resolve in plan phase.
- [ ] **Tooltip / popover on node hover/focus**: Should hovering a node show a tooltip with more
      detail (e.g., the resolved path for Wine Prefix, the specific validation error)? This adds
      value but increases implementation scope. **Recommendation**: include in v1 as progressive
      disclosure, but defer if effort is high.
- [ ] **"Launch" node semantics during live phase**: The rightmost "Launch" node is a summary node —
      when does it transition from "Configured" to "Active" to "Complete"? Options: (a) Active
      during `SessionActive` phase, (b) Complete once game process exits, (c) mirrors overall
      pipeline status. **Recommendation**: (a) — Active during `SessionActive`, returns to
      Configured after session ends.
- [ ] **Helper log path display**: The current runner indicator shows `helperLogPath`. Where does
      this go in the pipeline layout? Options: below the pipeline as a subtle line (like current),
      or inside a tooltip on the Launch node.

---

## Users & Context

### Primary User — Profile Configurator

- **Who**: A CrossHook user actively configuring or debugging a launch profile. May be setting up
  Proton for the first time, adding a trainer, or troubleshooting why a launch fails.
- **Current behavior**: Fills in profile fields across tabs → clicks Preview to check → reads the
  collapsible sections in the Preview modal → identifies what's wrong → goes back to fix it →
  repeats.
- **Trigger**: "I changed the Proton version, is my launch chain still valid?" or "Why is this
  profile failing to launch?"
- **Success state**: Glances at the pipeline → immediately sees the Wine Prefix node is red → knows
  exactly where to look.

### Secondary User — Quick Launcher

- **Who**: A user with a fully configured profile who just wants to launch and go.
- **Current behavior**: Selects profile → clicks Launch Game → glances at phase status to confirm
  it's running.
- **Trigger**: "Launch this game now."
- **Success state**: Pipeline shows all green → clicks Launch → watches nodes pulse through the
  sequence → game is running.

### Tertiary User — Steam Deck User

- **Who**: Using CrossHook on Steam Deck with controller input.
- **Current behavior**: Navigates with D-pad/analog stick, selects profile, launches.
- **Trigger**: Same as Quick Launcher, but with controller.
- **Success state**: Pipeline is readable at 1280x800 and doesn't require precise pointer
  interaction (it's read-only).

---

## Detailed Requirements

### FR-1: Pipeline Component

A new `LaunchPipeline` React component renders a horizontal chain of nodes connected by lines.

- **Inputs**: `GameProfile`, `LaunchPreview | null`, `LaunchPhase`, `LaunchMethod`
- **Output**: A horizontal stepper where each node is a status indicator (icon + label + status
  text) connected by `::after` pseudo-element lines
- **Node set**: Determined by `LaunchMethod` per the table in Proposed Solution
- **Status derivation**: See FR-2

### FR-2: Three-Tier Status Derivation

Node status is derived progressively:

**Tier 1 — Config-derived** (when `LaunchPreview` is null):

| Node          | Configured                                                        | Not Configured                     |
| ------------- | ----------------------------------------------------------------- | ---------------------------------- |
| Game          | `profile.game.path` is non-empty                                  | path empty                         |
| Wine Prefix   | `profile.runtime.wine_prefix` is non-empty                        | field empty                        |
| Proton        | `profile.runtime.proton_path` is non-empty                        | field empty                        |
| Steam         | method is `steam_applaunch` (always configured if method matches) | —                                  |
| Trainer       | `profile.trainer.path` is non-empty                               | path empty                         |
| Optimizations | `profile.launch.optimizations` has ≥1 enabled ID                  | no optimizations enabled           |
| Launch        | all prior nodes are "Configured"                                  | any prior node is "Not configured" |

**Tier 2 — Preview-derived** (when `LaunchPreview` is available): Upgrades Tier 1 with resolved data
from `LaunchPreview`:

| Node          | Error when                                           | Configured upgrades to                    |
| ------------- | ---------------------------------------------------- | ----------------------------------------- |
| Game          | `validation.issues` has fatal with game path context | `game_executable` resolved                |
| Wine Prefix   | `validation.issues` has fatal with prefix context    | `proton_setup.wine_prefix_path` resolved  |
| Proton        | `validation.issues` has fatal with proton context    | `proton_setup.proton_executable` resolved |
| Steam         | `validation.issues` has fatal with steam context     | Steam launch options resolved             |
| Trainer       | `validation.issues` has fatal with trainer context   | `trainer.path` resolved                   |
| Optimizations | `directives_error` is non-null                       | directives resolved, env vars present     |
| Launch        | any fatal validation issue                           | `effective_command` resolved, no fatals   |

**Tier 3 — Live launch** (during active launch): Overlays runtime phase animation on top of Tier 1
or 2 status:

| LaunchPhase         | Node animation                                                        |
| ------------------- | --------------------------------------------------------------------- |
| `GameLaunching`     | Game node pulses blue                                                 |
| `WaitingForTrainer` | Game node solid green (complete), Trainer node pulses amber (waiting) |
| `TrainerLaunching`  | Trainer node pulses blue                                              |
| `SessionActive`     | Launch node pulses blue (active session)                              |
| `Idle`              | No animation, revert to config/preview status                         |

### FR-3: Integration with LaunchPanel

- Replaces the `crosshook-launch-panel__runner-stack` div
- Preserves information parity: method is implied by node set, phase is shown via node animation,
  `helperLogPath` moves below the pipeline
- The `launchGuidanceText` (status + hint) remains as a line below the pipeline

### FR-4: Responsive Layout

- **Default (≥1024px content width)**: Horizontal layout, full labels
- **Compact (< 1024px content width)**: Compressed labels or icon-only with status dot, horizontal
- **Narrow (< 640px)**: Vertical layout (not expected in normal use but provides a safety net)

### FR-5: Accessibility

- Semantic `<ol>` with `<li>` per node
- `role="navigation"` on the container with `aria-label="Launch pipeline"`
- Active node marked with `aria-current="step"`
- Status communicated via `aria-label` on each node (e.g., "Game executable: configured")
- Status changes announced via `aria-live="polite"` region
- Color is never the sole status indicator — each status has icon + text + color (WCAG 1.4.1)
- Non-text contrast ≥ 3:1 (WCAG 1.4.11)

### FR-6: CSS Architecture

- BEM classes: `crosshook-launch-pipeline`, `crosshook-launch-pipeline__node`,
  `crosshook-launch-pipeline__connector`, `crosshook-launch-pipeline__node--configured`,
  `crosshook-launch-pipeline__node--error`, etc.
- Status-driven via `data-status` attribute selectors (matches existing `data-severity` pattern)
- CSS custom properties from `variables.css` for colors
- Connectors via `::after` pseudo-elements on nodes (flexbox with `flex: 1`)
- Pulse animation via `@keyframes` with `animation` property on `[data-status="active"]`

---

## Technical Architecture

### Data Flow

```
GameProfile ─────────────────────────────┐
                                         ├──▶ derivePipelineNodes(method, profile, preview, phase)
LaunchPreview (nullable) ────────────────┤       │
                                         │       ▼
LaunchPhase ─────────────────────────────┤   PipelineNode[]
                                         │       │
LaunchMethod ────────────────────────────┘       ▼
                                          <LaunchPipeline nodes={nodes} />
```

### New Types

```typescript
/** Status of a single pipeline node. */
type PipelineNodeStatus = 'configured' | 'not-configured' | 'error' | 'active' | 'complete';

/** A node in the launch pipeline visualization. */
interface PipelineNode {
  /** Stable identifier for the node (e.g., 'game', 'wine-prefix', 'proton'). */
  id: string;
  /** Display label (e.g., 'Game', 'Wine Prefix', 'Proton'). */
  label: string;
  /** Current status for visual rendering. */
  status: PipelineNodeStatus;
  /** Optional detail text (e.g., resolved path, error message). */
  detail?: string;
}
```

### New Files

| File                                                     | Purpose                                  |
| -------------------------------------------------------- | ---------------------------------------- |
| `src/crosshook-native/src/components/LaunchPipeline.tsx` | Pipeline component                       |
| `src/crosshook-native/src/styles/launch-pipeline.css`    | Pipeline styles                          |
| `src/crosshook-native/src/utils/derivePipelineNodes.ts`  | Pure function: inputs → `PipelineNode[]` |

### Modified Files

| File                                                  | Change                                                                                                                                                                                  |
| ----------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/LaunchPanel.tsx` | Replace `crosshook-launch-panel__runner-stack` div with `<LaunchPipeline>`. Pass `profile`, `preview`, `phase`, `method`. Move `helperLogPath` and `launchGuidanceText` below pipeline. |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`  | No change needed — pipeline is not a scroll container.                                                                                                                                  |

### No Backend Changes

All data needed is already available via existing types: `GameProfile`, `LaunchPreview`,
`LaunchPhase`, `LaunchMethod`. No new IPC commands, no Rust changes, no schema migrations.

---

## Storage Boundary

| Datum                          | Classification               | Notes                                                                                                |
| ------------------------------ | ---------------------------- | ---------------------------------------------------------------------------------------------------- |
| Pipeline node status           | **Runtime-only** (in-memory) | Derived from existing `GameProfile` + `LaunchPreview` + `LaunchPhase` on each render. Not persisted. |
| Pipeline layout preference     | **Not applicable**           | Layout is responsive, not user-configurable.                                                         |
| Preview data for tier-2 status | **Runtime-only** (in-memory) | `LaunchPreview` is already ephemeral in `usePreviewState`.                                           |

### Persistence & Usability

- **Migration / backward compatibility**: No new persisted data. No migration needed.
- **Offline behavior**: Pipeline derives from local profile data. Fully functional offline.
- **Degraded behavior**: If `LaunchPreview` is unavailable, falls back to Tier 1 (config-derived).
  If `GameProfile` fields are empty, all nodes show "Not configured." Never crashes or shows blank.
- **User visibility / editability**: Read-only visualization. Users edit configuration through
  existing profile sections and sub-tabs, not through the pipeline.

---

## Decisions Log

| #   | Decision                                                   | Alternatives Considered                                     | Rationale                                                                                                                            |
| --- | ---------------------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| D1  | **Read-only visualization**, not interactive controls      | Per-step toggles; clickable nodes that navigate to settings | Avoids redundant control paths. Pipeline communicates status; existing tabs/forms control config.                                    |
| D2  | **Replace runner indicator area**                          | New sub-tab; inside Preview modal; separate panel           | Runner indicator is underutilized real estate in the right location. Pipeline subsumes its info (method + phase).                    |
| D3  | **Method-adaptive node set** (different nodes per method)  | Fixed 6-node chain for all methods                          | Native launches don't have Wine/Proton; showing grayed-out irrelevant nodes is confusing. Adaptive chain is honest.                  |
| D4  | **Trainer node always visible** (even when not configured) | Hide when no trainer                                        | Consistent chain length per method. Communicates the capability exists. "Not configured" is informative.                             |
| D5  | **CSS-only rendering** (no visualization library)          | D3.js, SVG canvas, Framer Motion                            | Project policy: zero new JS dependencies. CSS steppers are production-viable for linear chains (Ahmad Shadeed, Piccalilli patterns). |
| D6  | **Three-tier progressive status derivation**               | Always require Preview call; always config-only             | Hybrid gives immediate value (Tier 1) while rewarding Preview/Launch with richer feedback (Tier 2/3).                                |
| D7  | **Horizontal layout with compact responsive fallback**     | Always vertical; always horizontal                          | Horizontal matches left-to-right flow mental model. Compact mode at ≤1024px ensures Steam Deck usability.                            |
| D8  | **Pulse animation for active launch nodes**                | Separate phase indicator below; no animation                | Brings the pipeline to life during launch. Users can watch the chain execute step by step. Phase indicator is subsumed.              |

---

## Implementation Phases

### Phase 1 — Core Pipeline Component + Tier 1 Status

- `derivePipelineNodes()` utility with config-derived status
- `LaunchPipeline` component with CSS-only horizontal stepper
- BEM styles with `data-status` attribute selectors
- Accessibility markup (`<ol>`, `aria-current`, `aria-label`)
- Integration into `LaunchPanel` (replace runner indicator)
- Responsive compact layout

### Phase 2 — Tier 2 Preview-Derived Status — **complete**

- `derivePipelineNodes()` uses `LaunchPreview` when present (Tier 1 fallback when `preview` is null)
- `ValidationError::issue()` populates machine-readable `code` (snake_case); frontend maps `code` to
  nodes via `mapValidationToNode` / `groupIssuesByNode` (no message-pattern matching)
- Node `detail` shows resolved paths, validation messages, or `directives_error` on Optimizations
- Mock `preview_launch` can return validation fixtures when `game_path` is empty or
  `__MOCK_VALIDATION_ERROR__` for browser dev mode

### Phase 3 — Tier 3 Live Launch Animation

- Map `LaunchPhase` transitions to node `active`/`complete` states
- CSS `@keyframes` pulse animation for active nodes
- Transition effects on status changes
- `helperLogPath` and `launchGuidanceText` integration below pipeline

### Phase 4 — Polish & Steam Deck Validation

- Visual tuning (spacing, colors, connector styles)
- Steam Deck testing at 1280x800
- Tooltip/popover on node hover (if scoped in)
- `aria-live` announcements for status transitions
- Browser dev mode mock handler for pipeline preview data

---

## Risks

| Risk                                                             | Likelihood | Impact | Mitigation                                                                                                                               |
| ---------------------------------------------------------------- | ---------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------- |
| **6-node horizontal chain doesn't fit at 1280x800**              | Medium     | Medium | Compact icon-only mode at breakpoint. Prototype early.                                                                                   |
| **Validation issues can't be reliably mapped to specific nodes** | Low        | Medium | Validation errors include `code` field; mapping table based on error codes. Fallback: unmapped errors show on the Launch (summary) node. |
| **Animation janks on WebKitGTK**                                 | Low        | Low    | Use `transform` and `opacity` only (GPU-composited). Test on actual Tauri WebView.                                                       |
| **Stale preview data shows misleading node status**              | Medium     | Low    | Show subtle "stale" indicator when preview is > 60s old (existing pattern from `isStale()` in LaunchPanel). Tier 1 is always current.    |
| **Replacing runner indicator breaks existing user mental model** | Low        | Medium | Pipeline communicates strictly more information. Method is implicit from node set. Phase is shown via animation. No information loss.    |
