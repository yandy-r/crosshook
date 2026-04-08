# Phase 2 Profile Collections — Frontend Discovery Report

Research for the single-pass implementation plan for issue #178 (Phase 2 —
frontend of profile collections). Phase 1 backend commit is `63d43e1`.

All line numbers are against the working tree at research time. This document
reports **what exists**, not what to build.

---

## Part A — 8 Discovery Categories

### 1. Similar Implementations

#### 1a. Hooks that wrap Tauri IPC with `useState + refresh` pattern

`src/crosshook-native/src/hooks/useLauncherManagement.ts` is the closest precedent in size and shape (one domain, list + CRUD, no events). Verbatim lines `src/crosshook-native/src/hooks/useLauncherManagement.ts:21-101`:

```ts
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

Key conventions:

- Single `error: string | null` for the whole hook, set on any IPC failure; caller reads `error` directly from the hook return.
- Per-op "busy" ids rather than a single `saving` flag (`deletingSlug`, `reexportingSlug`).
- Mutations **always** call the list refresh (`listLaunchers`) after success to keep in-memory state authoritative.
- Never throws from mutation callbacks — returns `boolean` for CRUD, or `void` for list refresh; error is surfaced via `setError`.

`src/crosshook-native/src/hooks/useCommunityProfiles.ts` is the larger precedent (loading/syncing/importing flags, initial load `useEffect`, event subscription, nested domain objects). Verbatim lines `src/crosshook-native/src/hooks/useCommunityProfiles.ts:207-266`:

```ts
export function useCommunityProfiles(options: UseCommunityProfilesOptions): UseCommunityProfilesResult {
  const [taps, setTaps] = useState<CommunityTapSubscription[]>([]);
  const [index, setIndex] = useState<CommunityProfileIndex>({
    entries: [],
    diagnostics: [],
  });
  const [lastSyncedCommits, setLastSyncedCommits] = useState<Record<string, string>>({});
  const [lastTapSyncResults, setLastTapSyncResults] = useState<CommunityTapSyncResult[]>([]);
  const [importedProfileNames, setImportedProfileNames] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
```

And the initial load `useEffect` at `useCommunityProfiles.ts:387-421`:

```ts
  useEffect(() => {
    let active = true;

    async function loadInitialState() {
      try {
        const settings = await callCommand<AppSettingsData>('settings_load');
        if (!active) {
          return;
        }

        setTaps(dedupeTaps(settings.community_taps));
        setError(null);
      } catch (loadError) {
        if (active) {
          setError(loadError instanceof Error ? loadError.message : String(loadError));
          setTaps([]);
        }
      } finally {
        if (active) {
          setLoading(false);
        }
      }
    }

    void loadInitialState();
```

Pattern: `let active = true` guard against setState after unmount. Hook-level `loading`, `syncing`, `importing` booleans. Single `error: string | null`. Mutations may `throw` after setting error (community hooks do — `throw addError;`), while launcher hooks return `boolean`. Phase 2 should pick **one** style and stay consistent.

`src/crosshook-native/src/hooks/useProfile.ts:600-624` — the canonical "list refresh" pattern:

```ts
const refreshProfiles = useCallback(async () => {
  try {
    const names = await callCommand<string[]>('profile_list');
    setProfiles(names);

    if (names.length === 0) {
      setSelectedProfile('');
      setProfileName('');
      setProfile(createEmptyProfile());
      setDirty(false);
      lastSavedLaunchOptimizationIdsRef.current = [];
      return;
    }

    if (selectedProfile && names.includes(selectedProfile)) {
      return;
    }

    if (options.autoSelectFirstProfile ?? true) {
      await loadProfile(names[0]);
    }
  } catch (err) {
    setError(err instanceof Error ? err.message : String(err));
  }
}, [loadProfile, options.autoSelectFirstProfile, selectedProfile]);
```

#### 1b. Portal modals (focus trap + body lock)

`src/crosshook-native/src/components/library/GameDetailsModal.tsx` is the single richest precedent — includes portal host creation, `crosshook-modal-open` body class, inert siblings, focus restore, Esc/Tab handling, backdrop click-to-close. Verbatim lines `GameDetailsModal.tsx:182-253`:

```ts
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

Focusable helpers (identical across every portal modal) — `GameDetailsModal.tsx:27-49`:

```ts
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

Tab trap (shift/forward) — `GameDetailsModal.tsx:255-294`:

```ts
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

function handleBackdropMouseDown(event: MouseEvent<HTMLDivElement>) {
  if (event.target !== event.currentTarget) {
    return;
  }
  onClose();
}
```

Render root — `GameDetailsModal.tsx:310-322`:

```tsx
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

Other modals using `createPortal`:

- `src/crosshook-native/src/components/LaunchPanel.tsx`
- `src/crosshook-native/src/components/OnboardingWizard.tsx`
- `src/crosshook-native/src/components/ProfilePreviewModal.tsx`
- `src/crosshook-native/src/components/ProfileReviewModal.tsx`
- `src/crosshook-native/src/components/LauncherPreviewModal.tsx`
- `src/crosshook-native/src/components/MigrationReviewModal.tsx`
- `src/crosshook-native/src/components/OfflineTrainerInfoModal.tsx`
- `src/crosshook-native/src/components/ConfigHistoryPanel.tsx`

Simpler subset (no sibling-inerting, no portal host) — `src/crosshook-native/src/components/OfflineTrainerInfoModal.tsx:106-176`:

```tsx
const node = (
  <div className="crosshook-modal" role="presentation">
    <div
      className="crosshook-modal__backdrop"
      aria-hidden="true"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) {
          onClose();
        }
      }}
    />
    <div
      ref={panelRef}
      className="crosshook-modal__surface crosshook-panel crosshook-focus-scope"
      role="dialog"
      aria-modal="true"
      aria-labelledby="crosshook-offline-trainer-info-title"
      data-crosshook-focus-root="modal"
      onKeyDown={handleKeyDown}
    >
      <header className="crosshook-modal__header">
        <div className="crosshook-modal__heading-block">
          <h2 id="crosshook-offline-trainer-info-title" className="crosshook-modal__title">
            {title}
          </h2>
        </div>
        <div className="crosshook-modal__header-actions">
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost crosshook-modal__close"
            onClick={onClose}
          >
            Close
          </button>
        </div>
      </header>
      ...
    </div>
  </div>
);

return createPortal(node, document.body);
```

Note: `OfflineTrainerInfoModal.tsx` only does focus restore + Tab trap, does **not** set `body.style.overflow` or create its own portal host node. It mounts directly on `document.body`. This is the "lite" precedent.

The focus-trap + body-lock block is duplicated nearly verbatim across at least six modals — **every file above repeats `FOCUSABLE_SELECTOR`, `getFocusableElements`, and the mount/cleanup pair**. This is strong evidence that a shared `<Modal>` primitive extraction is non-speculative: the code already exists six times.

#### 1c. Modal state hook precedent

`src/crosshook-native/src/components/library/useGameDetailsModalState.ts` (entire file, 28 lines):

```ts
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

#### 1d. Right-click context menus

`Grep` for `onContextMenu`, `context-menu`, `ContextMenu`, `contextmenu` returned **zero matches** across `src/crosshook-native/src`. There is no existing context-menu primitive or usage in the codebase. No `@radix-ui/react-context-menu` dependency either (see Dependencies below).

**GAP**: No existing right-click UI pattern to mirror. Phase 2 will be the first.

#### 1e. Sidebar nav item pattern

`src/crosshook-native/src/components/layout/Sidebar.tsx:63-86`:

```tsx
function SidebarTrigger({
  activeRoute,
  onNavigate,
  route,
  label,
  icon: Icon,
}: SidebarSectionItem & Pick<SidebarProps, 'activeRoute' | 'onNavigate'>) {
  const isCurrent = activeRoute === route;

  return (
    <Tabs.Trigger
      className="crosshook-sidebar__item"
      value={route}
      aria-current={isCurrent ? 'page' : undefined}
      onClick={() => onNavigate(route)}
      title={label}
    >
      <span className="crosshook-sidebar__item-icon" aria-hidden="true">
        <Icon />
      </span>
      <span className="crosshook-sidebar__item-label">{label}</span>
    </Tabs.Trigger>
  );
}
```

Sections structure — `Sidebar.tsx:36-61`:

```tsx
const SIDEBAR_SECTIONS: SidebarSection[] = [
  {
    label: 'Game',
    items: [
      { route: 'library', label: ROUTE_NAV_LABEL.library, icon: LibraryIcon },
      { route: 'profiles', label: ROUTE_NAV_LABEL.profiles, icon: ProfilesIcon },
      { route: 'launch', label: ROUTE_NAV_LABEL.launch, icon: LaunchIcon },
    ],
  },
  {
    label: 'Setup',
    items: [{ route: 'install', label: ROUTE_NAV_LABEL.install, icon: InstallIcon }],
  },
  {
    label: 'Dashboards',
    items: [{ route: 'health', label: ROUTE_NAV_LABEL.health, icon: HealthIcon }],
  },
  {
    label: 'Community',
    items: [
      { route: 'community', label: ROUTE_NAV_LABEL.community, icon: BrowseIcon },
      { route: 'discover', label: ROUTE_NAV_LABEL.discover, icon: DiscoverIcon },
      { route: 'compatibility', label: ROUTE_NAV_LABEL.compatibility, icon: CompatibilityIcon },
    ],
  },
];
```

The sidebar render pipeline maps `SIDEBAR_SECTIONS` to `<div className="crosshook-sidebar__section">` with a `__section-label` and `__section-items` wrapper — see `Sidebar.tsx:134-151`. Collections must plug into this structure (either as a new section, or as a conditional sub-list under "Game").

**Important**: all sidebar items are Radix `Tabs.Trigger` with a `value` bound to `AppRoute`. Collections are NOT routes — they do not navigate, they open a modal. A Collections section cannot use `Tabs.Trigger` at all; the existing `crosshook-sidebar__item` class is styled for tab triggers but can be re-used on a plain `<button>` so long as the trigger interplay with Radix `Tabs.List` is not broken.

#### 1f. Chip / badge patterns

`src/crosshook-native/src/components/PinnedProfilesStrip.tsx` (entire component, 62 lines) — the clearest chip precedent, and sits unused in production code (no current importer of `PinnedProfilesStrip`, confirmed via grep). Lines 1-59:

```tsx
interface PinnedProfilesStripProps {
  favoriteProfiles: string[];
  selectedProfile: string;
  onSelectProfile: (name: string) => Promise<void>;
  onToggleFavorite: (name: string, favorite: boolean) => Promise<void>;
}

export function PinnedProfilesStrip({
  favoriteProfiles,
  selectedProfile,
  onSelectProfile,
  onToggleFavorite,
}: PinnedProfilesStripProps) {
  if (favoriteProfiles.length === 0) return null;

  return (
    <section className="crosshook-pinned-strip" aria-label="Pinned profiles">
      <span className="crosshook-heading-eyebrow">Pinned Profiles</span>
      <div className="crosshook-pinned-strip__scroll">
        {favoriteProfiles.map((name) => {
          const isActive = name === selectedProfile;
          return (
            <div key={name} className="crosshook-pinned-strip__chip-container">
              <button
                type="button"
                className={`crosshook-pinned-strip__chip${isActive ? ' crosshook-pinned-strip__chip--active' : ''}`}
                onClick={() => void onSelectProfile(name)}
                aria-current={isActive ? 'true' : undefined}
                title={name}
              >
                <span className="crosshook-pinned-strip__chip-name">{name}</span>
              </button>
              <span
                role="button"
                tabIndex={0}
                className="crosshook-pinned-strip__unpin"
                aria-label={`Unpin ${name}`}
                title="Remove from pinned"
                onClick={(e) => {
                  e.stopPropagation();
                  void onToggleFavorite(name, false);
                }}
```

Empty-state convention: `if (favoriteProfiles.length === 0) return null;` — the strip hides itself. Phase 2 mirror: Sidebar `<CollectionsSection>` should use the same "return null when empty" pattern for the sidebar (PRD: render only when `collections.length > 0`), but a visible empty-state CTA will live in the modal.

Genre chips (different pattern, used inside the details modal) — `src/crosshook-native/src/components/library/GameDetailsModal.css:204-212`:

```css
.crosshook-game-details-modal__genre-chip {
  font-size: 0.75rem;
  font-weight: 600;
  padding: 4px 10px;
  border-radius: 999px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: rgba(255, 255, 255, 0.05);
  color: var(--crosshook-color-text-muted);
}
```

---

### 2. Naming Conventions

| Convention                   | Example file:line                                                                                                                                                                | Rule                                                                                        |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| React component              | `src/crosshook-native/src/components/library/GameDetailsModal.tsx:105` (`export function GameDetailsModal`)                                                                      | PascalCase. File name matches component, `.tsx`.                                            |
| Hook                         | `src/crosshook-native/src/hooks/useLauncherManagement.ts:21` (`export function useLauncherManagement`)                                                                           | camelCase, prefixed `use`.                                                                  |
| State hook (component-local) | `src/crosshook-native/src/components/library/useGameDetailsModalState.ts:12`                                                                                                     | Co-located with the component it serves under `components/<area>/`.                         |
| Utility pure functions       | `src/crosshook-native/src/components/library/game-details-actions.ts:6` (`export function gameDetailsLaunchThenNavigate`)                                                        | kebab-case filename, camelCase exports, no `use` prefix.                                    |
| Type                         | `src/crosshook-native/src/types/library.ts:3` (`export interface ProfileSummary`)                                                                                                | PascalCase, domain-segmented in `src/types/*.ts`, barrel-exported via `src/types/index.ts`. |
| IPC mirror type field        | `src/crosshook-native/src/types/launcher.ts:1-9` (`display_name: string; launcher_slug: string;`)                                                                                | snake_case — mirrors Rust `CollectionRow` serde default.                                    |
| IPC hook arg name            | `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts:94` (`callCommand('protondb_dismiss_suggestion', { profileName, appId, suggestionKey })`)                              | camelCase — Tauri v2 auto-converts Rust `profile_name: String` → JS `profileName`.          |
| CSS class                    | `src/crosshook-native/src/styles/theme.css:3699` (`.crosshook-modal`)                                                                                                            | BEM-like: `.crosshook-<block>__<element>--<modifier>`, always namespaced `crosshook-`.      |
| Directory layout for pages   | `src/crosshook-native/src/components/pages/LibraryPage.tsx`                                                                                                                      | Page components in `components/pages/`.                                                     |
| Domain component directory   | `src/crosshook-native/src/components/library/` (contains `GameDetailsModal.tsx`, `LibraryCard.tsx`, `LibraryGrid.tsx`, `useGameDetailsModalState.ts`, `game-details-actions.ts`) | Feature-scoped folder containing modal, cards, hooks, action helpers.                       |
| UI primitives                | `src/crosshook-native/src/components/ui/ThemedSelect.tsx`, `InfoTooltip.tsx`, `CollapsibleSection.tsx`                                                                           | Shared primitives live under `components/ui/`.                                              |
| Context                      | `src/crosshook-native/src/context/ProfileContext.tsx:26` (`const ProfileContext = createContext<ProfileContextValue                                                              | null>(null);`)                                                                              | PascalCase `<Name>Context`, with a `useXContext()` accessor that throws when null. |
| Icon component               | `src/crosshook-native/src/components/icons/SidebarIcons.tsx`                                                                                                                     | Named exports, consumed via `{ LibraryIcon, ProfilesIcon, ... }`.                           |

---

### 3. Error Handling

Two patterns coexist; both use `try/catch` with `setError` on a string message.

**Pattern A — swallow and surface** (`src/crosshook-native/src/hooks/useLauncherManagement.ts:31-45`):

```ts
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
```

**Pattern B — surface and rethrow** (`src/crosshook-native/src/hooks/useCommunityProfiles.ts:244-264`):

```ts
  const syncTaps = useCallback(async () => {
    setSyncing(true);
    setError(null);

    try {
      const results = await callCommand<CommunityTapSyncResult[]>('community_sync');
      setLastTapSyncResults(results);
      ...
    } catch (syncError) {
      setError(syncError instanceof Error ? syncError.message : String(syncError));
      throw syncError;
    } finally {
      setSyncing(false);
    }
  }, [refreshProfiles]);
```

**Pattern C — log + throw, no hook state** (`src/crosshook-native/src/hooks/useProfile.ts:536-547`):

```ts
const toggleFavorite = useCallback(
  async (name: string, favorite: boolean) => {
    try {
      await callCommand('profile_set_favorite', { name, favorite });
      await loadFavorites();
    } catch (err) {
      console.error('Failed to update profile favorite state', err);
      throw err;
    }
  },
  [loadFavorites]
);
```

**Invoke-error normaliser** — hand-written helper lives in `useProfile.ts:128-146` and is **not exported**:

```ts
/** Tauri invoke failures are sometimes plain objects, not Error instances. */
function formatInvokeError(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  if (typeof err === 'string') {
    return err;
  }
  if (err && typeof err === 'object') {
    const message = (err as { message?: unknown }).message;
    if (typeof message === 'string' && message.length > 0) {
      return message;
    }
  }
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}
```

The usual inline pattern is `err instanceof Error ? err.message : String(err)` (used in both `useLauncherManagement.ts:42` and `useCommunityProfiles.ts:260`). **GAP**: `formatInvokeError` is not exported nor reused; hooks either copy the inline ternary or (in `useProfile.ts`) have access to the private helper.

**Toast/banner surfacing** — ad-hoc, per-page. `src/crosshook-native/src/components/pages/ProfilesPage.tsx:44-326` implements its own `RenameToast` interface, session-scoped dismissal, timer refs. There is no shared toast primitive. The only shared CSS class found is `.crosshook-rename-toast-dismiss` in theme.css — specific to ProfilesPage.

**GAP**: No shared toast, snackbar, or banner component. If Phase 2 needs inline error feedback inside the Collection modal, the pattern is `error` state string + conditional render of `<p className="crosshook-game-details-modal__warn">` (see GameDetailsModal.tsx:414-416 for the prior art).

---

### 4. Logging Patterns

**Frontend logging**: direct `console.error` / `console.warn` / `console.debug`. No shared logger util exists under `src/crosshook-native/src/lib/`.

`src/crosshook-native/src/lib/ipc.dev.ts:29-31`:

```ts
if (import.meta.env.DEV) {
  console.debug('[mock] callCommand', name, args);
}
```

`src/crosshook-native/src/hooks/useProfile.ts:542`:

```ts
console.error('Failed to update profile favorite state', err);
```

`src/crosshook-native/src/hooks/useProtonDbSuggestions.ts:95`:

```ts
console.warn('[protondb] dismiss failed', { profileName, appId, suggestionKey }, err);
```

Convention: bracketed tag prefix (`[mock]`, `[protondb]`) on warn/debug, plain message on error. No structured logger.

---

### 5. Type Definitions

#### 5a. Where Rust-mirror types live

TS types that mirror serde-serialized Rust structs live under `src/crosshook-native/src/types/*.ts`, barrel-exported via `src/crosshook-native/src/types/index.ts:1-23`:

```ts
export * from './profile';
export * from './profile-history';
...
export * from './launcher';
export * from './library';
...
```

Every type is **hand-written**, not generated. Example mirror — `src/crosshook-native/src/types/launcher.ts:1-29`:

```ts
export interface LauncherInfo {
  display_name: string;
  launcher_slug: string;
  script_path: string;
  desktop_entry_path: string;
  script_exists: boolean;
  desktop_entry_exists: boolean;
  is_stale: boolean;
}

export interface LauncherDeleteResult {
  script_deleted: boolean;
  desktop_entry_deleted: boolean;
  script_path: string;
  desktop_entry_path: string;
  script_skipped_reason?: string | null;
  desktop_entry_skipped_reason?: string | null;
}

export interface LauncherRenameResult {
  old_slug: string;
  new_slug: string;
  new_script_path: string;
  new_desktop_entry_path: string;
  script_renamed: boolean;
  desktop_entry_renamed: boolean;
  old_script_cleanup_warning?: string | null;
  old_desktop_entry_cleanup_warning?: string | null;
}
```

Field names are **snake_case** because Rust serde serializes struct fields with snake_case defaults.

#### 5b. `CollectionRow` TS type — missing

`Grep` for `CollectionRow|Collection` in `src/crosshook-native/src/types/` returned **no matches**. No `collections.ts` file exists in `src/types/`. **GAP**: the Phase 2 implementer must create `src/crosshook-native/src/types/collections.ts` to mirror `CollectionRow` from `crates/crosshook-core/src/metadata/models.rs` (referenced by the mock file header comment at `src/crosshook-native/src/lib/mocks/handlers/collections.ts:9-10`).

#### 5c. Mock-only `MockCollectionRow`

`src/crosshook-native/src/lib/mocks/handlers/collections.ts:11-18`:

```ts
// Shape mirrors Rust `CollectionRow` in
// crates/crosshook-core/src/metadata/models.rs (snake_case per serde default).
interface MockCollectionRow {
  collection_id: string;
  name: string;
  description: string | null;
  profile_count: number;
  created_at: string;
  updated_at: string;
}
```

This is an internal interface in the mock handler file, not exported or re-used. Phase 2 should create the public type in `src/types/collections.ts` and the mock should import it (or maintain a separate copy — established precedent varies).

---

### 6. Test Patterns

CLAUDE.md says "There is **no** configured frontend test framework". **This is partially out of date.**

`src/crosshook-native/package.json:13-15`:

```json
    "test:smoke": "playwright test",
    "test:smoke:update": "playwright test --update-snapshots",
    "test:smoke:install": "playwright install chromium"
```

`src/crosshook-native/package.json:30` has `"@playwright/test": "^1.59.0"` in devDependencies. Single test file exists: `src/crosshook-native/tests/smoke.spec.ts`. The test walks all 9 sidebar routes in browser dev mode (`?fixture=populated`), captures screenshots, and asserts no page errors / console errors. Lines 1-22:

```ts
import { test, expect, type Page } from '@playwright/test';

import type { AppRoute } from '../src/components/layout/Sidebar';
import { ROUTE_NAV_LABEL } from '../src/components/layout/routeMetadata';

/**
 * Smoke test the 9 application routes in browser dev mode.
 *
 * Routing model: CrossHook does NOT use URL-based routing. The sidebar is a
 * Radix `Tabs.Root` whose `value` is held in React state inside `AppShell`.
 * Navigation happens by clicking `Tabs.Trigger` elements (rendered as
 * `role="tab"` with `aria-current="page"` once active). Each test:
 *
 *   1. Loads the app at `/` (with `?fixture=populated` so mock handlers
 *      seed the in-memory store with synthetic profiles).
 *   2. Confirms the dev-mode chip is rendered (proves `__WEB_DEV_MODE__`
 *      is true and the mock IPC chunk loaded successfully).
 *   3. Clicks the sidebar trigger for the route under test.
 *   4. Asserts `aria-current="page"` flips to that trigger.
 *   5. Captures a full-page screenshot into `test-results/`.
 *   6. Asserts no uncaught page errors or `console.error` calls.
```

There are no unit tests for hooks or components. No Vitest. No `*.test.ts` files under `src/`.

**Verification loop for Phase 2:**

- `pnpm --filter crosshook-native run build` runs `tsc && vite build` (see `package.json:10`). This is the type-check gate.
- `./scripts/dev-native.sh --browser` launches the loopback-only vite dev server with mocks for manual testing.
- `./scripts/dev-native.sh` runs full Tauri dev mode.
- `npm run test:smoke` runs the playwright route walk — will catch console errors triggered by new sidebar/modal code.
- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` — for Rust, not applicable to frontend-only Phase 2.
- `bash scripts/check-mock-coverage.sh` — mock coverage sentinel (`src/crosshook-native/package.json:9`: `"dev:browser:check": "bash ../../scripts/check-mock-coverage.sh"`).

---

### 7. Configuration

#### 7a. `callCommand` wrapper

`src/crosshook-native/src/lib/ipc.ts` (entire file, 17 lines):

```ts
// src/crosshook-native/src/lib/ipc.ts
import type { InvokeArgs } from '@tauri-apps/api/core';
import { isTauri } from './runtime';

declare const __WEB_DEV_MODE__: boolean;

export async function callCommand<T>(name: string, args?: InvokeArgs): Promise<T> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(name, args);
  }
  if (__WEB_DEV_MODE__) {
    const { runMockCommand } = await import('./ipc.dev');
    return runMockCommand<T>(name, args);
  }
  throw new Error('CrossHook commands require the Tauri desktop app or a webdev dev-server build.');
}
```

Every IPC call goes through `callCommand<T>(name, args?)`. It is a drop-in wrapper for Tauri's `invoke<T>` that transparently reroutes to the mock layer when the build is in browser dev mode. **Phase 2 hooks must use `callCommand`, not `invoke` directly** — the import path in hooks is `import { callCommand } from '@/lib/ipc';` (see `useLauncherManagement.ts:2`, `useCommunityProfiles.ts:2`).

#### 7b. Mock dispatcher

`src/crosshook-native/src/lib/ipc.dev.ts:21-33`:

```ts
export async function runMockCommand<T>(name: string, args?: InvokeArgs): Promise<T> {
  const map = await ensureMocks();
  const handler = map.get(name);
  if (!handler) {
    throw new Error(
      `[dev-mock] Unhandled command: ${name}. Add a handler in src/lib/mocks/handlers/<area>.ts — see lib/mocks/README.md`
    );
  }
  if (import.meta.env.DEV) {
    console.debug('[mock] callCommand', name, args);
  }
  return handler(args ?? {}) as Promise<T>;
}
```

#### 7c. ProfileContext shape

`src/crosshook-native/src/context/ProfileContext.tsx` (entire file, 78 lines, reproduced in full earlier). Relevant parts:

```tsx
export interface ProfileContextValue extends UseProfileResult {
  launchMethod: ResolvedLaunchMethod;
  steamClientInstallPath: string;
  targetHomePath: string;
}

const ProfileContext = createContext<ProfileContextValue | null>(null);

export function ProfileProvider({ children }: ProfileProviderProps) {
  const profileState = useProfile({ autoSelectFirstProfile: false });
  const launchMethod = resolveLaunchMethod(profileState.profile);
  ...
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

`ProfileProvider` wraps `useProfile` (a 55KB megahook) and exposes everything plus three derived values. **State shape today is `useState`-based inside `useProfile`, not `useReducer`.** Every mutation goes through `useCallback`-wrapped setters.

No `activeCollectionId` field exists yet. The PRD asks for it as ephemeral state on `ProfileContext`. Two precedents for adding state to `ProfileContext`:

- Extend `UseProfileResult` directly (additive — new useState in `useProfile.ts`, expose it in the hook return at `useProfile.ts:1611`).
- Extend `ProfileProviderProps`/`ProfileContextValue` without touching `useProfile` — add a local `useState` in `ProfileProvider`, merge into the memoized `value`.

The second approach keeps `useProfile` lean and is strictly safer for Phase 2 since collections are not profile CRUD. Both are viable; `ProfileContext.tsx:56-64` shows the memoize-merge pattern.

#### 7d. `useProfile` boot call pattern

`useProfile.ts:1280-1322` (effects for initial load/refresh); the relevant snippet at `useProfile.ts:1280-1284`:

```ts
    void refreshProfiles().catch((err: unknown) => {
      setError(err instanceof Error ? err.message : String(err));
    });
  }, [loadFavorites, refreshProfiles]);
```

---

### 8. Dependencies

`src/crosshook-native/package.json:17-28`:

```json
  "dependencies": {
    "@radix-ui/react-select": "^2.2.6",
    "@radix-ui/react-tabs": "^1.1.13",
    "@radix-ui/react-tooltip": "^1.2.8",
    "@tauri-apps/api": "^2.0.0",
    "@tauri-apps/plugin-dialog": "^2.7.0",
    "@tauri-apps/plugin-fs": "^2.5.0",
    "@tauri-apps/plugin-shell": "^2.3.5",
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "react-resizable-panels": "^4.7.6"
  },
```

- Radix UI installed: **Select, Tabs, Tooltip only.**
- **NOT installed**: `@radix-ui/react-dialog`, `@radix-ui/react-context-menu`, `@radix-ui/react-dropdown-menu`, `@radix-ui/react-popover`. The PRD forbids new deps, so modals and context menus must continue using the hand-rolled `createPortal` pattern.
- `createPortal` is imported from `react-dom`, e.g. `GameDetailsModal.tsx:1`.
- `useId` from `react` is used for `aria-labelledby`/`aria-describedby` (`GameDetailsModal.tsx:2-10`, `GameDetailsModal.tsx:125-126`).

---

## Part B — 5 Traces

### 1. Entry Points

**Sidebar mount**: `src/crosshook-native/src/App.tsx:96-102` renders `<Sidebar>` inside a `Tabs.Root` (`App.tsx:82-88`) whose value is `route: AppRoute` held in `useState` in `AppShell`:

```tsx
<Panel className="crosshook-shell-panel" defaultSize="20%" minSize="14%" maxSize="40%">
  <Sidebar activeRoute={route} onNavigate={setRoute} controllerMode={controllerMode} lastProfile={lastProfile} />
</Panel>
```

`onNavigate={setRoute}` is the only state-setter the Sidebar gets — so any button inside the Sidebar that opens the collection view modal has to call an alternative callback that **does not** set the route. Phase 2 must add a new prop or a parallel mechanism (likely a new prop `onOpenCollection: (id: string) => void` or similar).

**LibraryPage mount**: `src/crosshook-native/src/components/layout/ContentArea.tsx:54-55` — `case 'library': return <LibraryPage onNavigate={onNavigate} />;` inside `Tabs.Content` (`ContentArea.tsx:67-69`).

**Profile cards rendering**: `src/crosshook-native/src/components/pages/LibraryPage.tsx:127-137` renders `<LibraryGrid>`, which at `src/crosshook-native/src/components/library/LibraryGrid.tsx:43-55` maps to `<LibraryCard>` instances.

**Existing right-click on profile cards**: none. `src/crosshook-native/src/components/library/LibraryCard.tsx:70-151` has only click and keyboard handlers — no `onContextMenu` in the whole file. The card has a hit-box button for details (`LibraryCard.tsx:72-77`) and per-action buttons for Launch/Favorite/Edit (`LibraryCard.tsx:110-147`).

### 2. Data Flow (click sidebar collection → modal rendered with filtered profiles)

1. User clicks a chip/nav item inside `<CollectionsSection>` mounted in `Sidebar.tsx`. The Sidebar calls a new callback (NOT `onNavigate`).
2. Callback lives in `App.tsx` (or a new `CollectionsContext`) — sets `activeCollectionId: string | null` state, and sets `isCollectionModalOpen: boolean`. PRD says `activeCollectionId` should live in `ProfileContext` as ephemeral state.
3. `<CollectionViewModal>` (new) mounted somewhere above/adjacent to `ContentArea` reads `activeCollectionId` from context + a local `open` boolean.
4. Modal calls `useCollections().collections` to resolve the display name/description for the active id.
5. Modal calls `useCollectionMembers(activeCollectionId)` → IPC `collection_list_profiles({ collectionId })` → returns `string[]` of member profile names (see `crates/crosshook-core/src/metadata/collections.rs:125-129` list sort by sort_order, name — and the Tauri command `src-tauri/src/commands/collections.rs:55-63` which takes `collection_id: String` and returns `Vec<String>`).
6. Modal reads the full profile summaries from `useProfileContext().profiles` (which is `string[]`) — but **the library uses `useLibrarySummaries` to get `LibraryCardData[]`** (`src/crosshook-native/src/hooks/useLibrarySummaries.ts`, and `LibraryPage.tsx:31`). Summaries map profile names → card data. For the collection modal to show cards, it must either (a) re-use `useLibrarySummaries` OR (b) just render names.
7. Modal filters `summaries` by `new Set(memberNames)` → passes to `useLibraryProfiles(summaries, searchQuery)` (at `src/crosshook-native/src/hooks/useLibraryProfiles.ts:4-17`) for the inner search box:

```ts
export function useLibraryProfiles(profiles: LibraryCardData[], searchQuery: string): LibraryCardData[] {
  return useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) return profiles;
    return profiles.filter((p) => p.name.toLowerCase().includes(query) || p.gameName.toLowerCase().includes(query));
  }, [profiles, searchQuery]);
}
```

8. Modal launches/edits through `selectProfile()` + route change (same as `LibraryPage.tsx:54-74`):

```ts
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

The close-then-navigate indirection lives in `src/crosshook-native/src/components/library/game-details-actions.ts` (entire file reproduced earlier): `closeModal(); void launch(profileName);` — this guarantees focus returns to the originating chip/button before route change races. Phase 2 modal should reuse this exact pattern.

### 3. State Changes

`ProfileContext` has no dispatcher — state mutations flow through `useProfile` setters that are all `useCallback`-wrapped closures over `useState` (`useProfile.ts` uses ~40 `useState` calls across its 1600-line body; e.g. `useProfile.ts:600-624` `refreshProfiles` mutates `profiles`, `selectedProfile`, `profileName`, `profile`, `dirty`, `lastSavedLaunchOptimizationIdsRef`).

Favorites (closest precedent for "chip-like membership toggle"): `useProfile.ts:536-547` (`toggleFavorite`) → `callCommand('profile_set_favorite', { name, favorite })` → `await loadFavorites()` → `callCommand<string[]>('profile_list_favorites')` → `setFavoriteProfiles(names)`. The pattern is: mutate backend, then refresh the entire source of truth list.

**Where should `activeCollectionId` live?**

- Extending `ProfileContext` directly (per PRD) is the path of least disruption: add `activeCollectionId: string | null` + `setActiveCollectionId: (id: string | null) => void` to `ProfileContextValue` and `ProfileProvider`. This avoids adding a new Provider layer in `App.tsx:160-164`.
- Alternative: a new `CollectionsContext` in `src/context/CollectionsContext.tsx` that owns both `activeCollectionId` and wraps `useCollections`/`useCollectionMembers`. More modular but requires mounting an extra Provider in `App.tsx` (currently `ProfileProvider > ProfileHealthProvider > AppShell > PreferencesProvider > LaunchStateProvider`).

Either is consistent with repo conventions — the precedent for a dedicated context wrapping a single hook exists at `src/crosshook-native/src/context/LaunchStateContext.tsx:18-37`:

```tsx
export function LaunchStateProvider({ children }: { children: ReactNode }) {
  const profileState = useProfileContext();
  ...
  const launchState = useLaunchState({
    profileId,
    profileName: selectedName,
    method: profileState.launchMethod,
    request,
  });

  return <LaunchStateContext.Provider value={launchState}>{children}</LaunchStateContext.Provider>;
}
```

### 4. Contracts

**`useLibraryProfiles` contract** — `src/crosshook-native/src/hooks/useLibraryProfiles.ts:4-17` (entire file, reproduced above). Pure memo filter, `(profiles: LibraryCardData[], searchQuery: string) => LibraryCardData[]`. No IPC, no side effects. Safe to compose from the modal.

**`GameDetailsModal` props** — `src/crosshook-native/src/components/library/GameDetailsModal.tsx:91-103`:

```ts
export interface GameDetailsModalProps {
  open: boolean;
  summary: LibraryCardData | null;
  onClose: () => void;
  healthByName: Partial<Record<string, EnrichedProfileHealthReport>>;
  healthLoading: boolean;
  offlineReportFor: (profileName: string) => OfflineReadinessReport | undefined;
  offlineError: string | null;
  onLaunch: (name: string) => void | Promise<void>;
  onEdit: (name: string) => void | Promise<void>;
  onToggleFavorite: (name: string, current: boolean) => void;
  launchingName?: string;
}
```

Phase 2 `CollectionViewModal` props should mirror (open/onClose), but accept a `collectionId: string | null` instead of `summary`, and internally drive `useCollectionMembers(collectionId)` + `useLibrarySummaries()`.

**Select-then-navigate indirection** — `src/crosshook-native/src/components/library/game-details-actions.ts:6-22` (entire file):

```ts
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

Usage inside `GameDetailsModal.tsx:459` and `:474`:

```tsx
              onClick={() => gameDetailsLaunchThenNavigate(summary.name, onLaunch, onClose)}
              ...
              onClick={() => gameDetailsEditThenNavigate(summary.name, onEdit, onClose)}
```

Outer `onLaunch`/`onEdit` come from `LibraryPage.tsx:54-74` which are `selectProfile(name)` followed by `onNavigate?.('launch' | 'profiles')`.

**IPC arg contract for collection commands** — from `src/crosshook-native/src-tauri/src/commands/collections.rs` (reproduced earlier): `#[tauri::command] pub fn collection_add_profile(collection_id: String, profile_name: String, metadata_store: State<'_, MetadataStore>) -> Result<(), String>`. Tauri v2 converts Rust snake_case param names to camelCase in JS; the frontend must send `{ collectionId, profileName }`. See the **Gotchas** section for the mock handler inconsistency.

### 5. Patterns

**CSS variables** — `src/crosshook-native/src/styles/variables.css:1-50`. Uses `--crosshook-color-*`, `--crosshook-radius-*`, `--crosshook-shadow-*`, `--crosshook-font-*`. BEM-like `.crosshook-<block>__<element>--<modifier>`.

**Modal classes already in theme.css** — `src/crosshook-native/src/styles/theme.css:3690-3883`:

| Class                                                                    | Line      | Purpose                                                                                 |
| ------------------------------------------------------------------------ | --------- | --------------------------------------------------------------------------------------- |
| `.crosshook-modal-open`                                                  | 3690      | Body overflow lock class added by modal mount effect                                    |
| `.crosshook-modal-portal`                                                | 3694      | Portal host container (`z-index: 1200`)                                                 |
| `.crosshook-modal`                                                       | 3699      | Fixed positioning grid wrapper                                                          |
| `.crosshook-modal__backdrop`                                             | 3708      | Blurred backdrop, click-to-close target                                                 |
| `.crosshook-modal__surface`                                              | 3716      | Main panel, `min(1120px, ...)` width, grid-template-rows for header/summary/body/footer |
| `.crosshook-modal__header`                                               | 3734      | Sticky top header with border-bottom                                                    |
| `.crosshook-modal__heading-block`, `__title`, `__description`            | 3748-3765 | Heading stack                                                                           |
| `.crosshook-modal__header-actions`, `__close`                            | 3768-3778 | Close button placement                                                                  |
| `.crosshook-modal__summary`, `-item`, `-label`, `-value`, `-value--mono` | 3780-3813 | Summary chip grid                                                                       |
| `.crosshook-modal__status-chip` and `--neutral/success/warning/danger`   | 3815-3849 | Status pill variants                                                                    |
| `.crosshook-modal__body`                                                 | 3851      | `overflow-y: auto`, padding, background                                                 |
| `.crosshook-modal__footer`, `-copy`, `-actions`                          | 3859-3883 | Sticky bottom footer                                                                    |

**Scroll container registration** — `src/crosshook-native/src/hooks/useScrollEnhance.ts:8-9` (the single selector string consumed by the global wheel handler and the arrow-key handler):

```ts
const SCROLLABLE =
  '.crosshook-route-card-scroll, .crosshook-page-scroll-body, .crosshook-subtab-content__inner--scroll, .crosshook-console-drawer__body, .crosshook-modal__body, .crosshook-prefix-deps__log-output, .crosshook-discovery-results';
```

`.crosshook-modal__body` is already registered. Any **new** scroll container in Phase 2 (e.g. a collection member list inside the modal that is _not_ `.crosshook-modal__body`, or the sidebar collections list if it overflows) must be added to this string AND use `overscroll-behavior: contain`. See `GameDetailsModal.css:13-15`:

```css
.crosshook-game-details-modal__body {
  overscroll-behavior: contain;
}
```

**Architectural patterns observed**:

- **Megahook + thin page component.** `useProfile.ts` is 1600 lines, `LibraryPage.tsx` is 160 lines. Pages are render-only glue; domain logic lives in hooks. Phase 2 should follow: `useCollections.ts` + `useCollectionMembers.ts` hold logic, `CollectionsSection.tsx` and `CollectionViewModal.tsx` are thin render components.
- **Context as megahook wrapper.** `ProfileContext` wraps `useProfile` wholesale. `LaunchStateContext` wraps `useLaunchState`. Phase 2 either adds to `ProfileContext` (for `activeCollectionId`) or creates `CollectionsContext` wrapping the two new hooks.
- **Co-located domain folders.** `components/library/` holds every library-specific component, hook (`useGameDetailsModalState.ts`), action helper (`game-details-actions.ts`), CSS module (`GameDetailsModal.css`). Phase 2 should create `components/collections/` mirroring this layout.
- **Radix used only for Tabs, Select, Tooltip.** Modals are hand-rolled, not Radix Dialog. Context menus do not exist.
- **Browser dev-mode first.** Every IPC call has a mock, asserted by `scripts/check-mock-coverage.sh`. Phase 2 hooks are automatically covered because Phase 1 shipped the mocks — modulo the snake_case/camelCase mismatch flagged below.

---

## Part C — Critical files the Phase 2 implementer must read/touch/mirror

### Layout & routing

- `src/crosshook-native/src/App.tsx:78-142` — `AppShell` composition, Provider order, sidebar and ContentArea placement.
- `src/crosshook-native/src/components/layout/Sidebar.tsx:1-173` — entire file; `SIDEBAR_SECTIONS` (:36-61), `SidebarTrigger` (:63-86), `SidebarProps` (:18-23), render loop (:134-168).
- `src/crosshook-native/src/components/layout/ContentArea.tsx:1-77` — route → page switch.
- `src/crosshook-native/src/components/layout/routeMetadata.ts` — nav label source (used by sidebar and playwright smoke test).

### Modals (precedent for `<CollectionViewModal>` + shared `<Modal>` extraction)

- `src/crosshook-native/src/components/library/GameDetailsModal.tsx:1-487` — entire file; the richest portal modal precedent.
- `src/crosshook-native/src/components/library/GameDetailsModal.css:1-265` — modal-specific overrides, `overscroll-behavior: contain`, mobile breakpoint.
- `src/crosshook-native/src/components/library/useGameDetailsModalState.ts:1-28` — state hook precedent for open/close helpers.
- `src/crosshook-native/src/components/library/game-details-actions.ts:1-23` — close-then-navigate helpers.
- `src/crosshook-native/src/components/OfflineTrainerInfoModal.tsx:1-177` — minimal focus-trap modal precedent (no portal host, no inert siblings).
- `src/crosshook-native/src/components/LauncherPreviewModal.tsx:1-80` — portal-host pattern duplicated from `GameDetailsModal`.
- `src/crosshook-native/src/components/OnboardingWizard.tsx` — `.crosshook-onboarding-wizard.crosshook-modal__surface` variant (`theme.css:3885-3887`).
- `src/crosshook-native/src/components/ProfilePreviewModal.tsx`, `ProfileReviewModal.tsx`, `MigrationReviewModal.tsx`, `ConfigHistoryPanel.tsx`, `LaunchPanel.tsx` — additional `createPortal` callsites the Phase 2 `<Modal>` primitive could eventually subsume (out of Phase 2 scope but called out in PRD "Should" line).

### Library (mirror source for `<CollectionViewModal>` body)

- `src/crosshook-native/src/components/pages/LibraryPage.tsx:1-160` — entire file; `handleLaunch`, `handleEdit`, `handleToggleFavorite`, modal wiring.
- `src/crosshook-native/src/components/library/LibraryCard.tsx:1-152` — entire file; the card needs a new `onContextMenu` handler for the Phase 2 "Add to collection" right-click menu.
- `src/crosshook-native/src/components/library/LibraryGrid.tsx:1-61` — card list renderer.
- `src/crosshook-native/src/components/PinnedProfilesStrip.tsx:1-62` — chip/badge pattern, unpin-X precedent, `is_favorite`-style UX parallel.
- `src/crosshook-native/src/hooks/useLibraryProfiles.ts:1-18` — entire file; search filter. Must be composed inside `<CollectionViewModal>`.
- `src/crosshook-native/src/hooks/useLibrarySummaries.ts` — converts `profiles` + `favoriteProfiles` to `LibraryCardData[]`. The modal will need this to render cards.

### Pages the Active-Profile dropdown lives on

- `src/crosshook-native/src/components/pages/LaunchPage.tsx:287-306` — Active-Profile `ThemedSelect` slot:

  ```tsx
          profileSelectSlot={
            <ThemedSelect
              id="launch-profile-selector"
              value={profileState.selectedProfile}
              onValueChange={(name) => void profileState.selectProfile(name)}
              placeholder="Select a profile"
              pinnedValues={pinnedSet}
              onTogglePin={handleTogglePin}
              ariaLabelledby="launch-active-profile-label"
              options={profileState.profiles.map((name) => ({ value: name, label: name }))}
            />
          }
  ```

- `src/crosshook-native/src/components/pages/ProfilesPage.tsx:593-614` — Active-Profile select:

  ```tsx
  <div className="crosshook-launch-panel__profile-row-select">
    <ThemedSelect
      id="profile-selector-top"
      value={selectedProfile}
      onValueChange={(val) => void selectProfile(val)}
      placeholder="Create New"
      options={[{ value: '', label: 'Create New' }, ...profiles.map((name) => ({ value: name, label: name }))]}
    />
  </div>
  ```

Phase 2 filter-by-active-collection requires replacing `profileState.profiles.map(...)` in both places with a computed `filteredProfiles` that respects `activeCollectionId`. **Breaking change risk**: `LaunchPage` passes `pinnedValues` for favorites; the filtered list must still honour that. `ProfilesPage` includes a sentinel `{ value: '', label: 'Create New' }` item — the filter must not strip it.

### Context

- `src/crosshook-native/src/context/ProfileContext.tsx:1-78` — entire file; where `activeCollectionId` should live per PRD.
- `src/crosshook-native/src/context/LaunchStateContext.tsx:1-46` — entire file; precedent for a dedicated context wrapping a single hook.
- `src/crosshook-native/src/context/PreferencesContext.tsx:1-?` — may hold UI preferences; reference only.
- `src/crosshook-native/src/hooks/useProfile.ts:42-107, 536-547, 600-624, 1280-1322` — `UseProfileResult` shape, `toggleFavorite`, `refreshProfiles`, boot effect.

### IPC & mocks

- `src/crosshook-native/src/lib/ipc.ts:1-17` — `callCommand<T>` wrapper; Phase 2 hooks import this.
- `src/crosshook-native/src/lib/ipc.dev.ts:1-33` — mock dispatcher; errors prefixed `[dev-mock]`.
- `src/crosshook-native/src/lib/mocks/handlers/collections.ts:1-177` — **ALL 9 handlers** (pre-existing from Phase 1). Note the argument naming mismatch below.
- `src/crosshook-native/src/lib/mocks/index.ts:25, 51` — `registerCollections(map)` is already wired.
- `src/crosshook-native/src-tauri/src/commands/collections.rs:1-95` — entire file; canonical Rust argument names (`collection_id`, `profile_name`, `new_name`, `description`).
- `src/crosshook-native/src-tauri/src/lib.rs:281-289` — command registration in `generate_handler!`.
- `src/crosshook-native/crates/crosshook-core/src/metadata/collections.rs:1-120+` — backend logic; `ORDER BY sort_order ASC, name ASC` invariant (:12).

### Example hooks to mirror

- `src/crosshook-native/src/hooks/useLauncherManagement.ts:1-104` — **best structural precedent** for `useCollections`.
- `src/crosshook-native/src/hooks/useCommunityProfiles.ts:207-478` — larger precedent with initial load, event subscription, multi-flag state.

### UI primitives

- `src/crosshook-native/src/components/ui/ThemedSelect.tsx:1-215` — entire file; the select used in the two Active-Profile dropdowns.
- `src/crosshook-native/src/components/ui/InfoTooltip.tsx` — Radix Tooltip wrapper; pattern reference only.
- `src/crosshook-native/src/components/ui/CollapsibleSection.tsx` — disclosure widget pattern; may be useful inside the modal if multi-section.

### Styles

- `src/crosshook-native/src/styles/variables.css:1-50` — theme tokens (colors, radii, fonts).
- `src/crosshook-native/src/styles/theme.css:3690-3883` — shared modal classes (every new modal uses these).
- `src/crosshook-native/src/styles/sidebar.css:76-200` — sidebar section/item/status CSS; Phase 2 collections section should reuse `crosshook-sidebar__section` / `__section-label` / `__section-items`.
- `src/crosshook-native/src/styles/library.css:265-293` — `.crosshook-library-empty` empty-state CTA precedent.

### Types

- `src/crosshook-native/src/types/library.ts:1-14` — `LibraryCardData` shape (entire file, 14 lines).
- `src/crosshook-native/src/types/launcher.ts:1-29` — entire file; snake_case mirror convention.
- `src/crosshook-native/src/types/index.ts:1-23` — barrel exports.
- `src/crosshook-native/src/types/collections.ts` — **does not exist**, must be created.

### Scrolling / scroll enhancement

- `src/crosshook-native/src/hooks/useScrollEnhance.ts:8-9` — `SCROLLABLE` selector string. **Any new `overflow-y: auto` container in Phase 2 must be appended here** or the enhanced scroll will target a parent (see CLAUDE.md and Gotchas).

### Tests

- `src/crosshook-native/tests/smoke.spec.ts:1-60+` — playwright route-walk; will screenshot and console-check every sidebar click after the collections section is added.
- `src/crosshook-native/playwright.config.ts` — test runner config.
- `scripts/check-mock-coverage.sh` — mock coverage CI sentinel.

---

## Part D — Gotchas and warnings

### D1. Mock handlers use snake_case args; production hooks must use camelCase

**Severity: BLOCKER for Phase 2 browser-dev mode.**

Tauri v2 automatically converts Rust snake_case command parameter names to camelCase on the frontend. Every other hook in the codebase uses camelCase:

- `useLauncherManagement.ts:52-56` sends `{ launcherSlug, targetHomePath, steamClientInstallPath }` to Rust `launcher_slug: String, target_home_path: &str, steam_client_install_path: &str`.
- `useProtonDbSuggestions.ts:94` sends `{ profileName, appId, suggestionKey }` to Rust `profile_name: String, app_id: String, suggestion_key: String`.
- `useProfile.ts:1133-1135` sends `{ oldName: ..., newName: ... }` to Rust `old_name: String, new_name: String`.

Other mock handlers follow the same camelCase convention:

- `src/crosshook-native/src/lib/mocks/handlers/protondb.ts:135` — `const { appId, profileName } = args as { appId: string; profileName: string; };`.

**But `src/crosshook-native/src/lib/mocks/handlers/collections.ts` destructures snake_case keys** on nearly every command:

- `:82` — `const { collection_id } = args as { collection_id: string };`
- `:89-92` — `const { collection_id, profile_name } = args as { collection_id: string; profile_name: string; };`
- `:115-118` — same
- `:126` — `const { collection_id } = args as { collection_id: string };`
- `:132-135` — `const { collection_id, new_name } = args as { collection_id: string; new_name: string; };`
- `:153-156` — `const { collection_id, description } = args as { collection_id: string; description: string | null; };`
- `:170` — `const { profile_name } = args as { profile_name: string };`

When Phase 2 hooks call `callCommand('collection_add_profile', { collectionId, profileName })`, the mock handler will destructure `undefined` from `collection_id`/`profile_name` and every collection mutation will silently fail in browser dev mode. The production Tauri path will work.

**Phase 2 must either** (a) rewrite the mock handler to destructure camelCase keys (consistent with all other mocks + the Tauri auto-conversion), **or** (b) use snake_case arg names in the hooks and break the rest of the codebase's convention. Option (a) is the only consistent fix.

No `#[tauri::command(rename_all = "...")]` attribute is present on any collection command (verified via grep: only `serde(rename_all = ...)` attrs exist, on unrelated types). Default Tauri v2 behaviour is camelCase.

### D2. `.crosshook-modal__body` is already in the SCROLLABLE selector — new containers are not

`src/crosshook-native/src/hooks/useScrollEnhance.ts:8-9`:

```ts
const SCROLLABLE =
  '.crosshook-route-card-scroll, .crosshook-page-scroll-body, .crosshook-subtab-content__inner--scroll, .crosshook-console-drawer__body, .crosshook-modal__body, .crosshook-prefix-deps__log-output, .crosshook-discovery-results';
```

Using `.crosshook-modal__body` for the modal body inherits the scroll enhancement automatically. If Phase 2 adds:

- a **sidebar** collections list that overflows (likely above a certain count), add its class to SCROLLABLE.
- a **sub-scroller inside** the modal body (e.g. a dedicated grid container with `overflow-y: auto` nested inside `.crosshook-modal__body`), add it **and** set `overscroll-behavior: contain` to prevent the outer body from swallowing scrolls.
- a **member-search results area** with independent scroll, add it.

Forgetting this causes dual-scroll jank on WebKitGTK (CLAUDE.md explicitly warns).

### D3. Mock error strings must start with `[dev-mock]`

`src/crosshook-native/src/lib/mocks/handlers/collections.ts:1-5`:

```ts
// Mock IPC handlers for collection_* commands. See `lib/mocks/README.md`.
// All error messages MUST start with `[dev-mock]` to participate in the
// `.github/workflows/release.yml` "Verify no mock code in production bundle"
// sentinel.
```

Any new thrown error inside mock handlers (e.g. when fixing D1) must keep the `[dev-mock]` prefix intact.

### D4. IPC command names are snake_case; frontend arg keys are camelCase

Command **names** stay snake_case (`collection_list`, `collection_add_profile`, `collections_for_profile`) because they match `#[tauri::command] pub fn <name>`. Arg **keys** are camelCase because Tauri v2 normalises Rust snake_case param names. Do not confuse the two.

### D5. Sidebar collections section cannot be `Tabs.Trigger`

`src/crosshook-native/src/App.tsx:82` wraps the whole sidebar+content in `<Tabs.Root>`, and `Sidebar.tsx:134` renders its navigation inside `<Tabs.List>`. Radix enforces that `Tabs.Trigger` children of a `Tabs.List` have `value` matching a valid tab panel. Collections are not routes — clicking them opens a modal, not a route — so they must be rendered as plain `<button>` elements **outside** the `<Tabs.List>` region (probably as a new section in the sidebar footer above `StatusRow`), or as a separate `<div className="crosshook-sidebar__section">` placed between/after `Tabs.List` but still inside `<aside className="crosshook-sidebar">`.

Reusing the `.crosshook-sidebar__item` class on a plain `<button>` is safe (class is pure CSS), but the `data-state='active'` styling (sidebar.css:144-149) will need a manual counterpart if Phase 2 wants a highlighted "currently open" collection.

### D6. Browser dev mode transparency

`src/crosshook-native/src/lib/ipc.ts:7-17` handles the Tauri/browser-dev-mode split at one central chokepoint. Phase 2 hooks only ever import `callCommand` from `@/lib/ipc` and never import `invoke` from `@tauri-apps/api/core` directly. This is non-negotiable because the browser dev mode ships with no Tauri runtime — a direct `invoke` import would crash on module load.

### D7. `CollectionRow` TS type is missing entirely

`src/crosshook-native/src/types/collections.ts` does not exist. The only `CollectionRow`-shaped type in the TS codebase is the unexported `MockCollectionRow` interface at `collections.ts:11-18` (mock handler). Phase 2 must create the public type in `src/types/` and re-export it via `src/types/index.ts`. Use snake_case field names (`collection_id`, `profile_count`, `created_at`, `updated_at`) to match the serde output from `CollectionRow` in `crates/crosshook-core/src/metadata/models.rs`, consistent with `LauncherInfo` etc.

### D8. `PinnedProfilesStrip.tsx` is dead code at present

Grep for `PinnedProfilesStrip` returns only self-referential matches inside the file. No page or component imports it. The CSS classes (`crosshook-pinned-strip*`) may not be fully exercised in the running app. If Phase 2 re-uses chip CSS classes, verify they render by smoke-testing the sidebar first.

### D9. `formatInvokeError` is private to `useProfile.ts`

`useProfile.ts:128-146` has a well-written helper but it is unexported. Phase 2 hooks will need to either copy the inline ternary `err instanceof Error ? err.message : String(err)` (used by `useLauncherManagement` and `useCommunityProfiles`), or promote `formatInvokeError` into a shared util (e.g. `src/crosshook-native/src/utils/errors.ts`). Choose one and be consistent.

### D10. `CLAUDE.md` "no frontend test framework" is stale

`package.json:13-15` defines `test:smoke` (playwright). The CI currently runs this smoke test. Phase 2 changes to `Sidebar.tsx` or the route walk may require screenshot updates (`test:smoke:update`) and must not introduce `console.error` calls (the test asserts against page errors and `console.error` calls — see `smoke.spec.ts:54-62`). That means **hooks must not `console.error` on expected outcomes** (e.g. empty lists) during smoke-test navigation.

### D11. `handleGamepadBack` closes modals via a DOM attribute

`src/crosshook-native/src/App.tsx:39-45`:

```ts
function handleGamepadBack(): void {
  const closeButtons = document.querySelectorAll<HTMLButtonElement>(
    '[data-crosshook-focus-root="modal"] [data-crosshook-modal-close]'
  );
  const closeButton = closeButtons[closeButtons.length - 1];
  closeButton?.click();
}
```

The `<CollectionViewModal>` close button must carry `data-crosshook-modal-close` and the surface wrapper must carry `data-crosshook-focus-root="modal"` for controller back-button support. `GameDetailsModal.tsx:315-340` sets both.

### D12. Body lock + `crosshook-modal-open` class is additive

If two modals open simultaneously, the body lock remains. `OfflineTrainerInfoModal.tsx` does **not** touch `body.style.overflow` and **does not** add `crosshook-modal-open` — so opening it on top of another modal does not overwrite the parent's lock. `GameDetailsModal.tsx:209-211` does set both. Phase 2 should pick the fuller (`GameDetailsModal`) pattern since the collection modal is the top-level modal here.

### D13. Inert sibling technique is browser-gated

`GameDetailsModal.tsx:213-221` sets `inert` on every body child except the portal host. `inert` is a relatively recent HTML attribute but Tauri's WebKitGTK webview supports it. The cast `(element as HTMLElement & { inert?: boolean })` is because older TS lib.dom type definitions lacked `inert`. Phase 2 can copy-paste this idiom; no additional work needed if the `<Modal>` primitive is extracted from `GameDetailsModal`.

### D14. Shared `<Modal>` primitive extraction risk

Six portal modals already exist and each has minor variation (portal-host vs direct-body mount, inert vs not, `useId` vs hardcoded IDs). A shared primitive must accommodate all six to be a useful refactor; otherwise Phase 2 will ship a seventh variant. **Recommended approach during plan** (for caller synthesis, not a directive here): scope the extraction to a new `components/ui/Modal.tsx` that covers the "full" GameDetailsModal pattern, migrate the collection modal as the first consumer, and leave existing modals untouched until a follow-up task. The PRD listed this as a "should" not a "must" for a reason.

---

## GAPs

- **GAP**: No `src/types/collections.ts` exists. Phase 1 did not create a frontend type.
- **GAP**: No shared toast/snackbar/banner primitive. Error feedback inside the new modal must be ad-hoc (inline `<p className="crosshook-modal__body__warn">` or similar) or reuse ProfilesPage's pattern.
- **GAP**: No existing right-click / context menu implementation anywhere in the frontend. Phase 2 will be the first. No Radix context menu or popover dependency is installed.
- **GAP**: No multi-select pattern anywhere in the codebase for "assign to multiple collections" (grep for `Set<string>` + checkbox turned up only internal state like `enabled_option_ids`, not UI multi-select).
- **GAP**: Mock handler argument naming is inconsistent with the rest of the codebase. Phase 2 must resolve (see D1).
- **GAP**: `PinnedProfilesStrip.tsx` exists but is not mounted anywhere. No current reference page or user flow to verify the chip visuals against.
- **GAP**: No shared `formatInvokeError` util — private copy in `useProfile.ts:128-146`.
- **GAP**: No empty-state primitive beyond the ad-hoc `.crosshook-library-empty` class in `library.css:265-293`. The modal empty-state CTA will need similar ad-hoc CSS or a new `components/ui/EmptyState.tsx`.

---

## Summary table (for quick scan)

| Concern                                   | Answer                                                                                       | Source                                               |
| ----------------------------------------- | -------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| IPC wrapper                               | `callCommand<T>(name, args?)` from `@/lib/ipc`                                               | `src/crosshook-native/src/lib/ipc.ts:7`              |
| IPC arg convention                        | camelCase (Tauri v2 auto-converts)                                                           | `useProfile.ts:1133`, `useProtonDbSuggestions.ts:94` |
| IPC command name                          | snake_case                                                                                   | `src-tauri/src/commands/collections.rs:8-94`         |
| TS mirror field convention                | snake_case                                                                                   | `types/launcher.ts:1-29`                             |
| Modal root                                | hand-rolled `createPortal` + focus trap + body lock                                          | `GameDetailsModal.tsx:182-253, 310-483`              |
| Modal CSS                                 | `.crosshook-modal`, `__backdrop`, `__surface`, `__header`, `__summary`, `__body`, `__footer` | `theme.css:3690-3883`                                |
| Scroll container registration             | `useScrollEnhance.ts` SCROLLABLE selector                                                    | `useScrollEnhance.ts:8-9`                            |
| Modal body class already registered       | yes (`.crosshook-modal__body`)                                                               | `useScrollEnhance.ts:9`                              |
| Overscroll-contain precedent              | `GameDetailsModal.css:13-15`                                                                 | same                                                 |
| Sidebar layout                            | Radix `Tabs.Root` + `Tabs.List` + `Tabs.Trigger`                                             | `App.tsx:82-102`, `Sidebar.tsx:134`                  |
| Active profile dropdown locations         | `LaunchPage.tsx:295-306`, `ProfilesPage.tsx:603-614`                                         | —                                                    |
| Active-Profile component                  | `ThemedSelect` (Radix Select wrapper)                                                        | `components/ui/ThemedSelect.tsx:86-161`              |
| Profile CRUD error pattern                | inline `setError(err instanceof Error ? err.message : String(err))`                          | `useLauncherManagement.ts:42`                        |
| Refresh-after-mutate convention           | always call list refresh after mutation                                                      | `useLauncherManagement.ts:57`                        |
| Where `activeCollectionId` should live    | `ProfileContext` (per PRD) — extend `ProfileContextValue`                                    | `ProfileContext.tsx:16-20, 56-64`                    |
| Select-then-navigate indirection          | `close(); void launch(name);`                                                                | `game-details-actions.ts:6-22`                       |
| Empty-state CTA precedent                 | `crosshook-library-empty` div + CTA button                                                   | `LibraryGrid.tsx:27-40`, `library.css:265-293`       |
| Existing context menus                    | none                                                                                         | grep result                                          |
| Existing toast primitive                  | none                                                                                         | grep result                                          |
| Tauri `rename_all` on collection commands | none                                                                                         | grep result                                          |
| Playwright smoke test                     | walks 9 routes, screenshots, asserts no errors                                               | `tests/smoke.spec.ts:1-60`                           |
| Dependencies (usable)                     | `@radix-ui/react-select`, `react-tabs`, `react-tooltip`, `react`, `react-dom`                | `package.json:17-28`                                 |
