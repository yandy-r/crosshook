import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Inspector } from '@/components/layout/Inspector';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { InspectorSelectionProvider, useInspectorSelection } from '@/context/InspectorSelectionContext';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { renderWithMocks } from '@/test/render';
import { LibraryPage } from '../LibraryPage';

interface LibraryPageHarnessProps {
  onOpenCommandPalette?: () => void;
}

function LibraryPageWithInspector({ onOpenCommandPalette }: LibraryPageHarnessProps = {}) {
  const { inspectorSelection, libraryInspectorHandlers } = useInspectorSelection();
  return (
    <div style={{ display: 'flex' }}>
      <div style={{ flex: '1 1 50%', minWidth: 0 }}>
        <LibraryPage onOpenCommandPalette={onOpenCommandPalette} />
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

function renderLibraryHarness(options: Parameters<typeof renderWithMocks>[1] = {}, onOpenCommandPalette?: () => void) {
  return renderWithMocks(
    <ProfileProvider>
      <PreferencesProvider>
        <ProfileHealthProvider>
          <HostReadinessProvider>
            <CollectionsProvider>
              <InspectorSelectionProvider>
                <LibraryPageWithInspector onOpenCommandPalette={onOpenCommandPalette} />
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

  it('enters hero detail from the details control and returns on Back without losing inspector selection', async () => {
    const user = userEvent.setup();
    renderLibraryHarness();

    const selectButtons = await screen.findAllByRole('button', { name: /^Select /i });
    await user.click(selectButtons[0]!);
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
});
