# Fix Report — PR #186 Review

**Source review**: `docs/prps/reviews/pr-186-review.md`
**Date**: 2026-04-09
**Mode**: `--parallel --severity medium`
**Severity threshold**: MEDIUM (CRITICAL + HIGH + MEDIUM)

---

## Summary

| Metric                    | Count                  |
| ------------------------- | ---------------------- |
| Findings eligible         | 12 (F001–F012)         |
| Fixed                     | 11                     |
| Already fixed (stale)     | 1 (F001)               |
| Failed                    | 0                      |
| Skipped (below threshold) | 8 (F013–F020, LOW/NIT) |

## Validation

| Check                          | Result                 |
| ------------------------------ | ---------------------- |
| `tsc --noEmit`                 | Pass (zero errors)     |
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

## Remaining Open Findings

None — all 20 findings resolved.

---

## Pass 2 — LOW severity (2026-04-09)

**Mode**: `--parallel --severity low`
**Severity threshold**: LOW (CRITICAL + HIGH + MEDIUM + LOW)

| Metric                    | Count                      |
| ------------------------- | -------------------------- |
| Findings eligible         | 6 (F013–F018)              |
| Fixed                     | 5                          |
| Resolved by prior fix     | 1 (F017, resolved by F005) |
| Failed                    | 0                          |
| Skipped (below threshold) | 2 (F019–F020, NIT)         |

### Validation

| Check                          | Result                 |
| ------------------------------ | ---------------------- |
| `tsc --noEmit`                 | Pass (zero errors)     |
| `cargo test -p crosshook-core` | Pass (all tests green) |

### Fixes Applied

**F013** (LOW) — Removed orphaned `closeButtonRef` declarations and `ref=` JSX in `CollectionViewModal.tsx` and `CollectionImportReviewModal.tsx`.

**F014** (LOW) — Changed `onContextMenu` prop from `MouseEvent<HTMLDivElement>` to `{ x: number; y: number }` across `LibraryCard.tsx`, `LibraryGrid.tsx`, and `LibraryPage.tsx`. Eliminates type-unsafe cast.

**F015** (LOW) — Updated PR #186 body: "all 5 collection modals" → "four of the five collection modals" with explanation of CollectionAssignMenu's intentional divergence.

**F016** (LOW) — Updated Step 12 comment in `collections_jtbd_integration.rs` to clarify simulation via building blocks (no high-level apply function exists yet).

**F017** (LOW) — Resolved by F005. `BrowserDevPresetExplainerModal` is now gated behind `__WEB_DEV_MODE__`, so the `console.error` is tree-shaken in production.

**F018** (LOW) — Removed YAGNI `restoreFocusOnClose` option from `useFocusTrap` interface + all 4 callers. Focus restore is now hardcoded (always on).

---

## Pass 3 — NIT severity (2026-04-09)

**Mode**: `--parallel`

| Metric            | Count         |
| ----------------- | ------------- |
| Findings eligible | 2 (F019–F020) |
| Fixed             | 2             |

**F019** (NIT) — Added comment in `collections_jtbd_integration.rs` documenting that `/profiles/fixture-00.toml` is a synthetic path (AppWrite bypasses `fs::metadata`).

**F020** (NIT) — Added inline comment `// mode="create" — onSubmitEdit is never called here` in `CollectionsSidebar.tsx`.

---

## Cumulative Totals

| Metric                | Count    |
| --------------------- | -------- |
| Total findings        | 20       |
| Fixed                 | 18       |
| Already fixed (stale) | 1 (F001) |
| Resolved by prior fix | 1 (F017) |
| Remaining             | 0        |

All 20 findings from PR #186 review are resolved.
