import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ComponentProps } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Inspector } from '@/components/layout/Inspector';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { InspectorSelectionProvider, useInspectorSelection } from '@/context/InspectorSelectionContext';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { renderWithMocks } from '@/test/render';
import type { LibraryFilterIntent } from '@/types/navigation';
import { LibraryPage } from '../LibraryPage';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

interface LibraryPageHarnessProps {
  libraryFilterIntent?: LibraryFilterIntent | null;
  onOpenCommandPalette?: ComponentProps<typeof LibraryPage>['onOpenCommandPalette'];
}

function LibraryPageWithInspector({ libraryFilterIntent, onOpenCommandPalette }: LibraryPageHarnessProps = {}) {
  const { inspectorSelection, libraryInspectorHandlers, libraryShellMode } = useInspectorSelection();
  return (
    <div style={{ display: 'flex' }}>
      <div style={{ flex: '1 1 50%', minWidth: 0 }}>
        <LibraryPage libraryFilterIntent={libraryFilterIntent} onOpenCommandPalette={onOpenCommandPalette} />
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
      <span data-testid="library-shell-mode">{libraryShellMode}</span>
    </div>
  );
}

function renderLibraryHarness(
  options: Parameters<typeof renderWithMocks>[1] = {},
  onOpenCommandPalette?: ComponentProps<typeof LibraryPage>['onOpenCommandPalette'],
  libraryFilterIntent?: LibraryFilterIntent | null
) {
  return renderWithMocks(
    <ProfileProvider>
      <PreferencesProvider>
        <ProfileHealthProvider>
          <HostReadinessProvider>
            <CollectionsProvider>
              <InspectorSelectionProvider>
                <LibraryPageWithInspector
                  libraryFilterIntent={libraryFilterIntent}
                  onOpenCommandPalette={onOpenCommandPalette}
                />
              </InspectorSelectionProvider>
            </CollectionsProvider>
          </HostReadinessProvider>
        </ProfileHealthProvider>
      </PreferencesProvider>
    </ProfileProvider>,
    options
  );
}

describe('LibraryPage', () => {
  beforeEach(() => {
    vi.spyOn(console, 'debug').mockImplementation(() => {});
    const memory = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      get length() {
        return memory.size;
      },
      clear: (): void => {
        memory.clear();
      },
      getItem: (k: string): string | null => memory.get(k) ?? null,
      key: (i: number): string | null => Array.from(memory.keys())[i] ?? null,
      removeItem: (k: string): void => {
        memory.delete(k);
      },
      setItem: (k: string, v: string): void => {
        memory.set(k, v);
      },
    } as Storage);
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('updates inspector hero when a card is selected', async () => {
    const user = userEvent.setup();
    renderLibraryHarness();

    const hits = await screen.findAllByRole('button', { name: /^Select /i });
    const first = hits[0];
    if (!first) {
      throw new Error('expected at least one library card');
    }
    await user.click(first);

    await waitFor(() => {
      expect(screen.getByTestId('inspector')).toHaveTextContent('Test Game Alpha');
    });
  });

  it('updates inspector when a list row is selected', async () => {
    const user = userEvent.setup();
    renderLibraryHarness();

    await user.click(await screen.findByRole('button', { name: 'List view' }));

    await user.click(await screen.findByRole('button', { name: 'Select Test Game Alpha' }));

    await waitFor(() => {
      expect(screen.getByTestId('inspector')).toHaveTextContent('Test Game Alpha');
    });
  });

  it('activates the Name sort chip when clicked', async () => {
    const user = userEvent.setup();
    renderLibraryHarness();

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Name' })).toBeInTheDocument();
    });

    await user.click(screen.getByRole('button', { name: 'Name' }));
    expect(screen.getByRole('button', { name: 'Name' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('delegates the command-palette trigger to callback', async () => {
    const user = userEvent.setup();
    const onOpenCommandPalette = vi.fn();
    renderLibraryHarness({}, onOpenCommandPalette);

    await user.click(screen.getByRole('button', { name: 'Open command palette' }));
    expect(onOpenCommandPalette).toHaveBeenCalledTimes(1);
  });

  it('publishes library shell mode for shell consumers', async () => {
    const user = userEvent.setup();
    renderLibraryHarness();

    expect(screen.getByTestId('library-shell-mode')).toHaveTextContent('library');

    await user.click(await screen.findByRole('button', { name: 'View details for Test Game Alpha' }));
    await waitFor(() => {
      expect(screen.getByTestId('library-shell-mode')).toHaveTextContent('detail');
    });

    await user.click(screen.getByRole('button', { name: 'Back' }));
    await waitFor(() => {
      expect(screen.getByTestId('library-shell-mode')).toHaveTextContent('library');
    });
  });

  it('enters hero detail from the details control and returns on Back without losing inspector selection', async () => {
    const user = userEvent.setup();
    renderLibraryHarness();

    await user.click(await screen.findByRole('button', { name: 'Select Test Game Alpha' }));
    await waitFor(() => {
      expect(screen.getByTestId('inspector')).toHaveTextContent('Test Game Alpha');
    });

    await user.click(screen.getByRole('button', { name: 'View details for Test Game Alpha' }));

    await waitFor(() => {
      expect(screen.getByTestId('game-detail')).toBeInTheDocument();
    });
    expect(screen.getByRole('button', { name: 'Back' })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Back' }));

    await waitFor(() => {
      expect(screen.queryByTestId('game-detail')).not.toBeInTheDocument();
    });
    expect(screen.getByRole('button', { name: 'Open command palette' })).toBeInTheDocument();
    expect(screen.getByTestId('inspector')).toHaveTextContent('Test Game Alpha');
  });

  it('preserves library search when returning from hero detail', async () => {
    const user = userEvent.setup();
    renderLibraryHarness();

    const searchInput = await screen.findByRole('searchbox', { name: /Search games/i });
    await user.type(searchInput, 'alpha');

    await user.click(screen.getByRole('button', { name: 'View details for Test Game Alpha' }));
    await waitFor(() => {
      expect(screen.getByTestId('game-detail')).toBeInTheDocument();
    });

    await user.click(screen.getByRole('button', { name: 'Back' }));
    await waitFor(() => {
      expect(screen.queryByTestId('game-detail')).not.toBeInTheDocument();
    });
    const searchAfterBack = screen.getByRole('searchbox', { name: /Search games/i });
    expect(searchAfterBack).toHaveValue('alpha');
  });

  it('opens hero detail from list view via the details icon', async () => {
    const user = userEvent.setup();
    renderLibraryHarness();

    await user.click(await screen.findByRole('button', { name: 'List view' }));
    await user.click(screen.getByRole('button', { name: 'View details for Test Game Alpha' }));

    await waitFor(() => {
      expect(screen.getByTestId('game-detail')).toBeInTheDocument();
    });
  });

  it('applies incoming favorite filter intent to the toolbar', async () => {
    renderLibraryHarness({}, undefined, { filterKey: 'favorites', token: 1 });

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Favorites' })).toHaveAttribute('aria-pressed', 'true');
    });
  });

  it('filters to running profiles from the runtime read hook', async () => {
    renderLibraryHarness(
      {
        handlerOverrides: {
          list_running_profiles: async () => ['Test Game Alpha'],
        },
      },
      undefined,
      { filterKey: 'currentlyRunning', token: 1 }
    );

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Running' })).toHaveAttribute('aria-pressed', 'true');
      expect(screen.getByRole('button', { name: 'Select Test Game Alpha' })).toBeInTheDocument();
    });
    expect(screen.queryByRole('button', { name: 'Select Dev Game Beta' })).not.toBeInTheDocument();
  });
});
