import * as Tooltip from '@radix-ui/react-tooltip';
import { act, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ComponentProps } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Inspector } from '@/components/layout/Inspector';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { InspectorSelectionProvider, useInspectorSelection } from '@/context/InspectorSelectionContext';
import { LaunchStateProvider } from '@/context/LaunchStateContext';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import type { Handler } from '@/lib/mocks';
import { getStore } from '@/lib/mocks/store';
import { renderWithMocks } from '@/test/render';
import type { LibraryFilterIntent, OpenGameDetailIntent } from '@/types/navigation';
import { DEFAULT_APP_SETTINGS, DEFAULT_UI_SETTINGS } from '@/types/settings';
import { LibraryPage } from '../LibraryPage';

vi.mock('@/lib/ipc', async () => {
  // vi.mock factories run before module evaluation, so the shared mock dispatcher
  // must be imported lazily here (same convention as LibraryPage.test.tsx).
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});
vi.mock('@/components/OnboardingWizard', () => ({
  OnboardingWizard: () => null,
}));

// Capture the `ready` flag LibraryPage computes instead of firing real timers:
// the hook's contract (no fire while false) is covered in useBenchReady.test.ts.
const useBenchReadyMock = vi.hoisted(() => vi.fn());
vi.mock('../../../hooks/useBenchReady', () => ({ useBenchReady: useBenchReadyMock }));

interface LibraryPageHarnessProps {
  libraryFilterIntent?: LibraryFilterIntent | null;
  openGameDetailIntent?: OpenGameDetailIntent | null;
  onOpenCommandPalette?: ComponentProps<typeof LibraryPage>['onOpenCommandPalette'];
}

function LibraryPageWithInspector({
  libraryFilterIntent,
  openGameDetailIntent,
  onOpenCommandPalette,
}: LibraryPageHarnessProps = {}) {
  const { inspectorSelection, libraryInspectorHandlers } = useInspectorSelection();
  return (
    <div style={{ display: 'flex' }}>
      <div style={{ flex: '1 1 50%', minWidth: 0 }}>
        <LibraryPage
          libraryFilterIntent={libraryFilterIntent}
          openGameDetailIntent={openGameDetailIntent}
          onOpenCommandPalette={onOpenCommandPalette}
        />
      </div>
      <div style={{ flex: '0 0 320px' }}>
        <Inspector
          route="library"
          width={320}
          selection={inspectorSelection}
          onLaunch={libraryInspectorHandlers?.onLaunch}
          onEditProfile={libraryInspectorHandlers?.onEditProfile}
          onToggleFavorite={libraryInspectorHandlers?.onToggleFavorite}
        />
      </div>
    </div>
  );
}

function renderLibraryHarness(options: Parameters<typeof renderWithMocks>[1] = {}) {
  return renderWithMocks(
    <ProfileProvider>
      <PreferencesProvider>
        <Tooltip.Provider delayDuration={0}>
          <ProfileHealthProvider>
            <HostReadinessProvider>
              <CollectionsProvider>
                <LaunchStateProvider>
                  <InspectorSelectionProvider>
                    <LibraryPageWithInspector />
                  </InspectorSelectionProvider>
                </LaunchStateProvider>
              </CollectionsProvider>
            </HostReadinessProvider>
          </ProfileHealthProvider>
        </Tooltip.Provider>
      </PreferencesProvider>
    </ProfileProvider>,
    options
  );
}

describe('LibraryPage view mode from settings', () => {
  beforeEach(() => {
    useBenchReadyMock.mockClear();
    vi.spyOn(console, 'debug').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders list view when settings_load returns ui.library_view_mode = list (native-written value)', async () => {
    renderLibraryHarness({
      handlerOverrides: {
        settings_load: async () => ({
          ...DEFAULT_APP_SETTINGS,
          ui: { ...DEFAULT_UI_SETTINGS, library_view_mode: 'list' },
        }),
      },
    });

    const listButton = await screen.findByRole('button', { name: 'List view' });
    await waitFor(() => {
      expect(listButton).toHaveAttribute('aria-pressed', 'true');
    });
    // List rows render instead of grid cards.
    expect(await screen.findByRole('button', { name: 'Select Test Game Alpha' })).toBeInTheDocument();
    expect(document.querySelector('.crosshook-library-list-row')).not.toBeNull();
    expect(document.querySelector('.crosshook-library-card')).toBeNull();
  });

  it('toggle calls settings_set_library_view_mode and updates the mock store without touching localStorage', async () => {
    const user = userEvent.setup();
    const calls: unknown[] = [];
    renderLibraryHarness({
      seed: (handlers: Map<string, Handler>) => {
        const real = handlers.get('settings_set_library_view_mode');
        if (!real) {
          throw new Error('settings_set_library_view_mode mock not registered');
        }
        handlers.set('settings_set_library_view_mode', async (args) => {
          calls.push(args);
          return real(args);
        });
      },
    });

    await user.click(await screen.findByRole('button', { name: 'List view' }));

    await waitFor(() => {
      expect(calls).toEqual([{ mode: 'list' }]);
    });
    expect(getStore().settings.ui.library_view_mode).toBe('list');
    // The legacy localStorage key is never written anymore (and may be absent entirely in node).
    expect(globalThis.localStorage?.getItem('crosshook.library.viewMode') ?? null).toBeNull();
    expect(screen.getByRole('button', { name: 'List view' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('refreshes to the backend view mode when the command rejects', async () => {
    const user = userEvent.setup();
    const persistGate = Promise.withResolvers<void>();
    let loadCount = 0;
    vi.spyOn(console, 'error').mockImplementation(() => {});
    renderLibraryHarness({
      handlerOverrides: {
        settings_load: async () => {
          loadCount += 1;
          if (loadCount > 1) {
            // The recovery refresh fails too; the last confirmed mode is restored.
            throw new Error('load boom');
          }
          return {
            ...DEFAULT_APP_SETTINGS,
            ui: { ...DEFAULT_UI_SETTINGS, library_view_mode: 'grid' },
          };
        },
        settings_set_library_view_mode: () => persistGate.promise,
      },
    });
    await waitFor(() => expect(loadCount).toBe(1));

    await user.click(await screen.findByRole('button', { name: 'List view' }));
    expect(screen.getByRole('button', { name: 'List view' })).toHaveAttribute('aria-pressed', 'true');
    await act(async () => {
      persistGate.reject(new Error('persist boom'));
    });
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Grid view' })).toHaveAttribute('aria-pressed', 'true');
    });
    expect(loadCount).toBe(2);
    expect(document.querySelector('.crosshook-library-list-row')).toBeNull();
    expect(document.querySelector('.crosshook-library-card')).not.toBeNull();
  });

  it('keeps the optimistic toggle when a deferred settings_load resolves afterwards with the stale value', async () => {
    const user = userEvent.setup();
    const loadGate = Promise.withResolvers<unknown>();
    const persistGate = Promise.withResolvers<void>();
    renderLibraryHarness({
      handlerOverrides: {
        settings_load: () => loadGate.promise,
        settings_set_library_view_mode: async () => {
          const store = getStore();
          store.settings = {
            ...store.settings,
            ui: { ...store.settings.ui, library_view_mode: 'list' },
          };
          persistGate.resolve();
          return null;
        },
      },
    });

    // Toggle to list while the initial settings_load is still pending.
    await user.click(await screen.findByRole('button', { name: 'List view' }));
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'List view' })).toHaveAttribute('aria-pressed', 'true');
    });

    // The load now resolves with the stale grid value; the optimistic list mode must survive.
    loadGate.resolve({
      ...DEFAULT_APP_SETTINGS,
      ui: { ...DEFAULT_UI_SETTINGS, library_view_mode: 'grid' },
    });
    await act(async () => {});
    expect(screen.getByRole('button', { name: 'List view' })).toHaveAttribute('aria-pressed', 'true');
    expect(document.querySelector('.crosshook-library-list-row')).not.toBeNull();
    expect(document.querySelector('.crosshook-library-card')).toBeNull();

    // Persist settling does not itself replace the optimistic mode.
    await act(async () => {
      await persistGate.promise;
    });
    expect(screen.getByRole('button', { name: 'List view' })).toHaveAttribute('aria-pressed', 'true');
  });
});
