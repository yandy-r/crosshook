# PR Review: #183 — feat(ui): profile collections sidebar, view modal, shared state

**Reviewed**: 2026-04-08
**Author**: yandy-r
**Branch**: `feat/profile-collections-phase-2-sidebar-modal` → `main`
**Head**: `44c890a` (updated during review; originally `58ac19a`)
**Decision**: **APPROVE** with comments

## Summary

Phase 2 frontend for profile collections is cleanly architected and feature-complete. It introduces a shared `CollectionsProvider` (fixing the stale sidebar badge / empty view-modal issue from phase 2 research), camelCase mock IPC args, a `CollectionsSidebar`, `CollectionViewModal` + `CollectionEditModal`, right-click `CollectionAssignMenu` on Library cards, and Launch/Profiles active-collection filtering with a clear chip. CLAUDE.md pattern rules are respected (camelCase IPC args, snake_case wire types, `crosshook-*` BEM classes, new scroll containers registered with `useScrollEnhance`, `overscroll-behavior: contain` on inner scrollers). A mid-review follow-up commit (`44c890a`) also resolved several MEDIUM findings I was preparing to raise (session-scoped edit errors, member-fetch race, filter loading flicker, modal busy-state lock). Remaining findings are UX polish items — none block merge.

## Findings

### CRITICAL

None.

### HIGH

None.

### MEDIUM

**M1 — Silent description failure on the Create Collection path**
`src/crosshook-native/src/context/CollectionsContext.tsx:54-80`
`CollectionsContext.createCollection` writes the collection, then best-effort calls `collection_update_description`. On description failure it sets `error` — but the subsequent `await refresh()` calls `setError(null)` on success, wiping the failure before any consumer can render it. Upstream, `CollectionsSidebar.handleCreate` returns `id !== null` (always true when the collection itself was created) and the Edit modal closes. Net result: a new collection is created without its description and the user is never told.

The Edit flow was fixed in `44c890a` via `editSessionError` + `CollectionWriteResult`; the Create flow deserves the same treatment. Suggested fixes (pick one):
- Make `createCollection` return a `CollectionWriteResult`-like object that carries a partial success (`{ ok: true; descriptionFailed?: string }`), and surface it in `CollectionsSidebar`.
- Or queue the description retry toast (same pattern as `collectionDescriptionToast` in `App.tsx`) whenever the sidebar create path fails the description step.
- Or simply don't clear `error` inside `refresh()` when it isn't the source of the error — e.g. only clear on explicit mutation entry, not on refetch success.

**M2 — Stale `selectedProfile` when the active collection filter excludes it**
`src/crosshook-native/src/components/pages/LaunchPage.tsx:32-44`, `ProfilesPage.tsx:109-118`
When a collection filter is active and the currently-selected profile is not a member, the `ThemedSelect` trigger still displays the old value, but the dropdown omits it. The user can't reselect it without clearing the filter — and from the sidebar-open flow (`handleOpenCollection` in `App.tsx`), this is the common state right after filtering. Consider one of:
- Auto-select the first filtered profile when the current selection drops out of the filter.
- Clear the selection (`selectProfile('')`) when filtering excludes it.
- Show an in-chip note ("current selection not in filter") so the user understands why the dropdown can't reach their current choice.

### LOW

**L1 — Dead `null` check on `createCollection.id`**
`CollectionsContext.tsx:60` — `callCommand<string>` is typed to resolve to `string`, so `id !== null` is always true. Either delete the check or correctly type the return as `string | null` if the backend can in fact return nothing.

**L2 — Portal host div created unconditionally on mount**
`CollectionViewModal.tsx:122-136` — the `<div class="crosshook-modal-portal">` is appended to `document.body` in a mount-effect, even when the modal is never opened. Minor DOM pollution and tiny perf cost. Cheap fix: lazily create the host the first time `open` flips to `true`.

**L3 — `role="list"` without `role="listitem"` children in the collections sidebar**
`CollectionsSidebar.tsx:35-52` — the parent `<div>` has `role="list"` but each `<button>` child lacks `role="listitem"`. Some screen readers won't announce list structure correctly. Low-impact a11y nit.

**L4 — `CollectionAssignMenu` position does not react to window resize**
`CollectionAssignMenu.tsx:95-100` — clamp uses `window.innerWidth/innerHeight` inline in render. If the viewport is resized while the menu is open it won't recompute its position. The menu typically closes on most interactions so this is borderline acceptable, but a small `useLayoutEffect` on resize would be more robust.

**L5 — Mock uniqueness check is case-sensitive**
`src/crosshook-native/src/lib/mocks/handlers/collections.ts:61,144` — `c.name === trimmed` is strictly case-sensitive. If the Rust backend uses case-insensitive uniqueness (common with `COLLATE NOCASE` on SQLite), the mock and real backend will disagree on duplicate detection. Mock-only, so LOW, but worth a comment or alignment.

**L6 — `CollectionAssignMenu` shows no inline error on failed add/remove**
`CollectionAssignMenu.tsx:66-89` — when `addProfile` / `removeProfile` fail, the local `memberOf` set correctly stays in sync with the backend (not updated), but no feedback appears inside the menu; the error goes to the shared `useCollections().error` which the sidebar prints. Users performing a right-click assign may not look at the sidebar. Consider an inline `role="alert"` inside the popover.

### INFO / Notes

- **Follow-up commit `44c890a` ("fix(ui): session edit errors, modal busy state, and collection member races") lands during review.** It adds:
  - `CollectionWriteResult` discriminated union + `editSessionError` session-scoped state in `App.tsx` — fixes the Edit-flow variant of M1.
  - `requestSeqRef` guard in `useCollectionMembers` — protects against out-of-order responses when `collectionId` changes rapidly.
  - `membersForCollectionId` + `membersLoading` gating in `LaunchPage` / `ProfilesPage` — fixes the filter-flicker issue.
  - Stable effect dependencies in `CollectionViewModal` (swapped `collection` for `collectionId` + `collectionPresent`) — prevents the focus-trap effect from re-running on every `refresh()`.
  - `CollectionEditModal` busy-state lock (block Escape / backdrop / close while saving, `try/finally` around `setBusy`).
  - These are all good, targeted fixes that directly address issues I was preparing to raise.
- No Rust changes in this PR — `cargo test` not required.
- No frontend test framework is configured (per CLAUDE.md); `npm run test:smoke` (Playwright) was not run because Chromium binary is missing in the dev env. Manual verification with the full Tauri app is still pending per the PR checklist.
- No security issues: no `innerHTML`/`dangerouslySetInnerHTML`, no secrets, all IPC data flows through React rendering (escaped by default). External inputs (collection names, profile names, descriptions) are validated/trimmed at both mock and caller boundaries.
- Tauri IPC args are camelCase in JS (`collectionId`, `profileName`, `newName`); Rust `#[tauri::command]` signatures use snake_case (`collection_id`, `profile_name`, `new_name`) in `src-tauri/src/commands/collections.rs` — matches Tauri 2's default case conversion. ✓
- `SCROLLABLE` selector in `useScrollEnhance.ts` correctly registers the two new containers (`.crosshook-collections-sidebar__list`, `.crosshook-collection-assign-menu__list`), and both have `overscroll-behavior: contain`. ✓
- CSS additions live in the `Profile collections (Phase 2)` section of `theme.css` and a new reusable token `--crosshook-color-border-muted` in `variables.css` — follows the CSS-variable convention. ✓
- Mock errors all start with `[dev-mock]`, preserving the `verify:no-mocks` CI sentinel. ✓

## Validation Results

| Check        | Result                                                      |
| ------------ | ----------------------------------------------------------- |
| Type check   | ✅ Pass — `tsc` (via `npm run build`) at `44c890a`          |
| Lint         | ⏭ Skipped — no lint script in `package.json`               |
| Tests        | ⏭ Skipped — Playwright smoke needs Chromium install; no unit test framework |
| Build        | ✅ Pass — `vite build` at `44c890a` (dist produced)         |
| Mock sentinel| ✅ Pass — `npm run dev:browser:check` reports 0 missing handlers |

> Ran against a detached worktree at the PR head (`git worktree add --detach /tmp/crosshook-pr183-review <sha>`) to avoid confusion with uncommitted local WIP in the main worktree.

## Files Reviewed

All 25 files in the PR diff read in full at the head revision:

- `src/crosshook-native/src/App.tsx` (Modified)
- `src/crosshook-native/src/components/collections/CollectionAssignMenu.tsx` (Added)
- `src/crosshook-native/src/components/collections/CollectionEditModal.tsx` (Added)
- `src/crosshook-native/src/components/collections/CollectionViewModal.tsx` (Added)
- `src/crosshook-native/src/components/collections/CollectionViewModal.css` (Added)
- `src/crosshook-native/src/components/collections/CollectionsSidebar.tsx` (Added)
- `src/crosshook-native/src/components/collections/useCollectionViewModalState.ts` (Added)
- `src/crosshook-native/src/components/layout/Sidebar.tsx` (Modified)
- `src/crosshook-native/src/components/library/LibraryCard.tsx` (Modified)
- `src/crosshook-native/src/components/library/LibraryGrid.tsx` (Modified)
- `src/crosshook-native/src/components/pages/LaunchPage.tsx` (Modified)
- `src/crosshook-native/src/components/pages/LibraryPage.tsx` (Modified)
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx` (Modified)
- `src/crosshook-native/src/context/CollectionsContext.tsx` (Added)
- `src/crosshook-native/src/context/ProfileContext.tsx` (Modified)
- `src/crosshook-native/src/hooks/useCollectionMembers.ts` (Added)
- `src/crosshook-native/src/hooks/useCollections.ts` (Modified — now re-exports from context)
- `src/crosshook-native/src/hooks/useScrollEnhance.ts` (Modified)
- `src/crosshook-native/src/lib/focus-utils.ts` (Added)
- `src/crosshook-native/src/lib/mocks/handlers/collections.ts` (Modified — camelCase IPC args)
- `src/crosshook-native/src/styles/theme.css` (Modified — Phase 2 styles)
- `src/crosshook-native/src/styles/variables.css` (Modified — `--crosshook-color-border-muted`)
- `src/crosshook-native/src/types/collections.ts` (Added)
- `src/crosshook-native/src/types/index.ts` (Modified — re-export)
- `.claude/PRPs/reports/profile-collections-phase-2-sidebar-view-modal-report.md` (Added)

## Suggested Follow-ups (non-blocking)

1. **M1**: Propagate description-step failures on the Create path, mirroring the Edit path's `editSessionError` fix.
2. **M2**: Decide on UX when the active-collection filter excludes `selectedProfile` (auto-select first / clear / warn).
3. **L1**: Delete the dead `id !== null` check in `createCollection`.
4. **L2**: Lazy-create the `CollectionViewModal` portal host on first open.
5. **L3**: Add `role="listitem"` to the collection buttons in `CollectionsSidebar`.
6. **L6**: Add an inline error slot to `CollectionAssignMenu` for failed add/remove.
