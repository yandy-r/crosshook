import { act, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PreferencesProvider, usePreferencesContext } from '@/context/PreferencesContext';
import { renderWithMocks } from '@/test/render';
import type { AppSettingsData } from '@/types';
import { DEFAULT_APP_SETTINGS, DEFAULT_UI_SETTINGS } from '@/types/settings';

vi.mock('@/lib/ipc', async () => {
  // vi.mock factories run before module evaluation, so the shared mock dispatcher
  // must be imported lazily here (same convention as the other context tests).
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

function ViewModeProbe() {
  const { libraryViewMode, refreshPreferences, setLibraryViewMode } = usePreferencesContext();
  return (
    <div>
      <span data-testid="library-view-mode">{libraryViewMode}</span>
      <button type="button" data-testid="set-list" onClick={() => setLibraryViewMode('list')}>
        list
      </button>
      <button type="button" data-testid="set-grid" onClick={() => setLibraryViewMode('grid')}>
        grid
      </button>
      <button type="button" data-testid="refresh" onClick={() => void refreshPreferences()}>
        refresh
      </button>
    </div>
  );
}

function renderPreferences(options: Parameters<typeof renderWithMocks>[1] = {}) {
  return renderWithMocks(
    <PreferencesProvider>
      <ViewModeProbe />
    </PreferencesProvider>,
    options
  );
}

function settingsWithViewMode(mode: 'grid' | 'list'): AppSettingsData {
  return {
    ...DEFAULT_APP_SETTINGS,
    ui: { ...DEFAULT_UI_SETTINGS, library_view_mode: mode },
  };
}

describe('PreferencesContext library view mode', () => {
  beforeEach(() => {
    vi.spyOn(console, 'debug').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('reflects ui.library_view_mode from settings_load', async () => {
    renderPreferences({
      handlerOverrides: {
        settings_load: async () => ({
          ...DEFAULT_APP_SETTINGS,
          ui: { ...DEFAULT_UI_SETTINGS, library_view_mode: 'list' },
        }),
      },
    });

    await waitFor(() => {
      expect(screen.getByTestId('library-view-mode')).toHaveTextContent('list');
    });
  });

  it('keeps the optimistic toggle when a deferred settings_load resolves afterwards with the stale value', async () => {
    const user = userEvent.setup();
    const loadGate = Promise.withResolvers<AppSettingsData>();
    renderPreferences({
      handlerOverrides: {
        settings_load: () => loadGate.promise,
      },
    });

    // Optimistic default while the load is pending.
    expect(screen.getByTestId('library-view-mode')).toHaveTextContent('grid');

    await user.click(screen.getByTestId('set-list'));
    expect(screen.getByTestId('library-view-mode')).toHaveTextContent('list');

    // The load resolves with the stale grid value; the optimistic list mode must survive.
    loadGate.resolve(settingsWithViewMode('grid'));
    await act(async () => {});
    expect(screen.getByTestId('library-view-mode')).toHaveTextContent('list');
  });

  it('keeps the latest mode through out-of-order settles, then applies a later refresh', async () => {
    const user = userEvent.setup();
    const staleLoad = Promise.withResolvers<AppSettingsData>();
    const listPersist = Promise.withResolvers<void>();
    const gridPersist = Promise.withResolvers<void>();
    let loadCount = 0;
    let backendMode: 'grid' | 'list' = 'grid';

    renderPreferences({
      handlerOverrides: {
        settings_load: async () => {
          loadCount += 1;
          if (loadCount === 2) {
            return staleLoad.promise;
          }
          return settingsWithViewMode(backendMode);
        },
        settings_set_library_view_mode: (args) => {
          const { mode } = args as { mode?: string };
          return mode === 'list' ? listPersist.promise : gridPersist.promise;
        },
      },
    });
    await waitFor(() => expect(loadCount).toBe(1));

    await user.click(screen.getByTestId('set-list'));
    await user.click(screen.getByTestId('set-grid'));
    expect(screen.getByTestId('library-view-mode')).toHaveTextContent('grid');

    // Start a load before the latest persist settles.
    await user.click(screen.getByTestId('refresh'));
    await waitFor(() => expect(loadCount).toBe(2));

    await act(async () => {
      gridPersist.resolve();
    });
    backendMode = 'list';
    await act(async () => {
      staleLoad.resolve(settingsWithViewMode(backendMode));
    });
    expect(screen.getByTestId('library-view-mode')).toHaveTextContent('grid');

    await act(async () => {
      listPersist.resolve();
    });
    await user.click(screen.getByTestId('refresh'));
    await waitFor(() => {
      expect(screen.getByTestId('library-view-mode')).toHaveTextContent('list');
    });
  });

  it('applies a different backend value on refresh after the persist succeeds', async () => {
    const user = userEvent.setup();
    const persistGate = Promise.withResolvers<void>();
    const backendMode: 'grid' | 'list' = 'list';
    renderPreferences({
      handlerOverrides: {
        settings_load: async () => settingsWithViewMode(backendMode),
        settings_set_library_view_mode: () => persistGate.promise,
      },
    });

    await waitFor(() => {
      expect(screen.getByTestId('library-view-mode')).toHaveTextContent('list');
    });
    await user.click(screen.getByTestId('set-grid'));
    expect(screen.getByTestId('library-view-mode')).toHaveTextContent('grid');
    await act(async () => {
      persistGate.resolve();
    });

    await user.click(screen.getByTestId('refresh'));
    await waitFor(() => {
      expect(screen.getByTestId('library-view-mode')).toHaveTextContent('list');
    });
  });

  it('restores the backend value through refresh when the persist fails', async () => {
    const user = userEvent.setup();
    const persistGate = Promise.withResolvers<void>();
    let loadCount = 0;
    vi.spyOn(console, 'error').mockImplementation(() => {});
    renderPreferences({
      handlerOverrides: {
        settings_load: async () => {
          loadCount += 1;
          return settingsWithViewMode('grid');
        },
        settings_set_library_view_mode: () => persistGate.promise,
      },
    });
    await waitFor(() => expect(loadCount).toBe(1));

    await user.click(screen.getByTestId('set-list'));
    expect(screen.getByTestId('library-view-mode')).toHaveTextContent('list');
    await act(async () => {
      persistGate.reject(new Error('persist boom'));
    });

    await waitFor(() => {
      expect(screen.getByTestId('library-view-mode')).toHaveTextContent('grid');
    });
    expect(loadCount).toBe(2);
  });
});
