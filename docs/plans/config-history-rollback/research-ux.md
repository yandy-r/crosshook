# UX Research: Configuration History, Diff, and Rollback

## Goal and Context

Design a safe, fast workflow in CrossHook for:

1. inspecting profile configuration history (timeline),
2. comparing versions (diff),
3. restoring a prior snapshot without losing current work.

This guidance is implementation-oriented for a desktop app (Steam Deck/Linux) with a React profile editor.

---

## Recommended Information Architecture

### 1) Primary entry point

- Add `History` action in the existing profile editor action group (near Save/Duplicate/Delete).
- Open a **History panel** (right side drawer on desktop widths; full-screen modal/sheet on narrow viewports/Steam Deck mode).

### 2) History panel structure

- **Header**: profile name, total snapshots, filter/sort controls.
- **Snapshot list (timeline)**:
  - timestamp (relative + exact on hover),
  - origin (`Auto-save`, `Manual checkpoint`, `Import`, `Restore`),
  - actor/source (if available),
  - short note/message.
- **Selected snapshot preview area**:
  - metadata summary,
  - quick stats (`+N` / `-M` fields changed),
  - actions: `Compare with current`, `Compare with…`, `Restore`.

### 3) Diff view IA

- Use a separate **Compare modal** (or split pane in panel for wide layouts) with:
  - left/right version selectors,
  - grouped sections by config domain (Launch, Paths, Optimizations, Environment, etc.),
  - per-field status chips: `Added`, `Removed`, `Changed`.
- Include collapse/expand controls and “show changed only” toggle.

### 4) Restore IA

- Restore action always opens a **confirmation dialog** that states:
  - exactly which snapshot is being restored,
  - what happens to current state (saved as new snapshot first),
  - whether restore is reversible (undo window / history checkpoint).

**Why this IA**  
Side timeline + explicit compare + guarded restore aligns with common patterns from Docs/Figma/Notion history tools and supports “inspect before commit” behavior.
**Confidence**: High (multi-source pattern convergence from official product docs).

---

## Minimum Interaction Flow (Snapshot List, Compare, Restore)

### A. Open and inspect snapshots

1. User clicks `History`.
2. History panel opens with loading skeleton.
3. Timeline renders newest-first snapshots.
4. Selecting an item updates preview metadata and enables compare/restore actions.

### B. Compare flow

1. User clicks `Compare with current` (default baseline = currently loaded profile state).
2. Compare modal opens with side-by-side (or grouped inline) diff.
3. User can switch either side via version pickers.
4. User can filter to changed fields only.
5. User exits compare without side effects.

### C. Restore flow

1. User clicks `Restore`.
2. Confirmation dialog appears (destructive/major state change).
3. On confirm:
   - app first creates a pre-restore checkpoint of current state,
   - app applies selected snapshot,
   - app records a `Restored from <snapshot>` checkpoint.
4. Show completion toast/status with `Undo` action (time-boxed).
5. If undo is triggered, restore the pre-restore checkpoint.

**Why this flow**  
It combines strong pre-action validation with non-destructive history behavior (restore does not erase future history), which reduces anxiety and recovery cost.
**Confidence**: High (Figma and Google Docs both treat restore as non-destructive with retained history).

---

## States: Loading, Empty, Error

### Loading states

- History panel:
  - skeleton rows for timeline items,
  - disable compare/restore actions until selection exists.
- Compare modal:
  - preserve shell immediately, lazy-load diff content,
  - show per-section skeletons to avoid blank modal.

### Empty states

- No history yet:
  - message: `No snapshots yet`
  - helper: `Snapshots are created when you save or when changes are auto-captured.`
  - CTA: `Create snapshot now` (manual checkpoint).

### Error states

- Fetch failure:
  - inline alert with retry: `Couldn’t load history. Check storage access and try again.`
- Compare failure:
  - keep modal open, show recoverable inline error + `Retry`.
- Restore failure:
  - blocking error dialog with reason + safe fallback:
    - `Restore failed. Your current configuration was not changed.` (if atomic),
    - or `Partial restore detected. Revert to pre-restore snapshot?`.

**Confidence**: High (general resilient async UX + WCAG status/alert guidance).

---

## Accessibility Considerations (Must-Have)

- **Keyboard model**
  - `Tab/Shift+Tab` moves between major regions.
  - Arrow keys navigate timeline rows if implemented as composite list.
  - `Enter/Space` activates selected row action.
- **Focus management**
  - move focus into panel heading on open;
  - return focus to triggering `History` button on close;
  - on modal open, trap focus; on close, restore prior focus.
- **Announcements**
  - use `role="status" aria-atomic="true"` for non-critical updates (`History loaded`, `Snapshot restored`).
  - use `role="alert"` (or assertive live region) for restore/compare errors.
  - keep live region containers mounted before content updates.
- **Perception**
  - do not rely on color alone for diff changes; include text chips/icons.
  - ensure visible focus ring and distinction between focused vs selected timeline row.

**Confidence**: High (W3C ARIA APG + WCAG ARIA techniques).

---

## Safeguards and Risk Controls

### Confirmation and intent checks

- Use confirmation dialog only for restore (not for routine inspect/compare actions).
- Dialog copy must be specific (snapshot time/name + direct consequence).
- Action buttons should be explicit (`Restore snapshot`, `Keep current config`) instead of `Yes/No`.

### Undo affordance

- After restore, show toast with `Undo` (e.g., 8-15s window).
- Undo should map to pre-restore checkpoint to guarantee reversibility.

### Conflict and staleness warnings

- If editor has unsaved changes:
  - warn before compare/restore: `You have unsaved edits. Compare/restore from saved state or save first.`
- If snapshot schema/version differs:
  - show compatibility warning and fields that cannot be applied.
- If profile changed externally while panel open:
  - show stale badge and require refresh before restore.

### Integrity protections

- Restore should be transaction-like:
  - create pre-restore checkpoint,
  - apply snapshot,
  - verify write success,
  - emit post-restore checkpoint.
- On failure, auto-recover to pre-restore state.

**Confidence**: High (confirmation-dialog and undo heuristics + practical versioning behavior in mature products).

---

## Example Microcopy (Concise, Implementation-Ready)

### Timeline/List

- `History`
- `Latest`
- `Manual checkpoint`
- `Auto-saved`
- `Restored`
- `Compare with current`
- `Restore snapshot`

### Empty

- `No snapshots yet`
- `Create a checkpoint before major edits so you can roll back safely.`
- `Create snapshot`

### Confirm restore dialog

- Title: `Restore this configuration snapshot?`
- Body: `You’re restoring “Mar 30, 11:42”. Your current config will be saved as a new snapshot first.`
- Primary: `Restore snapshot`
- Secondary: `Keep current config`

### Success/undo

- `Snapshot restored.`
- `Undo`

### Errors

- `Couldn’t load configuration history.`
- `Restore failed. No changes were applied.`
- `This snapshot uses an older format. Some fields may not restore exactly.`

---

## MVP vs Enhanced UX Scope

### MVP (ship first)

- History entry point in profile editor.
- Timeline list with metadata (time, type, short message).
- Single compare mode: `selected snapshot` vs `current`.
- Field-level changed-only diff (no fancy visualizations).
- Restore with explicit confirmation.
- Pre-restore checkpoint + post-restore checkpoint.
- Success toast with undo.
- Core states: loading/empty/error.
- Basic keyboard and live-region accessibility.

### Enhanced (phase 2+)

- Compare any two snapshots (not only vs current).
- Rich diff grouping with collapse/expand and search.
- Named snapshots/tags (`Before patch`, `Stable build`).
- Retention and pinning controls (`Keep forever`, auto-prune policy).
- Conflict assistant for incompatible fields (guided merge choices).
- Activity provenance (source/action/user details), export/share snapshot diff.
- Optional “Only named snapshots” filter pattern.

**Confidence**: Medium-High (core MVP is strongly validated; enhanced items are practical extrapolations tailored to CrossHook).

---

## Practical Implementation Notes (React/Desktop)

- Reuse existing panel/modal primitives where possible; avoid introducing new interaction models.
- Keep history fetch/diff/restore APIs independent:
  - `listSnapshots(profileId)`
  - `getDiff(baseSnapshotId, targetSnapshotId | current)`
  - `restoreSnapshot(profileId, snapshotId, { createCheckpoint: true })`
  - `undoLastRestore(profileId, restoreOperationId)` (or implicit previous checkpoint restore)
- Treat compare as read-only operation; no writes from compare UI.
- Instrument events for UX validation:
  - `history_opened`, `snapshot_selected`, `compare_opened`, `restore_confirmed`, `restore_undone`, `restore_failed`.

---

## Sources

- Google Docs Help: Version history, named versions, restore/copy behavior  
  <https://support.google.com/docs/answer/190843?hl=en>
- Figma Help: file version history, permissions, non-destructive restore checkpoints  
  <https://help.figma.com/hc/en-us/articles/360038006754-View-a-file-s-version-history>
- Nielsen Norman Group: confirmation dialog best practices, specificity, undo emphasis  
  <https://www.nngroup.com/articles/confirmation-dialog/>
- W3C WCAG Technique ARIA22: `role="status"` for status messages  
  <https://www.w3.org/WAI/WCAG22/Techniques/aria/ARIA22.html>
- W3C WCAG Technique ARIA19: `role="alert"`/live regions for error messaging  
  <https://www.w3.org/WAI/WCAG21/Techniques/aria/ARIA19>
- WAI-ARIA APG Keyboard Interface: keyboard navigation and focus-management conventions  
  <https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/>

Accessed: 2026-03-30.
