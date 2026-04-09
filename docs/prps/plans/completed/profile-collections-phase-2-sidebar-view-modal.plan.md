# Plan: Profile Collections — Phase 2 (Sidebar + View Modal)

## Summary

Deliver the irreducible MVP of the profile collections feature: a sidebar Collections section, a `<CollectionViewModal>` mirroring `GameDetailsModal`, an `useCollections` + `useCollectionMembers` hook pair wrapping the 9 Phase-1 IPC commands, a right-click "Add to collection" multi-select context menu on profile cards, and an Active-Profile dropdown that filters by the current collection. This phase is **frontend-only** — Phase 1 already merged the backend (commit `63d43e1`). Zero new dependencies. Scope is 17 files (12 CREATE, 5 UPDATE) with one critical bug-fix on the existing mock handler.

## User Story

As a **power user with 50+ profiles**, I want to **jump to a sidebar collection, search inside it, and launch a profile in two clicks** so that **I never scroll an 80-item flat list again — and the same dropdown can be filtered to one collection while I'm playing**.

## Problem → Solution

**Current state**: 9 Tauri collection IPC commands exist + are registered + mocked, schema is at v19 with FK cascade and `sort_order`, but **zero frontend consumers**. The sidebar (`Sidebar.tsx:36-61`) renders only fixed routes. The Active-Profile dropdown (`LaunchPage.tsx:295-306`, `ProfilesPage.tsx:603-614`) is a flat `ThemedSelect`. Profile cards (`LibraryCard.tsx:70-151`) have no right-click handler. There is no `src/types/collections.ts`. The Phase-1 mock handler at `src/lib/mocks/handlers/collections.ts` destructures **snake_case** keys (`collection_id`, `profile_name`, `new_name`) but Tauri v2 will deliver **camelCase** keys to the JS layer — every collection IPC call from a Phase-2 hook will silently fail in `pnpm dev:browser` until the mock is fixed.

**Desired state**: Users can create collections, see them in a sidebar section, click one to open a modal that lists members with search/filter, launch or edit a profile through the existing select-then-navigate indirection, right-click a library card to multi-select-assign it to collections, and toggle the Active-Profile dropdown filter to the active collection. All ephemeral state lives in `ProfileContext`. No persisted data added in Phase 2 — it's a pure consumer of Phase 1's SQLite + IPC.

## Metadata

- **Complexity**: **Large** (17 files, ~1500 lines, frontend-only, follows established patterns)
- **Source PRD**: `docs/prps/prds/profile-collections.prd.md`
- **PRD Phase**: **Phase 2 — Sidebar + view modal**
- **Source Issue**: GitHub `yandy-r/crosshook#178`
- **Depends on**: Phase 1 (merged, commit `63d43e1`) — schema v19, all 9 IPC commands, mock handler, `CollectionRow` Rust struct
- **Blocks**: Phase 3 (per-collection launch defaults), Phase 4 (TOML export/import)
- **Estimated Files**: 17 (12 CREATE, 5 UPDATE)

## Storage / Persistence

**No new persisted data.** Phase 2 is a frontend-only consumer of Phase 1's SQLite schema v19.

| Datum / behavior                                                      | Classification             | Where it lives                                  | Migration / back-compat                                   |
| --------------------------------------------------------------------- | -------------------------- | ----------------------------------------------- | --------------------------------------------------------- |
| `activeCollectionId` (filter context for the Active-Profile dropdown) | **Runtime-only**           | React state in `ProfileContext` (merge pattern) | None — resets on app restart, intentionally ephemeral     |
| `openCollectionId` (which collection's modal is currently open)       | **Runtime-only**           | Local `useState` in `AppShell`                  | None — resets on modal close                              |
| Collection list (`CollectionRow[]`)                                   | Read-only mirror of SQLite | `useCollections` hook state                     | None — read-through cache, refreshed after every mutation |
| Collection membership (`string[]` of profile names per collection)    | Read-only mirror of SQLite | `useCollectionMembers(id)` hook state           | Same                                                      |
| Reverse lookup (`collections_for_profile`)                            | Read-only mirror of SQLite | Computed in `<CollectionAssignMenu>` on open    | Same                                                      |

**Offline behavior**: Fully offline. All operations go through `callCommand` against the local SQLite store. No network.

**Degraded fallback**: If `MetadataStore` is unavailable (Phase 1 documents this returns `Ok(vec![])` for read paths and `Ok(())` for writes via `with_conn` defaults), `useCollections.collections` will be an empty array. The sidebar `<CollectionsSidebar>` collapses to its empty-CTA state. Active-Profile dropdowns ignore the (null) filter. **No data loss.** The user simply loses the new organizational layer until the DB recovers. Matches the PRD's documented no-data-loss fallback.

**User visibility / editability**: Collections are user-named via the create/rename modals. Membership is toggled via right-click context menu or per-collection modal actions. All interactions are keyboard- and controller-reachable (no DnD; D-pad nav verified in Phase 5). Backend storage is **not** directly file-editable (collections live in SQLite, not TOML); collection presets exported as TOML are user-editable once Phase 4 ships.

---

## UX Design

### Before

```
+----------------------------------------------------+
| crosshook-sidebar                                  |
| > Game                                             |
|     • Library                                      |
|     • Profiles                                     |
|     • Launch                                       |
| > Setup                                            |
|     • Install                                      |
| > Dashboards / Community                           |
+----------------------------------------------------+
| LibraryPage:                                       |
|   [Search________]   [grid][list]                  |
|   ┌────┐ ┌────┐ ┌────┐  ← scroll, scroll, scroll   |
|   │card│ │card│ │card│  ← right-click does nothing |
|   └────┘ └────┘ └────┘                             |
+----------------------------------------------------+
| LaunchPage / ProfilesPage:                         |
|   Active Profile: [▼ Elden Ring]  ← flat 80 items  |
+----------------------------------------------------+
```

### After

```
+----------------------------------------------------+
| crosshook-sidebar                                  |
| > Game                                             |
|     • Library | Profiles | Launch                  |
| > Collections                                      |
|     ▣ Action / Adventure        (12)               |
|     ▣ Stable                    (47)               |
|     ▣ WIP                       ( 8)               |
|     [+ New collection]                             |
| > Setup / Dashboards / Community                   |
+----------------------------------------------------+
| <CollectionViewModal open>                         |
|   ┌──────────────────────────────────────────────┐ |
|   │ Action / Adventure         × Close          │ |
|   │ 12 profiles · "Open in Library →"          │ |
|   ├──────────────────────────────────────────────┤ |
|   │ [Search inside collection ___________]      │ |
|   │ ┌────┐ ┌────┐ ┌────┐                        │ |
|   │ │card│ │card│ │card│ ← reuses LibraryCard  │ |
|   │ └────┘ └────┘ └────┘                        │ |
|   ├──────────────────────────────────────────────┤ |
|   │ Rename | Edit description | Delete | Close  │ |
|   └──────────────────────────────────────────────┘ |
+----------------------------------------------------+
| LibraryCard right-click:                           |
|   ┌──────────────────────────┐                     |
|   │ Add to collection        │                     |
|   │   ☑ Action / Adventure   │                     |
|   │   ☐ Stable               │                     |
|   │   ☑ WIP                  │                     |
|   │   ─────────────────────  │                     |
|   │   [+ New collection…]    │                     |
|   └──────────────────────────┘                     |
+----------------------------------------------------+
| LaunchPage Active Profile dropdown:                |
|   Active Profile: [▼ Elden Ring]                   |
|   Filter: ▣ Action / Adventure ×  ← chip, click ×  |
|   Options narrow to the 12 members of the filter   |
+----------------------------------------------------+
```

### Interaction Changes

| Touchpoint                              | Before                                                                   | After                                                                                                                  | Notes                                                                                                  |
| --------------------------------------- | ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Sidebar                                 | Fixed routes only                                                        | Adds Collections section + "+ New" button                                                                              | Always renders the "+ New" button as the empty-state CTA; chip list only when `collections.length > 0` |
| Click sidebar collection chip           | n/a                                                                      | Opens `<CollectionViewModal>`; sets `activeCollectionId` (filter persists after close)                                 | New callback prop on `Sidebar`, sourced from `AppShell`                                                |
| Right-click `LibraryCard`               | nothing                                                                  | Opens `<CollectionAssignMenu>` portal popover with multi-select checkboxes                                             | First context menu in the codebase                                                                     |
| `LaunchPage` Active-Profile dropdown    | Lists all profiles                                                       | When `activeCollectionId !== null`, lists only collection members + chip showing the active filter with a clear button | Preserves `pinnedValues` for favorites                                                                 |
| `ProfilesPage` Active-Profile dropdown  | Lists all profiles + "Create New" sentinel                               | Same filter, **but the `{ value: '', label: 'Create New' }` sentinel is preserved at the top**                         | Failure mode: stripping the sentinel breaks the create-new flow                                        |
| Empty state                             | n/a                                                                      | Sidebar collections section shows only "+ Create your first collection" button when empty                              | Matches PRD "Sidebar opt-in rendering" decision                                                        |
| `pnpm dev:browser` collection IPC calls | Crashes — mock handlers expect snake_case args, frontend sends camelCase | Works                                                                                                                  | **D1 blocker fix** in Task 1                                                                           |

---

## Mandatory Reading

Read these files **before starting**. The plan assumes you have this context in head and will not re-search the codebase during implementation.

| Priority | File                                                                      | Lines         | Why                                                                                                                                                                                                |
| -------- | ------------------------------------------------------------------------- | ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **P0**   | `src/crosshook-native/src/lib/mocks/handlers/collections.ts`              | all (177)     | The arg-naming bug fix in Task 1. Mirror for new validation strings.                                                                                                                               |
| **P0**   | `src/crosshook-native/src/lib/ipc.ts`                                     | all (17)      | The single `callCommand<T>` chokepoint Phase 2 hooks must use.                                                                                                                                     |
| **P0**   | `src/crosshook-native/src/hooks/useLauncherManagement.ts`                 | all (101)     | **Best structural precedent for `useCollections`** — single error string, per-op busy ids, refresh-after-mutate, `boolean` return from CRUD.                                                       |
| **P0**   | `src/crosshook-native/src/components/library/GameDetailsModal.tsx`        | all (487)     | The richest portal modal precedent — focus trap, body lock, inert siblings, tab trap, `data-crosshook-focus-root="modal"`, `data-crosshook-modal-close`. Copy verbatim into `CollectionViewModal`. |
| **P0**   | `src/crosshook-native/src/components/library/GameDetailsModal.css`        | all (265)     | Modal-specific overrides; `overscroll-behavior: contain` precedent at lines 13-15.                                                                                                                 |
| **P0**   | `src/crosshook-native/src/components/library/useGameDetailsModalState.ts` | all (28)      | Open/close state hook precedent — mirror for `useCollectionViewModalState`.                                                                                                                        |
| **P0**   | `src/crosshook-native/src/components/library/game-details-actions.ts`     | all (23)      | The `closeModal(); void launch(name);` close-then-navigate idiom. Reuse exactly.                                                                                                                   |
| **P0**   | `src/crosshook-native/src/components/layout/Sidebar.tsx`                  | all (174)     | Where the new `<CollectionsSidebar>` mounts. Note the `Tabs.List` constraint — collection items **cannot** be `Tabs.Trigger`.                                                                      |
| **P0**   | `src/crosshook-native/src/App.tsx`                                        | 39-45, 78-142 | `handleGamepadBack` requires `data-crosshook-modal-close`; `AppShell` is where `<CollectionViewModal>` and `openCollectionId` state mount.                                                         |
| **P0**   | `src/crosshook-native/src/context/ProfileContext.tsx`                     | all (78)      | Where to add `activeCollectionId` via the merge pattern (lines 56-64).                                                                                                                             |
| **P0**   | `src/crosshook-native/src/components/pages/LibraryPage.tsx`               | all (160)     | Reference for `handleLaunch`/`handleEdit`/`handleToggleFavorite`. The `<CollectionViewModal>` body re-uses these patterns.                                                                         |
| **P0**   | `src/crosshook-native/src/components/library/LibraryCard.tsx`             | all (153)     | Receives the new `onContextMenu` for the assign menu.                                                                                                                                              |
| **P0**   | `src/crosshook-native/src/hooks/useLibraryProfiles.ts`                    | all (18)      | Pure memo filter. Compose inside `<CollectionViewModal>` for the inner search box.                                                                                                                 |
| **P0**   | `src/crosshook-native/src/hooks/useLibrarySummaries.ts`                   | all (69)      | Converts `string[]` profile names → `LibraryCardData[]`. The collection modal needs this to render cards.                                                                                          |
| **P0**   | `src/crosshook-native/src/components/pages/LaunchPage.tsx`                | 287-306       | Active-Profile dropdown integration point #1.                                                                                                                                                      |
| **P0**   | `src/crosshook-native/src/components/pages/ProfilesPage.tsx`              | 593-614       | Active-Profile dropdown integration point #2. **Has the `{ value: '', label: 'Create New' }` sentinel that must survive filtering.**                                                               |
| **P0**   | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                      | 8-9           | The `SCROLLABLE` selector. Any new `overflow-y: auto` container must be appended.                                                                                                                  |
| **P0**   | `src/crosshook-native/src-tauri/src/commands/collections.rs`              | all (95)      | Canonical Rust argument names — Tauri v2 converts these to camelCase on the JS side.                                                                                                               |
| **P1**   | `src/crosshook-native/src/types/launcher.ts`                              | all (29)      | snake_case mirror convention for the new `src/types/collections.ts`.                                                                                                                               |
| **P1**   | `src/crosshook-native/src/types/library.ts`                               | all (14)      | `LibraryCardData` shape — modal renders these.                                                                                                                                                     |
| **P1**   | `src/crosshook-native/src/components/PinnedProfilesStrip.tsx`             | all (62)      | Chip pattern + "return null when empty" idiom.                                                                                                                                                     |
| **P1**   | `src/crosshook-native/src/components/OfflineTrainerInfoModal.tsx`         | all (177)     | "Lite" portal modal precedent (no portal host, no inert siblings). Mirror for `<CollectionEditModal>` since it's smaller.                                                                          |
| **P1**   | `src/crosshook-native/src/components/ui/ThemedSelect.tsx`                 | 86-161        | The select used in both Active-Profile dropdowns. Filter input goes to `options` prop.                                                                                                             |
| **P1**   | `src/crosshook-native/src/styles/theme.css`                               | 3690-3883     | Shared modal classes. Re-use unchanged.                                                                                                                                                            |
| **P1**   | `src/crosshook-native/src/styles/sidebar.css`                             | 76-200        | Sidebar section/item CSS. Re-use `crosshook-sidebar__section`, `__section-label`, `__section-items`, `__item`.                                                                                     |
| **P2**   | `src/crosshook-native/tests/smoke.spec.ts`                                | all (60+)     | Playwright smoke test walks 9 routes — must not introduce `console.error` calls during boot/navigation.                                                                                            |
| **P2**   | `src/crosshook-native/src/components/library/GameDetailsModal.css`        | all           | Modal CSS overrides; `crosshook-game-details-modal__body` uses `overscroll-behavior: contain` (lines 13-15).                                                                                       |
| **P2**   | `.github/workflows/release.yml`                                           | 105-120       | Mock-string sentinel — every error in `collections.ts` mock must keep `[dev-mock]` prefix.                                                                                                         |

## External Documentation

**No external research needed.** Phase 2 uses only established internal patterns. Zero new dependencies. Radix UI is constrained to the already-installed `react-select`, `react-tabs`, `react-tooltip`. Modals continue to be hand-rolled `createPortal` per existing convention.

---

## Patterns to Mirror

All snippets are **verbatim from the codebase**. Follow them exactly.

### NAMING_CONVENTION — IPC arg keys are camelCase

Tauri v2 converts Rust `snake_case` command parameter names (e.g. `collection_id: String`) to JS `camelCase` (`collectionId`) on the wire. Every other hook in the codebase uses camelCase on the call side. **Mirror these exactly.**

```ts
// SOURCE: src/crosshook-native/src/hooks/useLauncherManagement.ts:51-56
await callCommand<LauncherDeleteResult>('delete_launcher_by_slug', {
  launcherSlug,
  targetHomePath,
  steamClientInstallPath,
});
```

```ts
// SOURCE: src/crosshook-native/src/hooks/useProtonDbSuggestions.ts:88-95
      try {
        await callCommand<void>('protondb_dismiss_suggestion', {
          profileName,
          appId,
          suggestionKey,
        });
      } catch (err) {
        console.warn('[protondb] dismiss failed', { profileName, appId, suggestionKey }, err);
```

The IPC **command name** stays `snake_case` (e.g. `'collection_add_profile'`); only the **argument keys** are camelCase.

### NAMING_CONVENTION — TS mirror types use snake_case fields

TS types that mirror Rust serde output use the snake_case field names from the Rust struct, because serde defaults to snake_case serialization.

```ts
// SOURCE: src/crosshook-native/src/types/launcher.ts:1-9
export interface LauncherInfo {
  display_name: string;
  launcher_slug: string;
  script_path: string;
  desktop_entry_path: string;
  script_exists: boolean;
  desktop_entry_exists: boolean;
  is_stale: boolean;
}
```

The field name convention only differs from the IPC arg convention because **arg keys** are normalized by Tauri runtime, but **return-value field names** flow through serde verbatim.

### HOOK_PATTERN — `useLauncherManagement` shape (best precedent for `useCollections`)

```ts
// SOURCE: src/crosshook-native/src/hooks/useLauncherManagement.ts:21-67
export function useLauncherManagement({
  targetHomePath,
  steamClientInstallPath,
}: UseLauncherManagementOptions): UseLauncherManagementResult {
  const [launchers, setLaunchers] = useState<LauncherInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isListing, setIsListing] = useState(false);
  const [deletingSlug, setDeletingSlug] = useState<string | null>(null);
  const [reexportingSlug, setReexportingSlug] = useState<string | null>(null);

  const listLaunchers = useCallback(async () => {
    setIsListing(true);
    try {
      const result = await callCommand<LauncherInfo[]>('list_launchers', {
        targetHomePath,
        steamClientInstallPath,
      });
      setLaunchers(result);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsListing(false);
    }
  }, [targetHomePath, steamClientInstallPath]);

  const deleteLauncher = useCallback(
    async (launcherSlug: string) => {
      setDeletingSlug(launcherSlug);
      setError(null);
      try {
        await callCommand<LauncherDeleteResult>('delete_launcher_by_slug', {
          launcherSlug,
          targetHomePath,
          steamClientInstallPath,
        });
        await listLaunchers();
        return true;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return false;
      } finally {
        setDeletingSlug(null);
      }
    },
    [listLaunchers, targetHomePath, steamClientInstallPath]
  );
```

Conventions:

- Single `error: string | null` for the whole hook.
- Per-op busy ids (`deletingSlug`, `reexportingSlug`) — not a single `saving` boolean.
- Inline error normaliser: `err instanceof Error ? err.message : String(err)`.
- **Mutations always call the list refresh after success** (`await listLaunchers()`) so the in-memory list stays authoritative.
- CRUD callbacks return `boolean` (success/failure); list refreshes return `void`.

### MODAL_PATTERN — Portal mount + body lock + inert siblings + focus trap

Copy this block essentially verbatim into `<CollectionViewModal>`. The 6 existing portal modals all duplicate it.

```ts
// SOURCE: src/crosshook-native/src/components/library/GameDetailsModal.tsx:182-253
useEffect(() => {
  if (typeof document === 'undefined') {
    return;
  }
  const host = document.createElement('div');
  host.className = 'crosshook-modal-portal';
  portalHostRef.current = host;
  document.body.appendChild(host);
  setIsMounted(true);
  return () => {
    host.remove();
    portalHostRef.current = null;
    setIsMounted(false);
  };
}, []);

useEffect(() => {
  if (!open || !summary || typeof document === 'undefined') {
    return;
  }
  const { body } = document;
  const portalHost = portalHostRef.current;
  if (!portalHost) {
    return;
  }

  previouslyFocusedRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  bodyStyleRef.current = body.style.overflow;
  body.style.overflow = 'hidden';
  body.classList.add('crosshook-modal-open');

  hiddenNodesRef.current = Array.from(body.children)
    .filter((child): child is HTMLElement => child instanceof HTMLElement && child !== portalHost)
    .map((element) => {
      const inertState = (element as HTMLElement & { inert?: boolean }).inert ?? false;
      const ariaHidden = element.getAttribute('aria-hidden');
      (element as HTMLElement & { inert?: boolean }).inert = true;
      element.setAttribute('aria-hidden', 'true');
      return { element, inert: inertState, ariaHidden };
    });

  const focusTarget = headingRef.current ?? closeButtonRef.current ?? null;
  const frame = window.requestAnimationFrame(() => {
    if (focusElement(focusTarget)) {
      return;
    }
    const focusable = surfaceRef.current ? getFocusableElements(surfaceRef.current) : [];
    if (focusable.length > 0) {
      focusElement(focusable[0]);
    }
  });

  return () => {
    window.cancelAnimationFrame(frame);
    body.style.overflow = bodyStyleRef.current;
    body.classList.remove('crosshook-modal-open');
    for (const { element, inert, ariaHidden } of hiddenNodesRef.current) {
      (element as HTMLElement & { inert?: boolean }).inert = inert;
      if (ariaHidden === null) {
        element.removeAttribute('aria-hidden');
      } else {
        element.setAttribute('aria-hidden', ariaHidden);
      }
    }
    hiddenNodesRef.current = [];
    const restoreTarget = previouslyFocusedRef.current;
    if (restoreTarget && restoreTarget.isConnected) {
      focusElement(restoreTarget);
    }
    previouslyFocusedRef.current = null;
  };
}, [open, summary]);
```

The focusable helpers:

```ts
// SOURCE: src/crosshook-native/src/components/library/GameDetailsModal.tsx:27-49
const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
  '[contenteditable="true"]',
].join(', ');

function getFocusableElements(container: HTMLElement) {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) => !element.hasAttribute('disabled') && element.tabIndex >= 0 && element.getClientRects().length > 0
  );
}

function focusElement(element: HTMLElement | null) {
  if (!element) {
    return false;
  }
  element.focus({ preventScroll: true });
  return document.activeElement === element;
}
```

The Tab/Esc handler:

```ts
// SOURCE: src/crosshook-native/src/components/library/GameDetailsModal.tsx:255-294
function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
  if (event.key === 'Escape') {
    event.stopPropagation();
    event.preventDefault();
    onClose();
    return;
  }
  if (event.key !== 'Tab') {
    return;
  }
  const container = surfaceRef.current;
  if (!container) {
    return;
  }
  const focusable = getFocusableElements(container);
  if (focusable.length === 0) {
    event.preventDefault();
    return;
  }
  const currentIndex = focusable.indexOf(document.activeElement as HTMLElement);
  const lastIndex = focusable.length - 1;
  if (event.shiftKey) {
    if (currentIndex <= 0) {
      event.preventDefault();
      focusElement(focusable[lastIndex]);
    }
    return;
  }
  if (currentIndex === -1 || currentIndex === lastIndex) {
    event.preventDefault();
    focusElement(focusable[0]);
  }
}
```

The render root attributes that the gamepad-back handler relies on:

```tsx
// SOURCE: src/crosshook-native/src/components/library/GameDetailsModal.tsx:310-322
  return createPortal(
    <div className="crosshook-modal" role="presentation">
      <div className="crosshook-modal__backdrop" aria-hidden="true" onMouseDown={handleBackdropMouseDown} />
      <div
        ref={surfaceRef}
        className="crosshook-modal__surface crosshook-panel crosshook-focus-scope crosshook-game-details-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        data-crosshook-focus-root="modal"
        onKeyDown={handleKeyDown}
      >
```

`data-crosshook-focus-root="modal"` and the close button's `data-crosshook-modal-close` attribute are what `App.tsx:39-45` `handleGamepadBack` looks for. Both are non-negotiable for controller back-button support.

### MODAL_STATE_HOOK_PATTERN — open/close helper

```ts
// SOURCE: src/crosshook-native/src/components/library/useGameDetailsModalState.ts:1-28
import { useCallback, useState } from 'react';

import type { LibraryCardData } from '../../types/library';

export interface UseGameDetailsModalStateResult {
  open: boolean;
  summary: LibraryCardData | null;
  openForCard: (card: LibraryCardData) => void;
  close: () => void;
}

export function useGameDetailsModalState(): UseGameDetailsModalStateResult {
  const [open, setOpen] = useState(false);
  const [summary, setSummary] = useState<LibraryCardData | null>(null);

  const close = useCallback(() => {
    setOpen(false);
    setSummary(null);
  }, []);

  const openForCard = useCallback((card: LibraryCardData) => {
    setSummary(card);
    setOpen(true);
  }, []);

  return { open, summary, openForCard, close };
}
```

### CLOSE_THEN_NAVIGATE_PATTERN — select-then-navigate indirection

```ts
// SOURCE: src/crosshook-native/src/components/library/game-details-actions.ts:1-23
/**
 * Thin orchestration for game-details quick actions: close the modal before
 * navigation-heavy flows so focus restoration and route stacks stay predictable.
 */

export function gameDetailsLaunchThenNavigate(
  profileName: string,
  launch: (name: string) => void | Promise<void>,
  closeModal: () => void
): void {
  closeModal();
  void launch(profileName);
}

export function gameDetailsEditThenNavigate(
  profileName: string,
  edit: (name: string) => void | Promise<void>,
  closeModal: () => void
): void {
  closeModal();
  void edit(profileName);
}
```

The outer `launch`/`edit` come from `LibraryPage.tsx:54-74`:

```ts
// SOURCE: src/crosshook-native/src/components/pages/LibraryPage.tsx:54-74
const handleLaunch = useCallback(
  async (name: string) => {
    setLaunchingName(name);
    try {
      await selectProfile(name);
      onNavigate?.('launch');
    } finally {
      setLaunchingName(undefined);
    }
  },
  [selectProfile, onNavigate]
);

const handleEdit = useCallback(
  async (name: string) => {
    await selectProfile(name);
    onNavigate?.('profiles');
  },
  [selectProfile, onNavigate]
);
```

`selectProfile` and `onNavigate?.('launch'|'profiles')` are the JTBD critical-path indirection. The collection modal must call **the same `handleLaunch`/`handleEdit` shape** so the user experience is identical to clicking a card on the Library page.

### CONTEXT_MERGE_PATTERN — adding ephemeral state to `ProfileContext`

```tsx
// SOURCE: src/crosshook-native/src/context/ProfileContext.tsx:28-66
export function ProfileProvider({ children }: ProfileProviderProps) {
  const profileState = useProfile({ autoSelectFirstProfile: false });
  const launchMethod = resolveLaunchMethod(profileState.profile);
  const steamClientInstallPath = deriveSteamClientInstallPath(profileState.profile.steam.compatdata_path);
  const targetHomePath = deriveTargetHomePath(steamClientInstallPath);

  // ... auto-load-profile effect ...

  const value = useMemo<ProfileContextValue>(
    () => ({
      ...profileState,
      launchMethod,
      steamClientInstallPath,
      targetHomePath,
    }),
    [launchMethod, profileState, steamClientInstallPath, targetHomePath]
  );

  return <ProfileContext.Provider value={value}>{children}</ProfileContext.Provider>;
}
```

Phase 2 adds `activeCollectionId: string | null` + `setActiveCollectionId` via a new `useState` in `ProfileProvider` and merges them into the memoized `value`. **`useProfile` is not touched** — collections are not profile CRUD.

### SIDEBAR_SECTION_PATTERN — section structure (re-use, but NOT `Tabs.Trigger`)

```tsx
// SOURCE: src/crosshook-native/src/components/layout/Sidebar.tsx:134-151
      <Tabs.List className="crosshook-sidebar__nav" aria-label="CrossHook sections">
        {SIDEBAR_SECTIONS.map((section) => (
          <div className="crosshook-sidebar__section" key={section.label}>
            <div className="crosshook-sidebar__section-label">{section.label}</div>
            <div className="crosshook-sidebar__section-items">
              {section.items.map((item) => (
                <SidebarTrigger
                  key={item.route}
                  activeRoute={activeRoute}
                  onNavigate={onNavigate}
                  route={item.route}
                  label={item.label}
                  icon={item.icon}
                />
              ))}
            </div>
          </div>
        ))}
```

**Constraint**: collection items open a modal, not a route. They **cannot** be `Tabs.Trigger` (Radix `Tabs.List` enforces that triggers map to tab panel values). The Phase-2 `<CollectionsSidebar>` renders plain `<button>` elements with the same `crosshook-sidebar__item` class for visual consistency, **inside** the `Tabs.List` element (Radix tolerates non-trigger children inside `Tabs.List`, but this should be smoke-tested).

Alternative if Radix complains: render the section **outside** the `Tabs.List` element but still inside the `<aside>` — e.g. between `Tabs.List` and the closing `</aside>` tag. The footer `crosshook-sidebar__footer` (`Sidebar.tsx:153-167`) sits inside `Tabs.List` today, so non-trigger children are clearly tolerated.

### ERROR_HANDLING_PATTERN — inline normaliser + setError

Pattern A (from `useLauncherManagement.ts:42`) is the canonical Phase 2 idiom:

```ts
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
```

**Do NOT** copy `formatInvokeError` from `useProfile.ts:128-146` — it is private to that file and consistently inlining keeps the new hooks self-contained.

### MOCK_HANDLER_PATTERN — `[dev-mock]` prefix is mandatory

```ts
// SOURCE: src/crosshook-native/src/lib/mocks/handlers/collections.ts:55-62
  map.set('collection_create', async (args): Promise<string> => {
    const { name } = args as { name: string };
    const trimmed = (name ?? '').trim();
    if (!trimmed) {
      throw new Error('[dev-mock] collection_create: collection name must not be empty');
    }
    if (collections.some((c) => c.name === trimmed)) {
      throw new Error(`[dev-mock] collection_create: duplicate collection name: ${trimmed}`);
    }
```

Every error string throws **must** start with `[dev-mock]` — `.github/workflows/release.yml:105-120` greps for it to verify no mock code leaked into the production bundle. This applies to the Task 1 fixes as well.

### CSS_CLASS_PATTERN — BEM-like `crosshook-*`

Existing modal classes are already in `theme.css:3690-3883` and re-used by every modal:

| Class                                                                    | Purpose                                               |
| ------------------------------------------------------------------------ | ----------------------------------------------------- |
| `crosshook-modal-open`                                                   | Body class added during modal mount                   |
| `crosshook-modal-portal`                                                 | Portal host container (`z-index: 1200`)               |
| `crosshook-modal`                                                        | Fixed-position outer wrapper                          |
| `crosshook-modal__backdrop`                                              | Click-to-close backdrop                               |
| `crosshook-modal__surface`                                               | Main panel grid                                       |
| `crosshook-modal__header`, `__heading-block`, `__title`, `__description` | Header stack                                          |
| `crosshook-modal__header-actions`, `__close`                             | Header actions                                        |
| `crosshook-modal__body`                                                  | **Already in `useScrollEnhance` SCROLLABLE selector** |
| `crosshook-modal__footer`, `__footer-actions`                            | Footer                                                |

Phase 2 adds **only** modal-specific override classes (`crosshook-collection-modal__*`), not new shared classes.

### SCROLL_ENHANCE_RULE — `SCROLLABLE` selector

```ts
// SOURCE: src/crosshook-native/src/hooks/useScrollEnhance.ts:8-9
const SCROLLABLE =
  '.crosshook-route-card-scroll, .crosshook-page-scroll-body, .crosshook-subtab-content__inner--scroll, .crosshook-console-drawer__body, .crosshook-modal__body, .crosshook-prefix-deps__log-output, .crosshook-discovery-results';
```

`.crosshook-modal__body` is **already** registered. Phase 2 only needs to update the selector if a new container has `overflow-y: auto` and is **not** inside `.crosshook-modal__body`. Examples:

- A sidebar collections list that overflows when many collections exist → add `.crosshook-collections-sidebar__list` to SCROLLABLE.
- A scrollable popover inside `<CollectionAssignMenu>` → add its class.

If the entire modal body uses `crosshook-modal__body`, no update is needed.

### EMPTY_STATE_PATTERN — `crosshook-library-empty`

```css
/* SOURCE: src/crosshook-native/src/styles/library.css:265-293 */
.crosshook-library-empty {
  /* ... */
}
```

Re-use a similar pattern (`crosshook-collections-empty`) for the modal's "No members yet" empty state.

---

## Files to Change

| #   | File                                                                             | Action                 | Justification                                                                                                                                          |
| --- | -------------------------------------------------------------------------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | `src/crosshook-native/src/lib/mocks/handlers/collections.ts`                     | UPDATE                 | **D1 BLOCKER**: Rewrite arg destructuring from snake_case (`collection_id`) to camelCase (`collectionId`) so Phase 2 hooks work in `pnpm dev:browser`. |
| 2   | `src/crosshook-native/src/types/collections.ts`                                  | CREATE                 | TS mirror of Rust `CollectionRow` (snake_case fields per serde).                                                                                       |
| 3   | `src/crosshook-native/src/types/index.ts`                                        | UPDATE                 | Export `./collections` from the barrel.                                                                                                                |
| 4   | `src/crosshook-native/src/hooks/useCollections.ts`                               | CREATE                 | Wraps the 9 IPC commands; mirrors `useLauncherManagement` shape.                                                                                       |
| 5   | `src/crosshook-native/src/hooks/useCollectionMembers.ts`                         | CREATE                 | Per-collection membership list — `collection_list_profiles` IPC + refresh.                                                                             |
| 6   | `src/crosshook-native/src/context/ProfileContext.tsx`                            | UPDATE                 | Add `activeCollectionId: string \| null` + `setActiveCollectionId` via merge pattern.                                                                  |
| 7   | `src/crosshook-native/src/components/collections/CollectionViewModal.tsx`        | CREATE                 | Portal modal mirroring `GameDetailsModal` — body lock, focus trap, search input, member grid.                                                          |
| 8   | `src/crosshook-native/src/components/collections/CollectionViewModal.css`        | CREATE                 | Modal-specific overrides + `overscroll-behavior: contain`.                                                                                             |
| 9   | `src/crosshook-native/src/components/collections/CollectionEditModal.tsx`        | CREATE                 | Lighter modal (mirrors `OfflineTrainerInfoModal`) for create / rename / edit description.                                                              |
| 10  | `src/crosshook-native/src/components/collections/CollectionsSidebar.tsx`         | CREATE                 | Sidebar section + chip list + "+ New" CTA.                                                                                                             |
| 11  | `src/crosshook-native/src/components/collections/CollectionAssignMenu.tsx`       | CREATE                 | Right-click portal popover with multi-select checkboxes. First context menu in the codebase.                                                           |
| 12  | `src/crosshook-native/src/components/collections/useCollectionViewModalState.ts` | CREATE                 | Open/close state hook (mirrors `useGameDetailsModalState`).                                                                                            |
| 13  | `src/crosshook-native/src/components/layout/Sidebar.tsx`                         | UPDATE                 | Mount `<CollectionsSidebar>`; add `onOpenCollection` prop to `SidebarProps`.                                                                           |
| 14  | `src/crosshook-native/src/App.tsx`                                               | UPDATE                 | Mount `<CollectionViewModal>` inside `AppShell`; manage `openCollectionId` state; pass `onOpenCollection` to `<Sidebar>`.                              |
| 15  | `src/crosshook-native/src/components/library/LibraryCard.tsx`                    | UPDATE                 | Add optional `onContextMenu` prop and forward to root div.                                                                                             |
| 16  | `src/crosshook-native/src/components/library/LibraryGrid.tsx`                    | UPDATE                 | Pass `onContextMenu` through from `LibraryPage`.                                                                                                       |
| 17  | `src/crosshook-native/src/components/pages/LibraryPage.tsx`                      | UPDATE                 | Wire `<CollectionAssignMenu>` to right-click on cards.                                                                                                 |
| 18  | `src/crosshook-native/src/components/pages/LaunchPage.tsx`                       | UPDATE                 | Filter `options` and `pinnedValues` by `activeCollectionId` when set; preserve favorites.                                                              |
| 19  | `src/crosshook-native/src/components/pages/ProfilesPage.tsx`                     | UPDATE                 | Same filter; **preserve the `{ value: '', label: 'Create New' }` sentinel**.                                                                           |
| 20  | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                             | UPDATE _(conditional)_ | Append `.crosshook-collections-sidebar__list` and `.crosshook-collection-assign-menu__list` to SCROLLABLE if those containers scroll.                  |
| 21  | `src/crosshook-native/src/styles/theme.css` _(or new `collections.css`)_         | UPDATE                 | New `crosshook-collections-*`, `crosshook-collection-modal__*`, `crosshook-collection-assign-menu__*` classes.                                         |

> **Note**: tasks 16 (`LibraryGrid`) and 17 (`LibraryPage` wiring) are minor pass-through edits but kept as separate tasks for clarity. The total CREATE count is **9** new files; UPDATE count is **8 + 1 conditional**.

## NOT Building

- **Per-collection launch defaults** (`CollectionDefaultsSection`, `effective_profile()` extension, `collection_launch_defaults` table, `collection_get_defaults`/`collection_set_defaults` commands) — **Phase 3** scope.
- **TOML export / import, import review modal** — **Phase 4** scope.
- **Drag-and-drop reordering / drag-to-assign** — out per PRD; Steam Deck constraint.
- **Dynamic / smart collections** — out per PRD; v2.
- **Bulk launch / bulk env-var apply** — out per PRD; v2.
- **Per-collection cover art / icons / colors** — out per PRD; v2.
- **Generic `Collection<T>` schema with `entity_kind` discriminator** — out per PRD.
- **`Favorites` consolidation into `Collection #0`** — out per PRD; v1.1.
- **Soft-delete of collections** — out per PRD; v1.
- **Generalized `useLaunch` hook** — out per PRD; v1 reuses select-then-navigate.
- **Shared `<Modal>` primitive extraction (Should-have)** — **deferred** to a follow-up issue. The new collection modals copy `GameDetailsModal` directly. Justification: 6 modals already duplicate this code; extracting now adds risk to Phase 2 single-pass implementation, and the PRD explicitly marks it as "Should-have, defer if scope runs long".
- **Sort-order setter / reorder UI** — Phase 1 added the column; Phase 2 reads it but does not write it. The chip list orders by Phase 1's `ORDER BY sort_order ASC, name ASC` automatically.
- **`onContextMenu` on `ProfilesPage` profile rows** — Phase 2 only adds it on `LibraryCard` (the high-traffic surface). Right-click parity on `ProfilesPage` is a follow-up.
- **Toast / snackbar primitive** — Phase 2 uses inline `<p className="...__warn">` for errors inside the modal, mirroring `GameDetailsModal.tsx:414-416`.
- **`formatInvokeError` shared util** — Phase 2 inlines `err instanceof Error ? err.message : String(err)` per existing convention. The shared util is a separate cleanup task.
- **Removing `MockCollectionRow` interface from the mock file** — Phase 2 leaves the mock's internal type alone (it's local to the file). The new `src/types/collections.ts` is the public type the hooks consume.

---

## Step-by-Step Tasks

### Task 1: Fix mock handler arg-name mismatch (D1 BLOCKER)

- **ACTION**: Rewrite every `args as { snake_case: ... }` destructure in `src/crosshook-native/src/lib/mocks/handlers/collections.ts` to use `camelCase` keys, matching Tauri v2's auto-conversion convention used by every other hook + mock.
- **IMPLEMENT**: In `src/crosshook-native/src/lib/mocks/handlers/collections.ts`, edit each handler:

  ```ts
  // collection_delete (line 81-86) — BEFORE
  map.set('collection_delete', async (args): Promise<null> => {
    const { collection_id } = args as { collection_id: string };
    collections = collections.filter((c) => c.collection_id !== collection_id);
    membership.delete(collection_id);
    return null;
  });
  // AFTER
  map.set('collection_delete', async (args): Promise<null> => {
    const { collectionId } = args as { collectionId: string };
    collections = collections.filter((c) => c.collection_id !== collectionId);
    membership.delete(collectionId);
    return null;
  });
  ```

  Apply the same camelCase rename to:
  - `collection_add_profile` (lines 88-112): `{ collection_id, profile_name }` → `{ collectionId, profileName }`
  - `collection_remove_profile` (lines 114-123): `{ collection_id, profile_name }` → `{ collectionId, profileName }`
  - `collection_list_profiles` (lines 125-129): `{ collection_id }` → `{ collectionId }`
  - `collection_rename` (lines 131-150): `{ collection_id, new_name }` → `{ collectionId, newName }`
  - `collection_update_description` (lines 152-167): `{ collection_id, description }` → `{ collectionId, description }` (description was already a single token; only the collection_id key changes)
  - `collections_for_profile` (lines 169-176): `{ profile_name }` → `{ profileName }`
  - `collection_create` (lines 55-79): `{ name }` is already a single token — **no change needed**.
  - `collection_list` (lines 48-53): no args — no change.

- **MIRROR**: `src/crosshook-native/src/lib/mocks/handlers/protondb.ts:135` — `const { appId, profileName } = args as { appId: string; profileName: string };`. Every other mock handler in the codebase uses camelCase.
- **IMPORTS**: none new.
- **GOTCHA**:
  - **Do NOT change the `MockCollectionRow` interface fields** (`collection_id`, `profile_count`, etc.) — those are the **return value** snake_case fields, mirroring serde output, and are correct as-is. Only the **incoming `args` keys** are camelCase.
  - **Do NOT change error string text** unnecessarily. `[dev-mock] collection_add_profile: profile_name must not be empty` is fine to leave (or update to `profileName` for consistency — implementer choice; the prefix is what matters for the CI sentinel).
  - The internal variables created by destructuring can be either `collectionId` (camelCase) for clarity or kept aligned with the surrounding snake_case mock state (`collection_id`). Recommended: use `collectionId` (camelCase) for the incoming destructure variable, but reference `c.collection_id` on the `MockCollectionRow` records. Mixing the two is normal because one is wire-protocol, the other is the row schema.
  - The `register.yml` sentinel does NOT care about variable names — only error string contents. Don't strip `[dev-mock]` prefixes.
  - The existing `getStore()` call inside `collection_add_profile` (line 102) reads `store.profiles` to validate the profile exists in the mock fixture. **Keep this**; it's the mock-side equivalent of Phase 1's typed-error check.
- **VALIDATE**:
  - `pnpm --filter crosshook-native run build` (= `tsc && vite build`) passes with zero TS errors.
  - `pnpm --dir src/crosshook-native run dev:browser:check` (mock coverage sentinel) passes.
  - In `pnpm --dir src/crosshook-native dev:browser` devtools console, `await callCommand('collection_add_profile', { collectionId: 'mock-collection-1', profileName: 'elden-ring' })` succeeds (assuming `elden-ring` exists in the fixture), and `await callCommand('collection_list')` returns the seed fixture.

### Task 2: Create `src/types/collections.ts`

- **ACTION**: Create the public TS mirror of Rust `CollectionRow` so hooks and components import a typed shape instead of duplicating it.
- **IMPLEMENT**: New file `src/crosshook-native/src/types/collections.ts`:

  ```ts
  /**
   * Mirror of Rust `CollectionRow` in
   * crates/crosshook-core/src/metadata/models.rs.
   *
   * Field names use snake_case to match serde's default serialization, matching
   * the convention in `types/launcher.ts` and others.
   */
  export interface CollectionRow {
    collection_id: string;
    name: string;
    description: string | null;
    profile_count: number;
    created_at: string;
    updated_at: string;
  }
  ```

- **MIRROR**: `src/crosshook-native/src/types/launcher.ts:1-9` `LauncherInfo` shape.
- **IMPORTS**: none.
- **GOTCHA**:
  - **Field names must be snake_case** (`collection_id`, `profile_count`, `created_at`, `updated_at`) — Rust serializes them this way per serde defaults. The Rust struct at `crates/crosshook-core/src/metadata/models.rs:294-303` has no `#[serde(rename_all = "camelCase")]` attribute. **The mock file's existing `MockCollectionRow` is the correct shape** — copy it.
  - `description` is `string | null` (not `string | undefined`), because Rust `Option<String>` serializes as `null` when `None`.
  - **Do NOT mark this `#[allow(dead_code)]`** — TS has no equivalent. The barrel export ensures it's reachable.
- **VALIDATE**: `pnpm --filter crosshook-native run build` succeeds.

### Task 3: Export collections types from the types barrel

- **ACTION**: Add `export * from './collections';` to `src/crosshook-native/src/types/index.ts` so hooks and components can import via `@/types`.
- **IMPLEMENT**: In `src/crosshook-native/src/types/index.ts`, add a new line in the appropriate alphabetical/grouping slot:

  ```ts
  export * from './collections';
  ```

- **MIRROR**: The existing barrel exports in `src/crosshook-native/src/types/index.ts:1-23`.
- **IMPORTS**: none.
- **GOTCHA**: Order is not enforced; place it near the other domain types (`./launcher`, `./library`).
- **VALIDATE**: `pnpm --filter crosshook-native run build` succeeds; importing `import { CollectionRow } from '@/types/collections'` (or `from '@/types'` if the alias resolves through the barrel) works in a downstream file.

### Task 4: Create `useCollections` hook

- **ACTION**: Create the primary hook that wraps all 9 collection IPC commands. Mirror `useLauncherManagement` structure exactly.
- **IMPLEMENT**: New file `src/crosshook-native/src/hooks/useCollections.ts`:

  ```ts
  import { useCallback, useEffect, useState } from 'react';

  import { callCommand } from '@/lib/ipc';
  import type { CollectionRow } from '../types/collections';

  export interface UseCollectionsResult {
    collections: CollectionRow[];
    error: string | null;
    isListing: boolean;
    creatingName: string | null;
    deletingId: string | null;
    renamingId: string | null;
    refresh: () => Promise<void>;
    createCollection: (name: string) => Promise<string | null>;
    deleteCollection: (collectionId: string) => Promise<boolean>;
    renameCollection: (collectionId: string, newName: string) => Promise<boolean>;
    updateDescription: (collectionId: string, description: string | null) => Promise<boolean>;
    addProfile: (collectionId: string, profileName: string) => Promise<boolean>;
    removeProfile: (collectionId: string, profileName: string) => Promise<boolean>;
    listMembers: (collectionId: string) => Promise<string[]>;
    collectionsForProfile: (profileName: string) => Promise<CollectionRow[]>;
  }

  export function useCollections(): UseCollectionsResult {
    const [collections, setCollections] = useState<CollectionRow[]>([]);
    const [error, setError] = useState<string | null>(null);
    const [isListing, setIsListing] = useState(false);
    const [creatingName, setCreatingName] = useState<string | null>(null);
    const [deletingId, setDeletingId] = useState<string | null>(null);
    const [renamingId, setRenamingId] = useState<string | null>(null);

    const refresh = useCallback(async () => {
      setIsListing(true);
      try {
        const result = await callCommand<CollectionRow[]>('collection_list');
        setCollections(result);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setIsListing(false);
      }
    }, []);

    const createCollection = useCallback(
      async (name: string): Promise<string | null> => {
        setCreatingName(name);
        setError(null);
        try {
          const id = await callCommand<string>('collection_create', { name });
          await refresh();
          return id;
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
          return null;
        } finally {
          setCreatingName(null);
        }
      },
      [refresh]
    );

    const deleteCollection = useCallback(
      async (collectionId: string): Promise<boolean> => {
        setDeletingId(collectionId);
        setError(null);
        try {
          await callCommand<null>('collection_delete', { collectionId });
          await refresh();
          return true;
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
          return false;
        } finally {
          setDeletingId(null);
        }
      },
      [refresh]
    );

    const renameCollection = useCallback(
      async (collectionId: string, newName: string): Promise<boolean> => {
        setRenamingId(collectionId);
        setError(null);
        try {
          await callCommand<null>('collection_rename', { collectionId, newName });
          await refresh();
          return true;
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
          return false;
        } finally {
          setRenamingId(null);
        }
      },
      [refresh]
    );

    const updateDescription = useCallback(
      async (collectionId: string, description: string | null): Promise<boolean> => {
        setError(null);
        try {
          await callCommand<null>('collection_update_description', { collectionId, description });
          await refresh();
          return true;
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
          return false;
        }
      },
      [refresh]
    );

    const addProfile = useCallback(
      async (collectionId: string, profileName: string): Promise<boolean> => {
        setError(null);
        try {
          await callCommand<null>('collection_add_profile', { collectionId, profileName });
          await refresh();
          return true;
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
          return false;
        }
      },
      [refresh]
    );

    const removeProfile = useCallback(
      async (collectionId: string, profileName: string): Promise<boolean> => {
        setError(null);
        try {
          await callCommand<null>('collection_remove_profile', { collectionId, profileName });
          await refresh();
          return true;
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
          return false;
        }
      },
      [refresh]
    );

    const listMembers = useCallback(async (collectionId: string): Promise<string[]> => {
      try {
        return await callCommand<string[]>('collection_list_profiles', { collectionId });
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return [];
      }
    }, []);

    const collectionsForProfile = useCallback(async (profileName: string): Promise<CollectionRow[]> => {
      try {
        return await callCommand<CollectionRow[]>('collections_for_profile', { profileName });
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return [];
      }
    }, []);

    useEffect(() => {
      void refresh();
    }, [refresh]);

    return {
      collections,
      error,
      isListing,
      creatingName,
      deletingId,
      renamingId,
      refresh,
      createCollection,
      deleteCollection,
      renameCollection,
      updateDescription,
      addProfile,
      removeProfile,
      listMembers,
      collectionsForProfile,
    };
  }
  ```

- **MIRROR**: `src/crosshook-native/src/hooks/useLauncherManagement.ts:21-101` (single error, per-op busy ids, refresh-after-mutate, boolean returns).
- **IMPORTS**: `useCallback`, `useEffect`, `useState` from React; `callCommand` from `@/lib/ipc`; `CollectionRow` from `../types/collections`.
- **GOTCHA**:
  - **All IPC arg keys are camelCase**: `collectionId`, `profileName`, `newName`. Tauri v2 auto-converts to Rust snake_case. **Don't use `collection_id` here** — that would only work after Task 1's mock fix and would be inconsistent with the rest of the codebase.
  - `collection_delete`, `collection_rename`, etc. all return `Result<(), String>` on the Rust side — JS sees `null` (Tauri's `Result<(), _>` serialization). Type the call as `callCommand<null>(...)` to match.
  - `useEffect(() => { void refresh(); }, [refresh])` is the boot effect. Mirrors `useCommunityProfiles.ts:387-421` and `useProfile.ts:1280-1284`.
  - **Do NOT throw from CRUD callbacks** — the `useLauncherManagement` convention is to return `boolean` and surface errors via `error` state. Callers can react to `error` changing or check the boolean return.
  - `createCollection` returns `string | null` (the new collection id, or `null` on failure). All other CRUD returns `boolean`.
  - `listMembers` and `collectionsForProfile` return data directly (not boolean) because they are read paths. Errors set `error` but don't throw.
  - **Refresh-after-mutate is non-negotiable**: every `add`/`remove`/`rename`/`update`/`delete` call MUST `await refresh()` so `collections.profile_count` stays current (Phase 1 computes this server-side via the `list_collections` SQL).
- **VALIDATE**:
  - `pnpm --filter crosshook-native run build` succeeds (zero TS errors).
  - In `pnpm dev:browser`, mounting a temporary test component that reads `useCollections()` and renders `JSON.stringify(collections)` shows the mock fixture.

### Task 5: Create `useCollectionMembers` hook

- **ACTION**: Create a focused hook that returns the member profile names for a given collection id. This is a thin wrapper around `collection_list_profiles` because the modal needs reactive membership state without round-tripping through `useCollections`.
- **IMPLEMENT**: New file `src/crosshook-native/src/hooks/useCollectionMembers.ts`:

  ```ts
  import { useCallback, useEffect, useState } from 'react';

  import { callCommand } from '@/lib/ipc';

  export interface UseCollectionMembersResult {
    memberNames: string[];
    loading: boolean;
    error: string | null;
    refresh: () => Promise<void>;
  }

  /**
   * Returns the member profile names for a single collection. Refreshes when
   * `collectionId` changes; exposes a manual `refresh()` for parent components
   * that mutate membership and need to re-sync.
   */
  export function useCollectionMembers(collectionId: string | null): UseCollectionMembersResult {
    const [memberNames, setMemberNames] = useState<string[]>([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const refresh = useCallback(async () => {
      if (collectionId === null) {
        setMemberNames([]);
        return;
      }
      setLoading(true);
      try {
        const result = await callCommand<string[]>('collection_list_profiles', { collectionId });
        setMemberNames(result);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        setMemberNames([]);
      } finally {
        setLoading(false);
      }
    }, [collectionId]);

    useEffect(() => {
      void refresh();
    }, [refresh]);

    return { memberNames, loading, error, refresh };
  }
  ```

- **MIRROR**: `src/crosshook-native/src/hooks/useLibrarySummaries.ts:19-68` (lighter version — no ref, no derived state).
- **IMPORTS**: `useCallback`, `useEffect`, `useState`; `callCommand`.
- **GOTCHA**:
  - `collectionId === null` is the "no collection selected" state. Return an empty array without calling IPC. This avoids a noisy mock-handler call when the modal is closed.
  - `useEffect` depends on `refresh` (which depends on `collectionId`) — when the parent passes a new id, the effect re-runs and fetches the new member list.
  - Parent component mutations (`addProfile`, `removeProfile`) must call **both** `useCollections.refresh()` (for `profile_count`) **and** `useCollectionMembers.refresh()` (for the visible member list). The plan does NOT cross-coupling these two hooks; the parent component (modal) coordinates them.
- **VALIDATE**: `pnpm --filter crosshook-native run build` succeeds.

### Task 6: Extend `ProfileContext` with `activeCollectionId`

- **ACTION**: Add `activeCollectionId: string | null` + `setActiveCollectionId` ephemeral state to `ProfileContext` via the merge pattern. Do NOT touch `useProfile`.
- **IMPLEMENT**: In `src/crosshook-native/src/context/ProfileContext.tsx`:

  ```tsx
  // 1. Update the value interface (line 16-20):
  export interface ProfileContextValue extends UseProfileResult {
    launchMethod: ResolvedLaunchMethod;
    steamClientInstallPath: string;
    targetHomePath: string;
    activeCollectionId: string | null;
    setActiveCollectionId: (id: string | null) => void;
  }

  // 2. Add state inside ProfileProvider (after line 32 — after targetHomePath):
  const [activeCollectionId, setActiveCollectionId] = useState<string | null>(null);

  // 3. Merge into the memoized value (replace lines 56-64):
  const value = useMemo<ProfileContextValue>(
    () => ({
      ...profileState,
      launchMethod,
      steamClientInstallPath,
      targetHomePath,
      activeCollectionId,
      setActiveCollectionId,
    }),
    [launchMethod, profileState, steamClientInstallPath, targetHomePath, activeCollectionId]
  );
  ```

  Add `useState` to the React import at the top of the file:

  ```tsx
  import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
  ```

- **MIRROR**: `ProfileContext.tsx:56-64` — the existing memoize pattern. The new state is appended; existing fields are untouched.
- **IMPORTS**: Add `useState` to the React import (line 9).
- **GOTCHA**:
  - **`setActiveCollectionId` is stable** because `useState`'s setter is referentially stable across renders. **Do NOT** wrap it in `useCallback` — it would only add a dependency on an already-stable identity.
  - **Do NOT** memoize `setActiveCollectionId` into the `value` deps array (it's already stable). Only include `activeCollectionId` itself.
  - **State lives in `ProfileProvider`, not in `useProfile`**. This is the safer of the two approaches surfaced in the research — `useProfile` is a 1600-line megahook and Phase 2 collections are not profile CRUD. Adding it here keeps the boundaries clean.
  - **Persistence**: This state is intentionally **not** persisted. PRD says "ephemeral runtime state in `ProfileContext` (resets on app restart)". No `localStorage`, no settings file, no event subscription.
  - **Initial value is `null`**: this means "no collection filter active" (dropdowns show all profiles). Setting it to a collection id triggers the filter.
- **VALIDATE**:
  - `pnpm --filter crosshook-native run build` succeeds (zero TS errors).
  - In a temporary test component, `useProfileContext().activeCollectionId === null` on mount; calling `setActiveCollectionId('some-id')` updates it.

### Task 7: Create `useCollectionViewModalState` hook

- **ACTION**: Create the open/close state hook for the new modal, mirroring `useGameDetailsModalState`.
- **IMPLEMENT**: New file `src/crosshook-native/src/components/collections/useCollectionViewModalState.ts`:

  ```ts
  import { useCallback, useState } from 'react';

  export interface UseCollectionViewModalStateResult {
    open: boolean;
    collectionId: string | null;
    openForCollection: (id: string) => void;
    close: () => void;
  }

  export function useCollectionViewModalState(): UseCollectionViewModalStateResult {
    const [open, setOpen] = useState(false);
    const [collectionId, setCollectionId] = useState<string | null>(null);

    const close = useCallback(() => {
      setOpen(false);
      setCollectionId(null);
    }, []);

    const openForCollection = useCallback((id: string) => {
      setCollectionId(id);
      setOpen(true);
    }, []);

    return { open, collectionId, openForCollection, close };
  }
  ```

- **MIRROR**: `src/crosshook-native/src/components/library/useGameDetailsModalState.ts:1-28` exactly.
- **IMPORTS**: `useCallback`, `useState` from React.
- **GOTCHA**:
  - This is **separate** from `activeCollectionId` in `ProfileContext`. The two state slots have different lifecycles:
    - `openCollectionId` (modal-only) — set when modal opens, cleared on close.
    - `activeCollectionId` (filter context) — set when sidebar collection clicked, **persists** until user clears the filter or app restart.
  - The Sidebar's click handler will set both: `setActiveCollectionId(id); openForCollection(id);`. The modal's close handler will only clear `openForCollection`.
- **VALIDATE**: `pnpm --filter crosshook-native run build` succeeds.

### Task 8: Create `<CollectionViewModal>` component + CSS

- **ACTION**: Create the main view modal that displays a collection's members with search, supports launch/edit through select-then-navigate, and exposes basic metadata actions (rename, edit description, delete).
- **IMPLEMENT**: New file `src/crosshook-native/src/components/collections/CollectionViewModal.tsx`. **Copy `GameDetailsModal.tsx` as a starting template** and adapt the body. Required structure:

  ```tsx
  import { useCallback, useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent } from 'react';
  import { createPortal } from 'react-dom';

  import type { CollectionRow } from '../../types/collections';
  import type { LibraryCardData } from '../../types/library';
  import { useLibraryProfiles } from '../../hooks/useLibraryProfiles';
  import { useLibrarySummaries } from '../../hooks/useLibrarySummaries';
  import { useProfileContext } from '../../context/ProfileContext';
  import { useCollections } from '../../hooks/useCollections';
  import { useCollectionMembers } from '../../hooks/useCollectionMembers';
  import { LibraryCard } from '../library/LibraryCard';
  import { gameDetailsLaunchThenNavigate, gameDetailsEditThenNavigate } from '../library/game-details-actions';

  import './CollectionViewModal.css';

  // 1. Copy FOCUSABLE_SELECTOR, getFocusableElements, focusElement from
  //    GameDetailsModal.tsx:27-49 verbatim.

  export interface CollectionViewModalProps {
    open: boolean;
    collectionId: string | null;
    onClose: () => void;
    onLaunch: (name: string) => void | Promise<void>;
    onEdit: (name: string) => void | Promise<void>;
    onRequestEditMetadata: (id: string) => void; // opens CollectionEditModal
    launchingName?: string;
  }

  export function CollectionViewModal({
    open,
    collectionId,
    onClose,
    onLaunch,
    onEdit,
    onRequestEditMetadata,
    launchingName,
  }: CollectionViewModalProps) {
    const titleId = useId();
    const descriptionId = useId();
    const surfaceRef = useRef<HTMLDivElement | null>(null);
    const headingRef = useRef<HTMLHeadingElement | null>(null);
    const closeButtonRef = useRef<HTMLButtonElement | null>(null);
    const portalHostRef = useRef<HTMLDivElement | null>(null);
    const previouslyFocusedRef = useRef<HTMLElement | null>(null);
    const bodyStyleRef = useRef<string>('');
    const hiddenNodesRef = useRef<Array<{ element: HTMLElement; inert: boolean; ariaHidden: string | null }>>([]);
    const [isMounted, setIsMounted] = useState(false);

    // 2. Copy the portal-host useEffect from GameDetailsModal.tsx:182-194 verbatim.
    // 3. Copy the body-lock + inert-siblings + focus useEffect from
    //    GameDetailsModal.tsx:196-253 verbatim. Replace the dependency array with
    //    `[open, collectionId]` instead of `[open, summary]`.
    // 4. Copy handleKeyDown from GameDetailsModal.tsx:255-294 verbatim.
    // 5. Copy handleBackdropMouseDown from GameDetailsModal.tsx:296-302 verbatim.

    // Data wiring
    const { collections, deleteCollection } = useCollections();
    const {
      memberNames,
      loading: membersLoading,
      refresh: refreshMembers,
    } = useCollectionMembers(open ? collectionId : null);
    const { profiles, favoriteProfiles, selectedProfile } = useProfileContext();
    const { summaries } = useLibrarySummaries(profiles, favoriteProfiles);

    const [searchQuery, setSearchQuery] = useState('');

    // Reset search when modal closes or collection changes
    useEffect(() => {
      setSearchQuery('');
    }, [open, collectionId]);

    const collection = useMemo<CollectionRow | null>(
      () => collections.find((c) => c.collection_id === collectionId) ?? null,
      [collections, collectionId]
    );

    // Filter library summaries to collection members, then apply search
    const memberSet = useMemo(() => new Set(memberNames), [memberNames]);
    const memberSummaries = useMemo<LibraryCardData[]>(
      () => summaries.filter((s) => memberSet.has(s.name)),
      [summaries, memberSet]
    );
    const filtered = useLibraryProfiles(memberSummaries, searchQuery);

    const handleLaunchClick = useCallback(
      (name: string) => {
        gameDetailsLaunchThenNavigate(name, onLaunch, onClose);
      },
      [onLaunch, onClose]
    );

    const handleEditClick = useCallback(
      (name: string) => {
        gameDetailsEditThenNavigate(name, onEdit, onClose);
      },
      [onEdit, onClose]
    );

    const handleRemoveMember = useCallback(
      async (name: string) => {
        if (collectionId === null) return;
        // Note: removeProfile is on useCollections; the modal calls it here and
        // refreshes both the parent collection list and the visible member list.
        // Parent: handled by useCollections internally. Children: refreshMembers.
        // ... implementation calls useCollections().removeProfile(...) then await refreshMembers()
      },
      [collectionId, refreshMembers]
    );

    const handleDeleteCollection = useCallback(async () => {
      if (collectionId === null) return;
      const ok = await deleteCollection(collectionId);
      if (ok) {
        onClose();
      }
    }, [collectionId, deleteCollection, onClose]);

    if (!open || !collection || !isMounted || !portalHostRef.current) {
      return null;
    }

    return createPortal(
      <div className="crosshook-modal" role="presentation">
        <div className="crosshook-modal__backdrop" aria-hidden="true" onMouseDown={handleBackdropMouseDown} />
        <div
          ref={surfaceRef}
          className="crosshook-modal__surface crosshook-panel crosshook-focus-scope crosshook-collection-modal"
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
          aria-describedby={descriptionId}
          data-crosshook-focus-root="modal"
          onKeyDown={handleKeyDown}
        >
          <header className="crosshook-modal__header">
            <div className="crosshook-modal__heading-block">
              <h2 ref={headingRef} id={titleId} className="crosshook-modal__title" tabIndex={-1}>
                {collection.name}
              </h2>
              <p id={descriptionId} className="crosshook-modal__description">
                {collection.profile_count} profile{collection.profile_count === 1 ? '' : 's'}
                {collection.description ? ` · ${collection.description}` : ''}
              </p>
            </div>
            <div className="crosshook-modal__header-actions">
              <button
                type="button"
                className="crosshook-button crosshook-button--ghost"
                onClick={() => onRequestEditMetadata(collection.collection_id)}
              >
                Edit
              </button>
              <button
                ref={closeButtonRef}
                type="button"
                className="crosshook-button crosshook-button--ghost crosshook-modal__close"
                data-crosshook-modal-close
                onClick={onClose}
              >
                Close
              </button>
            </div>
          </header>

          <div className="crosshook-modal__body crosshook-collection-modal__body">
            <div className="crosshook-collection-modal__search">
              <label className="crosshook-label" htmlFor={`${titleId}-search`}>
                Search this collection
              </label>
              <input
                id={`${titleId}-search`}
                type="text"
                className="crosshook-input"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Type to filter…"
                aria-controls={`${titleId}-results`}
              />
            </div>

            {membersLoading ? (
              <p className="crosshook-collection-modal__status">Loading members…</p>
            ) : filtered.length === 0 ? (
              <div className="crosshook-collection-modal__empty">
                {memberSummaries.length === 0
                  ? 'No profiles in this collection yet. Right-click a library card to add one.'
                  : 'No profiles match your search.'}
              </div>
            ) : (
              <div id={`${titleId}-results`} className="crosshook-collection-modal__grid" role="list">
                {filtered.map((card) => (
                  <LibraryCard
                    key={card.name}
                    profile={card}
                    isSelected={selectedProfile === card.name}
                    onOpenDetails={() => {
                      // Phase 2: clicking the hitbox launches via select-then-navigate.
                      // Phase 4 may upgrade to open GameDetailsModal-in-collection-context.
                      handleLaunchClick(card.name);
                    }}
                    onLaunch={handleLaunchClick}
                    onEdit={handleEditClick}
                    onToggleFavorite={() => {
                      // No-op in Phase 2; favorites are managed from LibraryPage.
                    }}
                    isLaunching={launchingName === card.name}
                  />
                ))}
              </div>
            )}
          </div>

          <footer className="crosshook-modal__footer">
            <div className="crosshook-modal__footer-actions">
              <button
                type="button"
                className="crosshook-button crosshook-button--danger"
                onClick={() => void handleDeleteCollection()}
              >
                Delete collection
              </button>
              <button type="button" className="crosshook-button" onClick={onClose}>
                Done
              </button>
            </div>
          </footer>
        </div>
      </div>,
      portalHostRef.current
    );
  }
  ```

  And the new CSS file `src/crosshook-native/src/components/collections/CollectionViewModal.css`:

  ```css
  .crosshook-collection-modal__body {
    overscroll-behavior: contain;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .crosshook-collection-modal__search {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .crosshook-collection-modal__grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 12px;
  }

  .crosshook-collection-modal__empty {
    padding: 24px;
    text-align: center;
    color: var(--crosshook-color-text-muted);
    border: 1px dashed rgba(255, 255, 255, 0.1);
    border-radius: var(--crosshook-radius-md);
  }

  .crosshook-collection-modal__status {
    padding: 24px;
    text-align: center;
    color: var(--crosshook-color-text-muted);
  }
  ```

- **MIRROR**: `src/crosshook-native/src/components/library/GameDetailsModal.tsx:1-487` (entire file). The portal mount, body lock, focus trap, Tab handling, backdrop click are copied verbatim. **Only the body content is replaced** with the search input + member grid.
- **IMPORTS**: see snippet above.
- **GOTCHA**:
  - **`overscroll-behavior: contain` MUST be set** on the body class because `crosshook-modal__body` already participates in WebKitGTK enhanced scroll via `useScrollEnhance.ts:9`. Without it, scroll-chaining flicks the parent.
  - **`crosshook-modal__body` is the `overflow-y: auto` container** — do NOT add a second `overflow-y: auto` div inside it without also registering the new class in `useScrollEnhance.ts`.
  - **`data-crosshook-modal-close` and `data-crosshook-focus-root="modal"` are mandatory** for `App.tsx:39-45` `handleGamepadBack` to work with controllers.
  - **`useId` for `aria-labelledby`/`aria-describedby`** — do NOT hardcode IDs (would collide if two collection modals could ever stack).
  - **`isMounted && portalHostRef.current` guard** before rendering — the portal host is created in a `useEffect`, so the first render returns `null`. Mirrors `GameDetailsModal.tsx:184-194, 304-308`.
  - **`useLibraryProfiles` is composed inside the modal** — it's a pure memo, no IPC. Safe to call.
  - **Reuse `<LibraryCard>` directly** — don't recreate the card UI. The `onOpenDetails` callback in this context launches via select-then-navigate (see Notes); future Phase 4 work may upgrade the click target to open `<GameDetailsModal>` in collection-context, but Phase 2 keeps it simple.
  - **`onToggleFavorite` is a no-op** in this modal — favorites are managed from LibraryPage. Pass `() => {}` to avoid breaking the LibraryCard prop contract. Document this in a comment.
  - **`handleRemoveMember` is sketched but not wired** to a UI affordance in this snippet. Phase 2 should add a small "Remove from collection" affordance — either as a per-card overflow menu, or as a separate "Manage members" mode. For the single-pass plan, **skip the per-card remove button** and rely on the right-click context menu (Task 11) for un-checking. Add a TODO comment in the code or file a follow-up issue.
  - **`handleDeleteCollection` calls `useCollections().deleteCollection()` then `onClose()`** — refresh-after-mutate happens inside `useCollections`.
  - **Focus restoration**: `headingRef.current ?? closeButtonRef.current` is the initial focus target. The `headingRef` element MUST have `tabIndex={-1}` for `focusElement` to work on a non-focusable heading.
  - **Search input clears on modal close or collection change** via the `useEffect([open, collectionId])` reset.
- **VALIDATE**:
  - `pnpm --filter crosshook-native run build` succeeds.
  - In `pnpm dev:browser`, after Task 13/14 wire it up, opening the modal renders the seed `mock-collection-1` collection name "Action / Adventure", search input is focusable via Tab, Esc closes the modal, click outside closes the modal.

### Task 9: Create `<CollectionEditModal>` (lighter modal — create / rename / edit description)

- **ACTION**: A separate, smaller modal for the create/rename/edit-description forms. Mirrors `OfflineTrainerInfoModal` (the "lite" portal modal pattern with no portal host node).
- **IMPLEMENT**: New file `src/crosshook-native/src/components/collections/CollectionEditModal.tsx`:

  Required props:

  ```ts
  export type CollectionEditMode = 'create' | 'edit';

  export interface CollectionEditModalProps {
    open: boolean;
    mode: CollectionEditMode;
    initialName?: string;
    initialDescription?: string | null;
    collectionId?: string; // present in 'edit' mode
    onClose: () => void;
    onSubmitCreate: (name: string, description: string | null) => Promise<boolean>;
    onSubmitEdit: (name: string, description: string | null) => Promise<boolean>;
  }
  ```

  Body structure:
  - `<input>` for collection name (required, trim, validate non-empty inline)
  - `<textarea>` for description (optional)
  - Submit button: disabled while busy
  - Inline error: `<p className="crosshook-collection-edit-modal__warn">` rendered when `error !== null`
  - Mode 'create' → "Create collection" title; 'edit' → "Edit collection" title
  - Submit calls `onSubmitCreate` or `onSubmitEdit`; on `true` return, calls `onClose()`; on `false`, leaves modal open with error visible
  - Same focus-trap + Esc-to-close + body-lock pattern, copied from `OfflineTrainerInfoModal.tsx:106-176` (the lite version — no inert siblings, no portal host node, just `createPortal(node, document.body)`).

- **MIRROR**: `src/crosshook-native/src/components/OfflineTrainerInfoModal.tsx:1-177` for the modal shell. `LauncherPreviewModal` for the form pattern (if it has one).
- **IMPORTS**: `useCallback`, `useEffect`, `useId`, `useRef`, `useState`, `type KeyboardEvent`, `type MouseEvent` from React; `createPortal` from `react-dom`.
- **GOTCHA**:
  - **Inline duplicate-name handling**: when `onSubmitCreate` returns `false`, the error in `useCollections.error` already contains the duplicate-name message. Display it via the parent's `error` prop OR re-call `onSubmitCreate` and surface the catch result. **Recommended**: have the parent pass the `error` from `useCollections` down via prop, and reset to `null` when the user starts typing.
  - **`open && initialName` are independent** — entering 'create' mode with `initialName=''` is the create-new-collection flow; entering 'edit' mode with a populated name is the rename-existing flow.
  - **Description normalization**: send `description || null` to the IPC (an empty string becomes `null` per `update_collection_description` Phase 1 normalization).
  - **`data-crosshook-modal-close` on the close button** for gamepad back support.
  - **Use the lite OfflineTrainerInfoModal pattern** instead of GameDetailsModal — fewer moving parts, lower copy-paste risk, and the modal is much smaller.
- **VALIDATE**:
  - `pnpm --filter crosshook-native run build` succeeds.
  - In `pnpm dev:browser`, after Task 10/13 wire it up, the create form opens, accepts a name, submits, and closes; an empty name shows an inline error; a duplicate name shows the IPC's error message inline.

### Task 10: Create `<CollectionsSidebar>` component

- **ACTION**: The sidebar section that mounts inside `Sidebar.tsx`'s nav region. Renders chip-list when collections exist; renders only the "+ New collection" CTA button when empty.
- **IMPLEMENT**: New file `src/crosshook-native/src/components/collections/CollectionsSidebar.tsx`:

  ```tsx
  import { useCallback, useState } from 'react';

  import { useCollections } from '../../hooks/useCollections';
  import { CollectionEditModal } from './CollectionEditModal';

  export interface CollectionsSidebarProps {
    onOpenCollection: (id: string) => void;
  }

  export function CollectionsSidebar({ onOpenCollection }: CollectionsSidebarProps) {
    const { collections, createCollection, error } = useCollections();
    const [createOpen, setCreateOpen] = useState(false);

    const handleCreate = useCallback(
      async (name: string, description: string | null): Promise<boolean> => {
        const id = await createCollection(name);
        if (id !== null && description) {
          // Optionally call updateDescription here, but Phase 2 keeps the
          // create-then-immediately-edit flow simple: created with name only,
          // description set in a follow-up edit.
        }
        return id !== null;
      },
      [createCollection]
    );

    const handleClickCollection = useCallback(
      (id: string) => {
        onOpenCollection(id);
      },
      [onOpenCollection]
    );

    return (
      <>
        <div className="crosshook-sidebar__section crosshook-collections-sidebar">
          {collections.length > 0 && (
            <>
              <div className="crosshook-sidebar__section-label">Collections</div>
              <div className="crosshook-sidebar__section-items crosshook-collections-sidebar__list" role="list">
                {collections.map((c) => (
                  <button
                    key={c.collection_id}
                    type="button"
                    className="crosshook-sidebar__item crosshook-collections-sidebar__item"
                    onClick={() => handleClickCollection(c.collection_id)}
                    title={c.name}
                  >
                    <span className="crosshook-collections-sidebar__item-name">{c.name}</span>
                    <span
                      className="crosshook-collections-sidebar__item-count"
                      aria-label={`${c.profile_count} profiles`}
                    >
                      {c.profile_count}
                    </span>
                  </button>
                ))}
              </div>
            </>
          )}

          <button
            type="button"
            className="crosshook-sidebar__item crosshook-collections-sidebar__cta"
            onClick={() => setCreateOpen(true)}
          >
            <span className="crosshook-sidebar__item-icon" aria-hidden="true">
              +
            </span>
            <span className="crosshook-sidebar__item-label">
              {collections.length === 0 ? 'Create your first collection' : 'New collection'}
            </span>
          </button>

          {error !== null && (
            <p className="crosshook-collections-sidebar__error" role="alert">
              {error}
            </p>
          )}
        </div>

        <CollectionEditModal
          open={createOpen}
          mode="create"
          onClose={() => setCreateOpen(false)}
          onSubmitCreate={handleCreate}
          onSubmitEdit={async () => false /* not used in create mode */}
        />
      </>
    );
  }
  ```

- **MIRROR**: `Sidebar.tsx:134-151` for section structure; `PinnedProfilesStrip.tsx:14-58` for the "return null when empty / minimal CTA when empty" idiom.
- **IMPORTS**: see snippet.
- **GOTCHA**:
  - **Plain `<button>` elements, NOT `Tabs.Trigger`**. Collection items don't navigate; they open a modal.
  - **Always render the "+ New collection" button** — even when `collections.length === 0` — because that's the empty-state CTA. The chip list is conditional on `collections.length > 0`.
  - **The button reuses `crosshook-sidebar__item` class** so visual height/padding match other sidebar entries. The CSS in Task 21 adds the count badge styling.
  - **`useCollections()` is called inside this component** — but `useCollections()` is also called inside `<CollectionViewModal>` (Task 8). This is FINE because each `useCollections()` call has its own state slot (each component reads/writes its own copy). For Phase 2 this is acceptable; if `profile_count` mismatches between sidebar and modal become noticeable in practice, hoist `useCollections()` to a `CollectionsContext` (Phase 5 polish).
  - **Each chip click sets BOTH `activeCollectionId` (filter) AND `openCollectionId` (modal)** — but this component only knows about `onOpenCollection`. The parent (Sidebar / AppShell) is responsible for setting `activeCollectionId` from `ProfileContext` when invoking the callback. Pattern: `onOpenCollection={(id) => { setActiveCollectionId(id); openModal(id); }}` in App.tsx.
  - **`error` is a string when present** — display it inline below the CTA. No toast.
- **VALIDATE**: `pnpm --filter crosshook-native run build` succeeds.

### Task 11: Create `<CollectionAssignMenu>` (right-click context menu)

- **ACTION**: A portal-rendered popover that appears at the cursor position on right-click of a `<LibraryCard>`. Shows checkboxes for every collection (multi-select) plus a "+ New collection" link.
- **IMPLEMENT**: New file `src/crosshook-native/src/components/collections/CollectionAssignMenu.tsx`:

  ```tsx
  import { useCallback, useEffect, useRef, useState } from 'react';
  import { createPortal } from 'react-dom';

  import { useCollections } from '../../hooks/useCollections';

  export interface CollectionAssignMenuProps {
    open: boolean;
    profileName: string | null;
    anchorPosition: { x: number; y: number } | null;
    onClose: () => void;
    onCreateNew: () => void;
  }

  export function CollectionAssignMenu({
    open,
    profileName,
    anchorPosition,
    onClose,
    onCreateNew,
  }: CollectionAssignMenuProps) {
    const { collections, addProfile, removeProfile, collectionsForProfile } = useCollections();
    const [memberOf, setMemberOf] = useState<Set<string>>(new Set());
    const [busy, setBusy] = useState(false);
    const popoverRef = useRef<HTMLDivElement | null>(null);

    // Load reverse-lookup when opened
    useEffect(() => {
      if (!open || profileName === null) return;
      let active = true;
      void (async () => {
        const result = await collectionsForProfile(profileName);
        if (active) {
          setMemberOf(new Set(result.map((c) => c.collection_id)));
        }
      })();
      return () => {
        active = false;
      };
    }, [open, profileName, collectionsForProfile]);

    // Click-outside / Esc to close
    useEffect(() => {
      if (!open) return;
      function onKeyDown(e: KeyboardEvent) {
        if (e.key === 'Escape') {
          e.stopPropagation();
          onClose();
        }
      }
      function onPointerDown(e: PointerEvent) {
        if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
          onClose();
        }
      }
      document.addEventListener('keydown', onKeyDown);
      document.addEventListener('pointerdown', onPointerDown, true);
      return () => {
        document.removeEventListener('keydown', onKeyDown);
        document.removeEventListener('pointerdown', onPointerDown, true);
      };
    }, [open, onClose]);

    const handleToggle = useCallback(
      async (collectionId: string, currentlyMember: boolean) => {
        if (profileName === null) return;
        setBusy(true);
        const ok = currentlyMember
          ? await removeProfile(collectionId, profileName)
          : await addProfile(collectionId, profileName);
        if (ok) {
          setMemberOf((prev) => {
            const next = new Set(prev);
            if (currentlyMember) {
              next.delete(collectionId);
            } else {
              next.add(collectionId);
            }
            return next;
          });
        }
        setBusy(false);
      },
      [profileName, addProfile, removeProfile]
    );

    if (!open || anchorPosition === null || profileName === null) {
      return null;
    }

    // Clamp position to viewport bounds (basic)
    const style: React.CSSProperties = {
      position: 'fixed',
      left: Math.min(anchorPosition.x, window.innerWidth - 280),
      top: Math.min(anchorPosition.y, window.innerHeight - 320),
      zIndex: 1300,
    };

    return createPortal(
      <div
        ref={popoverRef}
        className="crosshook-collection-assign-menu"
        role="menu"
        aria-label={`Add ${profileName} to collection`}
        style={style}
      >
        <div className="crosshook-collection-assign-menu__header">Add to collection</div>
        {collections.length === 0 ? (
          <p className="crosshook-collection-assign-menu__empty">No collections yet.</p>
        ) : (
          <div className="crosshook-collection-assign-menu__list" role="group">
            {collections.map((c) => {
              const isMember = memberOf.has(c.collection_id);
              return (
                <label key={c.collection_id} className="crosshook-collection-assign-menu__option">
                  <input
                    type="checkbox"
                    checked={isMember}
                    disabled={busy}
                    onChange={() => void handleToggle(c.collection_id, isMember)}
                  />
                  <span className="crosshook-collection-assign-menu__option-name">{c.name}</span>
                </label>
              );
            })}
          </div>
        )}
        <div className="crosshook-collection-assign-menu__divider" />
        <button
          type="button"
          className="crosshook-collection-assign-menu__create"
          onClick={() => {
            onClose();
            onCreateNew();
          }}
        >
          + New collection…
        </button>
      </div>,
      document.body
    );
  }
  ```

  CSS additions (to the new collections CSS file or theme.css):

  ```css
  .crosshook-collection-assign-menu {
    background: var(--crosshook-color-surface);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: var(--crosshook-radius-md);
    box-shadow: var(--crosshook-shadow-lg);
    padding: 8px;
    min-width: 240px;
    max-height: 320px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .crosshook-collection-assign-menu__header {
    font-size: 0.75rem;
    text-transform: uppercase;
    color: var(--crosshook-color-text-muted);
    padding: 4px 8px;
  }
  .crosshook-collection-assign-menu__list {
    overflow-y: auto;
    max-height: 220px;
    overscroll-behavior: contain;
  }
  .crosshook-collection-assign-menu__option {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: var(--crosshook-radius-sm);
    cursor: pointer;
  }
  .crosshook-collection-assign-menu__option:hover {
    background: rgba(255, 255, 255, 0.05);
  }
  .crosshook-collection-assign-menu__divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
    margin: 4px 0;
  }
  .crosshook-collection-assign-menu__create {
    background: transparent;
    border: none;
    color: var(--crosshook-color-accent);
    text-align: left;
    padding: 6px 8px;
    cursor: pointer;
    border-radius: var(--crosshook-radius-sm);
  }
  .crosshook-collection-assign-menu__create:hover {
    background: rgba(255, 255, 255, 0.05);
  }
  .crosshook-collection-assign-menu__empty {
    padding: 8px;
    color: var(--crosshook-color-text-muted);
    font-style: italic;
  }
  ```

- **MIRROR**: There is no existing context menu in the codebase. The closest pattern is `OfflineTrainerInfoModal.tsx`'s minimal `createPortal` shell. Borrow click-outside / Esc handling from `App.tsx:39-45` style.
- **IMPORTS**: see snippet.
- **GOTCHA**:
  - **Click-outside detection** uses `pointerdown` capture phase so it fires before any inner click handler. **Do NOT use `mousedown` or `click`** — `pointerdown` is the correct primitive for popover dismissal.
  - **`onPointerDown` ignores clicks on `popoverRef.current`** to avoid closing on internal interactions.
  - **`anchorPosition` clamping** keeps the popover on-screen when the user right-clicks near the right edge / bottom edge. The math is approximate (`window.innerWidth - 280`); a more sophisticated layout flip is out of scope for v1.
  - **`collectionsForProfile` is called on open** to populate the initial checkbox state. The request happens once per open; toggles are optimistic (local set update + IPC call).
  - **Optimistic toggle**: add/remove the id from `memberOf` only after the IPC call returns `true`. Failures leave the checkbox visually consistent with the actual state.
  - **The `busy` flag disables all checkboxes during a single mutation** — keeps the UX simple. A per-row busy state is fine but adds complexity for marginal benefit.
  - **`onCreateNew` is a callback to the parent** — Phase 2 wires it to "open `<CollectionEditModal>` in create mode". The parent is `LibraryPage` (Task 17).
  - **The right-click anchor position must be computed from the `MouseEvent` in `onContextMenu`** — see Task 15.
  - **`role="menu"` + `role="group"` + `aria-label`** for accessibility. Each `<label>` wraps an `<input type="checkbox">` for default keyboard support.
  - **`.crosshook-collection-assign-menu__list` has its own `overflow-y: auto`** — register this class in `useScrollEnhance.ts` SCROLLABLE selector (Task 20) OR rely on the popover being short enough that scroll-enhancement is not needed. For ≤10 collections, no scroll. For ≥20, enhance.
  - **Steam Deck note**: the `onContextMenu` event fires on pointer/touch right-click and on certain controller mappings. Phase 5 will validate the controller flow; v1 ships with right-click only.
- **VALIDATE**:
  - `pnpm --filter crosshook-native run build` succeeds.
  - In `pnpm dev:browser`, right-clicking a library card opens the popover, checkboxes reflect membership, toggling adds/removes via IPC and refreshes the seed fixture, Esc closes the popover.

### Task 12: Update `<LibraryCard>` to forward `onContextMenu`

- **ACTION**: Add an optional `onContextMenu` prop to `LibraryCard` that the parent (`LibraryGrid` → `LibraryPage`) can pass through.
- **IMPLEMENT**: In `src/crosshook-native/src/components/library/LibraryCard.tsx`:
  1. Add to `LibraryCardProps` (around line 6-14):

     ```ts
     interface LibraryCardProps {
       profile: LibraryCardData;
       isSelected?: boolean;
       onOpenDetails: LibraryOpenDetailsHandler;
       onLaunch: (name: string) => void;
       onEdit: (name: string) => void;
       onToggleFavorite: (name: string, current: boolean) => void;
       isLaunching?: boolean;
       onContextMenu?: (event: React.MouseEvent<HTMLDivElement>, profileName: string) => void;
     }
     ```

  2. Destructure it from props:

     ```tsx
     export function LibraryCard({
       profile,
       isSelected,
       onOpenDetails,
       onLaunch,
       onEdit,
       onToggleFavorite,
       isLaunching,
       onContextMenu,
     }: LibraryCardProps) {
     ```

  3. Forward it to the root `<div>` (line 70-71):

     ```tsx
     return (
       <div
         ref={cardRef}
         className={cardClass}
         role="listitem"
         onContextMenu={
           onContextMenu
             ? (e) => {
                 e.preventDefault();
                 onContextMenu(e, profile.name);
               }
             : undefined
         }
       >
     ```

- **MIRROR**: The existing event handler patterns on the card (`handleOpenDetailsClick` at line 66).
- **IMPORTS**: `React.MouseEvent` is already implicitly available; no new imports.
- **GOTCHA**:
  - **`e.preventDefault()` is mandatory** — without it, the browser's native context menu opens on top of the new popover.
  - **The handler is optional** — when not provided, the right-click does nothing (current behavior). This is backwards-compatible: `LibraryCard` is reused inside `<CollectionViewModal>` (Task 8) and that usage does NOT pass `onContextMenu` (no nested context menus).
  - **Pass the original `MouseEvent`** to the parent so it can extract `clientX`/`clientY` for the popover anchor position.
  - **Do NOT** add `onContextMenu` to the inner buttons (Launch, Favorite, Edit) — only the root div.
- **VALIDATE**: `pnpm --filter crosshook-native run build` succeeds. In `pnpm dev:browser`, right-clicking a card on the Library page (after Task 17 wires it up) opens the popover at the cursor position.

### Task 13: Update `<LibraryGrid>` to pass through `onContextMenu`

- **ACTION**: Add `onContextMenu` to `LibraryGridProps` and pass it through to each `<LibraryCard>`.
- **IMPLEMENT**: In `src/crosshook-native/src/components/library/LibraryGrid.tsx`:
  1. Extend the props interface to include `onContextMenu?: (event: React.MouseEvent<HTMLDivElement>, profileName: string) => void;`.
  2. In the `LibraryCard` mapping (around line 43-55), pass `onContextMenu={onContextMenu}` through.

- **MIRROR**: The existing prop pass-through pattern in the same file.
- **IMPORTS**: none new.
- **GOTCHA**: This is a pure pass-through. No event manipulation here.
- **VALIDATE**: `pnpm --filter crosshook-native run build` succeeds.

### Task 14: Update `<Sidebar>` to mount `<CollectionsSidebar>`

- **ACTION**: Add `onOpenCollection` prop to `SidebarProps`; render `<CollectionsSidebar onOpenCollection={onOpenCollection} />` after the existing `SIDEBAR_SECTIONS` map.
- **IMPLEMENT**: In `src/crosshook-native/src/components/layout/Sidebar.tsx`:
  1. Add to `SidebarProps` (line 18-23):

     ```ts
     export interface SidebarProps {
       activeRoute: AppRoute;
       onNavigate: (route: AppRoute) => void;
       controllerMode: boolean;
       lastProfile: string;
       onOpenCollection: (id: string) => void;
     }
     ```

  2. Destructure from props:

     ```tsx
     export function Sidebar({
       activeRoute,
       onNavigate,
       controllerMode,
       lastProfile,
       onOpenCollection,
     }: SidebarProps) {
     ```

  3. Import the new component at the top:

     ```tsx
     import { CollectionsSidebar } from '../collections/CollectionsSidebar';
     ```

  4. Render `<CollectionsSidebar>` inside `<Tabs.List>` after the `SIDEBAR_SECTIONS.map(...)` block (after line 151, before the closing `</Tabs.List>` divider into footer area):

     ```tsx
       </Tabs.List>
     ```

     becomes:

     ```tsx
         <CollectionsSidebar onOpenCollection={onOpenCollection} />

         <div className="crosshook-sidebar__footer">
           {/* ... existing footer ... */}
         </div>
       </Tabs.List>
     ```

     Insert the `<CollectionsSidebar>` immediately after the `SIDEBAR_SECTIONS.map(...)` closing `)` and before `<div className="crosshook-sidebar__footer">`.

- **MIRROR**: The existing section render loop in `Sidebar.tsx:135-151`.
- **IMPORTS**: Add `CollectionsSidebar` import.
- **GOTCHA**:
  - **`<CollectionsSidebar>` mounts inside `<Tabs.List>`** — Radix tolerates non-trigger children (the `<div className="crosshook-sidebar__footer">` already does this). If Radix complains in practice, move `<CollectionsSidebar>` outside `</Tabs.List>` but inside `<aside>`.
  - **`onOpenCollection` is required** — remove the `?` from the prop. Phase 2 always wires it; tests must update if they construct `<Sidebar>` directly.
  - **Smoke test compatibility**: `tests/smoke.spec.ts` constructs `<App>` not `<Sidebar>`, so adding a prop to Sidebar does not directly break it. But the test asserts no `console.error` on boot — verify the new section does not log errors.
- **VALIDATE**:
  - `pnpm --filter crosshook-native run build` succeeds.
  - `pnpm test:smoke` still passes (no new console errors during route walk).

### Task 15: Wire `<CollectionViewModal>` mount + `openCollectionId` state in `App.tsx`

- **ACTION**: In `AppShell` (`App.tsx:62-142`), add `useCollectionViewModalState`, mount `<CollectionViewModal>` and `<CollectionEditModal>` (for "Edit" button on the view modal), pass `onOpenCollection` to `<Sidebar>`, and wire the modal callbacks. Also add `selectProfile` + `onNavigate` for the launch/edit handlers.
- **IMPLEMENT**: In `src/crosshook-native/src/App.tsx`, edit `AppShell`:
  1. Add imports:

     ```tsx
     import { CollectionViewModal } from './components/collections/CollectionViewModal';
     import { CollectionEditModal } from './components/collections/CollectionEditModal';
     import { useCollectionViewModalState } from './components/collections/useCollectionViewModalState';
     import { useCollections } from './hooks/useCollections';
     ```

  2. Inside `AppShell`, after the existing hooks:

     ```tsx
     const { profileName, selectedProfile, selectProfile, setActiveCollectionId } = useProfileContext();
     // ... existing state ...

     const collectionViewModal = useCollectionViewModalState();
     const { renameCollection, updateDescription, collections } = useCollections();
     const [editingCollectionId, setEditingCollectionId] = useState<string | null>(null);
     const editingCollection = useMemo(
       () =>
         editingCollectionId === null
           ? null
           : (collections.find((c) => c.collection_id === editingCollectionId) ?? null),
       [collections, editingCollectionId]
     );

     const handleOpenCollection = useCallback(
       (id: string) => {
         setActiveCollectionId(id);
         collectionViewModal.openForCollection(id);
       },
       [collectionViewModal, setActiveCollectionId]
     );

     const handleLaunchFromCollection = useCallback(
       async (name: string) => {
         await selectProfile(name);
         setRoute('launch');
       },
       [selectProfile]
     );

     const handleEditFromCollection = useCallback(
       async (name: string) => {
         await selectProfile(name);
         setRoute('profiles');
       },
       [selectProfile]
     );

     const handleRequestEditMetadata = useCallback((id: string) => {
       setEditingCollectionId(id);
     }, []);

     const handleSubmitEdit = useCallback(
       async (name: string, description: string | null): Promise<boolean> => {
         if (editingCollectionId === null) return false;
         const renamed = await renameCollection(editingCollectionId, name);
         if (!renamed) return false;
         await updateDescription(editingCollectionId, description);
         setEditingCollectionId(null);
         return true;
       },
       [editingCollectionId, renameCollection, updateDescription]
     );
     ```

  3. Pass the new prop to `<Sidebar>`:

     ```tsx
     <Sidebar
       activeRoute={route}
       onNavigate={setRoute}
       controllerMode={controllerMode}
       lastProfile={lastProfile}
       onOpenCollection={handleOpenCollection}
     />
     ```

  4. Mount the modals after `</Tabs.Root>` (alongside the existing `<OnboardingWizard>` mount):

     ```tsx
     <CollectionViewModal
       open={collectionViewModal.open}
       collectionId={collectionViewModal.collectionId}
       onClose={collectionViewModal.close}
       onLaunch={handleLaunchFromCollection}
       onEdit={handleEditFromCollection}
       onRequestEditMetadata={handleRequestEditMetadata}
     />
     <CollectionEditModal
       open={editingCollection !== null}
       mode="edit"
       initialName={editingCollection?.name ?? ''}
       initialDescription={editingCollection?.description ?? null}
       collectionId={editingCollection?.collection_id}
       onClose={() => setEditingCollectionId(null)}
       onSubmitCreate={async () => false /* not used in edit mode */}
       onSubmitEdit={handleSubmitEdit}
     />
     ```

  5. Add `useCallback`, `useMemo`, `useState` to the React import at the top if not already present.

- **MIRROR**: `App.tsx:131-137` for the `<OnboardingWizard>` mount as a sibling of `<Tabs.Root>`.
- **IMPORTS**: see snippet.
- **GOTCHA**:
  - **`useCollections()` is called inside `AppShell`** — this triggers the boot fetch. Sidebar (`<CollectionsSidebar>`) also calls `useCollections()` — they each have their own state, but both refresh on mount. For Phase 2 this is acceptable; future polish would lift to a shared `CollectionsContext`.
  - **`activeCollectionId` is set on every sidebar click** — clicking a different collection updates the filter; clicking the same collection just re-opens the modal (idempotent).
  - **`activeCollectionId` does NOT clear on modal close** — that's the PRD-specified persistence: filter survives modal close.
  - **`handleLaunchFromCollection` mirrors `LibraryPage.handleLaunch`** — same pattern, no `setLaunchingName` (the modal owns its own busy state, but Phase 2 ships without per-card launch indicator inside the collection modal).
  - **`<CollectionEditModal open={editingCollection !== null}>`** — a derived boolean. The modal opens whenever there's an edit target. Closing the modal sets `editingCollectionId` to `null`.
  - **The `<CollectionEditModal>` for the create flow lives inside `<CollectionsSidebar>`** (Task 10). The `<CollectionEditModal>` here is for the **edit** flow only.
  - **`useId()` is unused at the AppShell level** — not needed.
- **VALIDATE**:
  - `pnpm --filter crosshook-native run build` succeeds.
  - In `pnpm dev:browser`: clicking the seed `mock-collection-1` chip in the sidebar opens the modal; clicking "Edit" opens the edit form pre-filled; submitting renames; closing the modal returns focus to the chip.

### Task 16: Wire `<CollectionAssignMenu>` from `LibraryPage`

- **ACTION**: In `LibraryPage`, add state for the assign menu (open, profileName, anchorPosition), handle the right-click event from `<LibraryCard>`, and pass through to `<LibraryGrid>`.
- **IMPLEMENT**: In `src/crosshook-native/src/components/pages/LibraryPage.tsx`:
  1. Add imports:

     ```tsx
     import { CollectionAssignMenu } from '../collections/CollectionAssignMenu';
     import { CollectionEditModal } from '../collections/CollectionEditModal';
     import { useCollections } from '../../hooks/useCollections';
     ```

  2. Add state inside the component:

     ```tsx
     const [assignMenuState, setAssignMenuState] = useState<{
       open: boolean;
       profileName: string | null;
       anchorPosition: { x: number; y: number } | null;
     }>({ open: false, profileName: null, anchorPosition: null });

     const [createCollectionFromMenuOpen, setCreateCollectionFromMenuOpen] = useState(false);
     const { createCollection } = useCollections();

     const handleCardContextMenu = useCallback((event: React.MouseEvent<HTMLDivElement>, profileName: string) => {
       setAssignMenuState({
         open: true,
         profileName,
         anchorPosition: { x: event.clientX, y: event.clientY },
       });
     }, []);

     const closeAssignMenu = useCallback(() => {
       setAssignMenuState({ open: false, profileName: null, anchorPosition: null });
     }, []);

     const handleCreateFromAssignMenu = useCallback(() => {
       setCreateCollectionFromMenuOpen(true);
     }, []);

     const handleSubmitCreateFromMenu = useCallback(
       async (name: string, _description: string | null): Promise<boolean> => {
         const id = await createCollection(name);
         return id !== null;
       },
       [createCollection]
     );
     ```

  3. Pass `onContextMenu={handleCardContextMenu}` to `<LibraryGrid>` (around line 127-137).

  4. Mount the assign menu and the (separate) create modal after `<GameDetailsModal>`:

     ```tsx
     <CollectionAssignMenu
       open={assignMenuState.open}
       profileName={assignMenuState.profileName}
       anchorPosition={assignMenuState.anchorPosition}
       onClose={closeAssignMenu}
       onCreateNew={handleCreateFromAssignMenu}
     />
     <CollectionEditModal
       open={createCollectionFromMenuOpen}
       mode="create"
       onClose={() => setCreateCollectionFromMenuOpen(false)}
       onSubmitCreate={handleSubmitCreateFromMenu}
       onSubmitEdit={async () => false}
     />
     ```

- **MIRROR**: The existing `useGameDetailsModalState` integration in `LibraryPage` is the closest analogue for "modal state object + handlers + JSX mount".
- **IMPORTS**: see snippet. Add `useState`, `useCallback` if not already present.
- **GOTCHA**:
  - **The assign menu's "+ New collection" CTA opens a separate `<CollectionEditModal>`** — not the one in `<CollectionsSidebar>`. This is intentional: the menu is on `LibraryPage`, the sidebar create modal is on `<CollectionsSidebar>` — they don't share state.
  - **`handleSubmitCreateFromMenu` ignores the description** because the assign-menu flow is "create with a name and immediately add the current profile". Adding the profile to the new collection is a follow-up — for v1, the user creates the collection then has to right-click again to assign. Document this in a comment.
    - **Optional Phase 2.1 enhancement** (not in scope): after `createCollection`, immediately call `addProfile(newId, profileNameAtTimeOfRightClick)`. This requires capturing the profile name when the user clicks "+ New collection…" inside the menu.
  - **`useCollections()` is called inside `LibraryPage` now** — this is the third call site (sidebar, AppShell, LibraryPage). Each has its own state. This is a known v1 simplification.
  - **`onContextMenu` event on `LibraryCard`** prevents the native browser menu via `e.preventDefault()` (Task 12).
- **VALIDATE**:
  - `pnpm --filter crosshook-native run build` succeeds.
  - In `pnpm dev:browser`, right-clicking a library card opens the popover at the cursor; toggling a checkbox calls `collection_add_profile` (visible in mock console.debug); clicking "+ New collection…" opens the create modal.

### Task 17: Filter the `LaunchPage` Active-Profile dropdown by `activeCollectionId`

- **ACTION**: In `LaunchPage.tsx:295-306`, replace the flat `profileState.profiles.map(...)` with a filtered list that respects `activeCollectionId` (when set). Add a small chip/affordance showing the active filter and a "clear" button.
- **IMPLEMENT**: In `src/crosshook-native/src/components/pages/LaunchPage.tsx`:
  1. Read `activeCollectionId`, `setActiveCollectionId` from `useProfileContext()` (already imported).
  2. Add a `useCollectionMembers` call gated on `activeCollectionId`:

     ```tsx
     import { useCollectionMembers } from '../../hooks/useCollectionMembers';
     import { useCollections } from '../../hooks/useCollections';

     // Inside LaunchPage:
     const { activeCollectionId, setActiveCollectionId } = profileState; // already from context
     const { collections } = useCollections();
     const { memberNames } = useCollectionMembers(activeCollectionId);
     const activeCollection = useMemo(
       () =>
         activeCollectionId === null ? null : (collections.find((c) => c.collection_id === activeCollectionId) ?? null),
       [collections, activeCollectionId]
     );
     ```

  3. Compute the filtered options:

     ```tsx
     const filteredProfiles = useMemo(() => {
       if (activeCollectionId === null || memberNames.length === 0) {
         return profileState.profiles;
       }
       const set = new Set(memberNames);
       return profileState.profiles.filter((name) => set.has(name));
     }, [profileState.profiles, activeCollectionId, memberNames]);
     ```

  4. Replace `options={profileState.profiles.map((name) => ({ value: name, label: name }))}` with `options={filteredProfiles.map((name) => ({ value: name, label: name }))}`.

  5. Render a small chip above or beside the select when `activeCollection !== null`:

     ```tsx
     {
       activeCollection !== null && (
         <div className="crosshook-launch-collection-filter">
           Filtering by: <strong>{activeCollection.name}</strong>
           <button
             type="button"
             className="crosshook-button crosshook-button--ghost crosshook-button--small"
             onClick={() => setActiveCollectionId(null)}
             aria-label="Clear collection filter"
           >
             ×
           </button>
         </div>
       );
     }
     ```

     Place this immediately above the `<ThemedSelect>` slot or as a sibling inside the same row.

- **MIRROR**: There is no existing chip-with-clear-button precedent — this is Phase 2's pattern. Mirror the visual style of `crosshook-modal__status-chip` from `theme.css:3815-3849`.
- **IMPORTS**: `useMemo`; `useCollections`, `useCollectionMembers`.
- **GOTCHA**:
  - **`pinnedValues` (favorites) must still be honored**. The filtered list respects collection membership, but `<ThemedSelect>` separately receives `pinnedValues={pinnedSet}` (line 301). Favorites that are NOT in the active collection would not appear in the options after filtering — which is the expected behavior.
  - **If `memberNames.length === 0` while `activeCollectionId !== null`**, fall back to the unfiltered list. This avoids leaving the user with an empty dropdown when a collection has no members yet (otherwise they couldn't even select a profile to use the app).
  - **`useCollectionMembers(null)` returns empty without calling IPC** (Task 5 GOTCHA), so this is cheap when no filter is active.
  - **The filter is read-only here** — `setActiveCollectionId(null)` clears it via the chip's × button.
- **VALIDATE**:
  - `pnpm --filter crosshook-native run build` succeeds.
  - In `pnpm dev:browser`: when no collection is active, the dropdown lists all profiles; after clicking a sidebar collection, the dropdown narrows to its members; the chip × clears the filter.

### Task 18: Filter the `ProfilesPage` Active-Profile dropdown — preserving the "Create New" sentinel

- **ACTION**: Same filter logic as Task 17, but the `ProfilesPage` dropdown has a `{ value: '', label: 'Create New' }` sentinel that **must always be present** at the top of the options list, regardless of filter state.
- **IMPLEMENT**: In `src/crosshook-native/src/components/pages/ProfilesPage.tsx`, around lines 593-614:
  1. Add the same `useCollectionMembers` + `activeCollectionId` reading.
  2. Compute filtered profiles:

     ```tsx
     const filteredProfiles = useMemo(() => {
       if (activeCollectionId === null || memberNames.length === 0) {
         return profiles;
       }
       const set = new Set(memberNames);
       return profiles.filter((name) => set.has(name));
     }, [profiles, activeCollectionId, memberNames]);
     ```

  3. Update the `ThemedSelect` options:

     ```tsx
     options={[
       { value: '', label: 'Create New' },
       ...filteredProfiles.map((name) => ({ value: name, label: name })),
     ]}
     ```

  4. Render the same filter chip near the dropdown.

- **MIRROR**: Task 17's chip pattern.
- **IMPORTS**: same as Task 17.
- **GOTCHA**:
  - **The sentinel `{ value: '', label: 'Create New' }` MUST stay at index 0**. The existing code spreads it before `profiles.map(...)`. Replace `profiles` with `filteredProfiles`; do NOT touch the sentinel. Stripping the sentinel breaks the create-new-profile flow.
  - The `selectedProfile` value can be `''` when the user is creating a new profile — the filter must not break this case. Since the filter only narrows the options list (not the value), and `''` is in the options via the sentinel, this is safe.
- **VALIDATE**:
  - `pnpm --filter crosshook-native run build` succeeds.
  - In `pnpm dev:browser`: the "Create New" option is always present; selecting it works regardless of active filter; the filter narrows the rest of the options.

### Task 19: Add CSS for new collection components (theme.css or new collections.css)

- **ACTION**: Add the new BEM-namespaced classes referenced by `<CollectionsSidebar>`, `<CollectionViewModal>`, `<CollectionAssignMenu>`, `<CollectionEditModal>`. Decision: append to `src/crosshook-native/src/styles/theme.css` (existing convention) OR create `src/crosshook-native/src/styles/collections.css` and import it from `App.tsx`. **Recommendation**: add to `theme.css` to match existing modal/sidebar conventions; create new files only if total CSS exceeds ~100 lines.
- **IMPLEMENT**: Append to `src/crosshook-native/src/styles/theme.css` (or new file):

  ```css
  /* Collections sidebar section */
  .crosshook-collections-sidebar {
    /* uses existing crosshook-sidebar__section */
  }
  .crosshook-collections-sidebar__list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 320px;
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .crosshook-collections-sidebar__item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 12px;
    background: transparent;
    border: none;
    color: var(--crosshook-color-text);
    text-align: left;
    cursor: pointer;
    border-radius: var(--crosshook-radius-sm);
  }
  .crosshook-collections-sidebar__item:hover {
    background: rgba(255, 255, 255, 0.04);
  }
  .crosshook-collections-sidebar__item-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .crosshook-collections-sidebar__item-count {
    font-size: 0.7rem;
    background: rgba(255, 255, 255, 0.08);
    color: var(--crosshook-color-text-muted);
    padding: 2px 6px;
    border-radius: 999px;
  }
  .crosshook-collections-sidebar__cta {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: transparent;
    border: 1px dashed rgba(255, 255, 255, 0.15);
    color: var(--crosshook-color-accent);
    cursor: pointer;
    border-radius: var(--crosshook-radius-sm);
    margin-top: 4px;
  }
  .crosshook-collections-sidebar__cta:hover {
    background: rgba(255, 255, 255, 0.04);
  }
  .crosshook-collections-sidebar__error {
    color: var(--crosshook-color-danger);
    font-size: 0.75rem;
    padding: 4px 12px;
  }

  /* Launch / Profiles page filter chip */
  .crosshook-launch-collection-filter {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 999px;
    padding: 2px 4px 2px 10px;
    font-size: 0.75rem;
    color: var(--crosshook-color-text-muted);
  }

  /* Collection edit modal warn text */
  .crosshook-collection-edit-modal__warn {
    color: var(--crosshook-color-danger);
    font-size: 0.85rem;
  }
  ```

  (The `<CollectionAssignMenu>` and `<CollectionViewModal>` CSS lives in their respective `.css` files imported from the components.)

- **MIRROR**: Existing modal classes at `theme.css:3690-3883`; sidebar classes at `sidebar.css:76-200`.
- **IMPORTS**: none (CSS file).
- **GOTCHA**:
  - **`.crosshook-collections-sidebar__list` has its own `overflow-y: auto`** — must be added to `useScrollEnhance.ts` SCROLLABLE selector (Task 20).
  - **`overscroll-behavior: contain`** is set on the list to prevent scroll-chaining to the sidebar parent.
  - **Use existing CSS variables** (`--crosshook-color-text`, `--crosshook-color-accent`, `--crosshook-radius-sm`, etc.) — defined in `variables.css`.
- **VALIDATE**: Visual inspection in `pnpm dev:browser`. No type errors (CSS is not type-checked).

### Task 20: Update `useScrollEnhance.ts` SCROLLABLE selector

- **ACTION**: Add `.crosshook-collections-sidebar__list` and `.crosshook-collection-assign-menu__list` to the `SCROLLABLE` constant in `useScrollEnhance.ts:8-9`. The other new scroll containers either use `.crosshook-modal__body` (already registered) or are short enough to not need enhancement.
- **IMPLEMENT**: In `src/crosshook-native/src/hooks/useScrollEnhance.ts`:

  ```ts
  // BEFORE (line 8-9):
  const SCROLLABLE =
    '.crosshook-route-card-scroll, .crosshook-page-scroll-body, .crosshook-subtab-content__inner--scroll, .crosshook-console-drawer__body, .crosshook-modal__body, .crosshook-prefix-deps__log-output, .crosshook-discovery-results';

  // AFTER:
  const SCROLLABLE =
    '.crosshook-route-card-scroll, .crosshook-page-scroll-body, .crosshook-subtab-content__inner--scroll, .crosshook-console-drawer__body, .crosshook-modal__body, .crosshook-prefix-deps__log-output, .crosshook-discovery-results, .crosshook-collections-sidebar__list, .crosshook-collection-assign-menu__list';
  ```

- **MIRROR**: The existing single-line constant.
- **IMPORTS**: none.
- **GOTCHA**:
  - **Both new classes MUST also have `overscroll-behavior: contain`** in CSS (Tasks 19, 11). Without it, the WebKitGTK enhanced scroll will chain to the parent and cause dual-scroll jank.
  - **Do NOT remove existing entries** — that would break unrelated UI.
  - **The selector is comma-separated, single-line** — keep it on one line for parity with the existing style.
- **VALIDATE**:
  - `pnpm --filter crosshook-native run build` succeeds.
  - Manual: in `pnpm dev:browser`, hovering over a long collections list and using the wheel scrolls the list (not the sidebar parent).

### Task 21: Manual smoke test + Playwright update

- **ACTION**: After Tasks 1-20 are merged, manually validate the JTBD flow end-to-end and update the Playwright smoke screenshot baseline if the sidebar layout shifts.
- **IMPLEMENT**:
  1. Run `pnpm --filter crosshook-native run build` — must pass.
  2. Run `pnpm test:smoke` — if it fails on screenshot diffs **only** (no console errors), run `pnpm test:smoke:update` to refresh baselines for `/library`, `/profiles`, `/launch` routes that now show the new sidebar section.
  3. Run `./scripts/dev-native.sh --browser`. Open devtools console, verify zero red errors on boot. Click through:
     - Sidebar shows "+ Create your first collection" (because mock fixture has 1 collection — actually shows the "Action / Adventure" chip + "+ New collection" button).
     - Click the "Action / Adventure" chip → modal opens, search input focusable, member grid shows seed members.
     - Type in search → list narrows.
     - Click "Edit" → edit modal opens with name pre-filled.
     - Right-click a Library card → assign menu opens at cursor; toggle a checkbox → IPC mock fires; close via Esc.
     - Go to Launch page → Active-Profile dropdown shows the filter chip with "Action / Adventure"; options are narrowed; click × → filter clears.
     - Go to Profiles page → same chip behavior; "Create New" sentinel is still at the top.
  4. **Steam Deck check (deferred to Phase 5)**: defer to Phase 5 acceptance gate.
- **MIRROR**: existing smoke-test conventions in `tests/smoke.spec.ts`.
- **IMPORTS**: n/a.
- **GOTCHA**:
  - **`pnpm test:smoke` may fail on screenshot diffs** — that's expected because the sidebar layout changed. Update baselines deliberately, not blindly.
  - **`pnpm test:smoke` fails the route walk if any new code throws or logs `console.error` on boot** — regress here means a hook is calling IPC with bad args (likely D1-related; verify Task 1 was applied correctly).
  - **No new test files** — Phase 2 has no unit tests because the codebase has no Vitest setup. The verification gate is `pnpm test:smoke` + manual click-through.
- **VALIDATE**:
  - `pnpm test:smoke` passes (or screenshots updated).
  - JTBD flow ≤4 clicks confirmed in dev session.
  - No console errors.

---

## Testing Strategy

### Unit Tests

**No new unit tests.** The codebase has no Vitest / Jest configuration. Phase 2 verification is via:

1. `pnpm --filter crosshook-native run build` (= `tsc && vite build`) — type-check gate.
2. `pnpm test:smoke` (Playwright) — route walk + console error assertions.
3. Manual `pnpm dev:browser` click-through.

### Integration / Smoke Test Coverage (Playwright)

| Test                | Input                                                       | Expected                                                                                                      |
| ------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Existing route walk | Boot `?fixture=populated`, click each sidebar route in turn | All 9 routes navigate, no `console.error`, no page errors. **New collection section must not throw on boot.** |
| Updated screenshots | After sidebar layout shift                                  | New baselines updated via `pnpm test:smoke:update`.                                                           |

### Manual JTBD Flow (post-Task 21)

- [ ] Open `pnpm dev:browser` → sidebar shows the seed `Action / Adventure` collection chip + count badge.
- [ ] Click the chip → modal opens, member grid populates from `useLibrarySummaries` ∩ `memberNames`.
- [ ] Type in inner search → list narrows.
- [ ] Click a card → modal closes, Launch page opens, profile is selected (select-then-navigate).
- [ ] Click Launch → flow proceeds normally.
- [ ] Right-click a Library card → assign menu opens at cursor, checkboxes reflect membership.
- [ ] Toggle a checkbox → mock IPC fires, sidebar count badge updates after refresh.
- [ ] Click "+ New collection…" inside the assign menu → create modal opens.
- [ ] Submit a new collection name → mock IPC creates it, sidebar shows new chip.
- [ ] Click the new chip → empty modal with "No profiles in this collection yet" empty state.
- [ ] Navigate to Launch page → filter chip shows the active collection, options are narrowed.
- [ ] Click the chip × → filter clears, options return to full list.
- [ ] Navigate to Profiles page → filter chip + narrowed options + "Create New" sentinel still at top.
- [ ] Esc closes any modal; backdrop click closes any modal.
- [ ] Tab navigation cycles within the modal (focus trap).
- [ ] Resize the window — modal stays centered and scrollable.

### Edge Cases Checklist

- [x] **Empty collection list** — sidebar shows only the "+ Create your first collection" CTA.
- [x] **Empty collection (no members)** — modal body shows "No profiles in this collection yet" empty state.
- [x] **Search inside collection with zero matches** — modal body shows "No profiles match your search".
- [x] **`activeCollectionId` set but `memberNames.length === 0`** — Active-Profile dropdown falls back to unfiltered list (avoid empty dropdown).
- [x] **Collection deletion while modal is open** — modal calls `onClose()` after `deleteCollection` returns true.
- [x] **Duplicate collection name on create** — IPC error surfaces in `useCollections.error`; create modal displays it inline.
- [x] **Right-click near right edge / bottom edge** — popover position clamped to viewport.
- [x] **Reverse-lookup fails for unknown profile** — Phase 1 returns empty vec (not error); UI shows zero checked checkboxes.
- [x] **Esc closes the topmost modal only** — gamepad back uses the last `[data-crosshook-modal-close]` button.
- [ ] **Two collections with the same `profile_count` displayed in the same order** — Phase 1 sort order is `sort_order ASC, name ASC`; tie-break is alphabetical.
- [ ] **Concurrent IPC mutations** — `useCollections` per-op busy ids prevent double-click; not bullet-proof but acceptable for v1.
- [ ] **`useScrollEnhance` registered but the new scroll container is empty** — no-op (the wheel handler does nothing if there are no scrollable contents).

---

## Validation Commands

### Static Analysis (TypeScript)

```bash
pnpm --filter crosshook-native run build
```

**EXPECT**: Zero TS errors. The build script runs `tsc && vite build` (`package.json:10`), so this validates both the type system and the production bundle.

### Browser Dev Mode Smoke

```bash
./scripts/dev-native.sh --browser
# Open the loopback URL in a browser, open devtools console.
# Verify zero red errors on boot.
# Manually click through the JTBD flow above.
```

**EXPECT**: No `[dev-mock] Unhandled command: collection_*` errors (Task 1 fixes). No uncaught exceptions. New sidebar section renders. Modal opens on click. Right-click menu appears.

### Mock Coverage Sentinel

```bash
pnpm --dir src/crosshook-native run dev:browser:check
# Or directly: bash scripts/check-mock-coverage.sh
```

**EXPECT**: All collection IPC commands have mock handlers. The script reads `src/lib/mocks/handlers/collections.ts` and verifies coverage.

### Playwright Smoke

```bash
pnpm --dir src/crosshook-native test:smoke
# If screenshots diff because of the new sidebar section:
pnpm --dir src/crosshook-native test:smoke:update
```

**EXPECT**: All 9 route walks pass. Console errors === 0. Screenshots updated deliberately.

### Production Bundle Sentinel (matches CI)

```bash
./scripts/build-native.sh --binary-only
grep -l '\[dev-mock\]\|getMockRegistry\|registerMocks\|MOCK MODE' \
  src/crosshook-native/dist/assets/*.js 2>/dev/null \
  && echo "❌ mock code leaked into production bundle" \
  || echo "✅ no mock code in production bundle"
```

**EXPECT**: `✅ no mock code in production bundle`.

### Manual Tauri Build (link check)

```bash
./scripts/dev-native.sh
```

**EXPECT**: Full Tauri dev mode launches; all 9 collection IPC commands work against the real Rust backend (Phase 1's deliverables); manual click-through of the JTBD flow succeeds against real SQLite.

### Manual Validation Checklist

- [ ] `tsc --noEmit` (via build script) passes
- [ ] `pnpm test:smoke` passes or baselines updated
- [ ] `pnpm dev:browser` JTBD flow completes in ≤4 clicks
- [ ] No regressions on the Library page when no collections exist
- [ ] No regressions in the Active-Profile dropdown when `activeCollectionId === null`
- [ ] Esc / Tab / backdrop click work in `<CollectionViewModal>` and `<CollectionEditModal>`
- [ ] Right-click popover dismisses on Esc and click-outside
- [ ] Filter chip clears via × button
- [ ] Create modal duplicate-name error renders inline

---

## Acceptance Criteria

Direct mapping to issue #178:

- [ ] `useCollections` hook exposes list, create, delete, rename, update_description, add/remove member, list members, reverse lookup (Task 4)
- [ ] `<CollectionsSidebar>` renders chips when `collections.length > 0`; renders "+ Create your first collection" CTA when empty (Tasks 10, 14)
- [ ] Collection entries show name + `profile_count` badge (Tasks 10, 19)
- [ ] `<CollectionViewModal>` opens on sidebar click, supports inline search + filter via `useLibraryProfiles` (Tasks 7, 8, 15)
- [ ] Right-click on a profile card shows "Add to collection → …" multi-select popover (Tasks 11, 12, 13, 16)
- [ ] Multi-select assign-to-multiple via the right-click menu (Task 11)
- [ ] Launch + edit a profile from within the modal via select-then-navigate indirection works end-to-end (Tasks 8, 15)
- [ ] Active-Profile dropdown is filterable to the active collection on both `LaunchPage` and `ProfilesPage` (Tasks 17, 18)
- [ ] Empty state "Create your first collection" CTA renders when `collections.length === 0` (Task 10)
- [ ] Any new `overflow-y: auto` container added to `SCROLLABLE` selector + uses `overscroll-behavior: contain` (Tasks 11, 19, 20)
- [ ] JTBD flow end-to-end: sidebar → collection view modal → filter → click profile → Launch page → Launch — **in ≤4 clicks** (Task 21)
- [ ] No regressions in Library page or Active-Profile dropdown when `collections.length === 0` (Task 21)
- [ ] `pnpm tsc --noEmit` (via build) passes; `pnpm dev:browser` session has no red console errors on boot (Task 21)
- [ ] Manual Steam Deck / `gamescope` D-pad navigation audit passes — **deferred to Phase 5**

## Completion Checklist

- [ ] Code follows discovered patterns (`callCommand` from `@/lib/ipc`, single error string, per-op busy ids, refresh-after-mutate)
- [ ] All IPC arg keys are camelCase (`collectionId`, `profileName`, `newName`)
- [ ] Mock handler arg keys updated to camelCase (D1 fix)
- [ ] All mock error messages still start with `[dev-mock]`
- [ ] All new modals have `data-crosshook-focus-root="modal"` and a button with `data-crosshook-modal-close`
- [ ] All new `overflow-y: auto` containers added to `useScrollEnhance.ts` SCROLLABLE selector
- [ ] All new `overflow-y: auto` containers use `overscroll-behavior: contain` in CSS
- [ ] No new dependencies added (`package.json` unchanged in `dependencies`)
- [ ] No `@tauri-apps/api/core` direct import in any new file (only `callCommand` from `@/lib/ipc`)
- [ ] No `console.error` calls during smoke-test boot
- [ ] CSS classes follow `crosshook-*` BEM-like convention
- [ ] Component files use PascalCase; hook files use `use*` camelCase; type files match domain
- [ ] No `any` type used anywhere — strict TS satisfied
- [ ] Self-contained — no questions needed during implementation

## Risks

| Risk                                                                                                                                                                                  | Likelihood                            | Impact                                                                               | Mitigation                                                                                                                                                                                                       |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- | ------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **D1 not understood by implementer**: keeps mock handler arg keys snake_case, hooks use camelCase, every collection IPC silently fails in browser dev mode                            | **Low** (plan calls it out as Task 1) | **High** (entire phase non-functional in browser dev)                                | Task 1 is the **first** task; manual validation in Task 21 hits an IPC call within 30 seconds of opening dev mode                                                                                                |
| Three call sites of `useCollections()` (sidebar, AppShell, LibraryPage) hold independent state — `profile_count` displayed in sidebar is stale immediately after a right-click toggle | **Medium**                            | **Medium** (UX inconsistency, not data corruption)                                   | Each `useCollections` instance refreshes after every mutation; 99% of cases the user closes the assign menu before noticing. Phase 5 polish: lift to a shared `CollectionsContext`.                              |
| `<CollectionsSidebar>` mounted inside `<Tabs.List>` causes Radix to warn or misroute focus                                                                                            | **Low**                               | **Medium**                                                                           | The existing `crosshook-sidebar__footer` already mounts non-trigger children inside `Tabs.List`, so the precedent works. Smoke-test on first commit. Fallback: move outside `</Tabs.List>` but inside `<aside>`. |
| Right-click context menu is the first context menu in the codebase — Steam Deck controller navigation may not invoke `onContextMenu` at all                                           | **Medium**                            | **Low** in Phase 2 (Phase 5 deferred)                                                | Phase 5 deals with Deck validation. v1 users with mouse/keyboard get the menu; controller users still have the sidebar chip + view-modal flow.                                                                   |
| Copying `GameDetailsModal.tsx` verbatim into `CollectionViewModal.tsx` introduces a 7th near-duplicate of the focus-trap + body-lock + inert-siblings code                            | **High** (it's the explicit plan)     | **Low** (existing 6 modals work; Phase 5 polish task to extract `<Modal>` primitive) | PRD marked the extraction as Should-have. Skip for v1; file as a follow-up issue.                                                                                                                                |
| `useCollections` boot-fetch causes an extra IPC call on app launch (from `<CollectionsSidebar>` mount)                                                                                | **Low**                               | **Low**                                                                              | One IPC call to `collection_list`, returns instantly against SQLite. Negligible.                                                                                                                                 |
| `activeCollectionId` filter persisting after the user navigates away from a collection makes the dropdown "mysteriously" filtered with no UI hint                                     | **Medium**                            | **Medium**                                                                           | Filter chip on both `LaunchPage` and `ProfilesPage` always renders when `activeCollectionId !== null`, with × to clear. Visible affordance.                                                                      |
| `ProfilesPage`'s "Create New" sentinel stripped by accident                                                                                                                           | **Low**                               | **High** (breaks profile creation flow)                                              | Task 18 GOTCHA spells out the order. Manual smoke-test in Task 21 exercises it.                                                                                                                                  |
| Mock handler `getStore().profiles` validation rejects a profile name added via right-click that isn't in the mock fixture                                                             | **Low**                               | **Low**                                                                              | The mock fixture (`src/lib/mocks/store.ts` — populated profiles) covers the typical browser-dev scenarios. Real Tauri mode hits real profiles.                                                                   |
| `pnpm test:smoke` screenshot diffs cause CI to fail                                                                                                                                   | **Medium** (sidebar visibly changed)  | **Low**                                                                              | Task 21 calls out the screenshot update. CI can run `test:smoke:update` after manual review.                                                                                                                     |
| `<CollectionAssignMenu>` popover positioned offscreen on small viewports                                                                                                              | **Low**                               | **Low**                                                                              | Basic clamping (`Math.min(x, innerWidth - 280)`) handles the common case. v1 acceptable.                                                                                                                         |
| `useLibrarySummaries(profiles, favoriteProfiles)` called inside `<CollectionViewModal>` triggers a `profile_list_summaries` IPC fetch every time the modal opens                      | **Low**                               | **Low**                                                                              | The modal mounts only when open; the fetch is deduplicated by React's useEffect deps. Negligible perf cost.                                                                                                      |
| TS strict mode rejects `args as { collectionId: string }` with a non-trivial type                                                                                                     | **Very Low**                          | **Low**                                                                              | The exact `as { ... }` shape is used by every other mock handler in the codebase (`protondb.ts:135` etc.). Pattern is well-established.                                                                          |

## Notes

### Things that look concerning but are intentional

- **`onToggleFavorite={() => {}}` inside `<CollectionViewModal>`**: Phase 2 ships without per-card favorite toggling inside the collection modal — the modal is a "view" surface, not an "edit favorites" surface. Favorites are managed from the LibraryPage. The no-op preserves the `<LibraryCard>` prop contract.
- **`onOpenDetails` in the modal launches the profile** (calls `handleLaunchClick(card.name)`) instead of opening `<GameDetailsModal>`-in-collection-context: Phase 2 ships the simpler "click → launch" UX for the high-traffic JTBD path; opening a nested modal-in-modal is deferred to Phase 4 polish. The PRD's user flow has 4 clicks: sidebar → collection → click profile (launches via select-then-navigate) → Launch button. Adding a card-details intermediary would add a click.
- **Three `useCollections()` instances** (sidebar, AppShell, LibraryPage): each is independent state. This is the simplest pattern and works correctly for Phase 2; the `refresh()` call after every mutation keeps each consumer in sync within ~50ms. The PRD's "Should: extract shared `<CollectionsContext>`" is deferred until concrete UX issues surface in practice.
- **`activeCollectionId` lives in `ProfileContext`, not a new `CollectionsContext`**: the PRD specifies this and it's the path of least disruption. `useProfile` is not touched; only `ProfileProvider` adds a `useState`.
- **"+ New collection" button is rendered twice** (once in `<CollectionsSidebar>`, once in `<CollectionAssignMenu>`): each opens its own `<CollectionEditModal>` instance because the contexts are different (one is sidebar create, the other is right-click create-and-assign). This is intentional duplication; sharing the modal would require lifting state to a shared context.
- **`<CollectionEditModal>` is mounted twice in the tree** (once in `<CollectionsSidebar>`, once in `AppShell` for the edit flow, once in `<LibraryPage>` for the right-click create flow): each instance handles a single mode and closes when done. Duplication is acceptable for v1.
- **`useGameDetailsModalState` is NOT reused for `useCollectionViewModalState`**: it has a `LibraryCardData` shape that doesn't fit collections. A separate hook with `collectionId: string | null` is the correct mirror.
- **The seed mock fixture name is `Action / Adventure` with a slash** — this tests UI rendering of names with special characters. Phase 2 should render it correctly (no escaping needed for HTML, no special-case for the chip label).

### Conventional Commit suggestions (CLAUDE.md MUST)

Pick one per logical grouping; collections → `area:profiles, area:ui, type:feature`:

```text
fix(ui): mock handlers for collection_* commands accept camelCase args (D1 blocker)
feat(ui): useCollections + useCollectionMembers hooks wrapping Phase 1 IPC
feat(ui): CollectionsSidebar section with chip list + create CTA
feat(ui): CollectionViewModal mirroring GameDetailsModal portal pattern
feat(ui): right-click "Add to collection" multi-select context menu on library cards
feat(ui): Active-Profile dropdown filter by active collection on Launch + Profiles pages
feat(ui): activeCollectionId ephemeral state in ProfileContext
```

Tag the PR with `type:feature`, `area:profiles`, `area:ui`, `priority:high`, `platform:linux`, `platform:steam-deck`. Link with `Closes #178` (the source GitHub issue).

### Things that depend on Phase 2 landing first

- **Phase 3** (per-collection launch defaults) requires `activeCollectionId` to be threaded through context (Task 6). The Rust merge layer reads it from `LaunchStateContext` later but the originating value lives in `ProfileContext` here.
- **Phase 4** (TOML export/import) requires the `<CollectionViewModal>` to host an "Export…" / "Import…" button — Phase 4 will add the button to the modal footer.
- **Phase 5** (polish + Steam Deck validation) gates on Phase 2's click-through working end-to-end.

### What I deliberately did NOT capture in this plan

- **Detailed CSS layouts for the new modals** — `theme.css` already has 200+ lines of modal CSS (`crosshook-modal__*`) that the new modals inherit. Phase 2 only adds modal-specific overrides (~50 lines) referenced inline in Tasks 8, 9, 11, 19.
- **Exact CSS pixel values for the chip badge** — the badge styling is documented in Task 19 but visual polish is owed to Phase 5.
- **Per-card "remove from collection" button inside `<CollectionViewModal>`** — sketched in Task 8 GOTCHA but not wired. Right-click context menu (Task 11) handles removal via the multi-select checkbox toggling. Add the per-card affordance in Phase 5 if user feedback requests it.
- **Keyboard shortcut documentation** — Tab/Esc/Enter all work via the inherited focus trap.

### Confidence

**Confidence Score: 8/10** for single-pass implementation. The plan is bounded (frontend-only, zero deps, established patterns), the D1 blocker is called out as Task 1, and every new component has an explicit precedent file to mirror. The 2-point deduction accounts for: (a) the right-click context menu being the first of its kind in the codebase (no prior art to mirror), and (b) the three `useCollections()` instances being a known v1 simplification that may need correction in Phase 5.

---

_Generated: 2026-04-08 from `docs/prps/prds/profile-collections.prd.md` Phase 2 + GitHub issue #178._
