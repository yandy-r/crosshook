# Fix Report — PR #186 Review

**Source review**: `docs/prps/reviews/pr-186-review.md`
**Date**: 2026-04-09
**Mode**: `--parallel --severity medium`
**Severity threshold**: MEDIUM (CRITICAL + HIGH + MEDIUM)

---

## Summary

| Metric | Count |
|--------|-------|
| Findings eligible | 12 (F001–F012) |
| Fixed | 11 |
| Already fixed (stale) | 1 (F001) |
| Failed | 0 |
| Skipped (below threshold) | 8 (F013–F020, LOW/NIT) |

## Validation

| Check | Result |
|-------|--------|
| `tsc --noEmit` | Pass (zero errors) |
| `cargo test -p crosshook-core` | Pass (all tests green) |

---

## Fixes Applied

### F001 — useFocusTrap phantom `onClose` dependency (HIGH)
- **Status**: Already fixed (stale)
- **Detail**: The dependency array at `useFocusTrap.ts:191` already reads `[open, panelRef, initialFocusRef, restoreFocusOnClose]` — `onClose` was not present. The fix was applied in a prior commit on this branch.

### F002 — CollectionAssignMenu dual focus trap documentation (HIGH)
- **Status**: Fixed
- **File**: `src/crosshook-native/src/components/collections/CollectionAssignMenu.tsx`
- **Change**: Added JSDoc comment block after imports explaining intentional divergence from `useFocusTrap` (popover context: no body-lock/inert, ArrowUp/Down roving unique to checkbox list).

### F003 — Playwright deterministic assertion (MEDIUM)
- **Status**: Fixed
- **File**: `src/crosshook-native/tests/collections.spec.ts`
- **Change**: Replaced `waitForTimeout(1_000)` + `.catch(() => {})` with deterministic `toBeVisible({ timeout: 5_000 })` / `not.toBeVisible({ timeout: 5_000 })` assertions on the review dialog.

### F004 — `:focus-visible` on label never fires (MEDIUM)
- **Status**: Fixed
- **File**: `src/crosshook-native/src/styles/theme.css`
- **Change**: Changed `.crosshook-collection-assign-menu__option:focus-visible` to `:focus-within` so the focus ring renders when the nested checkbox receives keyboard focus.

### F005 — Browser-dev modal ships in production bundle (MEDIUM)
- **Status**: Fixed
- **File**: `src/crosshook-native/src/components/collections/CollectionsSidebar.tsx`
- **Changes**:
  - Replaced `isBrowserDevUi()` runtime guard with `__WEB_DEV_MODE__` compile-time constant (matching established pattern in `App.tsx`).
  - Wrapped `<BrowserDevPresetExplainerModal>` JSX in `{__WEB_DEV_MODE__ && (...)}` guard.
  - Removed unused `isBrowserDevUi` import.

### F006 — Resize handler re-renders on every pixel (MEDIUM)
- **Status**: Fixed
- **File**: `src/crosshook-native/src/components/collections/CollectionAssignMenu.tsx`
- **Change**: Gated `setViewportTick` with `requestAnimationFrame` + `cancelAnimationFrame` to coalesce rapid resize events.

### F007 — `<h2>` missing margin reset (MEDIUM)
- **Status**: Fixed
- **File**: `src/crosshook-native/src/styles/sidebar.css`
- **Change**: Added `margin: 0;` to `.crosshook-sidebar__section-label` before the existing `margin-bottom` value, resetting browser-default `<h2>` margins.

### F008 — Hardcoded `rgba` instead of CSS variable (MEDIUM)
- **Status**: Fixed
- **Files**: `src/crosshook-native/src/styles/theme.css`, `src/crosshook-native/src/styles/variables.css`
- **Changes**:
  - Added `--crosshook-focus-ring-inner: rgba(255, 255, 255, 0.06);` to `variables.css`.
  - Replaced raw `rgba(255, 255, 255, 0.06)` in both focus ring rules with `var(--crosshook-focus-ring-inner)`.

### F009 — Inline `onClose` not stabilized with `useCallback` (MEDIUM)
- **Status**: Fixed (all 3 callers)
- **Files**:
  - `CollectionEditModal.tsx` — `guardedOnClose = useCallback(() => { if (!busy) onClose(); }, [busy, onClose])`
  - `CollectionImportReviewModal.tsx` — `guardedOnClose = useCallback(() => { if (!applying) onClose(); }, [applying, onClose])`
  - `BrowserDevPresetExplainerModal.tsx` — `guardedOnClose = useCallback(() => { if (!busy) onClose(); }, [busy, onClose])`

### F010 — GameDetailsModal FOCUSABLE_SELECTOR drift (MEDIUM)
- **Status**: Fixed
- **File**: `src/crosshook-native/src/components/library/GameDetailsModal.tsx`
- **Change**: Added TODO comment above `FOCUSABLE_SELECTOR` referencing `useFocusTrap` and `lib/focus-utils.ts`, explaining the intentional private copy from PR #186.

### F011 — Inert `overscroll-behavior: contain` (MEDIUM)
- **Status**: Fixed
- **File**: `src/crosshook-native/src/components/collections/CollectionViewModal.css`
- **Change**: Removed `overscroll-behavior: contain` from `.crosshook-collection-modal__body` (not a scroll container).

### F012 — Phase comments in production CSS (MEDIUM)
- **Status**: Fixed
- **File**: `src/crosshook-native/src/styles/theme.css`
- **Changes**:
  - `/* Profile collections (Phase 2) */` → `/* Profile collections */`
  - `/* Phase 5: focus-visible rings ... */` → `/* Profile collections sidebar + assign menu focus rings */`

---

## Remaining Open Findings (below threshold)

| ID | Severity | Summary |
|----|----------|---------|
| F013 | LOW | Orphaned `closeButtonRef` in CollectionViewModal + CollectionImportReviewModal |
| F014 | LOW | LibraryCard keyboard context-menu casts partial to MouseEvent |
| F015 | LOW | PR body overstates adoption scope ("all 5 collection modals") |
| F016 | LOW | JTBD test Step 12 doesn't exercise high-level apply function |
| F017 | LOW | `console.error` ships in production bundle |
| F018 | LOW | `restoreFocusOnClose` option is YAGNI |
| F019 | NIT | Rust integration test synthetic path lacks inline docs |
| F020 | NIT | `onSubmitEdit={async () => false}` placeholder unexplained |

---

## Next Steps

1. Run `/ycc:code-review` to verify fixes resolved the findings.
2. Run `/ycc:git-workflow` or `/ycc:prp-commit` to commit the fixes.
3. Optionally address LOW/NIT findings (F013–F020) in a follow-up.
