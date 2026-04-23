import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { makeLibraryCardData } from '@/test/fixtures';
import { renderWithMocks } from '@/test/render';
import type { LaunchHistoryEntry } from '@/types/library';
import GameInspector from '../GameInspector';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand: cmd } = await import('@/test/render');
  return { callCommand: cmd };
});

function mount(ui: ReactElement, options?: Parameters<typeof renderWithMocks>[1]) {
  return renderWithMocks(
    <ProfileProvider>
      <ProfileHealthProvider>{ui}</ProfileHealthProvider>
    </ProfileProvider>,
    options
  );
}

describe('GameInspector', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows empty state when selection is missing', () => {
    mount(<GameInspector />);
    expect(screen.getByText('Select a game to see details')).toBeInTheDocument();
  });

  it('renders hero and actions when selection is set', async () => {
    const user = userEvent.setup();
    const onLaunch = vi.fn();
    const onEditProfile = vi.fn();
    const onToggleFavorite = vi.fn();
    const selection = makeLibraryCardData({ name: 'Alpha', gameName: 'Alpha Game' });

    mount(
      <GameInspector
        selection={selection}
        onLaunch={onLaunch}
        onEditProfile={onEditProfile}
        onToggleFavorite={onToggleFavorite}
      />
    );

    expect(screen.getByText('Alpha Game')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /Launch Alpha Game/i }));
    expect(onLaunch).toHaveBeenCalledWith('Alpha');
    await user.click(screen.getByRole('button', { name: 'Edit profile' }));
    expect(onEditProfile).toHaveBeenCalledWith('Alpha');
    await user.click(screen.getByRole('button', { name: 'Add favorite' }));
    expect(onToggleFavorite).toHaveBeenCalledWith('Alpha', false);
  });

  it('renders the health section heading for a selection', () => {
    const selection = makeLibraryCardData({ name: 'HealthGame' });
    mount(<GameInspector selection={selection} />);
    expect(screen.getByRole('heading', { name: 'Health' })).toBeInTheDocument();
  });

  it('renders recent launches from list_launch_history_for_profile', async () => {
    const rows: LaunchHistoryEntry[] = [
      {
        operation_id: 'op-test-1',
        launch_method: 'native',
        status: 'succeeded',
        started_at: '2026-02-01T10:00:00.000Z',
        finished_at: '2026-02-01T10:00:10.000Z',
        exit_code: 0,
        signal: null,
        severity: 'info',
        failure_mode: 'clean_exit',
      },
    ];
    const selection = makeLibraryCardData({ name: 'LaunchGame' });
    mount(<GameInspector selection={selection} />, {
      handlerOverrides: {
        list_launch_history_for_profile: async () => rows,
      },
    });

    await waitFor(() => {
      expect(screen.getByText('Succeeded')).toBeInTheDocument();
    });
    expect(screen.getByText(/native/)).toBeInTheDocument();
  });

  it('shows empty copy when launch history is empty', async () => {
    const selection = makeLibraryCardData({ name: 'NoHistory' });
    mount(<GameInspector selection={selection} />, {
      handlerOverrides: {
        list_launch_history_for_profile: async () => [],
      },
    });
    await waitFor(() => {
      expect(screen.getByText('No recent launches recorded for this profile.')).toBeInTheDocument();
    });
  });
});
