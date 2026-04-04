# Task Structure Analysis: game-details-modal

## Executive Summary

The feature should be split into a shell-first implementation flow: modal foundation, library integration, data enrichment, then UX hardening. The most critical dependency is card interaction restructuring so details-open does not conflict with existing footer actions. Most enrichment work can be parallelized once modal API and page wiring are stable.

## Recommended Phase Structure

### Phase 1: Foundation and Entry Wiring

**Purpose**: Establish modal shell, state ownership, and entry interactions.
**Suggested Tasks**:

- Create `GameDetailsModal` shell with portal/focus/close behavior.
- Add modal state and render wiring in `LibraryPage`.
- Split card body vs footer interaction semantics for details opening.
  **Parallelization**: 1 independent task initially; 2 additional tasks can run after modal props contract is defined.

### Phase 2: Data and Actions MVP

**Purpose**: Load core profile/details content and wire minimal quick actions.
**Suggested Tasks**:

- Load selected profile details via existing command/hook patterns.
- Render baseline metadata/art with section-level states.
- Delegate Launch/Edit/Favorite actions to existing page handlers.
  **Dependencies**: Phase 1 complete.
  **Parallelization**: 2 tasks can run in parallel once modal state contract is stable.

### Phase 3: Enrichment and Hardening

**Purpose**: Add optional detail sections, robust async behavior, and UX polish.
**Suggested Tasks**:

- Integrate optional Proton/health/offline sections with explicit degraded states.
- Add stale-request guards for rapid profile switching.
- Complete responsive/focus/scroll hardening and manual verification checklist.
  **Dependencies**: Phase 2 complete.

## Task Granularity Recommendations

### Appropriate Task Sizes

- "Create modal shell and accessibility semantics" (2 files).
- "Wire library page modal state and rendering" (1 file).
- "Split card interaction behavior for details vs footer actions" (2-3 files).
- "Add section-level metadata/proton enrichment" (1-2 files).

### Tasks to Split

- Avoid combining card interaction changes with data-enrichment changes in a single task.
- Keep `useScrollEnhance` selector updates isolated from modal feature logic.

### Tasks to Combine

- Responsive CSS polish and focus-order polish can be combined in one UX-focused task.

## Dependency Analysis

### Independent Tasks (Can Run in Parallel)

- Task: Build modal shell component - File(s): `/src/crosshook-native/src/components/library/GameDetailsModal.tsx`, `/src/crosshook-native/src/components/library/GameDetailsModal.css`
- Task: Prepare enrichment section subcomponents - File(s): modal component files only (after shell contract exists)

### Sequential Dependencies

- Modal shell must exist before page wiring can mount it cleanly.
- Card interaction update should land before validating quick-action regression coverage.
- Baseline profile-loading state should land before optional enrichment sections.

### Potential Bottlenecks

- `LibraryCard` event behavior is shared and regression-prone.
- `LibraryPage` is central for modal orchestration and quick-action delegation.

## File-to-Task Mapping

### Files to Create

| File                                                                | Suggested Task                          | Phase | Dependencies |
| ------------------------------------------------------------------- | --------------------------------------- | ----- | ------------ |
| `/src/crosshook-native/src/components/library/GameDetailsModal.tsx` | Build modal shell and data section host | 1     | none         |
| `/src/crosshook-native/src/components/library/GameDetailsModal.css` | Add modal-specific layout styling       | 1     | none         |

### Files to Modify

| File                                                           | Suggested Task                                        | Phase | Dependencies         |
| -------------------------------------------------------------- | ----------------------------------------------------- | ----- | -------------------- |
| `/src/crosshook-native/src/components/pages/LibraryPage.tsx`   | Modal state and action delegation                     | 1-2   | Phase 1 shell        |
| `/src/crosshook-native/src/components/library/LibraryGrid.tsx` | Thread details-open callback                          | 1     | Phase 1 shell        |
| `/src/crosshook-native/src/components/library/LibraryCard.tsx` | Body/footer interaction split                         | 1     | Phase 1 shell        |
| `/src/crosshook-native/src/hooks/useScrollEnhance.ts`          | Register any new inner scroller selectors (if needed) | 3     | Phase 2 layout shape |

## Optimization Opportunities

### Maximize Parallelism

- Keep shell and wiring separated from enrichment section implementation.
- Build optional sections against a stable modal props/view-model contract.

### Minimize Risk

- Isolate card interaction changes and verify footer action regressions early.
- Keep persistence untouched in v1 to avoid broad coordination costs.

## Security-Critical Sequencing

- Add safe text rendering and explicit no-HTML rendering guardrails before rendering remote metadata fields.
- Review capability/CSP implications before introducing external link or remote-media behavior.
- Preserve normalized/fixed-input command usage before considering new IPC.

## Reuse Recommendations

- Extend modal behavior from existing `ProfileReviewModal`/`ProfilePreviewModal` patterns.
- Reuse `useGameMetadata`, `useGameCoverArt`, and existing page action handlers.
- Keep modal-specific view-model logic feature-local first; do not abstract prematurely.

## Implementation Strategy Recommendations

- Start with a functional shell that can open/close and display summary identity.
- Add quick actions by delegating existing handlers, then enrich sections incrementally.
- End with a hardening pass focused on async race handling, focus semantics, and scroll behavior.
