import { fireEvent, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeLibraryCardData } from '@/test/fixtures';
import { mockCallCommand, renderWithMocks } from '@/test/render';
import { triggerIntersection } from '@/test/setup';
import { LibraryCard } from '../LibraryCard';

vi.mock('@/lib/ipc', () => ({
  callCommand: mockCallCommand,
}));

describe('LibraryCard', () => {
  const defaultProps = {
    profile: makeLibraryCardData(),
    onOpenDetails: vi.fn(),
    onLaunch: vi.fn(),
    onEdit: vi.fn(),
    onToggleFavorite: vi.fn(),
    onContextMenu: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('loads cover art after the card enters the viewport', async () => {
    const profile = makeLibraryCardData({ customPortraitArtPath: '' });

    renderWithMocks(<LibraryCard {...defaultProps} profile={profile} />, {
      handlerOverrides: {
        fetch_game_cover_art: async () => '/mock/media/synthetic-quest.png',
      },
    });

    const listItem = screen.getByRole('listitem');

    triggerIntersection(listItem, true);

    await waitFor(() => {
      expect(screen.getByRole('img', { name: 'Synthetic Quest' })).toHaveAttribute(
        'src',
        '/mock/media/synthetic-quest.png'
      );
    });
  });

  it('opens the context menu from mouse and keyboard shortcuts', async () => {
    const user = userEvent.setup();
    const onContextMenu = vi.fn();

    renderWithMocks(<LibraryCard {...defaultProps} onContextMenu={onContextMenu} />);

    const listItem = screen.getByRole('listitem');
    const detailsButton = screen.getByRole('button', { name: 'View details for Synthetic Quest' });

    fireEvent.contextMenu(listItem, { clientX: 12, clientY: 18 });
    expect(onContextMenu).toHaveBeenCalledWith({ x: 12, y: 18 }, 'Synthetic Quest', listItem);

    detailsButton.focus();
    await user.keyboard('{Shift>}{F10}{/Shift}');

    expect(onContextMenu).toHaveBeenCalledTimes(2);
    expect(onContextMenu.mock.calls[1]?.[1]).toBe('Synthetic Quest');
  });

  it('invokes launch, favorite, and edit callbacks from footer actions', async () => {
    const user = userEvent.setup();
    const onLaunch = vi.fn();
    const onEdit = vi.fn();
    const onToggleFavorite = vi.fn();

    renderWithMocks(
      <LibraryCard
        {...defaultProps}
        onLaunch={onLaunch}
        onEdit={onEdit}
        onToggleFavorite={onToggleFavorite}
        profile={makeLibraryCardData({ isFavorite: true })}
      />
    );

    await user.click(screen.getByRole('button', { name: 'Launch Synthetic Quest' }));
    await user.click(screen.getByRole('button', { name: 'Unfavorite Synthetic Quest' }));
    await user.click(screen.getByRole('button', { name: 'Edit Synthetic Quest' }));

    expect(onLaunch).toHaveBeenCalledWith('Synthetic Quest');
    expect(onToggleFavorite).toHaveBeenCalledWith('Synthetic Quest', true);
    expect(onEdit).toHaveBeenCalledWith('Synthetic Quest');
  });
});
