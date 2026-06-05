import { act, screen, waitFor, within } from '@testing-library/react';
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
import { emitMockEvent } from '@/lib/events';
import { renderWithMocks } from '@/test/render';
import type { LibraryFilterIntent, OpenGameDetailIntent } from '@/types/navigation';
import { LibraryPage } from '../LibraryPage';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

let lastWizardProps: Record<string, unknown> = {};

vi.mock('@/components/OnboardingWizard', () => ({
  OnboardingWizard: (props: Record<string, unknown>) => {
    lastWizardProps = props;
    return props['open'] ? <div role="dialog">Onboarding Wizard</div> : null;
  },
}));

interface LibraryPageHarnessProps {
  libraryFilterIntent?: LibraryFilterIntent | null;
  openGameDetailIntent?: OpenGameDetailIntent | null;
  onOpenCommandPalette?: ComponentProps<typeof LibraryPage>['onOpenCommandPalette'];
  onNavigate?: ComponentProps<typeof LibraryPage>['onNavigate'];
}

function LibraryPageWithInspector({
  libraryFilterIntent,
  openGameDetailIntent,
  onOpenCommandPalette,
  onNavigate,
}: LibraryPageHarnessProps = {}) {
  const { inspectorSelection, libraryInspectorHandlers, libraryShellMode } = useInspectorSelection();
  return (
    <div style={{ display: 'flex' }}>
      <div style={{ flex: '1 1 50%', minWidth: 0 }}>
        <LibraryPage
          libraryFilterIntent={libraryFilterIntent}
          openGameDetailIntent={openGameDetailIntent}
          onOpenCommandPalette={onOpenCommandPalette}
          onNavigate={onNavigate}
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
      <span data-testid="library-shell-mode">{libraryShellMode}</span>
    </div>
  );
}

function renderLibraryHarness(
  options: Parameters<typeof renderWithMocks>[1] = {},
  onOpenCommandPalette?: ComponentProps<typeof LibraryPage>['onOpenCommandPalette'],
  libraryFilterIntent?: LibraryFilterIntent | null,
  openGameDetailIntent?: OpenGameDetailIntent | null,
  onNavigate?: ComponentProps<typeof LibraryPage>['onNavigate']
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
                  openGameDetailIntent={openGameDetailIntent}
                  onOpenCommandPalette={onOpenCommandPalette}
                  onNavigate={onNavigate}
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
    lastWizardProps = {};
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

  it('calls onNavigate with gameDetailOrigin when Edit profile is triggered', async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    renderLibraryHarness({}, undefined, undefined, undefined, onNavigate);

    // Open game detail for Test Game Alpha
    await user.click(await screen.findByRole('button', { name: 'View details for Test Game Alpha' }));
    const gameDetail = await screen.findByTestId('game-detail');

    // Click Edit profile scoped to the game-detail panel (inspector also has one)
    // triggers gameDetailsEditThenNavigate (onBack then onEdit)
    await user.click(within(gameDetail).getByRole('button', { name: 'Edit profile' }));

    await waitFor(() => {
      expect(onNavigate).toHaveBeenCalledWith(
        'profiles',
        expect.objectContaining({
          gameDetailOrigin: {
            profileName: 'Test Game Alpha',
            displayName: 'Test Game Alpha',
          },
        })
      );
    });
  });

  it('calls onNavigate with gameDetailOrigin when Launch is triggered', async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    renderLibraryHarness({}, undefined, undefined, undefined, onNavigate);

    // Open game detail for Test Game Alpha
    await user.click(await screen.findByRole('button', { name: 'View details for Test Game Alpha' }));
    const gameDetail = await screen.findByTestId('game-detail');

    // Click Launch scoped to the game-detail panel
    // triggers gameDetailsLaunchThenNavigate (onBack then onLaunch)
    await user.click(within(gameDetail).getByRole('button', { name: 'Launch' }));

    await waitFor(() => {
      expect(onNavigate).toHaveBeenCalledWith(
        'launch',
        expect.objectContaining({
          gameDetailOrigin: {
            profileName: 'Test Game Alpha',
            displayName: 'Test Game Alpha',
          },
        })
      );
    });
  });

  it('opens game detail via openGameDetailIntent when the profile exists', async () => {
    renderLibraryHarness({}, undefined, undefined, { profileName: 'Test Game Alpha', token: 1 });

    await waitFor(() => {
      expect(screen.getByTestId('game-detail')).toBeInTheDocument();
    });
  });

  it('silently drops openGameDetailIntent for an unknown profile', async () => {
    renderLibraryHarness({}, undefined, undefined, { profileName: 'does-not-exist', token: 1 });

    // Wait for summaries to settle (a known card must be visible)
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Select Test Game Alpha' })).toBeInTheDocument();
    });

    expect(screen.queryByTestId('game-detail')).not.toBeInTheDocument();
  });

  describe('add-game wizard', () => {
    it('opens create-mode wizard without seed from the toolbar Add game button', async () => {
      const user = userEvent.setup();
      renderLibraryHarness();

      await waitFor(() => {
        expect(screen.getByRole('button', { name: 'Add game' })).toBeInTheDocument();
      });

      await user.click(screen.getByRole('button', { name: 'Add game' }));

      expect(screen.getByRole('dialog')).toBeInTheDocument();
      expect(lastWizardProps['mode']).toBe('create');
      expect(lastWizardProps['createSeed']).toBeUndefined();
    });

    it('opens the same wizard from the empty-library CTA', async () => {
      const user = userEvent.setup();
      renderLibraryHarness({
        handlerOverrides: {
          profile_list_summaries: async () => [],
          profile_list: async () => [],
          profile_list_favorites: async () => [],
        },
      });

      await waitFor(() => {
        expect(screen.getByRole('button', { name: 'Add game' })).toBeInTheDocument();
      });

      const addButtons = screen.getAllByRole('button', { name: 'Add game' });
      await user.click(addButtons[addButtons.length - 1]!);

      expect(screen.getByRole('dialog')).toBeInTheDocument();
      expect(lastWizardProps['mode']).toBe('create');
      expect(lastWizardProps['createSeed']).toBeUndefined();
    });

    it('selects the created profile and updates the inspector on complete', async () => {
      const user = userEvent.setup();
      renderLibraryHarness();

      await waitFor(() => {
        expect(screen.getByRole('button', { name: 'Select Test Game Alpha' })).toBeInTheDocument();
      });

      await user.click(screen.getByRole('button', { name: 'Select Test Game Alpha' }));
      await waitFor(() => {
        expect(screen.getByTestId('inspector')).toHaveTextContent('Test Game Alpha');
      });

      await user.click(screen.getByRole('button', { name: 'Add game' }));
      expect(screen.getByRole('dialog')).toBeInTheDocument();

      const onComplete = lastWizardProps['onComplete'] as (name?: string) => void;
      act(() => {
        onComplete('Dev Game Beta');
      });

      await waitFor(() => {
        expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
      });
      expect(screen.getByTestId('inspector')).toHaveTextContent('Dev Game Beta');
    });

    it('refreshes library cards when profiles-changed fires', async () => {
      let fetchCount = 0;
      renderLibraryHarness({
        handlerOverrides: {
          profile_list_summaries: async () => {
            fetchCount += 1;
            if (fetchCount === 1) {
              return [];
            }
            return [
              {
                name: 'Fresh Game',
                gameName: 'Fresh Game',
                steamAppId: '9999099',
                networkIsolation: false,
              },
            ];
          },
          profile_list: async () => (fetchCount > 1 ? ['Fresh Game'] : []),
        },
      });

      await waitFor(() => {
        expect(screen.getByText('Add your first game')).toBeInTheDocument();
      });

      emitMockEvent('profiles-changed', '');

      await waitFor(() => {
        expect(screen.getByRole('button', { name: 'Select Fresh Game' })).toBeInTheDocument();
      });
    });

    it('restores prior inspector selection when the wizard is dismissed', async () => {
      const user = userEvent.setup();
      renderLibraryHarness();

      await waitFor(() => {
        expect(screen.getByRole('button', { name: 'Select Test Game Alpha' })).toBeInTheDocument();
      });

      await user.click(screen.getByRole('button', { name: 'Select Test Game Alpha' }));
      await waitFor(() => {
        expect(screen.getByTestId('inspector')).toHaveTextContent('Test Game Alpha');
      });

      await user.click(screen.getByRole('button', { name: 'Add game' }));

      const onDismiss = lastWizardProps['onDismiss'] as () => void;
      act(() => {
        onDismiss();
      });

      await waitFor(() => {
        expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
      });
      expect(screen.getByTestId('inspector')).toHaveTextContent('Test Game Alpha');
    });
  });
});
