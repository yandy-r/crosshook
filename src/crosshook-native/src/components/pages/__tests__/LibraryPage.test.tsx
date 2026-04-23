import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Inspector } from '@/components/layout/Inspector';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { InspectorSelectionProvider, useInspectorSelection } from '@/context/InspectorSelectionContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { renderWithMocks } from '@/test/render';
import { LibraryPage } from '../LibraryPage';

function LibraryPageWithInspector() {
  const { inspectorSelection, libraryInspectorHandlers } = useInspectorSelection();
  return (
    <div style={{ display: 'flex' }}>
      <div style={{ flex: '1 1 50%', minWidth: 0 }}>
        <LibraryPage />
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

function renderLibraryHarness() {
  return renderWithMocks(
    <ProfileProvider>
      <ProfileHealthProvider>
        <HostReadinessProvider>
          <CollectionsProvider>
            <InspectorSelectionProvider>
              <LibraryPageWithInspector />
            </InspectorSelectionProvider>
          </CollectionsProvider>
        </HostReadinessProvider>
      </ProfileHealthProvider>
    </ProfileProvider>
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

  it('fires the command palette placeholder on ⌘K trigger', async () => {
    const user = userEvent.setup();
    const debug = vi.mocked(console.debug);
    renderLibraryHarness();

    await user.click(screen.getByRole('button', { name: 'Open command palette' }));
    expect(debug).toHaveBeenCalled();
  });
});
