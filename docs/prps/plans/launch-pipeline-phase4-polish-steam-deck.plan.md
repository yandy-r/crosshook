# Plan: Launch Pipeline Visualization — Phase 4 (Polish & Steam Deck Validation)

## Summary

Polish the launch pipeline visualization with accessibility improvements (`aria-live` status
announcements), CSS visual tuning (connector colors for active/error states, not-configured contrast
bump, vertical connector lines), Radix-based tooltips replacing native `title` attributes, enhanced
browser dev mode mock coverage (warning-severity validation fixtures), and Steam Deck responsive
validation at 1280x800.

Phase 4 is the final phase. It touches only frontend files — no new IPC commands, backend types,
schema changes, or dependencies. Radix `@radix-ui/react-tooltip` is already a dependency with a
`<Tooltip.Provider>` at the app root.

## User Story

As a **profile configurator**, I want to hover a pipeline node and see detailed status (resolved
path, validation error) in a styled tooltip, and have screen reader announcements when nodes change
status during launch, so I can understand the pipeline without sighted inspection.

As a **Steam Deck user**, I want the pipeline to be fully readable and usable at 1280x800 with
controller input, with no horizontal overflow or clipped labels.

## Problem → Solution

**Current state**: The pipeline renders Tier 1/2/3 status correctly, but:

1. **No `aria-live` region** — screen readers are not notified when nodes change status during
   launch. The `aria-label` on each `<li>` updates but no live region announces transitions. This
   violates PRD FR-5 and WCAG 4.1.3.
2. **Native `title` tooltips only** — `title={node.detail}` on `<li>` is unreliable for screen
   readers, unstyled, inaccessible on touch, and has no keyboard access. The project already has
   `@radix-ui/react-tooltip` and a reusable `InfoTooltip` pattern.
3. **Connector color gaps** — `active` (non-waiting) and `error` nodes keep the default gray
   connector. Only `configured`, `complete`, and `waiting` have styled connectors.
4. **Not-configured indicator contrast** — `rgba(224,224,224,0.15)` composites to ~1.2:1 against the
   dark background, below WCAG 1.4.11's 3:1 threshold for non-text contrast. Mitigated by text/icon
   but improvable.
5. **No vertical connectors** — at `<=640px` vertical layout, connectors are `display: none`. Visual
   continuity is lost.
6. **No warning-severity mock fixtures** — the `preview_launch` mock only returns fatal issues or
   zero issues. No warning-severity path exercises Tier 2 node styling for warnings.
7. **No pipeline-specific Playwright smoke test** — existing `smoke.spec.ts` screenshots the Launch
   page but does not assert pipeline DOM structure or status attributes.

**Desired state**: All seven gaps addressed. Pipeline is WCAG 2.1 AA compliant, visually polished
with consistent connector coloring, Radix tooltips on detail-bearing nodes, and comprehensive mock
fixtures for dev-mode testing.

## Metadata

- **Complexity**: Medium
- **Source PRD**: `docs/prps/prds/launch-pipeline-visualization.prd.md`
- **PRD Phase**: Phase 4 — Polish & Steam Deck Validation
- **Estimated Files**: 7

---

## UX Design

### Before

```text
[Game ✓]---[Wine Prefix ✓]---[Proton ✓]---[Trainer —]---[Optimizations ✓]---[Launch —]
 game.exe    mock-prefix       proton       Not configured   3 env vars     Complete steps above
                                                             (title tooltip, native browser)
```

- Connectors always gray except configured/complete (green-tinted)
- Hovering shows browser `title` tooltip (delayed, unstyled)
- No screen reader announcement when status changes

### After

```text
[Game ✓]━━━[Wine Prefix ✓]━━━[Proton ✓]---[Trainer —]---[Optimizations ✓]---[Launch —]
 game.exe    mock-prefix       proton       Not configured   3 env vars     Complete steps above
                                        (Radix tooltip on hover/focus with detail)

[aria-live region]: "Trainer: Not configured. Launch: Complete steps above."
```

- Connectors colored per preceding node status (green for configured/complete, blue for active,
  red-tinted for error, amber for waiting)
- Not-configured indicator bg bumped to `rgba(224,224,224,0.22)` for better contrast
- Radix tooltip on nodes that have `detail` text, triggered by hover/focus
- Visually-hidden `aria-live="polite"` region announces status summary on change
- Vertical layout at `<=640px` shows vertical connector lines between nodes

### Interaction Changes

| Touchpoint       | Before                                         | After                                                      | Notes                                               |
| ---------------- | ---------------------------------------------- | ---------------------------------------------------------- | --------------------------------------------------- |
| Node hover       | Browser `title` tooltip (1-2s delay, unstyled) | Radix tooltip (200ms delay, styled, keyboard-accessible)   | Only shown when `node.detail` is non-empty          |
| Node focus       | Not focusable                                  | Focusable via `tabIndex={0}` when detail present           | Read-only; focus only enables tooltip access        |
| Status change    | Silent                                         | `aria-live="polite"` announces summary                     | PatternFly pattern: separate visually-hidden region |
| Vertical layout  | No connectors                                  | Vertical connector lines via `::before` on non-first nodes | Maintains visual chain at narrow widths             |
| Active connector | Gray (default)                                 | Blue-tinted (`accent-strong 40%` mix)                      | Matches active indicator color                      |
| Error connector  | Gray (default)                                 | Red-tinted (`danger 35%` mix)                              | Signals the error propagation point                 |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                               | Lines   | Why                                                           |
| -------------- | ------------------------------------------------------------------ | ------- | ------------------------------------------------------------- |
| P0 (critical)  | `src/crosshook-native/src/components/LaunchPipeline.tsx`           | 1-80    | Current render contract, `aria-current`, `title`, STATUS maps |
| P0 (critical)  | `src/crosshook-native/src/styles/launch-pipeline.css`              | 1-297   | All status selectors, connectors, breakpoints, animations     |
| P0 (critical)  | `src/crosshook-native/src/styles/variables.css`                    | 1-194   | Color tokens, spacing, not-configured colors, controller mode |
| P1 (important) | `src/crosshook-native/src/utils/derivePipelineNodes.ts`            | 1-266   | Node derivation, Tier 1/2 logic, phase overlay                |
| P1 (important) | `src/crosshook-native/src/types/launch.ts`                         | 160-191 | `PipelineNode`, `PipelineNodeStatus`, `PipelineNodeTone`      |
| P1 (important) | `src/crosshook-native/src/components/ui/InfoTooltip.tsx`           | 1-71    | Radix tooltip pattern to mirror for pipeline tooltips         |
| P1 (important) | `src/crosshook-native/src/components/LaunchPanel.tsx`              | 899-920 | Runner-stack integration, adjacent `aria-live` usage          |
| P2 (reference) | `src/crosshook-native/src/lib/mocks/handlers/launch.ts`            | 209-302 | `preview_launch` mock, validation fixtures, sentinel strings  |
| P2 (reference) | `src/crosshook-native/src/lib/dev-indicator.css`                   | 21-31   | `crosshook-visually-hidden` class for `aria-live` region      |
| P2 (reference) | `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx` | 260-311 | Alternative CSS tooltip pattern (reference only)              |
| P2 (reference) | `src/crosshook-native/tests/smoke.spec.ts`                         | 1-end   | Existing Playwright smoke test pattern                        |

## External Documentation

| Topic                          | Source                                                                                               | Key Takeaway                                                                                                                           |
| ------------------------------ | ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Stepper `aria-live` pattern    | [PatternFly Progress Stepper](https://www.patternfly.org/components/progress-stepper/accessibility/) | Use a separate visually-hidden `<div aria-live="polite" aria-atomic="true">` with summary text, not a live region on the `<ol>` itself |
| Read-only stepper focus        | [USWDS Step Indicator](https://designsystem.digital.gov/components/step-indicator/)                  | Non-interactive steps do not need to be focusable; only make them focusable if they contain interactive content (tooltip trigger)      |
| `aria-atomic` for live regions | [a11y-blog.dev](https://a11y-blog.dev/en/articles/aria-live-regions/)                                | `aria-atomic="true"` ensures the full summary is announced as a unit                                                                   |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### RADIX_TOOLTIP

```tsx
// SOURCE: src/crosshook-native/src/components/ui/InfoTooltip.tsx:18-69
// Radix tooltip with portal, arrow, and focus support. The app root already
// provides <Tooltip.Provider delayDuration={200}> at App.tsx:177.
<Tooltip.Root>
  <Tooltip.Trigger asChild>
    <span role="button" tabIndex={0} aria-label="Info" ... >
      <InfoCircleIcon />
    </span>
  </Tooltip.Trigger>
  <Tooltip.Portal>
    <Tooltip.Content side="top" sideOffset={6} style={{ maxWidth: 320, ... }}>
      {content}
      <Tooltip.Arrow />
    </Tooltip.Content>
  </Tooltip.Portal>
</Tooltip.Root>
```

For pipeline nodes, use `<Tooltip.Root>` wrapping the `<li>`, with `<Tooltip.Trigger asChild>` on a
focusable wrapper inside the `<li>`. Only render the tooltip when `node.detail` is non-empty.

### ARIA_LIVE_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/LaunchSubTabs.tsx:303-304
// Atomic polite live region for status announcements.
<span aria-live="polite" aria-atomic="true">{statusText}</span>

// SOURCE: src/crosshook-native/src/components/LaunchPanel.tsx:916-917
// Alert live region adjacent to the pipeline.
<div role="alert" aria-live="polite">{versionMismatchContent}</div>
```

For the pipeline, add a visually-hidden `<div>` (using `crosshook-visually-hidden` class from
`dev-indicator.css:21-31`) with `aria-live="polite"` and `aria-atomic="true"` after the `<ol>`.
Update its text content when the node summary changes.

### DATA_STATUS_CONNECTOR

```css
/* SOURCE: src/crosshook-native/src/styles/launch-pipeline.css:94-96 */
/* Configured connector uses color-mix with success. */
.crosshook-launch-pipeline__node[data-status='configured']::after {
  background: var(--crosshook-color-pipeline-connector-success);
}

/* SOURCE: src/crosshook-native/src/styles/launch-pipeline.css:140-142 */
/* Waiting connector uses color-mix with warning. */
.crosshook-launch-pipeline__node[data-status='active'][data-tone='waiting']::after {
  background: color-mix(in srgb, var(--crosshook-color-warning) 40%, var(--crosshook-color-border-strong));
}
```

Follow the same `color-mix()` pattern for `active` (accent-strong) and `error` (danger) connectors.

### MOCK_HANDLER

```ts
// SOURCE: src/crosshook-native/src/lib/mocks/handlers/launch.ts:219-262
// Fixture-driven mock: check gamePath against sentinel, return validation issues.
if (gamePath === '' || gamePath === '__MOCK_VALIDATION_ERROR__') {
  const issues = [{ message: '...', help: '...', severity: 'fatal', code: '...' }];
  return { ...previewWithIssues };
}
```

Add a new sentinel value for warning-severity issues following the same pattern. New sentinel strings
must be added to the CI grep in `.github/workflows/release.yml:112` to ensure they don't leak into
production builds.

### VISUALLY_HIDDEN

```css
/* SOURCE: src/crosshook-native/src/lib/dev-indicator.css:21-31 */
.crosshook-visually-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}
```

Use this class for the `aria-live` announcement region.

### SMOKE_TEST

```ts
// SOURCE: src/crosshook-native/tests/smoke.spec.ts
// Pattern: navigate to route, assert sidebar active, screenshot, check console errors.
test('Launch page', async ({ page }) => {
  await page.click('[data-sidebar-route="launch"]');
  await expect(page.locator('[data-sidebar-route="launch"]')).toHaveAttribute('aria-current', 'page');
  await page.screenshot({ path: 'tests/screenshots/launch.png' });
});
```

---

## Files to Change

| File                                                     | Action | Justification                                                                                                  |
| -------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/LaunchPipeline.tsx` | UPDATE | Add `aria-live` region, Radix tooltips on detail-bearing nodes, remove `title` attributes                      |
| `src/crosshook-native/src/styles/launch-pipeline.css`    | UPDATE | Active/error connector colors, not-configured contrast bump, vertical connectors, tooltip-trigger focus styles |
| `src/crosshook-native/src/styles/variables.css`          | UPDATE | Add `--crosshook-color-pipeline-connector-active` and `--crosshook-color-pipeline-connector-error` tokens      |
| `src/crosshook-native/src/lib/mocks/handlers/launch.ts`  | UPDATE | Add `__MOCK_VALIDATION_WARNING__` sentinel for warning-severity preview fixtures                               |
| `.github/workflows/release.yml`                          | UPDATE | Add `__MOCK_VALIDATION_WARNING__` to CI sentinel grep pattern                                                  |
| `src/crosshook-native/tests/smoke.spec.ts`               | UPDATE | Add pipeline-specific assertions (node count, `data-status` values, tooltip presence)                          |
| `src/crosshook-native/tests/pipeline.spec.ts`            | CREATE | Dedicated pipeline Playwright spec: method-adaptive node sets, validation fixtures, tooltip interaction        |

## Storage / Persistence

No new persisted data is required for Phase 4.

| Datum                         | Classification | Notes                                            |
| ----------------------------- | -------------- | ------------------------------------------------ |
| `aria-live` announcement text | Runtime-only   | Derived from `PipelineNode[]` on each render     |
| Tooltip content               | Runtime-only   | Reads `node.detail` from existing pipeline nodes |
| New CSS variables             | Static (CSS)   | Design tokens in `variables.css`, not user data  |

### Persistence & Usability

- **Migration / backward compatibility**: No new settings, schemas, or IPC payload migrations.
- **Offline behavior**: Unchanged; all pipeline data is locally derived.
- **Degraded behavior**: If Radix tooltip fails to render, the `aria-label` on the `<li>` still
  communicates status. The `aria-live` region degrades to silence (no crash).
- **User visibility / editability**: Read-only visualization. No new user controls.

## NOT Building

- New Tauri commands, backend events, or schema changes.
- Per-node interactive controls (toggles, navigation links).
- Pipeline visualization inside the Preview modal.
- Animated connector lines or particle effects.
- Custom node ordering or hiding.
- New JS dependencies (Radix tooltip is already installed).

---

## Batches

| Batch       | Tasks         | Parallelizable? | Notes                                                                           |
| ----------- | ------------- | --------------- | ------------------------------------------------------------------------------- |
| **Batch 1** | Tasks 1, 2, 3 | Yes             | Independent CSS, accessibility, and mock work                                   |
| **Batch 2** | Tasks 4, 5    | Yes             | Tooltip component + Playwright tests; depend on Batch 1 for CSS/a11y foundation |
| **Batch 3** | Task 6        | No              | Build validation + manual checklist; depends on all prior tasks                 |

---

## Step-by-Step Tasks

### Task 1: CSS visual polish — connector colors, contrast, vertical connectors

- **Depends on**: None
- **ACTION**: Update `src/crosshook-native/src/styles/variables.css` and
  `src/crosshook-native/src/styles/launch-pipeline.css`.
- **IMPLEMENT**:
  1. In `variables.css`, add two new connector tokens after the existing
     `--crosshook-color-pipeline-connector-success` (line 31-35):

     ```css
     --crosshook-color-pipeline-connector-active: color-mix(
       in srgb,
       var(--crosshook-color-accent-strong) 40%,
       var(--crosshook-color-border-strong)
     );
     --crosshook-color-pipeline-connector-error: color-mix(
       in srgb,
       var(--crosshook-color-danger) 35%,
       var(--crosshook-color-border-strong)
     );
     ```

  2. In `variables.css`, bump `--crosshook-color-not-configured-bg` from `rgba(224, 224, 224, 0.15)`
     to `rgba(224, 224, 224, 0.22)` to improve non-text contrast toward WCAG 1.4.11.
  3. In `launch-pipeline.css`, add connector rules after the existing `active` block (~line 127):

     ```css
     .crosshook-launch-pipeline__node[data-status='active']::after {
       background: var(--crosshook-color-pipeline-connector-active);
     }
     ```

     And after the `error` block (~line 116):

     ```css
     .crosshook-launch-pipeline__node[data-status='error']::after {
       background: var(--crosshook-color-pipeline-connector-error);
     }
     ```

  4. In the vertical fallback (`<=640px`, line 262+), replace the connector `display: none` with
     vertical connector lines using `::before` pseudo-elements on non-first nodes:

     ```css
     .crosshook-launch-pipeline__node::after {
       display: none;
     }
     .crosshook-launch-pipeline__node + .crosshook-launch-pipeline__node::before {
       content: '';
       position: absolute;
       left: 14px; /* center of indicator */
       top: -0.5rem;
       height: 0.5rem;
       width: 2px;
       background: var(--crosshook-color-border-strong);
     }
     ```

     Status-driven vertical connector colors should mirror horizontal connector rules using the same
     general sibling pattern or `:has()` if WebKitGTK support allows, otherwise accept default color
     for vertical connectors (simple and non-critical).
- **MIRROR**: `DATA_STATUS_CONNECTOR`
- **IMPORTS**: CSS custom properties from `variables.css` only
- **GOTCHA**: WebKitGTK (Tauri's renderer) may not support `:has()`. Test vertical connectors in
  `./scripts/dev-native.sh` (full Tauri) not just browser dev mode. If `:has()` is unsupported,
  vertical connectors use the default border-strong color — acceptable since vertical layout is a
  rare fallback.
- **GOTCHA**: The `not-configured-bg` bump from `0.15` to `0.22` changes the visual weight slightly.
  Verify it still looks intentionally subtle and doesn't visually compete with `configured`.
- **VALIDATE**: `npm run build` in `src/crosshook-native`. Visual inspection in
  `./scripts/dev-native.sh --browser` with `?fixture=populated` (all configured), empty game path
  (error nodes), and native method (fewer nodes).

### Task 2: Add `aria-live` announcement region for pipeline status transitions

- **Depends on**: None
- **ACTION**: Update `src/crosshook-native/src/components/LaunchPipeline.tsx`.
- **IMPLEMENT**:
  1. Import the `crosshook-visually-hidden` class. Since it's defined in `dev-indicator.css` and
     already imported at app level, use the class name directly. If not globally available, import
     `../lib/dev-indicator.css` or inline the visually-hidden styles.
  2. Derive a summary string from the current nodes. Use a `useMemo` to compute it:

     ```ts
     const announcement = useMemo(() => {
       const issues = nodes.filter(
         (n) => n.status === 'error' || n.status === 'not-configured' || n.status === 'active'
       );
       if (issues.length === 0) return 'All pipeline steps configured.';
       return (
         issues
           .map((n) => {
             const text = n.detail || STATUS_LABEL[n.status];
             return `${n.label}: ${text}`;
           })
           .join('. ') + '.'
       );
     }, [nodes]);
     ```

  3. Add a visually-hidden `<div>` after the `<ol>` inside the `<nav>`:

     ```tsx
     <div className="crosshook-visually-hidden" aria-live="polite" aria-atomic="true">
       {announcement}
     </div>
     ```

  4. The summary only announces non-trivial statuses (errors, not-configured, active). When all
     nodes are configured/complete, it announces "All pipeline steps configured."
- **MIRROR**: `ARIA_LIVE_PATTERN`, `VISUALLY_HIDDEN`
- **IMPORTS**: No new dependencies. Uses existing `crosshook-visually-hidden` class.
- **GOTCHA**: Making the `<ol>` itself `aria-live` would cause screen readers to re-read the entire
  list on every status change. Use a separate element per the PatternFly pattern.
- **GOTCHA**: The `useMemo` dependency array must include `nodes` (the array reference). Since
  `derivePipelineNodes` returns a new array on each call and the parent `useMemo` in
  `LaunchPipeline` already depends on `[method, profile, preview, phase]`, the announcement will
  update when any input changes.
- **GOTCHA**: Ensure `dev-indicator.css` is imported or the `crosshook-visually-hidden` class is
  available globally. Check by searching for the import chain. If the class is only scoped to the
  dev indicator component, either duplicate the styles inline or add a shared utility CSS import.
- **VALIDATE**: `npm run build`. Screen reader testing (if available) or manual DOM inspection
  confirming the `aria-live` div content updates when pipeline status changes in browser dev mode.

### Task 3: Add warning-severity mock fixtures for `preview_launch`

- **Depends on**: None
- **ACTION**: Update `src/crosshook-native/src/lib/mocks/handlers/launch.ts` and
  `.github/workflows/release.yml`.
- **IMPLEMENT**:
  1. In `handlers/launch.ts`, add a new sentinel check after the existing
     `__MOCK_VALIDATION_ERROR__` block (~line 219). Add a second sentinel
     `__MOCK_VALIDATION_WARNING__` that returns a preview with warning-severity (non-fatal) issues:

     ```ts
     if (gamePath === '__MOCK_VALIDATION_WARNING__') {
       const issues: LaunchPreview['validation']['issues'] = [
         {
           message: 'Trainer binary hash does not match the community checksum.',
           help: 'Re-download or verify the trainer file integrity.',
           severity: 'warning' as const,
           code: 'trainer_hash_mismatch',
         },
       ];
       // Return a mostly-valid preview with warning issues but no fatals
       const previewWithWarnings: LaunchPreview = {
         resolved_method: method,
         validation: { issues },
         environment: makePreviewEnvVars(),
         cleared_variables: ['LD_PRELOAD'],
         wrappers: ['gamescope'],
         effective_command: '/path/to/game.exe',
         directives_error: null,
         steam_launch_options: method === 'steam_applaunch' ? '%command%' : null,
         proton_setup:
           method !== 'native'
             ? {
                 /* same as populated path */
               }
             : null,
         working_directory: '/home/devuser/Games/TestGameAlpha',
         game_executable: '/home/devuser/Games/TestGameAlpha/game.exe',
         game_executable_name: 'game.exe',
         trainer: {
           path: '/home/devuser/Trainers/mock-trainer.exe',
           host_path: '/home/devuser/Trainers/mock-trainer.exe',
           loading_mode: 'source_directory',
           staged_path: null,
         },
         generated_at: new Date().toISOString(),
         display_text: 'Mock preview with warning-severity validation.',
       };
       return previewWithWarnings;
     }
     ```

     Copy the `proton_setup` object from the existing populated path for consistency.
  2. In `.github/workflows/release.yml`, add `__MOCK_VALIDATION_WARNING__` to the sentinel grep
     pattern at the same line where `__MOCK_VALIDATION_ERROR__` is listed (~line 112):

     ```yaml
     grep -rl '__MOCK_VALIDATION_ERROR__\|__MOCK_VALIDATION_WARNING__' ...
     ```

- **MIRROR**: `MOCK_HANDLER`
- **IMPORTS**: Existing `LaunchPreview` type, `makePreviewEnvVars`, `getStore` from mock utilities
- **GOTCHA**: New sentinel strings must be added to the CI grep in `release.yml` or they will leak
  into production builds undetected. This is a **blocking** requirement.
- **GOTCHA**: The `trainer_hash_mismatch` code must be handled by `mapValidationToNode` (maps
  `trainer_*` prefix → `'trainer'` node). Verify the mapping covers this code.
- **VALIDATE**: `npm run build`. Run `./scripts/dev-native.sh --browser` and create a mock profile
  with game path set to `__MOCK_VALIDATION_WARNING__`, click Preview, and verify the pipeline shows
  the trainer node with a warning status or the Launch summary node reflects the warning.

### Task 4: Replace native `title` tooltips with Radix tooltips on detail-bearing nodes

- **Depends on**: Task 1 (CSS foundation), Task 2 (`aria-live` region in place)
- **ACTION**: Update `src/crosshook-native/src/components/LaunchPipeline.tsx` and
  `src/crosshook-native/src/styles/launch-pipeline.css`.
- **IMPLEMENT**:
  1. Import `* as Tooltip from '@radix-ui/react-tooltip'` in `LaunchPipeline.tsx`.
  2. Remove the `title={node.detail}` attribute from the `<li>`.
  3. For nodes where `node.detail` is non-empty, wrap the node indicator + label in a
     `<Tooltip.Root>` / `<Tooltip.Trigger asChild>` / `<Tooltip.Portal>` / `<Tooltip.Content>`
     structure. The trigger should be the indicator span (or a wrapper `<span>`) with
     `tabIndex={0}` and `aria-label` for keyboard access.
  4. For nodes where `node.detail` is empty/undefined, render the node without tooltip wrapping.
  5. Style the tooltip content consistently with `InfoTooltip.tsx` — use the same inline styles
     (or extract to CSS class):

     ```ts
     style={{
       maxWidth: 280,
       padding: '6px 10px',
       borderRadius: 8,
       fontSize: '0.8rem',
       lineHeight: 1.4,
       color: 'var(--crosshook-color-text)',
       background: 'var(--crosshook-color-surface-raised, #2a2a2e)',
       border: '1px solid var(--crosshook-color-border-strong)',
       boxShadow: '0 4px 12px rgba(0,0,0,0.35)',
       zIndex: 9999,
     }}
     ```

  6. In `launch-pipeline.css`, add a focus-visible style for the tooltip trigger to show a subtle
     outline:

     ```css
     .crosshook-launch-pipeline__node-trigger:focus-visible {
       outline: 2px solid var(--crosshook-color-accent-strong);
       outline-offset: 2px;
       border-radius: var(--crosshook-radius-sm);
     }
     ```

- **MIRROR**: `RADIX_TOOLTIP`
- **IMPORTS**: `@radix-ui/react-tooltip` (already installed, `<Tooltip.Provider>` at app root)
- **GOTCHA**: The `<Tooltip.Trigger asChild>` must wrap a single focusable element. If wrapping
  the entire `<li>`, note that `<li>` is not focusable by default — add `tabIndex={0}` to the
  trigger element only, not to `<li>` directly.
- **GOTCHA**: Tooltip should show `side="top"` to avoid interfering with the guidance text below.
  At compact breakpoints (`<=1023px`), status text is hidden — the tooltip becomes the primary
  way to see detail. Ensure tooltip still works at compact sizes.
- **GOTCHA**: Keep `aria-label` on the `<li>` for screen readers that don't trigger tooltips.
  The tooltip provides supplementary detail, not the primary accessible name.
- **VALIDATE**: `npm run build`. Browser dev mode: hover each node → tooltip appears with detail
  text. Tab through nodes → tooltip appears on focus. Press Escape → tooltip dismisses.

### Task 5: Pipeline-specific Playwright smoke tests

- **Depends on**: Task 1 (CSS changes), Task 3 (mock fixtures)
- **ACTION**: Create `src/crosshook-native/tests/pipeline.spec.ts` and optionally update
  `src/crosshook-native/tests/smoke.spec.ts`.
- **IMPLEMENT**:
  1. Create `pipeline.spec.ts` following the `smoke.spec.ts` and `collections.spec.ts` patterns
     (same `attachConsoleCapture`, same `baseURL` from playwright config).
  2. Test cases:
     - **Populated fixture, proton_run**: Navigate to Launch page. Assert
       `.crosshook-launch-pipeline__node` count is 6 (game, wine-prefix, proton, trainer,
       optimizations, launch). Assert `data-status` attributes are present on all nodes.
     - **Populated fixture, configured profile**: Assert all non-trainer nodes have
       `data-status="configured"`. Assert `aria-label` on each `<li>` contains status text.
     - **Validation error fixture**: Set game path to empty or `__MOCK_VALIDATION_ERROR__`, click
       Preview, assert game node has `data-status="error"`.
     - **Warning fixture**: Set game path to `__MOCK_VALIDATION_WARNING__`, click Preview, assert
       trainer node reflects the warning (if pipeline shows warnings distinctly) or launch summary
       shows it.
     - **Tooltip presence**: Hover a configured node with detail, assert
       `[role="tooltip"]` appears in the DOM.
     - **`aria-live` region**: Assert `.crosshook-visually-hidden[aria-live="polite"]` exists
       inside `.crosshook-launch-pipeline`.
  3. In `smoke.spec.ts`, optionally add a lightweight assertion to the existing Launch page test:

     ```ts
     await expect(page.locator('.crosshook-launch-pipeline')).toBeVisible();
     await expect(page.locator('.crosshook-launch-pipeline__node')).toHaveCount(6);
     ```

- **MIRROR**: `SMOKE_TEST`
- **IMPORTS**: Playwright `test`, `expect` from `@playwright/test`
- **GOTCHA**: Playwright browser binary must be installed (`npx playwright install`). Tests are
  not a CI gate per existing config (`fullyParallel: false, workers: 1`). They are a dev-time
  polish tool.
- **GOTCHA**: The mock fixture system uses URL parameters (`?fixture=populated`). The Playwright
  config already boots `npm run dev:browser` as the web server. Profile manipulation in tests
  requires navigating to the Launch page and interacting with the profile selector.
- **VALIDATE**: `npm run test:smoke` (if Playwright browsers installed). Otherwise, manual
  verification that test file compiles via `npm run build`.

### Task 6: Build validation, Steam Deck responsive check, and manual test checklist

- **Depends on**: Tasks 1-5
- **ACTION**: Run validation commands and perform manual checks.
- **IMPLEMENT**:
  1. Run `npm run build` in `src/crosshook-native` — zero errors.
  2. Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` — no
     regressions.
  3. Run `./scripts/dev-native.sh --browser` and complete the manual checklist below.
  4. Run `./scripts/dev-native.sh` (full Tauri) and verify pipeline renders correctly.
  5. If Playwright browsers are installed, run `npm run test:smoke`.
- **VALIDATE**: All commands pass. Manual checklist items verified.

---

## Testing Strategy

### Unit Tests

| Test                                     | Input                                   | Expected Output                                                     | Edge Case? |
| ---------------------------------------- | --------------------------------------- | ------------------------------------------------------------------- | ---------- |
| `aria-live` summary for all-configured   | All nodes status `configured`           | "All pipeline steps configured."                                    | No         |
| `aria-live` summary with errors          | Game node `error`, others `configured`  | "Game: [error detail]."                                             | No         |
| `aria-live` summary during live launch   | Game `active`, rest `configured`        | "Game: Running."                                                    | No         |
| Tooltip only on detail-bearing nodes     | Nodes with/without `detail`             | Tooltip wrapper present only when `detail` is truthy                | Yes        |
| Warning fixture returns non-fatal issues | `__MOCK_VALIDATION_WARNING__` game path | Preview with `severity: 'warning'`, `code: 'trainer_hash_mismatch'` | No         |

### Edge Cases Checklist

- [ ] Pipeline with zero detail on any node — no tooltips render, no errors
- [ ] Native method with only 3 nodes — tooltip and `aria-live` still work
- [ ] `prefers-reduced-motion: reduce` — animations disabled, tooltips still work
- [ ] Compact layout (`<=1023px`) — status text hidden, tooltip is the only way to see detail
- [ ] Vertical layout (`<=640px`) — vertical connectors render, tooltips still work
- [ ] Controller mode — pipeline is non-interactive (read-only), no controller-specific regressions
- [ ] Multiple rapid status changes during launch — `aria-live` region doesn't flood announcements
- [ ] `preview` is null (Tier 1 only) — no detail text on most nodes, no tooltips, `aria-live`
      shows "Not configured" nodes

---

## Validation Commands

### Static Analysis

```bash
cd src/crosshook-native && npm run build
```

EXPECT: Zero TypeScript or Vite build errors.

### Backend Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: All existing tests pass. No backend changes in Phase 4.

### Smoke Tests

```bash
cd src/crosshook-native && npm run test:smoke
```

EXPECT: All existing + new pipeline assertions pass (requires `npx playwright install`).

### Browser Validation

```bash
./scripts/dev-native.sh --browser
```

EXPECT: Pipeline renders with Radix tooltips, `aria-live` region in DOM, connector colors for all
status types, vertical layout at `<=640px` shows vertical connector lines.

### Tauri Validation

```bash
./scripts/dev-native.sh
```

EXPECT: Same visual behavior as browser dev mode. Verify WebKitGTK renders the `color-mix()`
connector colors correctly and the vertical `::before` connector in `<=640px` mode works.

### Manual Validation

- [ ] Open Launch page with populated `proton_run` profile → 6 nodes, all configured, green
      connectors between configured nodes
- [ ] Hover a configured node with detail → Radix tooltip shows resolved path/value
- [ ] Tab through nodes → tooltip appears on focus, Escape dismisses it
- [ ] Set game path to empty, click Preview → Game node shows `error`, connector turns red-tinted
- [ ] Set game path to `__MOCK_VALIDATION_WARNING__`, click Preview → trainer node or launch summary
      reflects warning
- [ ] Trigger a launch (browser dev mode) → active node pulse, blue connector on active node,
      `aria-live` region announces active step
- [ ] Resize browser to 1280x800 → pipeline compact mode, status text hidden, all labels readable
- [ ] Resize browser to <640px → vertical layout with vertical connector lines between nodes
- [ ] Enable `prefers-reduced-motion: reduce` → pulse animations stop, tooltips still work
- [ ] Open DevTools → confirm `[aria-live="polite"]` div inside `.crosshook-launch-pipeline` has
      updated content matching current pipeline status
- [ ] Confirm no console errors throughout all checks

### Steam Deck Simulation

- [ ] Resize browser to exactly 1280x800
- [ ] Verify all 6 nodes (proton_run) visible without horizontal scrolling
- [ ] Verify labels are readable (not clipped to illegibility)
- [ ] Verify connector lines are visible between all nodes
- [ ] With sidebar expanded: verify pipeline still fits (sidebar ~200px → ~960px content width)
- [ ] With sidebar collapsed: verify pipeline has more breathing room (~1104px content width)

---

## Acceptance Criteria

- [ ] `aria-live="polite"` region inside `<nav>` announces pipeline status summary on change
- [ ] Radix tooltips appear on hover/focus for nodes with `detail` text
- [ ] Active node connectors are blue-tinted (`color-mix` with `accent-strong`)
- [ ] Error node connectors are red-tinted (`color-mix` with `danger`)
- [ ] Not-configured indicator bg bumped to `rgba(224,224,224,0.22)`
- [ ] Vertical layout (`<=640px`) shows vertical connector lines between nodes
- [ ] `__MOCK_VALIDATION_WARNING__` sentinel added to mock and CI grep
- [ ] Pipeline-specific Playwright tests exist in `pipeline.spec.ts`
- [ ] All nodes visible and readable at 1280x800 (Steam Deck viewport)
- [ ] No new dependencies added
- [ ] `npm run build` passes with zero errors

## Completion Checklist

- [ ] Code follows existing pipeline BEM naming and `data-status` selector pattern
- [ ] Radix tooltip usage matches `InfoTooltip.tsx` pattern
- [ ] `aria-live` region uses PatternFly-style separate hidden div (not on `<ol>`)
- [ ] New CSS variables follow `--crosshook-color-pipeline-connector-*` naming
- [ ] CI sentinel grep updated for new mock sentinel string
- [ ] Manual validation performed in browser mock mode and real Tauri mode
- [ ] Steam Deck viewport (1280x800) manually validated
- [ ] No unnecessary scope additions beyond Phase 4 requirements
- [ ] Plan is self-contained for single or parallel implementation pass

## Risks

| Risk                                                                                      | Likelihood | Impact | Mitigation                                                                                                                                                                                  |
| ----------------------------------------------------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| WebKitGTK doesn't support `color-mix()` for new connector tokens                          | Low        | Medium | `color-mix()` is already used in Phase 1-3 connectors. If it fails in WebKitGTK, all connectors break (not just new ones). Fallback: hardcoded hex values.                                  |
| Radix tooltip portal z-index conflicts with other overlays                                | Low        | Low    | Set `zIndex: 9999` per `InfoTooltip` pattern. Pipeline tooltip is lower-priority than modals.                                                                                               |
| `aria-live` floods screen reader with announcements during rapid launch phase transitions | Medium     | Medium | Only announce non-trivial statuses (error, not-configured, active). `useMemo` ensures the string only updates when nodes actually change. `aria-atomic="true"` ensures atomic announcement. |
| Vertical connectors look wrong in WebKitGTK due to `::before` positioning                 | Low        | Low    | Vertical layout is a rare fallback (`<=640px`). If `::before` fails, connectors are simply absent (same as current behavior).                                                               |
| Not-configured contrast bump makes the indicator too prominent                            | Low        | Low    | `0.15` → `0.22` is subtle. Verify visually and adjust if needed.                                                                                                                            |
| New Playwright tests are flaky due to fixture timing                                      | Medium     | Low    | Tests are dev-time tools, not CI gates. Use `waitFor` and generous timeouts per existing patterns.                                                                                          |

## Notes

- Phase 4 is the final phase of the launch pipeline visualization feature (issue #74).
- After Phase 4, the PRD status should be updated from DRAFT to COMPLETE.
- The `__MOCK_VALIDATION_WARNING__` sentinel is the first warning-severity mock fixture. Future
  mock enhancements can follow the same pattern.
- The `crosshook-visually-hidden` class in `dev-indicator.css` is reusable project-wide. If more
  components need it, consider extracting to a shared `utilities.css` — but don't do that in this
  phase.
