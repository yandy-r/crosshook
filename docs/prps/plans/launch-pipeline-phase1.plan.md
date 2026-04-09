# Launch Pipeline Visualization — Phase 1: Core Component + Tier 1 Status

> **PRD**: `docs/prps/prds/launch-pipeline-visualization.prd.md` — Phase 1 **Issue**:
> [#187](https://github.com/yandy-r/crosshook/issues/187) **Branch**: `feat/launch-pipeline-phase1`
> **Generated**: 2026-04-09

---

## Overview

Build the `LaunchPipeline` React component — a CSS-only horizontal stepper replacing the runner
indicator area (`crosshook-launch-panel__runner-stack`) in `LaunchPanel`. Phase 1 implements **Tier
1 (config-derived) status only**: node status derived from `GameProfile` field presence, no new IPC
calls.

### Scope summary

- 3 new files: `LaunchPipeline.tsx`, `launch-pipeline.css`, `derivePipelineNodes.ts`
- 1 modified file: `LaunchPanel.tsx` (replace runner indicator with `<LaunchPipeline>`)
- 0 backend changes, 0 new dependencies

---

## Patterns to Mirror

These conventions were extracted from the current codebase and **must** be followed.

### Component conventions

| Convention            | Source                    | Rule                                                                                |
| --------------------- | ------------------------- | ----------------------------------------------------------------------------------- |
| Function components   | `LaunchPanel.tsx:602`     | Named export `export function LaunchPipeline(...)` + `export default` at bottom     |
| Props interface       | `LaunchPanel.tsx:575-592` | Inline `interface LaunchPipelineProps` above component; JSDoc on non-obvious fields |
| Hooks at top          | `LaunchPanel.tsx:612-635` | All hooks before derived values; derived values before event handlers               |
| Helper functions      | `LaunchPanel.tsx:52-114`  | Pure helpers (e.g., `severityIcon()`) defined at module scope, above component      |
| CSS import            | `LaunchPanel.tsx:24`      | Side-effect import: `import '../styles/launch-pipeline.css';`                       |
| Conditional rendering | `LaunchPanel.tsx:726`     | Ternary `? ... : null` pattern, never `&&` short-circuit for JSX                    |

### CSS conventions

| Convention           | Source                     | Rule                                                                                                                         |
| -------------------- | -------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| BEM naming           | `theme.css` throughout     | `crosshook-launch-pipeline`, `crosshook-launch-pipeline__node`, `crosshook-launch-pipeline__connector`                       |
| State via data attrs | `LaunchPanel.tsx:905, 918` | `data-status="configured"` not `--configured` modifier classes                                                               |
| Color tokens         | `variables.css:1-122`      | Use `--crosshook-color-success`, `--crosshook-color-danger`, `--crosshook-color-text-muted`, `--crosshook-color-text-subtle` |
| Transition tokens    | `variables.css:98-99`      | `--crosshook-transition-fast: 140ms`, `--crosshook-transition-standard: 220ms`                                               |
| Radius tokens        | `variables.css`            | `--crosshook-radius-sm: 10px`, `--crosshook-radius-md: 14px`                                                                 |
| Breakpoints          | `variables.css:145-171`    | Width: 1360px, 900px. Height: 820px (Steam Deck). No container queries in codebase                                           |
| Reduced motion       | `theme.css:4643-4649`      | Global `prefers-reduced-motion` catch-all already kills all animations — no per-component override needed                    |
| Animations           | `theme.css:3419-3422`      | Existing `crosshook-pulse` keyframe for active dots; follow same pattern for pipeline                                        |

### Type conventions

| Convention             | Source                    | Rule                                                                                    |
| ---------------------- | ------------------------- | --------------------------------------------------------------------------------------- |
| `LaunchMethod`         | `types/profile.ts:18`     | `type LaunchMethod = '' \| 'steam_applaunch' \| 'proton_run' \| 'native'`               |
| `ResolvedLaunchMethod` | `types/launch.ts:139`     | `Exclude<LaunchMethod, ''>` — this is what `LaunchPanel` receives as `method` prop      |
| `LaunchPhase`          | `types/launch.ts:15-21`   | Enum: `Idle`, `GameLaunching`, `WaitingForTrainer`, `TrainerLaunching`, `SessionActive` |
| `GameProfile`          | `types/profile.ts:92-165` | Nested sections: `game`, `trainer`, `steam`, `runtime`, `launch`                        |

### Accessibility conventions

| Convention           | Source                      | Rule                                    |
| -------------------- | --------------------------- | --------------------------------------- |
| `aria-hidden="true"` | Icons/dots throughout       | Decorative glyphs always `aria-hidden`  |
| `aria-live="polite"` | 17+ instances               | Status messages use polite live region  |
| `aria-label`         | `LaunchPanel.tsx:918`       | Descriptive labels on non-text elements |
| `role="list"`        | `ReadinessChecklist.tsx:93` | Explicit roles on semantic lists        |
| `useId()`            | `LaunchPanel.tsx:635`       | Unique IDs for aria relationships       |

### Icon conventions

| Convention           | Source                                                | Rule                                                                 |
| -------------------- | ----------------------------------------------------- | -------------------------------------------------------------------- |
| Unicode glyphs       | `LaunchPanel.tsx:52-62`, `HealthBadge.tsx:17-21`      | Status icons use unicode: `✓` (`\u2713`), `✗` (`\u2717`), `—` (dash) |
| Always `aria-hidden` | Throughout                                            | Glyph spans get `aria-hidden="true"`                                 |
| Lookup tables        | `HealthBadge.tsx:17-21`, `wizard/checkBadges.ts:8-19` | `Record<Status, string>` maps for icons/labels                       |

### Data attribute patterns

| Attribute              | Values                                                                  | Source                                                               |
| ---------------------- | ----------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `data-state`           | `'active' \| 'waiting' \| 'running' \| 'idle'`                          | `LaunchPanel.tsx:905`                                                |
| `data-phase`           | `LaunchPhase` enum values                                               | `LaunchPanel.tsx:918`                                                |
| `data-severity`        | `'fatal' \| 'warning' \| 'info'`                                        | `LaunchPanel.tsx:730`                                                |
| **New: `data-status`** | `'configured' \| 'not-configured' \| 'error' \| 'active' \| 'complete'` | Pipeline nodes (Phase 1 uses `configured` and `not-configured` only) |

---

## PRD Field Name → Actual Code Field Name Reconciliation

The PRD uses simplified field names. The actual TypeScript `GameProfile` fields are:

| PRD says                       | Actual field                                      | TS type location                                         |
| ------------------------------ | ------------------------------------------------- | -------------------------------------------------------- |
| `profile.game.path`            | `profile.game.executable_path`                    | `types/profile.ts:95`                                    |
| `profile.runtime.wine_prefix`  | `profile.runtime.prefix_path`                     | `types/profile.ts:126`                                   |
| `profile.runtime.proton_path`  | `profile.runtime.proton_path`                     | `types/profile.ts:127` (correct)                         |
| `profile.trainer.path`         | `profile.trainer.path`                            | `types/profile.ts:101` (correct)                         |
| `profile.launch.optimizations` | `profile.launch.optimizations.enabled_option_ids` | `types/profile.ts:133` → `types/launch-optimizations.ts` |

---

## Data Flow

```
LaunchPage
├── useProfileContext() → { profile, launchMethod, ... }
├── useLaunchStateContext() → { phase, ... }
│
└── <LaunchPanel profile={profile} method={launchMethod} ...>
     ├── phase, helperLogPath, statusText, hintText ← useLaunchStateContext()
     │
     └── <LaunchPipeline
           method={method}
           profile={profile}
           phase={phase}
           preview={null}   ← always null in Phase 1
         />
         │
         └── derivePipelineNodes(method, profile, null, phase)
              → PipelineNode[]
```

**Key**: `LaunchPanel` does not currently receive `profile` as a prop. This plan adds it.
`LaunchPage` already has `profile` from `profileState.profile` (line 389+). The pipeline also
receives `phase` from `useLaunchStateContext()` (already available inside `LaunchPanel`). In Phase 1
the pipeline renders `phase` for information parity but does NOT animate nodes based on it (that's
Phase 3).

---

## Storage Boundary

| Datum                | Classification               | Notes                                     |
| -------------------- | ---------------------------- | ----------------------------------------- |
| Pipeline node status | **Runtime-only** (in-memory) | Derived from `GameProfile` on each render |
| Pipeline layout      | **Not applicable**           | Responsive CSS, not user-configurable     |

No migration, no backend changes, no persistence.

---

## Tasks

### Task 1: Types — `PipelineNode` and `PipelineNodeStatus`

**File**: `src/crosshook-native/src/types/launch.ts` (append)

Add the pipeline types to the existing launch types file:

```typescript
/** Status of a single pipeline node. */
export type PipelineNodeStatus = 'configured' | 'not-configured' | 'error' | 'active' | 'complete';

/** A node in the launch pipeline visualization. */
export interface PipelineNode {
  /** Stable identifier (e.g., 'game', 'wine-prefix', 'proton'). */
  id: string;
  /** Display label (e.g., 'Game', 'Wine Prefix'). */
  label: string;
  /** Current status for visual rendering. */
  status: PipelineNodeStatus;
  /** Optional detail text (e.g., resolved path, error message). */
  detail?: string;
}
```

**Phase 1 scope**: Only `configured` and `not-configured` statuses are used. The `error`, `active`,
and `complete` statuses exist in the type for forward compatibility with Phases 2-3 but are not
derived yet.

**Verification**: `npx tsc --noEmit` passes.

---

### Task 2: Pure function — `derivePipelineNodes()`

**New file**: `src/crosshook-native/src/utils/derivePipelineNodes.ts`

A pure, side-effect-free function that derives the `PipelineNode[]` from inputs.

**Signature**:

```typescript
import type { GameProfile } from '../types/profile';
import type { LaunchPreview } from '../types/launch';
import type { LaunchPhase } from '../types/launch';
import type { PipelineNode } from '../types/launch';
import type { ResolvedLaunchMethod } from '../utils/launch';

export function derivePipelineNodes(
  method: ResolvedLaunchMethod,
  profile: GameProfile,
  preview: LaunchPreview | null,
  phase: LaunchPhase
): PipelineNode[];
```

**Node sets per method** (ordered left-to-right):

| `method`          | Node IDs                                                              |
| ----------------- | --------------------------------------------------------------------- |
| `proton_run`      | `game`, `wine-prefix`, `proton`, `trainer`, `optimizations`, `launch` |
| `steam_applaunch` | `game`, `steam`, `trainer`, `optimizations`, `launch`                 |
| `native`          | `game`, `trainer`, `launch`                                           |

**Tier 1 status derivation** (when `preview` is null — always in Phase 1):

| Node ID         | `configured` when                                            | `not-configured` when              |
| --------------- | ------------------------------------------------------------ | ---------------------------------- |
| `game`          | `profile.game.executable_path.trim() !== ''`                 | otherwise                          |
| `wine-prefix`   | `profile.runtime.prefix_path.trim() !== ''`                  | otherwise                          |
| `proton`        | `profile.runtime.proton_path.trim() !== ''`                  | otherwise                          |
| `steam`         | Always `configured` (method is `steam_applaunch`)            | —                                  |
| `trainer`       | `profile.trainer.path.trim() !== ''`                         | otherwise                          |
| `optimizations` | `profile.launch.optimizations.enabled_option_ids.length > 0` | otherwise                          |
| `launch`        | All prior nodes in the chain have status `configured`        | Any prior node is `not-configured` |

**Implementation notes**:

- Define a `NODE_DEFS` constant mapping each node ID to `{ label: string }`.
- Define a `METHOD_NODE_IDS` constant mapping each method to its ordered node ID array.
- Use `.trim()` checks for string fields to avoid empty-but-whitespace false positives.
- The `launch` node is a **summary node** — its status is the logical AND of all preceding nodes.
- `preview` and `phase` parameters are accepted but ignored in Phase 1 (they enable Phases 2-3
  without signature changes).
- The function must be pure — no hooks, no side effects, no imports from React.

**Verification**: Write a minimal inline doc comment showing expected output for each method.
`npx tsc --noEmit` passes.

---

### Task 3: Component — `LaunchPipeline.tsx`

**New file**: `src/crosshook-native/src/components/LaunchPipeline.tsx`

**Props interface**:

```typescript
import type { GameProfile } from '../types/profile';
import type { LaunchPreview, LaunchPhase, PipelineNode } from '../types/launch';
import type { ResolvedLaunchMethod } from '../utils/launch';

interface LaunchPipelineProps {
  method: ResolvedLaunchMethod;
  profile: GameProfile;
  preview: LaunchPreview | null;
  phase: LaunchPhase;
}
```

**Component structure**:

```tsx
export function LaunchPipeline({ method, profile, preview, phase }: LaunchPipelineProps) {
  const nodes = derivePipelineNodes(method, profile, preview, phase);

  return (
    <nav className="crosshook-launch-pipeline" aria-label="Launch pipeline">
      <ol className="crosshook-launch-pipeline__steps">
        {nodes.map((node, index) => (
          <li
            key={node.id}
            className="crosshook-launch-pipeline__node"
            data-status={node.status}
            aria-current={/* Phase 3: active node */ undefined}
            aria-label={`${node.label}: ${node.status === 'configured' ? 'configured' : 'not configured'}`}
          >
            <span className="crosshook-launch-pipeline__node-indicator" aria-hidden="true">
              {STATUS_ICON[node.status]}
            </span>
            <span className="crosshook-launch-pipeline__node-label">{node.label}</span>
            <span className="crosshook-launch-pipeline__node-status">
              {STATUS_LABEL[node.status]}
            </span>
          </li>
        ))}
      </ol>
    </nav>
  );
}
```

**Module-scope constants** (above the component, following `LaunchPanel.tsx:52-114` pattern):

```typescript
const STATUS_ICON: Record<PipelineNodeStatus, string> = {
  configured: '\u2713', // ✓
  'not-configured': '\u2014', // —
  error: '\u2717', // ✗
  active: '\u25CF', // ● (Phase 3)
  complete: '\u2713', // ✓ (Phase 3)
};

const STATUS_LABEL: Record<PipelineNodeStatus, string> = {
  configured: 'Ready',
  'not-configured': 'Not configured',
  error: 'Error',
  active: 'Running',
  complete: 'Done',
};
```

**Key requirements**:

- Import `../styles/launch-pipeline.css` as side-effect.
- No local state, no hooks (pure render from props).
- `<nav>` with `aria-label="Launch pipeline"` wraps the `<ol>`.
- Each `<li>` has a full `aria-label` describing the node name and status.
- Indicator spans use `aria-hidden="true"`.
- Named export + `export default LaunchPipeline;` at bottom.

**Verification**: Component renders without errors in browser dev mode.

---

### Task 4: CSS — `launch-pipeline.css`

**New file**: `src/crosshook-native/src/styles/launch-pipeline.css`

**Architecture**:

- BEM classes: `crosshook-launch-pipeline`, `__steps`, `__node`, `__node-indicator`, `__node-label`,
  `__node-status`
- Status driven by `data-status` attribute on `__node`
- Connectors via `::after` pseudo-element on each `__node` except the last
  (`:last-child::after { display: none }`)
- Flexbox horizontal layout; `<ol>` is `display: flex` with `list-style: none`

**Layout structure**:

```
[nav.crosshook-launch-pipeline]
  [ol.crosshook-launch-pipeline__steps]  ← display: flex; gap: 0;
    [li.__node]                          ← flex: 1; display: flex; flex-direction: column; align-items: center; position: relative;
      [span.__node-indicator]            ← 28px circle, centered
      [span.__node-label]               ← font-size: 0.8rem
      [span.__node-status]              ← font-size: 0.7rem, text-muted
      [::after]                          ← connector line (2px height, between indicator circles)
```

**Connector implementation**:

```css
.crosshook-launch-pipeline__node::after {
  content: '';
  position: absolute;
  top: 14px; /* center of 28px indicator */
  left: calc(50% + 18px); /* past the indicator circle */
  right: calc(-50% + 18px);
  height: 2px;
  background: var(--crosshook-color-border-strong);
}

.crosshook-launch-pipeline__node:last-child::after {
  display: none;
}
```

**Status colors** (via `data-status`):

| Status           | Indicator bg                                                                     | Indicator text                       | Label color                         |
| ---------------- | -------------------------------------------------------------------------------- | ------------------------------------ | ----------------------------------- |
| `configured`     | `var(--crosshook-color-success)`                                                 | `#fff`                               | `var(--crosshook-color-text)`       |
| `not-configured` | `rgba(224,224,224,0.15)`                                                         | `var(--crosshook-color-text-subtle)` | `var(--crosshook-color-text-muted)` |
| `error`          | `var(--crosshook-color-danger)`                                                  | `#fff`                               | `var(--crosshook-color-danger)`     |
| `active`         | `var(--crosshook-color-accent-strong)` with `crosshook-pipeline-pulse` animation | `#fff`                               | `var(--crosshook-color-text)`       |
| `complete`       | `var(--crosshook-color-success)`                                                 | `#fff`                               | `var(--crosshook-color-success)`    |

**Responsive breakpoints** (match existing codebase breakpoints):

| Breakpoint                   | Behavior                                                             |
| ---------------------------- | -------------------------------------------------------------------- |
| Default (>= 1024px content)  | Full labels + status text visible                                    |
| `@media (max-width: 1360px)` | Reduce node indicator to 24px, reduce font sizes                     |
| `@media (max-height: 820px)` | Compact: tighter gap, smaller indicators (Steam Deck)                |
| `@media (max-width: 900px)`  | Compact labels: abbreviate or hide status text, icon-only indicators |
| `@media (max-width: 640px)`  | Vertical layout fallback: `flex-direction: column` on `__steps`      |

**Key CSS requirements**:

- WCAG 1.4.1: Status is never color-only — each status has icon + text + color.
- WCAG 1.4.11: Non-text contrast >= 3:1 for indicator circles against panel background.
- Transition:
  `transition: background var(--crosshook-transition-fast), color var(--crosshook-transition-fast)`
  on indicators and connector lines for smooth status changes.
- No new `@keyframes` needed in Phase 1 (only `configured` and `not-configured`). The `active` pulse
  is Phase 3.
- Connector line color matches status: configured nodes have a green-tinted connector segment.
  Simplification for Phase 1: all connectors use `--crosshook-color-border-strong` (uniform).

**Verification**: Visual inspection in browser dev mode at 1920x1080, 1360x768, 1280x800 (Steam
Deck), and 640x480.

---

### Task 5: Integration — Modify `LaunchPanel.tsx`

**File**: `src/crosshook-native/src/components/LaunchPanel.tsx`

**Changes**:

#### 5a. Add `profile` prop to `LaunchPanelProps`

```typescript
interface LaunchPanelProps {
  profileId: string;
  method: Exclude<LaunchMethod, ''>;
  request: LaunchRequest | null;
  profile: GameProfile; // ← NEW
  profileSelectSlot?: ReactNode;
  beforeActions?: ReactNode;
  infoSlot?: ReactNode;
  tabsSlot?: ReactNode;
  onBeforeLaunch?: (action: 'game' | 'trainer') => Promise<boolean>;
}
```

Add `import type { GameProfile } from '../types/profile';` to the imports.

#### 5b. Add `profile` to destructured props

At `LaunchPanel.tsx:602`:

```typescript
export function LaunchPanel({
  profileId,
  method,
  request,
  profile,      // ← NEW
  profileSelectSlot,
  ...
}: LaunchPanelProps) {
```

#### 5c. Add `LaunchPipeline` import

```typescript
import { LaunchPipeline } from './LaunchPipeline';
```

#### 5d. Replace runner-stack (lines 902-930) with `LaunchPipeline`

Replace the entire `<div className="crosshook-launch-panel__runner-stack">...</div>` block with:

```tsx
<LaunchPipeline method={method} profile={profile} preview={null} phase={phase} />;

{
  helperLogPath ? (
    <span className="crosshook-launch-panel__indicator-copy">Log: {helperLogPath}</span>
  ) : null;
}

{
  launchGuidanceText ? (
    <p id={launchGuidanceId} className="crosshook-launch-panel__indicator-guidance">
      {launchGuidanceText}
    </p>
  ) : null;
}
```

**Key**: `helperLogPath` and `launchGuidanceText` move **below** the pipeline (they were inside the
runner-stack). They keep their existing class names and styling. The `launchGuidanceId` is still
used for `aria-describedby` on the launch buttons.

#### 5e. Remove unused runner-stack CSS selectors (optional cleanup)

The following CSS selectors in `theme.css:3364-3427` become dead code after this change:

- `.crosshook-launch-panel__runner-stack`
- `.crosshook-launch-panel__runner-primary-row`
- `.crosshook-launch-panel__indicator`
- `.crosshook-launch-panel__indicator-row`
- `.crosshook-launch-panel__indicator-dot`
- `.crosshook-launch-panel__indicator-label`
- `.crosshook-launch-panel__status` (with `data-phase` selectors in `theme.css:3154-3177`)

**Decision**: Leave the dead CSS in place for Phase 1 — removing it is low-risk cleanup that can
happen in a follow-up. This keeps the diff focused on the new feature.

**Verification**: `npx tsc --noEmit` passes. Visual comparison in browser dev mode.

---

### Task 6: Integration — Modify `LaunchPage.tsx`

**File**: `src/crosshook-native/src/components/pages/LaunchPage.tsx`

**Change**: Pass `profile` prop to `<LaunchPanel>`.

At `LaunchPage.tsx:347`:

```tsx
<LaunchPanel
  profileId={profileId}
  method={profileState.launchMethod}
  request={launchRequest}
  profile={profileState.profile}   // ← NEW
  profileSelectSlot={...}
  ...
/>
```

`profileState.profile` is already available from `useProfileContext()` at the `LaunchPage` level.

**Check**: Verify that `LaunchPanel` is also used on the Profiles page. If so, that call site also
needs the `profile` prop. Search for all `<LaunchPanel` usages.

**Verification**: `npx tsc --noEmit` passes. Both LaunchPage and any other LaunchPanel call sites
compile.

---

### Task 7: Smoke test validation

**Validation steps** (manual, in browser dev mode):

1. `./scripts/dev-native.sh --browser` → navigate to Launch page
2. **With a populated fixture** (`?fixture=populated`):
   - Pipeline renders horizontally with connected nodes
   - Node count matches the launch method (6 for proton_run, 5 for steam_applaunch, 3 for native)
   - Configured nodes show green indicator + checkmark
   - Non-configured nodes show gray indicator + dash
   - The Launch (summary) node correctly reflects the AND of all prior nodes
3. **With an empty fixture** (`?fixture=empty`):
   - All nodes show "Not configured" except method-implied ones (Steam always configured for
     steam_applaunch)
4. **Responsive**:
   - At 1280x800 (Steam Deck): all nodes visible, labels readable
   - At 640x480: vertical fallback layout
5. **Accessibility**:
   - Inspect DOM: `<nav>` with `aria-label`, `<ol>` with `<li>`, each `<li>` has `aria-label`
   - Screen reader test (optional): navigate pipeline, hear "Game: configured", "Trainer: not
     configured", etc.
6. **Information parity**:
   - Runner method is communicated by the node set (proton nodes = proton method)
   - Phase pill is visible (via `launchGuidanceText` below pipeline)
   - `helperLogPath` is visible below pipeline
7. **No regressions**:
   - Launch Game, Launch Trainer, Preview, Reset buttons all function normally
   - Feedback cards (validation, diagnostic) still appear above the pipeline area

**Verification command**:
`cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` (no Rust changes,
should pass unchanged).

---

## Risks & Mitigations

| Risk                                                                           | Likelihood | Impact | Mitigation                                                                                                     |
| ------------------------------------------------------------------------------ | ---------- | ------ | -------------------------------------------------------------------------------------------------------------- |
| `profile` prop threading breaks other `LaunchPanel` call sites                 | Medium     | Low    | Search all `<LaunchPanel` usages before implementation; each call site already has profile access via context  |
| 6-node horizontal chain doesn't fit at 1280x800                                | Medium     | Medium | Responsive CSS with compact mode at Steam Deck breakpoint; label truncation; measure in browser dev mode early |
| Connector line positioning breaks across different node counts                 | Low        | Low    | Use flexbox with `position: absolute` connector; test all 3 method variants                                    |
| Dead CSS from old runner-stack causes confusion                                | Low        | Low    | Leave in place for Phase 1; document in PR description                                                         |
| Phase 2/3 parameters (`preview`, `phase`) in function signatures add confusion | Low        | Low    | JSDoc comments explaining they're forward-compatible placeholders                                              |

---

## Files Changed Summary

| File                                                       | Action  | Purpose                                                          |
| ---------------------------------------------------------- | ------- | ---------------------------------------------------------------- |
| `src/crosshook-native/src/types/launch.ts`                 | Append  | `PipelineNodeStatus` type, `PipelineNode` interface              |
| `src/crosshook-native/src/utils/derivePipelineNodes.ts`    | **New** | Pure function: inputs → `PipelineNode[]`                         |
| `src/crosshook-native/src/components/LaunchPipeline.tsx`   | **New** | Pipeline component                                               |
| `src/crosshook-native/src/styles/launch-pipeline.css`      | **New** | Pipeline styles                                                  |
| `src/crosshook-native/src/components/LaunchPanel.tsx`      | Modify  | Add `profile` prop; replace runner-stack with `<LaunchPipeline>` |
| `src/crosshook-native/src/components/pages/LaunchPage.tsx` | Modify  | Pass `profile` prop to `<LaunchPanel>`                           |

---

## Acceptance Criteria (from issue #187)

- [ ] `derivePipelineNodes()` returns correct node set for each `LaunchMethod`
- [ ] Node status correctly reflects `GameProfile` field presence
- [ ] Horizontal stepper renders with connected nodes via CSS
- [ ] Runner indicator area is replaced — method and phase info preserved
- [ ] Responsive: compact layout at <1024px, vertical at <640px
- [ ] Accessibility: semantic `<ol>`, `aria-label`, `aria-current="step"`, WCAG contrast
- [ ] BEM classes follow `crosshook-launch-pipeline*` naming
- [ ] No new JS dependencies (CSS-only rendering)
- [ ] No backend/IPC changes
