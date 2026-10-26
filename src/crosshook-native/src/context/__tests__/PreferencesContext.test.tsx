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
  const { libraryViewMode, setLibraryViewMode } = usePreferencesContext();
  return (
    <div>
      <span data-testid="library-view-mode">{libraryViewMode}</span>
      <button type="button" data-testid="set-list" onClick={() => setLibraryViewMode('list')}>
        list
      </button>
      <button type="button" data-testid="set-grid" onClick={() => setLibraryViewMode('grid')}>
        grid
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
    loadGate.resolve({
      ...DEFAULT_APP_SETTINGS,
      ui: { ...DEFAULT_UI_SETTINGS, library_view_mode: 'grid' },
    });
    await act(async () => {});
    expect(screen.getByTestId('library-view-mode')).toHaveTextContent('list');

    // Once the persist settles, the pending guard lifts and a later refresh applies as written.
    await act(async () => {});
  });
});
