# Plan: Profile Collections — Phase 5 (Polish, Integration Tests, Steam Deck Validation)

## Summary

Ship the **quality gate** for the profile-collections feature: end-to-end Rust + Playwright test
fixtures exercising the JTBD critical path, a shared `useFocusTrap` hook extracted from
`GameDetailsModal.tsx` and applied to all five collection modals, a full keyboard-navigation audit
(Esc/Enter/Tab/arrow keys), controller-reachable assign-menu invocation, semantic/ARIA fixes
(sidebar `<ul>`/`<li>`/`<button>`, `aria-describedby`, deterministic heading focus), focus-visible
CSS rules for sidebar and assign-menu surfaces, empty-state copy matching `CommunityBrowser` tone,
an overscroll-contain fix on the import-review modal, four new internal docs (merge layer, TOML
schema v1, browser mocks, Steam Deck checklist), manual Steam Deck / `gamescope` walkthrough, manual
regression sweep, the PRD row update, and the Phase 5 implementation report. **No new persisted
data; no new dependencies; no new IPC.** All changes are tests, refactors, polish, and
documentation.

## User Story

As a **power user with 50+ profiles on a Steam Deck**, I want **every collection interaction to be
reachable via D-pad, keyboard, and mouse with zero focus traps, zero missing ARIA semantics, and
zero empty-state placeholder copy** so that **the profile-collections feature feels ship-ready — not
a "works if you hold it right" prototype — and I can trust it with my real 80-profile library**.

## Problem → Solution

**Current state (after Phases 1-4 merged, commits `63d43e1`, `619e455`, `31e391d`, `f018e4a`)**:

- **No end-to-end tests** exercise the full JTBD path (create 3 collections → assign 10 profiles
  each → filter/launch → export → re-import). Only the 7 unit tests in
  `profile/collection_exchange.rs` and the 5 integration tests in `metadata/mod.rs` touch
  collections, and each exercises one concern in isolation.
- **No frontend tests** reference any collection component. The Playwright smoke harness
  (`tests/smoke.spec.ts`) only clicks each sidebar tab and asserts no console errors.
- **`useFocusTrap` is not extracted** — `GameDetailsModal.tsx:182-287` contains the reference
  focus-trap (Tab cycle, Esc close, `inert`/`aria-hidden` siblings, body-scroll lock, focus
  save/restore, deterministic heading focus). Four out of five collection modals reimplement this
  inline with copy-paste drift, and `CollectionEditModal.tsx:56-130` is missing body-lock and
  sibling `inert`, so background scroll and clicks still register while it is open.
- **`CollectionAssignMenu.tsx:62-171` has no keyboard navigation**: no Tab trap, no ArrowUp/Down
  roving focus, no focus-on-open, no focus restore, `role="menu"` but child checkboxes are not
  `role="menuitemcheckbox"`, and there is **no keyboard or controller path to invoke it** —
  `LibraryCard.tsx:77-84` only wires `onContextMenu` to a mouse right-click.
- **`CollectionsSidebar.tsx:127-144` uses `<div role="list">` containing
  `<button role="listitem">`** — the `role` override strips the implicit `button` role, so assistive
  technologies no longer announce collection entries as buttons, and the section label `<div>` is
  not a heading element. The sidebar has **no empty-state copy** when `collections.length === 0` —
  users see only the two CTAs.
- **No `:focus-visible` rules** exist for `.crosshook-collections-sidebar__item`,
  `.crosshook-collections-sidebar__cta`, or `.crosshook-collection-assign-menu__option` / `__create`
  (`theme.css:5850-5987`) — keyboard and controller users have no visible indicator on those
  surfaces. The `.crosshook-collection-import-review__body` lacks `overscroll-behavior: contain`
  even though its sibling `.crosshook-collection-modal__body` has it (`CollectionViewModal.css:1-6`
  vs `CollectionImportReviewModal.css:1-5`).
- **No internal docs exist** for the Phase 3 merge layer, the Phase 4 TOML schema v1, the
  browser-mock collection fixtures, or Steam Deck a11y validation. The directory
  `docs/internal-docs/` exists with only two unrelated files (`local-build-publish.md`,
  `dev-web-frontend-plan-validation.md`).
- **The PRD phase table (line 211)** still marks Phase 5 as `pending` with `PRP Plan = -`.
- **The Phase 2 plan is stranded** at
  `docs/prps/plans/profile-collections-phase-2-sidebar-view-modal.plan.md` — every other phase plan
  was moved to `archived/` or `plans/completed/` post-merge; Phase 2's never was.

**Desired state**: every collection interaction is reachable via D-pad, keyboard, and mouse; every
modal uses the same `useFocusTrap` hook; the sidebar announces itself correctly to screen readers
and has tonally-consistent empty-state copy; four internal docs describe the architecture, TOML
schema, browser mocks, and a Steam Deck validation checklist; a Rust integration test and a
Playwright spec exercise the full JTBD path; manual Steam Deck + regression validation is documented
in the Phase 5 report; the PRD is updated; the Phase 2 and Phase 5 plan files are in
`plans/completed/`.

## Metadata

- **Complexity**: **Medium-Large** (22 tasks across 4 batches; ~14 files UPDATE, 5 files CREATE, 1
  git-mv; no backend-breaking changes; all refactors preserve existing behavior)
- **Source PRD**: [`docs/prps/prds/profile-collections.prd.md`](../prds/profile-collections.prd.md)
  §Phase 5 (line 211, 274-284)
- **Source Issue**: [`yandy-r/crosshook#181`](https://github.com/yandy-r/crosshook/issues/181)
- **Parent epic**: [`yandy-r/crosshook#73`](https://github.com/yandy-r/crosshook/issues/73) —
  **Phase 5 merge closes #73**
- **Depends on**:
  - Phase 1 merged (`63d43e1`, schema v19 + 9 IPC commands + mocks)
  - Phase 2 merged (`619e455`, sidebar + view modal + hooks)
  - Phase 3 merged (`31e391d`, `effective_profile_with` + `collection_get/set_defaults` + schema
    v20)
  - Phase 4 merged (`f018e4a`, `*.crosshook-collection.toml` export/import + review modal)
- **Blocks**: closing epic #73 and the release that bundles the feature
- **Estimated files**: ~20 (14 UPDATE, 5 CREATE, 1 git-mv)
  - CREATE: `src/crosshook-native/src/hooks/useFocusTrap.ts`;
    `src/crosshook-native/crates/crosshook-core/tests/collections_jtbd_integration.rs`;
    `src/crosshook-native/tests/collections.spec.ts`;
    `docs/internal-docs/profile-collections-merge-layer.md`;
    `docs/internal-docs/profile-collections-toml-schema-v1.md`;
    `docs/internal-docs/profile-collections-browser-mocks.md`;
    `docs/internal-docs/steam-deck-validation-checklist.md`;
    `docs/prps/reports/profile-collections-phase-5-polish-tests-steam-deck.report.md`
  - UPDATE: `CollectionsSidebar.tsx`, `CollectionViewModal.tsx`, `CollectionEditModal.tsx`,
    `CollectionImportReviewModal.tsx`, `BrowserDevPresetExplainerModal.tsx`,
    `CollectionAssignMenu.tsx`, `CollectionLaunchDefaultsEditor.tsx`, `LibraryCard.tsx`,
    `LibraryPage.tsx`, `theme.css` (focus-visible rules), `CollectionImportReviewModal.css`
    (overscroll), `docs/prps/prds/profile-collections.prd.md` (phase row)
  - MOVE: `docs/prps/plans/profile-collections-phase-2-sidebar-view-modal.plan.md` →
    `docs/prps/plans/completed/`

## Storage / Persistence

**No new persisted data in Phase 5.** This is a test/polish/docs phase.

| Datum / behavior                                            | Classification     | Where it lives                                         | Migration / back-compat                                                                      |
| ----------------------------------------------------------- | ------------------ | ------------------------------------------------------ | -------------------------------------------------------------------------------------------- |
| Rust integration-test fixture (50 profiles + 3 collections) | **Runtime-only**   | `tempdir()` + `MetadataStore::open_in_memory()`        | Torn down per test run; no on-disk state                                                     |
| Playwright mock-store fixture                               | **Runtime-only**   | module-scoped `MockStore` in `handlers/collections.ts` | Resets only on full page reload; tests must clean up or use a fresh page per `test.describe` |
| Internal docs (`docs/internal-docs/*.md`)                   | **Source control** | Committed files                                        | N/A                                                                                          |
| PRD phase row update                                        | **Source control** | `docs/prps/prds/profile-collections.prd.md:211`        | N/A                                                                                          |
| Phase 2 and Phase 5 plan archival                           | **Source control** | `docs/prps/plans/completed/`                           | `git mv`, no content change                                                                  |

**Offline**: all Phase 5 deliverables are fully offline — Rust tests, Playwright smoke tests
(browser-dev mock mode), docs, manual walkthroughs.

**Degraded fallback**: the manual regression sweep must verify Phase 1's documented
`MetadataStore::disabled()` path still makes the sidebar hide cleanly, the dropdown filter become a
no-op, and no data loss occur. Phase 1's fallback tests (`metadata/mod.rs:1708-1736`) stay green.

**User visibility / editability**: Phase 5 adds **no new user surfaces**. It hardens existing
surfaces from Phases 2-4. The Steam Deck checklist is operator-facing (internal); the changelog
entry is user-facing via a `feat(ui): …` commit on the merge.

---

## UX / Touchpoint Changes

### 1. Shared focus trap (refactor, behavior-preserving)

All five collection modals + `GameDetailsModal` currently reimplement the same trap inline (Tab
cycle, Esc close, `inert`/`aria-hidden` siblings, body-scroll lock, focus save/restore). Extract
into `useFocusTrap({ open, panelRef, onClose, initialFocusRef, restoreFocusOnClose })` at
`src/hooks/useFocusTrap.ts`. Each consumer drops its inline handlers and uses the hook.
`CollectionEditModal` gains body-lock + sibling `inert` as a side-effect of adopting the hook. **No
visible behavior change** for the four modals that already had the full trap; `CollectionEditModal`
gains correct background inertness.

### 2. Collections sidebar semantic fix

```
Before                                      After
───────────────────────────────────────      ───────────────────────────────────────
<div class="…-section">                      <nav class="…-section" aria-label="Collections">
  <div class="…-section-label">                 <h2 class="…-section-label">Collections</h2>
    Collections                                 <ul class="…-collections-sidebar__list">
  </div>                                          <li>
  <div role="list">                                 <button type="button" …>…</button>
    <button role="listitem">…</button>            </li>
  </div>                                        </ul>
</div>                                        </nav>
```

Keyboard and screen-reader behavior: collections announce as "list of 3 buttons, Action/Adventure
button" instead of "list with 3 items". No CSS class rename — `.crosshook-collections-sidebar__item`
stays as-is so theme rules at `theme.css:5850-5898` continue to apply.

### 3. Sidebar empty state (new copy)

When `collections.length === 0`, render a short explanatory paragraph above the CTAs, tone-matched
to `CommunityBrowser.tsx:459-461`:

> "No collections yet. Create one or import a preset to group your profiles."

### 4. `CollectionAssignMenu` keyboard navigation rewrite

- Focus-on-open: first enabled checkbox (or "+ New collection…" when `collections.length === 0`).
- Roving tab index / arrow key navigation between checkboxes + "+ New collection…".
- Tab trap inside the popover; Shift-Tab wraps.
- Save invoking element (`document.activeElement` at open) and restore on close.
- `data-crosshook-focus-root="modal"` so `useGamepadNav` scopes D-pad input to the popover when open
  (see `useGamepadNav.ts:61, 94-100`).
- Replace `<input type="checkbox">` + `role="menu"` parent with `role="menu"` +
  `role="menuitemcheckbox"` children exposing `aria-checked` (or migrate to `role="dialog"` since
  the popover has form-like content — pick one and stay consistent).
- Preserve current visual styling and mouse click-to-toggle semantics.

### 5. `CollectionAssignMenu` keyboard/controller invocation path

`LibraryCard.tsx:77-84` currently reaches the assign menu only via mouse right-click
(`onContextMenu`). Add a second invocation path:

- Keyboard: `Shift+F10` or `ContextMenu` key when the card is focused synthesizes an anchor at the
  card's center (compute via `boundingClientRect`) and calls the same
  `onContextMenu(event, profile.name)` callback `LibraryPage.tsx:137-143` already accepts.
- Controller: `useGamepadNav` already walks focusable cards; a dedicated "Add to collection" button
  in the card's overflow area is the cleanest path — or, if no overflow area exists, the keyboard
  synthesis above fires from the same `onKeyDown` and `useGamepadNav` maps the B/Y button to
  `Shift+F10` synthetically via a `key` event dispatch.
- The `LibraryPage` handler signature does NOT change:
  `setAssignMenuState({ open, profileName, anchorPosition })` continues to take `{ x, y }`. Only the
  synthesis source moves from `clientX/clientY` (mouse) to card-center (keyboard).

### 6. Focus-visible CSS polish

Add `:focus-visible` rules to:

- `.crosshook-collections-sidebar__item` + `.crosshook-collections-sidebar__cta`
  (`theme.css:5850-5898`)
- `.crosshook-collection-assign-menu__option` + `.crosshook-collection-assign-menu__create`
  (`theme.css:5956-5987`)

Reuse the existing hard-coded ring from `focus.css:32-35`
(`box-shadow: 0 0 0 2px rgba(255,255,255,0.06), 0 0 0 4px var(--crosshook-color-accent-soft)`) — do
not introduce a new CSS variable.

### 7. `CollectionImportReviewModal` polish

- Wrap the dialog body `<div>` at `:262-371` in a `<form onSubmit={handleConfirm}>` and change the
  Confirm button to `type="submit"` — Enter on the name input now submits.
- Add `overscroll-behavior: contain` to `.crosshook-collection-import-review__body`
  (`CollectionImportReviewModal.css:1-5`).

### 8. `CollectionLaunchDefaultsEditor` a11y polish

- Include the env var name in the remove-row button's `aria-label` (currently `"Remove env var row"`
  → `"Remove env var {name}"`).
- The "Active" badge on the `<summary>` element needs an `aria-label` on the summary announcing the
  active state, OR an `aria-live="polite"` region updated after save.

---

## Patterns to Mirror

| Pattern                                                                   | Source                                                                                                                                                                 | Use it for                                                                                                                               |
| ------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Focus trap, body lock, inert siblings, focus save/restore                 | `src/components/library/GameDetailsModal.tsx:182-287`                                                                                                                  | Extraction source for `useFocusTrap` hook; `previouslyFocusedRef` pattern at `:208, 247-250`; heading-focus at `:223-232`                |
| Integration test skeleton (public API, tempdir + open_in_memory)          | `crates/crosshook-core/tests/config_history_integration.rs`                                                                                                            | New `collections_jtbd_integration.rs` mirrors the top-level structure: no `#[cfg(test)] mod tests`, uses only public API, helpers at top |
| Rust fixture builder (minimal `GameProfile::default()` + field mutation)  | `profile/collection_exchange.rs:435-440` (`sample_profile_named`) + `:442-458` (`register_profile`)                                                                    | 50-profile seed loop in the new integration test                                                                                         |
| Roundtrip export → preview → re-seed assertion                            | `profile/collection_exchange.rs:492-537`                                                                                                                               | Second half of the JTBD test                                                                                                             |
| Playwright smoke test (attachConsoleCapture, fixture=populated)           | `tests/smoke.spec.ts:54-106`                                                                                                                                           | New `collections.spec.ts` — mirror `attachConsoleCapture`, `?fixture=populated`, `zero console.error` assertion                          |
| Empty-state copy tone                                                     | `src/components/CommunityBrowser.tsx:459-461`                                                                                                                          | Sidebar empty state and (optionally) review modal descriptor — verb-led, action-suggesting                                               |
| Internal doc house style (H1, intro, H2 subsections, tables, code blocks) | `docs/internal-docs/local-build-publish.md`                                                                                                                            | Four new docs under `docs/internal-docs/`                                                                                                |
| Phase report skeleton                                                     | `docs/prps/reports/profile-collections-phase-3-launch-defaults.report.md` + `phase-1-backend-foundation.report.md`                                                     | Phase 5 report at `docs/prps/reports/profile-collections-phase-5-polish-tests-steam-deck.report.md`                                      |
| Conventional-commit title shape                                           | Phase 2 commit `619e455` (`feat(ui): profile collections sidebar, view modal, shared state`), Phase 3 `31e391d` (`feat(ui): per-collection launch defaults — Phase 3`) | Final Phase 5 squash commit: `feat(ui): profile collections polish, integration tests, Steam Deck validation — Phase 5`                  |

---

## References (file:line)

### Code under audit

- `src/crosshook-native/src/components/collections/CollectionsSidebar.tsx:124-222`
- `src/crosshook-native/src/components/collections/CollectionViewModal.tsx:174-398`
- `src/crosshook-native/src/components/collections/CollectionEditModal.tsx:56-260`
- `src/crosshook-native/src/components/collections/CollectionImportReviewModal.tsx:74-371`
- `src/crosshook-native/src/components/collections/BrowserDevPresetExplainerModal.tsx:88-260`
- `src/crosshook-native/src/components/collections/CollectionAssignMenu.tsx:15-172`
- `src/crosshook-native/src/components/collections/CollectionLaunchDefaultsEditor.tsx:180-310`
- `src/crosshook-native/src/components/library/LibraryCard.tsx:77-84`
- `src/crosshook-native/src/components/pages/LibraryPage.tsx:137-143`

### Patterns to reference

- `src/crosshook-native/src/components/library/GameDetailsModal.tsx:182-287` (focus trap precedent)
- `src/crosshook-native/src/lib/focus-utils.ts:1-16` (`getFocusableElements` helper already
  exported)
- `src/crosshook-native/src/styles/focus.css:7-43` (`crosshook-focus-scope` mixin; hard-coded ring)
- `src/crosshook-native/src/styles/theme.css:5850-5987` (sidebar + assign menu rules — add
  `:focus-visible`)
- `src/crosshook-native/src/hooks/useScrollEnhance.ts:9` (`SCROLLABLE` selector — confirmed no gaps
  today)
- `src/crosshook-native/src/hooks/useGamepadNav.ts:48-100` (`data-crosshook-focus-root` discovery)
- `src/crosshook-native/src/components/CommunityBrowser.tsx:459-461` (empty-state tone)

### Test scaffolding

- `src/crosshook-native/crates/crosshook-core/tests/config_history_integration.rs` (whole file;
  integration test skeleton)
- `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange.rs:430-647` (fixture
  helpers + roundtrip test)
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:1511-1554` (alternative
  `sample_profile` — richer fixture)
- `src/crosshook-native/tests/smoke.spec.ts:1-107` (Playwright smoke harness)
- `src/crosshook-native/playwright.config.ts` (`fullyParallel: false`, `workers: 1`, loopback only)

### Docs conventions

- `docs/internal-docs/local-build-publish.md` (house style reference)
- `.git-cliff.toml:45-48` (`docs(internal):` is `skip = true`)
- `scripts/validate-release-notes.sh:62-69` (allowed changelog section headings)
- `scripts/prepare-release.sh:110-155` (release flow; Phase 5 does NOT run this — release is
  downstream)
- `.github/workflows/release.yml:105-120` (`verify:no-mocks` sentinel)

### PRD + parent issue

- `docs/prps/prds/profile-collections.prd.md:211` (row to flip to `complete`)
- `docs/prps/prds/profile-collections.prd.md:274-284` (Phase 5 scope)
- Issue #181 body (acceptance criteria transcribed below)
- Issue #73 (parent epic — closes on merge)

### Sibling artifacts

- `docs/prps/plans/completed/profile-collections-phase-4-toml-export-import-preset.plan.md`
  (archival precedent path)
- `docs/prps/archived/profile-collections-phase-1-backend-foundation.plan.md` (older archival
  precedent)
- `docs/prps/plans/profile-collections-phase-2-sidebar-view-modal.plan.md` (stranded plan — Phase 5
  also moves it)

---

## Gotchas / Risks

| #   | Risk                                                                                                                          | Likelihood | Mitigation                                                                                                                                                                                                                                                                                                                                                                                                                 |
| --- | ----------------------------------------------------------------------------------------------------------------------------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Extracting `useFocusTrap` from `GameDetailsModal` risks breaking it                                                           | **High**   | Extract with **no behavior change** (1:1 port of existing logic into a hook), then adopt in each modal one-by-one. Add a Playwright smoke assertion for the library route's modal open/close cycle as a safety net. `CollectionEditModal` is the ONLY modal gaining new behavior (body-lock + inert) — call it out in the task description.                                                                                |
| 2   | `CollectionsSidebar` semantic change (`<div>` → `<ul>`/`<li>`) could break CSS selectors                                      | **Medium** | The only class changes are structural (add `<ul>`/`<li>` wrappers); the existing `.crosshook-collections-sidebar__item` class moves from `<button role="listitem">` to `<button>`. Audit `theme.css:5820-5898` for `.crosshook-collections-sidebar__list > button` style selectors that relied on `button` being a direct child, and update to `> li > button` if needed.                                                  |
| 3   | `CollectionAssignMenu` rewrite is the most complex single task                                                                | **Medium** | Split into two phases in the task itself: (a) add focus management + ArrowUp/Down without changing roles, (b) swap to `menuitemcheckbox` semantics. Validate each with manual keyboard walkthrough before moving on.                                                                                                                                                                                                       |
| 4   | Keyboard invocation path for assign menu changes `LibraryCard` → `LibraryPage` event flow                                     | **Medium** | Don't change the `LibraryPage` handler signature. Synthesize `{ x, y }` from `boundingClientRect()` and call the same `onContextMenu(event, profile.name)` callback. Preserve the mouse path untouched.                                                                                                                                                                                                                    |
| 5   | Rust 50-profile seed loop could slow the test suite                                                                           | **Low**    | Use `GameProfile::default()` + only mutate `game.name` and `steam.app_id`; each profile is ~1 KB TOML. 50 profiles = ~50 ms write latency against `tempdir()`. Acceptable. Keep the test marked `#[test]` (not `#[ignore]`).                                                                                                                                                                                               |
| 6   | Playwright test cross-contamination via module-scoped `MockStore`                                                             | **Medium** | `playwright.config.ts` already pins `fullyParallel: false`, `workers: 1`. Use `test.beforeEach(async ({ page }) => page.goto('/?fixture=populated'))` to reset via full reload (module state re-initializes on page load). Add a comment linking to this gotcha.                                                                                                                                                           |
| 7   | `docs/internal-docs/` vs `docs/internal/` wording inconsistency                                                               | **Low**    | CLAUDE.md says `./docs/internal` for the `docs(internal):` commit-prefix rule, but the on-disk directory is `docs/internal-docs/`. The cliff rule at `.git-cliff.toml:45` matches the **commit prefix** `docs(internal):`, not the path — so any commit touching `docs/internal-docs/` with that prefix is correctly skipped from the changelog. Phase 5 uses `docs/internal-docs/` and `docs(internal):` commit prefixes. |
| 8   | `CollectionEditModal` gaining body-lock changes background scroll behavior mid-modal                                          | **Low**    | This is the **intended fix** per the a11y audit. Flag in the commit body. The user-visible effect is that the background page no longer scrolls while the edit modal is open — matches every other modal in the repo.                                                                                                                                                                                                      |
| 9   | `ProfilesPage` editor-safety invariant: `useProfile.loadProfile(name, { collectionId })` MUST NOT be called from ProfilesPage | **Medium** | Regression-sweep step must explicitly verify that `ProfilesPage.tsx:669` still calls `selectProfile(name)` with **no** `collectionId`, matching the editor-safety note at `useProfile.ts:68-74`. Phase 3 guarded this — Phase 5 just re-verifies it is still held.                                                                                                                                                         |
| 10  | Phase 2 plan file is stranded in `plans/` (not archived on merge)                                                             | **Low**    | Phase 5 does the `git mv` as part of T7. Include in the `docs(internal):` commit alongside the Phase 5 plan archival.                                                                                                                                                                                                                                                                                                      |
| 11  | Manual Steam Deck validation requires physical hardware access                                                                | **Medium** | The Phase 5 report should document the test environment. If hardware is unavailable, substitute `gamescope` session on desktop per PRD Phase 5 scope (`profile-collections.prd.md:281`). Flag in task T19 that either environment is acceptable, with hardware preferred.                                                                                                                                                  |
| 12  | The Playwright smoke test cannot exercise real file I/O for TOML export/import (mock layer returns a struct; no file write)   | **Low**    | The Rust integration test covers real-file roundtrip. The Playwright test exercises the UI flow + `BrowserDevPresetExplainerModal` path. Call this out in `docs/internal-docs/profile-collections-browser-mocks.md`.                                                                                                                                                                                                       |
| 13  | `role="listitem"` stripping button semantics on `CollectionsSidebar` is a regression introduced in Phase 2                    | **Low**    | Phase 2 shipped with this bug. Phase 5 fixes it. Flag in the commit body as "fixes regression from Phase 2 (PR #183)" so the reviewer can trace the history.                                                                                                                                                                                                                                                               |

---

## Acceptance Criteria (1:1 from issue #181)

| AC                                                                                                                               | Satisfied by tasks                                                         |
| -------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| End-to-end test fixtures exercise the JTBD critical path (create → assign → filter → launch)                                     | T1 (Rust integration), T18 (Playwright smoke)                              |
| Roundtrip test: export → reset → import → identical state                                                                        | T1 (second half of the Rust integration test)                              |
| Manual Steam Deck / `gamescope` validation completed and documented                                                              | T11 (checklist doc), T19 (execution), T22 (report)                         |
| Keyboard navigation audit (Tab order, Esc, Enter, focus return) — zero issues                                                    | T2, T12, T13, T14, T15, T16, T17, T19                                      |
| Empty-state copy reviewed and matches repo tone                                                                                  | T3                                                                         |
| ARIA + focus-visible accessibility pass                                                                                          | T3, T4, T6, T12, T13, T14, T15, T16                                        |
| `docs/internal-docs/` updated with architecture notes, TOML schema, Steam Deck checklist                                         | T8, T9, T10, T11                                                           |
| User-facing changelog entry (`feat(ui):`) following Conventional Commits per CLAUDE.md — title is release-note-ready             | Final merge commit (not a plan task; tone set above in Patterns to Mirror) |
| Regression sweep: Library page, Active-Profile dropdown, `GameDetailsModal`, `CommunityImportWizardModal`, launch flows all pass | T20                                                                        |
| All acceptance criteria from parent issue #73 are closed                                                                         | Merge commit references `Closes #73`                                       |

---

## Test Plan

### Automated — Rust integration (T1)

New file: `src/crosshook-native/crates/crosshook-core/tests/collections_jtbd_integration.rs`. Single
`#[test]` function exercising the full JTBD path:

1. **Setup**: `tempdir()` → `ProfileStore::with_base_path(tempdir/profiles)` +
   `MetadataStore::open_in_memory()`. Helpers `register_profile` + `sample_profile_named` lifted
   from `profile/collection_exchange.rs:435-458`.
2. **Seed 50 profiles**: loop `i in 0..50`,
   `sample_profile_named(&format!("fixture-{:02}", i), &(1_000_000 + i).to_string())`. Give each a
   distinct `trainer.community_trainer_sha256` (`format!("{:064x}", i)`) so multi-field matching has
   disambiguating data.
3. **Create 3 collections**: `metadata.create_collection("Action")`, `"Stable"`, `"WIP"`.
4. **Assign 10 profiles to each collection** via `metadata.add_profile_to_collection(&cid, &name)` —
   30 memberships total. Some profiles intentionally belong to multiple collections to exercise
   multi-membership.
5. **Filter assertion**: `metadata.list_profiles_in_collection("action_cid")` returns exactly 10
   names; `metadata.collections_for_profile("fixture-05")` returns all collections that contain it.
6. **Per-collection launch defaults**:
   `metadata.set_collection_defaults(&action_cid, Some(&CollectionDefaultsSection { custom_env_vars: [("DXVK_HUD", "fps")].into(), ..Default::default() }))`.
7. **Effective profile with collection context**: load `fixture-05`, call
   `profile.effective_profile_with(Some(&defaults))`, assert the merged `launch.custom_env_vars`
   contains `DXVK_HUD=fps` and all pre-existing profile env vars still present.
8. **Effective profile WITHOUT collection context**: call `profile.effective_profile_with(None)`,
   assert the same profile does NOT receive `DXVK_HUD`.
9. **Export**:
   `export_collection_preset_to_toml(&metadata, &store, &action_cid, &tempdir.path().join("action.crosshook-collection.toml"))`.
   Assert file exists and parses as valid TOML with `schema_version = "1"`.
10. **Reset**: drop the current `MetadataStore`, create a **new** `MetadataStore::open_in_memory()`
    (profiles stay on disk via the same `ProfileStore`).
11. **Re-import**: `preview_collection_preset_import(&store, &exported_path)`. Assert
    `preview.matched.len() == 10`, `ambiguous.is_empty()`, `unmatched.is_empty()`,
    `preview.manifest.name == "Action"`,
    `preview.manifest.defaults.custom_env_vars["DXVK_HUD"] == "fps"`.
12. **Apply**: walk `preview.matched`, re-create the collection on the new store
    (`create_collection` → `add_profile_to_collection` for each) + `set_collection_defaults`. Assert
    the resulting `list_profiles_in_collection` + `get_collection_defaults` equal the originals.

Test must pass under
`cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --test collections_jtbd_integration`.

### Automated — Playwright smoke (T18)

New file: `src/crosshook-native/tests/collections.spec.ts`. Mirror `smoke.spec.ts` skeleton
(`attachConsoleCapture`, `?fixture=populated`, zero-error assertion). Test steps:

1. `page.goto('/?fixture=populated')` and wait for dev chip.
2. Click sidebar "Library" tab.
3. Right-click the first library card → assign menu opens.
4. Press `ArrowDown` twice → focus moves between checkboxes (use `expect(page.locator(':focus'))`).
5. Press `Escape` → assign menu closes → focus restored to the library card (use
   `document.activeElement` check).
6. Create collection flow: click "New Collection" CTA in sidebar → `CollectionEditModal` opens →
   type name → press Enter → modal closes → new collection appears in sidebar.
7. Click the new collection entry → `CollectionViewModal` opens.
8. Press Escape → modal closes.
9. Click "Import Preset" CTA → `BrowserDevPresetExplainerModal` opens (browser dev mode path) →
   click Continue → `CollectionImportReviewModal` opens → press Escape → closes.
10. Assert no console errors throughout.

Test must pass under `npm run test:smoke -- tests/collections.spec.ts`. Use `test.beforeEach` to
re-`page.goto` for a clean `MockStore` per test case.

### Manual — Steam Deck / gamescope walkthrough (T19)

Operator follows `docs/internal-docs/steam-deck-validation-checklist.md`. Checklist items (also
embedded in T11):

- Sidebar Collections section is reachable via D-pad up/down
- A button opens `CollectionViewModal`; B button (or equivalent) closes it
- Right-click / keyboard menu-key path reaches `CollectionAssignMenu` on a library card
- ArrowUp/Down inside assign menu walks checkboxes
- Space on a focused checkbox toggles membership
- Escape / B button closes assign menu and restores focus to the invoking card
- No action in the full JTBD flow requires mouse or touchpad
- D-pad inside `CollectionViewModal` body walks library cards without scroll-jank (WebKitGTK scroll
  enhance verification)
- Right panel scroll inside `CollectionLaunchDefaultsEditor` works via D-pad
- Collection import review modal is reachable and navigable via D-pad

Results documented in the Phase 5 report (T22) as a table: `| Check | Result | Notes |`.

### Manual — Regression sweep (T20)

Operator walks through:

1. **Library page**: grid renders, right-click assign menu still works via mouse, no visual
   regressions on card hover/focus.
2. **Active-Profile dropdown — LaunchPage**: with `activeCollectionId === undefined` → dropdown
   shows all profiles; with `activeCollectionId` set → dropdown filters to collection members only.
   Changing the dropdown selection calls `loadProfile(name, { collectionId })` — verify via
   `profile_load` mock log in browser dev mode.
3. **Active-Profile dropdown — ProfilesPage (editor-safety invariant)**: changing the dropdown
   selection calls `loadProfile(name)` WITHOUT `collectionId`. Verify by opening the profile editor
   and inspecting the payload in the browser dev mode console.
4. **GameDetailsModal**: still opens from library card click, focus-traps correctly (regression
   check for T2's `useFocusTrap` refactor).
5. **CommunityImportWizardModal**: unchanged — open it via the Community page → Import button and
   verify it still works.
6. **Launch flow with collection defaults**: select a collection in the sidebar → open
   `CollectionViewModal` → click "Edit defaults" → set `DXVK_HUD=fps` → save → launch a member
   profile from the Launch page with `activeCollectionId` set. Verify the launched game's
   environment contains `DXVK_HUD=fps` via `printenv` test (per Phase 3 report lines 209-232).
7. **Launch flow WITHOUT collection defaults**: launch the same profile with
   `activeCollectionId === undefined`. Verify `DXVK_HUD` is NOT set.
8. **`MetadataStore::disabled()` fallback**: simulate via the browser dev mock `?errors=true` flag →
   sidebar Collections section should hide cleanly, Active-Profile dropdown filter becomes a no-op,
   no data loss.

Results documented in the Phase 5 report (T22) as a table.

### Automated — Validation commands

```bash
# Rust (primary gate per CLAUDE.md)
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core

# Rust (scoped to Phase 5 integration)
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --test collections_jtbd_integration

# Frontend typecheck + build
cd src/crosshook-native && npm run build

# Playwright smoke (all routes)
cd src/crosshook-native && npm run test:smoke

# Playwright smoke (Phase 5 collections spec only)
cd src/crosshook-native && npm run test:smoke -- tests/collections.spec.ts

# Mock coverage drift (advisory)
cd src/crosshook-native && npm run dev:browser:check

# Full native build (optional — exercises verify:no-mocks sentinel locally)
./scripts/build-native.sh
```

---

## Tasks

Each task is self-contained with file paths, dependencies, and explicit acceptance. Tasks annotated
`Depends on [...]` form a DAG for parallel execution.

### Batch A — independent / fan-out (all parallel-safe)

**T1 — Rust JTBD integration test** _Depends on []._ Create
`src/crosshook-native/crates/crosshook-core/tests/collections_jtbd_integration.rs`. Mirror the
skeleton of `tests/config_history_integration.rs` (helpers at top, single `#[test]` function, only
public API). Implement the 12-step JTBD path documented in the Test Plan above. Run
`cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --test collections_jtbd_integration`
and confirm pass. Acceptance: single green test asserting create → seed 50 profiles → 3 collections
→ 30 memberships → defaults merge (with/without context) → export → fresh-store re-import →
identical state.

**T2 — Extract `useFocusTrap` hook** _Depends on []._ Create
`src/crosshook-native/src/hooks/useFocusTrap.ts` with signature
`useFocusTrap({ open, panelRef, onClose, initialFocusRef?, restoreFocusOnClose?: boolean })`. **1:1
port** of the logic at `GameDetailsModal.tsx:182-287`: Tab cycle via `getFocusableElements` from
`lib/focus-utils.ts:12`, Esc handler, body-scroll lock, sibling `inert`/`aria-hidden`, focus save
(`previouslyFocusedRef`) + restore on close, deterministic heading focus via `initialFocusRef`. No
behavior change; no consumer changes in this task (consumers adopted in Batch B). Unit test is N/A
(no test framework) — JSDoc the API thoroughly so Batch B tasks can consume it without reading the
source. Acceptance: hook file compiles under `npm run build` with no new TS errors; JSDoc covers all
parameters.

**T3 — `CollectionsSidebar` semantic + empty-state fix** _Depends on []._ Update
`src/crosshook-native/src/components/collections/CollectionsSidebar.tsx:124-172`:

- Replace outer `<div className="crosshook-sidebar__section crosshook-collections-sidebar">` with
  `<nav aria-label="Collections">`.
- Replace `<div className="crosshook-sidebar__section-label">Collections</div>` with
  `<h2 className="crosshook-sidebar__section-label">Collections</h2>`.
- Replace `<div role="list">` + `<button role="listitem">` with `<ul>` + `<li>` + `<button>` (no
  role override).
- When `collections.length === 0`, render
  `<p className="crosshook-collections-sidebar__empty-copy">No collections yet. Create one or import a preset to group your profiles.</p>`
  above the two CTAs.
- Audit `theme.css:5820-5898` for `.crosshook-collections-sidebar__list > button` direct-child
  selectors; if present, update to `> li > button`. Add a rule for
  `.crosshook-collections-sidebar__empty-copy` (muted color, smaller font, top/bottom margin
  matching existing CTA spacing). Acceptance: sidebar renders identically in mouse walkthrough;
  screen reader announces "Collections, nav, list of 3 buttons" (verify manually in T19); build
  passes.

**T4 — Focus-visible CSS rules** _Depends on []._ Update `src/crosshook-native/src/styles/theme.css`
to add `:focus-visible` rules for: `.crosshook-collections-sidebar__item`,
`.crosshook-collections-sidebar__cta`, `.crosshook-collection-assign-menu__option`,
`.crosshook-collection-assign-menu__create`. Reuse the existing ring
`box-shadow: 0 0 0 2px rgba(255,255,255,0.06), 0 0 0 4px var(--crosshook-color-accent-soft)` from
`focus.css:32-35` — do NOT introduce a new CSS variable. Acceptance: keyboard Tab through sidebar
and assign-menu surfaces shows a visible ring; build passes.

**T5 — `overscroll-behavior: contain` on import review modal** _Depends on []._ Update
`src/crosshook-native/src/components/collections/CollectionImportReviewModal.css:1-5`
(`.crosshook-collection-import-review__body`): add `overscroll-behavior: contain;`. Match
`CollectionViewModal.css:1-6`. Acceptance: build passes; manual scroll on the import-review body
does not bounce out to the page scroll.

**T6 — `CollectionLaunchDefaultsEditor` a11y polish** _Depends on []._ Update
`src/crosshook-native/src/components/collections/CollectionLaunchDefaultsEditor.tsx`:

- Change the remove-row button's `aria-label` from `"Remove env var row"` to
  `"Remove env var {name}"` (interpolate the row's key). File around line 288.
- Add an `aria-label` to the summary element or an `aria-live="polite"` announcement when the
  "Active" badge (`:191-193`) toggles after save. Acceptance: build passes; screen-reader
  announcement of the active state is verifiable in T19.

**T7 — Move stranded Phase 2 plan** _Depends on []._
`git mv docs/prps/plans/profile-collections-phase-2-sidebar-view-modal.plan.md docs/prps/plans/completed/`.
Acceptance: file is under `plans/completed/`; `git status` shows a rename; include this in the
`docs(internal):` commit alongside the Phase 5 plan archival later.

**T8 — Write `docs/internal-docs/profile-collections-merge-layer.md`** _Depends on []._ New file
documenting the Phase 3 merge layer: (1) `CollectionDefaultsSection` fields and semantics, (2) the
`effective_profile_with(Option<&CollectionDefaultsSection>)` 3-layer precedence
(`base → collection defaults → local_override`), (3) editor-safety invariant on
`useProfile.loadProfile.collectionId` (ProfilesPage must NOT pass `collectionId`), (4) the 4
merge-layer tests at `profile/models.rs:1288-1380`, (5) schema v20 migration note, (6) the
`profile_load` extended IPC signature with `collection_id: Option<String>`. Use
`local-build-publish.md` house style (H1 title, intro paragraph, H2 subsections, fenced code blocks,
tables). Cite file:line for every claim. Acceptance: file exists; renders in markdown preview; all
cited line numbers match current HEAD.

**T9 — Write `docs/internal-docs/profile-collections-toml-schema-v1.md`** _Depends on []._ New file
documenting the Phase 4 `*.crosshook-collection.toml` schema v1: every top-level field, required vs
optional, `schema_version = "1"` forward-compat rule, multi-field profile descriptor matching order
(`steam_app_id` via `resolve_art_app_id()` semantics →
`(game_name, trainer_community_trainer_sha256)` pair fallback → user-disambiguation review modal),
schema rejection behavior for future versions, roundtrip contract tested at
`profile/collection_exchange.rs:492-537`. Include a minimal example TOML block and a commented
example. Cite the schema source files (`profile/collection_schema.rs`,
`profile/collection_exchange.rs`). Acceptance: file exists; example TOML is valid under
`toml::from_str` (operator verifies manually by pasting into a throwaway Rust file); build of Rust
tests still passes.

**T10 — Write `docs/internal-docs/profile-collections-browser-mocks.md`** _Depends on []._ New file
documenting the browser dev-mode mock strategy for collections: (1) where collection mocks live
(`src/lib/mocks/handlers/collections.ts`, `profile.ts`), (2) `wrapHandler.ts`
`EXPLICIT_READ_COMMANDS` vs `READ_VERB_RE` classification, (3) the `[dev-mock]` / `getMockRegistry`
/ `registerMocks` / `MOCK MODE` string sentinel enforced at `.github/workflows/release.yml:105-120`,
(4) the `check-mock-coverage.sh` drift report (advisory only, always `exit 0`), (5) the
`BrowserDevPresetExplainerModal` pattern for surfacing file-picker-dependent flows in browser mode
where `chooseSaveFile`/`chooseFile` cannot touch disk, (6) the module-scoped `MockStore` singleton
gotcha (no reset between tests). Acceptance: file exists; all cited line numbers and filenames match
current HEAD.

**T11 — Write `docs/internal-docs/steam-deck-validation-checklist.md`** _Depends on []._ New file:
Steam Deck / `gamescope` validation checklist. Contents: (1) environment setup (hardware or
gamescope on desktop), (2) full JTBD-path controller walkthrough, (3) assign-menu controller flow,
(4) collection-view-modal controller flow, (5) per-collection defaults editor controller flow, (6)
TOML import/export controller flow, (7) regression checks for library page, launch dropdown, game
details modal, (8) expected `printenv` assertion for per-collection env var merge (referenced from
Phase 3 report lines 209-232), (9) pass/fail criteria for each check. Use a markdown table format
per check: `| # | Check | Pass/Fail | Notes |`. Cite `useScrollEnhance.ts:9` `SCROLLABLE` selector
as a gotcha to verify. Acceptance: file exists; T19 uses it as the operator script.

### Batch B — a11y refactors (depend on T2 for `useFocusTrap`; T17 depends on T16)

**T12 — Adopt `useFocusTrap` in `CollectionViewModal`** _Depends on [T2]._ Update
`src/crosshook-native/src/components/collections/CollectionViewModal.tsx:174-288`. Remove inline
trap logic (Tab cycle, Esc handler, body lock, sibling inert, focus save/restore). Replace with
`useFocusTrap({ open: props.open, panelRef, onClose, initialFocusRef: headingRef, restoreFocusOnClose: true })`.
Keep the heading `tabIndex={-1}` for `initialFocusRef` compatibility. No user-visible behavior
change. Acceptance: build passes; manual open/close preserves Esc/Tab/focus-return behavior; no
regressions in T18 smoke test.

**T13 — `CollectionEditModal` a11y upgrade** _Depends on [T2]._ Update
`src/crosshook-native/src/components/collections/CollectionEditModal.tsx:56-260`:

- Adopt `useFocusTrap` (gains body-lock + sibling inert as a side-effect — previously missing).
- Add a heading `<h2>` with `ref={headingRef}` and `tabIndex={-1}`; pass
  `initialFocusRef: headingRef` to `useFocusTrap`.
- Add `aria-describedby={descriptionId}` to the dialog element, pointing at the form description
  paragraph.
- Preserve existing form `onSubmit` behavior (Enter already submits). Acceptance: opening the edit
  modal now locks background scroll and makes siblings inert (was previously scrollable/clickable);
  screen reader announces title first; build passes.

**T14 — `CollectionImportReviewModal` Enter-submit + focus trap** _Depends on [T2]._ Update
`src/crosshook-native/src/components/collections/CollectionImportReviewModal.tsx:74-371`:

- Adopt `useFocusTrap` (replaces inline logic at `:164-201`).
- Wrap the dialog body region at `:262-371` in a `<form onSubmit={handleConfirm}>`.
- Change the Confirm button to `type="submit"` so Enter on any input (including the name input)
  submits.
- Guard `handleConfirm` against double-submit while `applying === true` (already done — verify it
  survives the refactor). Acceptance: build passes; Enter on the name input now triggers Confirm
  when not already applying; Esc still closes; T18 smoke test covers the Escape path.

**T15 — `BrowserDevPresetExplainerModal` a11y upgrade** _Depends on [T2]._ Update
`src/crosshook-native/src/components/collections/BrowserDevPresetExplainerModal.tsx:88-260`:

- Adopt `useFocusTrap`.
- Add heading ref + deterministic initial focus (replace current first-focusable behavior at
  `:126-135`).
- Add `aria-describedby` pointing at the body paragraph. Acceptance: build passes; manual
  walkthrough shows heading announced first.

**T16 — `CollectionAssignMenu` keyboard navigation rewrite** _Depends on [T2]._ Update
`src/crosshook-native/src/components/collections/CollectionAssignMenu.tsx:15-172`:

- Adopt `useFocusTrap` for Tab trap + focus save/restore + Esc close (replace inline logic at
  `:62-83`).
- On open: focus first enabled checkbox (or "+ New collection…" when `collections.length === 0`) via
  `initialFocusRef`.
- Add ArrowUp/Down handlers inside `onKeyDown` to rove focus between checkboxes + "+ New
  collection…". Use `getFocusableElements(popoverRef.current)`.
- Add `data-crosshook-focus-root="modal"` to the popover root so `useGamepadNav` scopes D-pad input
  to the popover.
- Replace `role="menu"` + `<input type="checkbox">` children with `role="menu"` +
  `role="menuitemcheckbox"` exposing `aria-checked`. Keep the visible checkbox input for mouse
  click, but add `aria-checked` via `aria-labelledby` or similar. Alternative: swap `role="menu"`
  for `role="dialog"` since the popover has form-like content — pick one approach and document the
  choice in the commit body.
- Preserve existing pointerdown-outside close and visible styling. Acceptance: build passes; manual
  keyboard walkthrough (Tab in, ArrowUp/Down, Space to toggle, Esc out, focus restored) works; T18
  smoke test covers the arrow-key path.

**T17 — Keyboard invocation path for assign menu** _Depends on [T16]._ Update
`src/crosshook-native/src/components/library/LibraryCard.tsx:70-151` and
`src/crosshook-native/src/components/pages/LibraryPage.tsx:137-143`:

- In `LibraryCard`, add an `onKeyDown` handler: when `Shift+F10` or `ContextMenu` key fires, compute
  an anchor at the card's `boundingClientRect()` center, call
  `onContextMenu(syntheticEvent, profile.name)` with a synthesized event carrying
  `clientX`/`clientY` fields (a plain object cast to the event type is acceptable).
- In `LibraryPage`, confirm the handler signature tolerates a synthetic event (it only reads
  `event.clientX`/`event.clientY`, so a partial event is safe).
- Document the keyboard shortcut in the Steam Deck checklist (T11). Acceptance: keyboard walkthrough
  — focus a library card, press Shift+F10, assign menu opens at card center; build passes; no
  regression in mouse right-click path.

### Batch C — Playwright test (depends on Batch B + T3 stability)

**T18 — Playwright collections smoke test** _Depends on [T3, T12, T13, T14, T15, T16, T17]._ Create
`src/crosshook-native/tests/collections.spec.ts`. Mirror `smoke.spec.ts` skeleton
(`attachConsoleCapture`, `?fixture=populated`, zero-error assertion). Implement the 10-step flow
documented in the Test Plan above. Use `test.beforeEach` to `page.goto('/?fixture=populated')` for a
clean `MockStore`. Cite the module-scoped mock gotcha in a file-top comment. Run
`npm run test:smoke -- tests/collections.spec.ts` and confirm pass. Acceptance: test file green; no
console errors; covers sidebar → create → view → assign → close flow.

### Batch D — manual validation (operator tasks, sequential with prior batches)

**T19 — Manual Steam Deck / `gamescope` walkthrough** _Depends on [T11, T3, T12, T13, T14, T15, T16,
T17, T18]._ Operator follows `docs/internal-docs/steam-deck-validation-checklist.md` end-to-end.
Record results in a markdown table: `| # | Check | Pass/Fail | Notes |`. Embed the filled table in
the Phase 5 report (T22). If any check fails, file a follow-up task and mark Phase 5 blocked.
Acceptance: completed checklist with results; any fails have follow-up tracking.

**T20 — Manual regression sweep** _Depends on [T3, T12, T13, T14, T15, T16, T17, T18]._ Operator
executes the 8-step regression sweep documented in the Test Plan above. Record results in a markdown
table. Specific emphasis:

- **Editor-safety invariant**: ProfilesPage dropdown MUST NOT pass `collectionId` to `loadProfile`
  (verify via browser dev mode console).
- **Phase 3 env-var merge**: launch with `activeCollectionId` set → `printenv | grep DXVK_HUD` in
  the launched game's environment must match the collection default.
- **`MetadataStore::disabled()` fallback**: use `?errors=true` flag to simulate; sidebar hides,
  dropdown filter no-ops, no data loss. Acceptance: completed sweep table embedded in the Phase 5
  report (T22); zero regressions.

### Batch E — closing tasks (depend on all prior batches)

**T21 — Update PRD phase 5 row** _Depends on [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12,
T13, T14, T15, T16, T17, T18, T19, T20]._ Edit `docs/prps/prds/profile-collections.prd.md:211`.
Change:

- `Status`: `pending` → `complete`
- `PRP Plan`: `-` →
  `[profile-collections-phase-5-polish-tests-steam-deck.plan.md](../plans/completed/profile-collections-phase-5-polish-tests-steam-deck.plan.md)`
  Optionally update the PRD top-of-file `Status` at line 5 from `DRAFT — needs validation` to
  `SHIPPED — v{next-release}` if the release tag is known. Acceptance: row reflects completion;
  markdown table alignment preserved.

**T22 — Write Phase 5 implementation report** _Depends on [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10,
T11, T12, T13, T14, T15, T16, T17, T18, T19, T20]._ Create
`docs/prps/reports/profile-collections-phase-5-polish-tests-steam-deck.report.md`. Mirror the
skeleton from `profile-collections-phase-3-launch-defaults.report.md` and
`profile-collections-phase-4-toml-export-import-preset-report.md`:

- Front-matter (Date, Branch, Source Plan, Source PRD, Source Issue #181, Status, Parent Issue #73)
- `## Overview`
- `## Files Changed` (table: # | File | Action | Notes)
- `## Features Delivered` (sub-headings per deliverable — a11y, tests, docs, empty state, polish)
- `## Tests` (table covering T1 Rust integration + T18 Playwright + manual checklists)
- `## Validation Results` (5 levels: static `npm run build`, unit `cargo test -p crosshook-core`,
  build `./scripts/build-native.sh` optional, integration
  `cargo test --test collections_jtbd_integration + playwright`, manual T19 + T20 tables)
- `## Manual Steam Deck validation` (embedded T19 table)
- `## Manual regression sweep` (embedded T20 table)
- `## Risks Materialized` (from Gotchas list)
- `## Conventional commit suggestions` (final
  `feat(ui): profile collections polish, integration tests, Steam Deck validation — Phase 5 (#181)`
  plus `docs(internal):` for docs changes plus any `fix(ui):` scope needed for tested regressions)
- `## Next steps` (post-merge: archive this plan to `plans/completed/`; close #73 via merge commit;
  monitor for community adoption metrics per PRD §Success Metrics)
- `## Addendum` (blank placeholder for post-review follow-up) Acceptance: report file exists; all
  sections populated; all file:line citations match post-implementation HEAD.

---

## Batches (parallel execution summary)

| Batch | Tasks                                                       | Parallel?                    | Depends on                       | Rationale                                                                                                                                                                                |
| ----- | ----------------------------------------------------------- | ---------------------------- | -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A** | T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11 (**11 tasks**) | yes                          | -                                | All disjoint files; no shared state. Rust test, hook extraction, sidebar semantics, CSS polish, LaunchDefaultsEditor, git-mv, 4 docs.                                                    |
| **B** | T12, T13, T14, T15, T16, T17 (**6 tasks**)                  | mostly                       | T2 (all); T16 (T17)              | T12-T16 all depend on T2 (`useFocusTrap`). T17 depends on T16 only conceptually — disjoint files, can run alongside if implementor is careful; conservative ordering puts T17 after T16. |
| **C** | T18 (**1 task**)                                            | n/a                          | T3, T12, T13, T14, T15, T16, T17 | Playwright test needs semantics stable across all 6 a11y refactors.                                                                                                                      |
| **D** | T19, T20 (**2 tasks**)                                      | yes (operator can alternate) | T11, T18, + all Batch B/C        | Operator-driven manual validation. Can interleave.                                                                                                                                       |
| **E** | T21, T22 (**2 tasks**)                                      | no                           | All of T1-T20                    | PRD row update + Phase 5 report must reflect final state. T21 before T22 (report cites the updated PRD row).                                                                             |

**Total tasks**: 22 across 5 batches.

**Single-engineer fallback**: serial A → B → C → D → E also works; parallelism is an optimization,
not a requirement.

---

## Post-merge housekeeping

Not plan tasks — these happen as part of the merge/release flow, not the implementation agent's
work:

1. Squash-merge the Phase 5 PR with title
   `feat(ui): profile collections polish, integration tests, Steam Deck validation — Phase 5 (#181)`
   and body `Closes #181. Closes #73.` (the parent epic).
2. After merge, a separate `docs(internal):` commit moves
   `docs/prps/plans/profile-collections-phase-5-polish-tests-steam-deck.plan.md` →
   `docs/prps/plans/completed/` (mirrors Phase 4 precedent `f018e4a`).
3. **Release is downstream** — not Phase 5's job. When a release is cut,
   `./scripts/prepare-release.sh --version X.Y.Z` will pick up the Phase 5 commits; the `feat(ui):`
   commit lands in the `### Features` section of `CHANGELOG.md`; all `docs(internal):` commits are
   filtered out per `.git-cliff.toml:45`.
4. `verify:no-mocks` CI sentinel at `.github/workflows/release.yml:105-120` runs on the eventual
   `v*` tag push and fails if any mock sentinel string leaks into `dist/assets/*.js`. No Phase 5
   change should affect this gate, but call out in the PR description as a sanity check.

---

## Out of scope (Phase 5 is the last phase)

- **Bulk launch**, **bulk env-var apply**, **dynamic/smart collections**, **drag-and-drop**,
  **per-collection cover art**, **generic `Collection<T>` schema**, **soft-delete of collections**,
  **generalized `useLaunch` hook**, **Favorites consolidation into collections** — all deferred to
  v2+ per the PRD's §What We're NOT Building table (line 38-48).
- **Native Tauri E2E** (outside the Vite browser dev mode) — not set up; issue #181 explicitly
  accepts manual Steam Deck validation as the v1 bar.
- **Automated a11y audit via axe-core or Playwright a11y snapshots** — no tooling installed; issue
  #181 scope is manual audit only.
- **Performance profiling** of the 50-profile fixture in production builds — out of scope; the
  fixture is for correctness, not perf.
- **Telemetry for success metrics** (§Success Metrics in the PRD) — measured via anecdotal
  GitHub/Discord/Reddit reports per PRD decision, no code needed.

---

_Generated: 2026-04-08_ _Source:
`/ycc:prp-plan --parallel docs/prps/prds/profile-collections.prd.md phase 5 github issue 181`_
