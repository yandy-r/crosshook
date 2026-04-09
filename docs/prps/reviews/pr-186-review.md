# PR #186 Review — Profile Collections Phase 5

**PR**: [#186](https://github.com/yandy-r/crosshook/pull/186) — "feat(ui): profile collections polish, integration tests, Steam Deck validation — Phase 5"
**Head SHA**: `350cd143412585e49f3723891a72fe68e88fe9d0`
**Base**: `main`
**Author**: @yandy-r
**Reviewer**: `/ycc:code-review --parallel` (3 agents: correctness, security, quality)
**Date**: 2026-04-08
**Decision**: **REQUEST CHANGES**

---

## Summary

Phase 5 ships the quality gate for Profile Collections: a shared `useFocusTrap` hook extracted from `GameDetailsModal`, keyboard-navigable `CollectionAssignMenu`, semantic-HTML sidebar, a Rust JTBD integration test, four Playwright smoke tests, `:focus-visible` styles, and four internal docs. The PR body correctly states "no new persisted data, no new dependencies, no new IPC commands" — **verified**: `src-tauri/` has zero diff, no new `#[tauri::command]`, no new `invoke()` call sites, no new crate dependencies, no committed secrets.

**Validation status**:

| Check | Result |
|-------|--------|
| `cargo test -p crosshook-core` | ✅ 776 pass + 1 new JTBD integration test pass |
| `npx tsc --noEmit` | ✅ clean |
| Playwright smoke | ⏸ not executed (needs dev server; PR body claims 13/13 green) |

**Why REQUEST CHANGES**: One HIGH-severity correctness bug in `useFocusTrap` was independently found by all three parallel reviewers: `onClose` is listed in the `useEffect` dependency array but never read inside the effect body. Three of the four consumers pass a new inline-arrow `onClose` on every render, which causes the effect to tear down and re-execute — unlocking and re-locking body scroll, clearing and re-applying sibling `inert`, and firing a fresh focus jump via `requestAnimationFrame` — on **every parent re-render while the modal is open**. In `CollectionEditModal` this fires on every keystroke in the name/description inputs and on every toggle of the `busy` flag. For keyboard and screen-reader users this is a directly observable regression in the feature's primary authoring flow. The fix is a one-line removal from the dep array.

The refactor is also structurally incomplete in two ways: (a) `CollectionAssignMenu` keeps its own hand-rolled focus trap instead of adopting `useFocusTrap`, leaving two focus-trap implementations to maintain; the PR body misrepresents this as "all 5 collection modals" adopting the hook; (b) `closeButtonRef` in `CollectionViewModal` and `CollectionImportReviewModal` is declared, attached to the JSX, but never read — a refactor leftover.

Additionally there are four MEDIUM CSS/bundle hygiene issues and eight LOW/NIT findings worth cleaning up. None of the remaining findings block merge on their own, but the one HIGH bug does.

---

## Findings

### F001 — `useFocusTrap` phantom `onClose` dependency thrashes the trap on every render

- **Severity**: HIGH
- **Category**: Correctness / Performance / Maintainability (3× reviewer consensus)
- **File**: `src/crosshook-native/src/hooks/useFocusTrap.ts:191`
- **Status**: Open

**Description**: `onClose` appears in the `useEffect` dependency array at line 191 but is **not referenced** inside the effect body (lines 131–190) or its cleanup. It is only called inside `handleKeyDown`, which is a plain function defined outside the effect. Three of the four consumers pass an inline arrow as `onClose`:

- `src/crosshook-native/src/components/collections/CollectionEditModal.tsx:51-53`
  ```ts
  onClose: () => { if (!busy) onClose(); },
  ```
- `src/crosshook-native/src/components/collections/CollectionImportReviewModal.tsx:81`
- `src/crosshook-native/src/components/collections/BrowserDevPresetExplainerModal.tsx:105`

Every render (e.g., keystroke in the name field, `busy` toggling during submit) creates a new `onClose` reference, which fires the effect cleanup (unlocks body scroll, clears sibling `inert` + `aria-hidden`, runs focus restore) and immediately re-executes the effect (re-locks body, walks `body.children` to re-`inert` all siblings, fires a fresh `requestAnimationFrame` focus move). This is directly observable as focus and scroll jank, and on assistive technology it generates spurious aria-live announcements.

The inner sibling-inert loop (`Array.from(body.children).filter(...).map(...)`, lines 149–158) is O(n) over every top-level body child and fires per re-render cycle.

**Suggested fix**: Remove `onClose` from the dependency array — it is not used in the effect body:

```ts
// src/crosshook-native/src/hooks/useFocusTrap.ts:191
}, [open, panelRef, initialFocusRef, restoreFocusOnClose]);
```

If the linter complains, store `onClose` in a `useRef` and read it from `handleKeyDown`:

```ts
const onCloseRef = useRef(onClose);
useEffect(() => { onCloseRef.current = onClose; });
// then in handleKeyDown: onCloseRef.current();
```

As a secondary improvement, stabilize the inline `onClose` wrappers at the call sites with `useCallback` to eliminate the re-render trigger entirely:

```ts
const guardedOnClose = useCallback(() => {
  if (!busy) onClose();
}, [busy, onClose]);
```

---

### F002 — `CollectionAssignMenu` keeps its own focus trap; dual implementation drifts from `useFocusTrap`

- **Severity**: HIGH
- **Category**: Maintainability
- **File**: `src/crosshook-native/src/components/collections/CollectionAssignMenu.tsx:29-150`
- **Status**: Open

**Description**: The PR body states `useFocusTrap` is adopted in "all 5 collection modals." In practice, four modals adopt the hook. `CollectionAssignMenu` has its own bespoke focus-management implementation: it manually saves/restores focus (lines 64–86), manually traps Tab (lines 111–132), handles ArrowUp/ArrowDown roving navigation (lines 135–149), and handles Escape (lines 104–109). The correctness-reviewer verified this implementation is functionally correct for a popover, and the divergence from a full-screen modal is defensible (no body-lock, no sibling `inert`). However:

1. Two focus-trap implementations will have to be kept in sync forever — when `getFocusableElements` selector changes, or when Tab wrap logic gains a bugfix, the fix must be applied twice.
2. There is no code comment or `// TODO:` explaining why `CollectionAssignMenu` diverged — the next developer will see the inconsistency and either (a) lose time "fixing" it by migrating or (b) fail to migrate when the hook gains a shared feature.
3. The PR body documentation is wrong — it says 5 modals, actual is 4.

**Suggested fix**: Either

(a) extend `useFocusTrap` with an optional `arrowNavigation?: 'roving' | 'none'` flag and migrate `CollectionAssignMenu`, or
(b) add a clear top-of-file comment in `CollectionAssignMenu.tsx` explaining the intentional divergence — body-lock and sibling `inert` are inappropriate for a popover, and the ArrowUp/Down roving is unique to the checkbox list. Reference the hook so future devs know it exists.

Also correct the PR body: "adopted in **4 of the 5 collection modals** — `CollectionAssignMenu` retains its popover-specific implementation."

---

### F003 — Playwright import preset test uses `waitForTimeout(1_000)` instead of deterministic assertion

- **Severity**: MEDIUM
- **Category**: Correctness / Test Quality (3× reviewer consensus)
- **File**: `src/crosshook-native/tests/collections.spec.ts:150-157`
- **Status**: Open

**Description**: After clicking "Continue" in the `BrowserDevPresetExplainerModal`, the test sleeps for 1 second, then presses Escape and `.catch(() => {})` swallows any error. Consequences:

1. If the import review modal takes >1s to mount under CI load, the Escape keypress fires against the wrong state and the test never verifies the flow it claims to cover.
2. If `handleImportExplainerContinue` throws or the IPC mock silently fails, the test still passes (the `capture.errors` check runs but any caught exception skips it).
3. The comment "Wait for the import review dialog. Or the explainer closes into it." acknowledges the test expectation itself is underspecified.

**Suggested fix**: Replace the hard sleep with a deterministic visibility assertion:

```ts
// src/crosshook-native/tests/collections.spec.ts:152
const reviewDialog = page.getByRole('dialog', { name: /import collection preset/i });
await expect(reviewDialog).toBeVisible({ timeout: 5_000 });
await page.keyboard.press('Escape');
await expect(reviewDialog).not.toBeVisible({ timeout: 5_000 });
```

---

### F004 — `:focus-visible` CSS on `<label>` never fires; checkbox inside receives focus instead

- **Severity**: MEDIUM
- **Category**: Correctness (a11y)
- **File**: `src/crosshook-native/src/styles/theme.css:6010-6012`
- **Status**: Open

**Description**: The new focus ring targets `.crosshook-collection-assign-menu__option:focus-visible`. That class is on a `<label>` (CollectionAssignMenu.tsx:215). The actual focusable element is the `<input type="checkbox">` nested inside. `:focus-visible` on the `<label>` only fires when the label itself receives focus (which it never does in keyboard navigation). The intended visual ring around the entire label row will **not** render when the user Tabs to a checkbox.

**Suggested fix**: Use `:focus-within` so the rule fires when any descendant receives focus:

```css
/* src/crosshook-native/src/styles/theme.css */
.crosshook-collection-assign-menu__option:focus-within {
  outline: 0;
  box-shadow:
    0 0 0 2px rgba(255, 255, 255, 0.06),
    0 0 0 4px var(--crosshook-color-accent-soft);
}
```

---

### F005 — `BrowserDevPresetExplainerModal` and `browser://` mock constants ship in production bundle

- **Severity**: MEDIUM
- **Category**: Security / Bundle Hygiene
- **File**: `src/crosshook-native/src/components/collections/CollectionsSidebar.tsx:61`, `CollectionViewModal.tsx:93`, `src/components/collections/BrowserDevPresetExplainerModal.tsx`, `src/constants/browserDevPresetPaths.ts`
- **Status**: Open

**Description**: The `isBrowserDevUi()` runtime guard is evaluated at runtime via `typeof window !== 'undefined' && !isTauri()`, not via the compile-time `__WEB_DEV_MODE__` constant. Vite cannot tree-shake branches on this check, so `BrowserDevPresetExplainerModal` and the `BROWSER_DEV_IMPORT_PRESET_PATH` / `BROWSER_DEV_EXPORT_PRESET_PATH` strings (`browser://mock-import.crosshook-collection.toml`) are included in the production AppImage.

The `.github/workflows/release.yml` `verify:no-mocks` CI sentinel scans for `[dev-mock]`, `getMockRegistry`, `registerMocks`, `MOCK MODE` — it will **not** catch these strings. This is not an exploitable vulnerability (the Tauri WebView never triggers the `!isTauri()` branch, and the constants are never passed to URL navigation or `fetch`), but it is dead weight in the production binary and a violation of the project's mock-isolation policy.

**Suggested fix**: Gate the branch behind the compile-time constant so Vite can dead-code-eliminate the import:

```tsx
// src/crosshook-native/src/components/collections/CollectionsSidebar.tsx:61
if (import.meta.env.DEV || __WEB_DEV_MODE__) {
  setImportExplainerOpen(true);
  return;
}
```

Also consider adding `BrowserDevPresetExplainerModal` or `browser://mock-` to the `verify:no-mocks` sentinel patterns in `.github/workflows/release.yml`.

---

### F006 — `CollectionAssignMenu` resize handler re-renders on every pixel without debounce

- **Severity**: MEDIUM
- **Category**: Performance
- **File**: `src/crosshook-native/src/components/collections/CollectionAssignMenu.tsx:37-46`
- **Status**: Open

**Description**: `onResize` calls `setViewportTick((t) => t + 1)` on every `resize` event with no rAF gate or debounce. Each event triggers a full re-render of the popover and its children. The only reason the state bump exists is to recalculate `Math.min(anchorPosition.x, window.innerWidth - 280)` — that only matters at the viewport edge.

**Suggested fix**: Gate with `requestAnimationFrame`:

```ts
useLayoutEffect(() => {
  if (!open) return;
  let frame = 0;
  function onResize() {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => setViewportTick((t) => t + 1));
  }
  window.addEventListener('resize', onResize);
  return () => {
    cancelAnimationFrame(frame);
    window.removeEventListener('resize', onResize);
  };
}, [open]);
```

---

### F007 — `<h2 class="crosshook-sidebar__section-label">` missing margin reset; browser default inflates sidebar spacing

- **Severity**: MEDIUM
- **Category**: Pattern Compliance (CSS / visual regression)
- **File**: `src/crosshook-native/src/styles/sidebar.css:81` (or wherever `.crosshook-sidebar__section-label` lives)
- **Status**: Open

**Description**: `CollectionsSidebar` was correctly upgraded from `<div>` to `<h2>`. The CSS rule for `.crosshook-sidebar__section-label` overrides `font-size`, `font-weight`, `letter-spacing`, `text-transform`, `padding`, and `margin-bottom` — but does **not** set `margin-top: 0`. There is no global heading reset. A browser-default `<h2>` carries ≈`0.83em` top margin, which will visually inflate the space above the "Collections" label in both WebKitGTK (Tauri target) and Chromium (dev).

**Suggested fix**:

```css
.crosshook-sidebar__section-label {
  margin: 0;
  margin-bottom: /* existing value */;
  /* rest unchanged */
}
```

---

### F008 — `theme.css` `focus-visible` rules hardcode `rgba(255, 255, 255, 0.06)` instead of using a CSS variable

- **Severity**: MEDIUM
- **Category**: Pattern Compliance (CSS conventions)
- **File**: `src/crosshook-native/src/styles/theme.css:6004-6015`
- **Status**: Open

**Description**: The two new `focus-visible` blocks (sidebar item, assign menu) use a raw `rgba(255, 255, 255, 0.06)` as an inner glow color. The existing focus-visible pattern at `theme.css:919-922` uses only `var(--crosshook-color-accent-soft)` / `var(--crosshook-color-accent-strong)` — no raw rgba. The new value is not declared in `variables.css`, so a future theme change cannot update it centrally.

**Suggested fix**: Either drop the inner ring to match the existing pattern, or promote `rgba(255, 255, 255, 0.06)` to a CSS variable:

```css
/* src/crosshook-native/src/styles/variables.css */
--crosshook-focus-ring-inner: rgba(255, 255, 255, 0.06);
```

Then reference it in `theme.css`.

---

### F009 — Inline `onClose: () => { if (!busy) onClose(); }` not stabilized with `useCallback`

- **Severity**: MEDIUM
- **Category**: Performance / Correctness (companion to F001)
- **Files**:
  - `src/crosshook-native/src/components/collections/CollectionEditModal.tsx:51-53`
  - `src/crosshook-native/src/components/collections/CollectionImportReviewModal.tsx:81-83`
  - `src/crosshook-native/src/components/collections/BrowserDevPresetExplainerModal.tsx:105`
- **Status**: Open

**Description**: Even if F001 is fixed (removing `onClose` from the dep array), the three callers still create a fresh inline function on every render. `handleKeyDown` closes over that reference and reads stale `busy` on each re-render. Stabilizing with `useCallback` removes the unnecessary re-render pressure and produces a stable closure.

**Suggested fix**:

```ts
const guardedOnClose = useCallback(() => {
  if (!busy) onClose();
}, [busy, onClose]);

const { handleKeyDown } = useFocusTrap({
  open,
  panelRef,
  onClose: guardedOnClose,
  initialFocusRef: headingRef,
  restoreFocusOnClose: true,
});
```

---

### F010 — `GameDetailsModal` retains a private copy of `FOCUSABLE_SELECTOR` that will drift from `lib/focus-utils.ts`

- **Severity**: MEDIUM
- **Category**: Maintainability
- **File**: `src/crosshook-native/src/components/library/GameDetailsModal.tsx:27-41` vs `src/crosshook-native/src/lib/focus-utils.ts`
- **Status**: Open

**Description**: The PR body explicitly notes `GameDetailsModal` was **not** migrated to `useFocusTrap` to avoid risk. Consequently there are now two copies of `FOCUSABLE_SELECTOR` + focus-walking logic in the codebase. These are currently identical but will diverge as soon as someone adds a focusable-element pattern (e.g., `[contenteditable]`) to the shared util without updating `GameDetailsModal`, or vice versa. There is no lint rule, test, or comment to prevent this.

**Suggested fix**: Add a top-of-file `// TODO:` in `GameDetailsModal.tsx` that names the shared util and references a tracking issue so future maintainers know the migration is pending:

```ts
// TODO(#<issue>): migrate to useFocusTrap from src/hooks/useFocusTrap.ts.
//                 This file intentionally keeps a private copy to avoid risk in PR #186.
```

Ideally file a tracking issue and link it.

---

### F011 — `CollectionViewModal.css` adds `overscroll-behavior: contain` to a non-scrolling element (inert CSS)

- **Severity**: MEDIUM
- **Category**: Pattern Compliance (CSS / scroll rule)
- **File**: `src/crosshook-native/src/components/collections/CollectionViewModal.css:2` (`.crosshook-collection-modal__body`)
- **Status**: Open

**Description**: `overscroll-behavior: contain` only takes effect on elements that are themselves scroll containers (`overflow-y: auto` or similar). `.crosshook-collection-modal__body` is a nested element inside `.crosshook-modal__body` which already handles scrolling and is correctly listed in the `SCROLLABLE` selector in `useScrollEnhance.ts`. The new property on the inner element is inert and will confuse future maintainers into thinking the nested element is a scroll container that needs registration.

Note: `CollectionImportReviewModal.css`'s `.crosshook-collection-import-review__body` is on a `<form>` that uses `crosshook-modal__body` — the scroll container is already registered via the parent class, so this one is fine.

**Suggested fix**: Remove `overscroll-behavior: contain` from `.crosshook-collection-modal__body` in `CollectionViewModal.css`.

---

### F012 — Planning-artifact comments (`Phase 2`, `Phase 5`) baked into production CSS

- **Severity**: MEDIUM
- **Category**: Pattern Compliance
- **File**: `src/crosshook-native/src/styles/theme.css:5839, 6000`
- **Status**: Open

**Description**: The CSS contains `/* Profile collections (Phase 2) */` and `/* Phase 5: focus-visible rings for sidebar + assign menu surfaces */`. The CLAUDE.md policy uses the `docs(internal):` prefix to keep phase vocabulary out of user-facing artifacts; production CSS should follow the same spirit. "Phase 5" has no meaning six months from now.

**Suggested fix**: Replace with component-semantic comments:

```css
/* Profile collections sidebar + assign menu focus rings */
```

---

### F013 — Orphaned `closeButtonRef` after refactor

- **Severity**: LOW
- **Category**: Completeness / Maintainability (2× reviewer consensus)
- **Files**:
  - `src/crosshook-native/src/components/collections/CollectionViewModal.tsx:62`, attached at line 237
  - `src/crosshook-native/src/components/collections/CollectionImportReviewModal.tsx:45`, attached at line 150
- **Status**: Open

**Description**: Both files declare `closeButtonRef = useRef<HTMLButtonElement>(null)`, attach `ref={closeButtonRef}` to the Close button, but never read `closeButtonRef.current` anywhere. In the pre-refactor pattern, this served as a fallback initial focus target (`headingRef.current ?? closeButtonRef.current`). After adopting `useFocusTrap`, the hook handles fallback focus internally and the ref is orphaned. Not a bug — a refactor leftover.

**Suggested fix**: Remove both declarations and the `ref=` JSX attributes.

---

### F014 — `LibraryCard` keyboard context-menu path casts `{clientX, clientY, preventDefault}` to full `MouseEvent`

- **Severity**: LOW
- **Category**: Type Safety
- **File**: `src/crosshook-native/src/components/library/LibraryCard.tsx:100`
- **Status**: Open

**Description**:

```ts
onContextMenu(
  { clientX: x, clientY: y, preventDefault: () => {} } as MouseEvent<HTMLDivElement>,
  profile.name
);
```

The cast tells TypeScript this partial object is a full `MouseEvent`. Current consumers only read `clientX`/`clientY`, so it works today. Any future access to `event.target`, `event.button`, `event.currentTarget` will silently return `undefined` at runtime with no type error.

**Suggested fix**: Change the `onContextMenu` prop shape to accept a position object, not a synthetic event:

```ts
// LibraryCard prop:
onContextMenu?: (position: { x: number; y: number }, profileName: string) => void;

// Keyboard path:
onContextMenu({ x, y }, profile.name);

// Mouse path:
onContextMenu({ x: e.clientX, y: e.clientY }, profile.name);
```

Small refactor in `LibraryGrid.tsx` / `LibraryPage.tsx` to match.

---

### F015 — PR body overstates adoption scope ("all 5 collection modals")

- **Severity**: LOW
- **Category**: Completeness (documentation)
- **File**: PR #186 body
- **Status**: Open

**Description**: The PR body says `useFocusTrap` is adopted in "all 5 collection modals." Actual adoption: 4 (`CollectionViewModal`, `CollectionEditModal`, `CollectionImportReviewModal`, `BrowserDevPresetExplainerModal`). `CollectionAssignMenu` intentionally keeps its own implementation. This mis-documentation may cause a future reviewer to think the hook is universal and to "fix" the assign menu unnecessarily.

**Suggested fix**: Update the PR body line:
> Extract shared `useFocusTrap` hook from `GameDetailsModal` and adopt in **four of the five** collection modals. `CollectionAssignMenu` retains its popover-specific focus management (no body-lock, no sibling-inert, adds ArrowUp/Down roving).

---

### F016 — JTBD integration test Step 12 does not exercise a high-level "apply imported collection" function

- **Severity**: LOW
- **Category**: Completeness
- **File**: `src/crosshook-native/crates/crosshook-core/tests/collections_jtbd_integration.rs:163`
- **Status**: Open

**Description**: Step 12 simulates "fresh-store re-import" by manually calling `create_collection`, `observe_profile_write`, `add_profile_to_collection`, and `set_collection_defaults` on a new in-memory store. This tests the building blocks but does not exercise any higher-level "apply import" function — if such a function exists (e.g., `apply_imported_collection`), it is not covered. The step's comment overstates coverage as "applying" the import.

**Suggested fix**: If a higher-level apply function exists in `crosshook-core`, call it. Otherwise, update the test comment to clarify: "simulate apply via individual building blocks (higher-level apply function TODO)."

---

### F017 — `console.error('BrowserDevPresetExplainerModal: onContinue failed', error)` ships in production bundle

- **Severity**: LOW
- **Category**: Security / Bundle Hygiene
- **File**: `src/crosshook-native/src/components/collections/BrowserDevPresetExplainerModal.tsx:115`
- **Status**: Open

**Description**: The `console.error` surfaces exception details including potential path strings to the DevTools console. In a Tauri production build DevTools is restricted, so real-world impact is minimal. Combined with F005 (this component ships in production without dead-code elimination), the log statement is dead weight.

**Suggested fix**: Remove or gate behind `__WEB_DEV_MODE__` — or, better, fix F005 first so the entire component is tree-shaken.

---

### F018 — `useFocusTrap` `restoreFocusOnClose` option is YAGNI (always `true`)

- **Severity**: LOW
- **Category**: Maintainability
- **File**: `src/crosshook-native/src/hooks/useFocusTrap.ts:31-34`
- **Status**: Open

**Description**: All four callers pass `restoreFocusOnClose: true` explicitly (default is also `true`). No caller ever passes `false`. The option is speculative configurability.

**Suggested fix**: Remove the option. Add it back when a caller actually needs `false`. Since the hook is internal (not exported from a package), the API cost of adding it later is negligible.

---

### F019 — Rust integration test synthetic path `/profiles/...` lacks inline documentation

- **Severity**: NIT
- **Category**: Maintainability
- **File**: `src/crosshook-native/crates/crosshook-core/tests/collections_jtbd_integration.rs:37, 205`
- **Status**: Open

**Description**: The path `/profiles/fixture-00.toml` does not exist on disk. Because `SyncSource::AppWrite` bypasses `fs::metadata`, the synthetic path is only stored as a string in the in-memory SQLite. Not a bug — intentional scaffolding. A one-line comment would make the intent explicit.

**Suggested fix**:

```rust
// Synthetic path — the AppWrite sync source does not call fs::metadata,
// so no real file is needed; the string is only persisted in metadata.
```

---

### F020 — `CollectionsSidebar` passes `onSubmitEdit={async () => false}` placeholder without explanation

- **Severity**: NIT
- **Category**: Maintainability
- **File**: `src/crosshook-native/src/components/collections/CollectionsSidebar.tsx:193`
- **Status**: Open

**Description**: `CollectionEditModal` accepts both `onSubmitCreate` and `onSubmitEdit`; the sidebar uses `mode="create"` exclusively, and the `onSubmitEdit` prop is satisfied with `async () => false`. Correct at runtime (the `mode` guards prevent the wrong callback from firing), but misleading to a reader.

**Suggested fix**: Add an inline comment: `// mode="create" — onSubmitEdit is never called here`.

---

## Findings Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH     | 2 |
| MEDIUM   | 10 |
| LOW      | 6 |
| NIT      | 2 |
| **Total**| **20** |

**Per-file heatmap**:

| File | Findings |
|------|---------:|
| `src/hooks/useFocusTrap.ts` | F001, F018 |
| `src/components/collections/CollectionAssignMenu.tsx` | F002, F006 |
| `src/components/collections/CollectionEditModal.tsx` | F009 |
| `src/components/collections/CollectionImportReviewModal.tsx` | F009, F013 |
| `src/components/collections/CollectionViewModal.tsx` | F013 |
| `src/components/collections/CollectionViewModal.css` | F011 |
| `src/components/collections/BrowserDevPresetExplainerModal.tsx` | F009, F017 |
| `src/components/collections/CollectionsSidebar.tsx` | F005, F020 |
| `src/components/library/LibraryCard.tsx` | F014 |
| `src/components/library/GameDetailsModal.tsx` | F010 |
| `src/styles/theme.css` | F004, F008, F012 |
| `src/styles/sidebar.css` | F007 |
| `tests/collections.spec.ts` | F003 |
| `crates/crosshook-core/tests/collections_jtbd_integration.rs` | F016, F019 |
| PR body (docs) | F015 |

---

## What's Good

- **JTBD integration test is substantive**: 50 profiles, 3 collections, defaults merge, export → re-import; meaningful assertions (not `.is_ok()` spam). 776 existing + 1 new test all green.
- **Semantic sidebar rewrite is correct**: `<nav>`/`<ul>`/`<li>` replacing `<div role="list">` + `<button role="listitem">` is the right fix for the Phase 2 regression.
- **`CollectionEditModal` body-lock regression is genuinely fixed** (via `useFocusTrap` adoption).
- **Shift+F10 / ContextMenu key path** on `LibraryCard` is a real Steam Deck / controller-accessibility improvement.
- **PR claim "no new IPC, no new persisted data, no new dependencies" is verified**: `src-tauri/` has zero diff; no `#[tauri::command]` added; no new `invoke()` sites; no new crate deps; no committed secrets.
- **TypeScript and Rust tests pass clean** — no lint/type regressions.
- **PR body is thorough** (Closes #181/#73, reviewer notes call out intentional decisions).

---

## Next Steps

1. Fix **F001** (one-line dep array change) — this is the blocker.
2. Fix **F009** (useCallback the inline `onClose` wrappers) — companion to F001.
3. Address **F002** (either migrate CollectionAssignMenu or document the divergence).
4. Correct the PR body (**F015**).
5. Fix **F003** (deterministic Playwright assertion).
6. Fix **F004** (`:focus-within` vs `:focus-visible`).
7. Address remaining MEDIUM findings (F005–F012) — most are small CSS/bundle changes.
8. LOW/NIT findings can follow in a polish PR if scope discipline matters.

---

## Decision

**REQUEST CHANGES** — the `useFocusTrap` dep-array bug (F001) produces an observable a11y regression in the feature's primary authoring flow and was independently identified by all three reviewers. The dual focus-trap implementation (F002) should at minimum be documented before merge. Remaining findings are polish; none block individually.
