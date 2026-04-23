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

  it('renders hover-reveal layer and favorite heart with expected aria state', () => {
    const { rerender } = renderWithMocks(
      <LibraryCard {...defaultProps} profile={makeLibraryCardData({ isFavorite: false })} />
    );

    expect(document.querySelector('.crosshook-library-card__hover-reveal')).toBeInTheDocument();
    const heart = screen.getByRole('button', { name: 'Toggle favorite: Synthetic Quest' });
    expect(heart).toHaveAttribute('aria-pressed', 'false');

    rerender(<LibraryCard {...defaultProps} profile={makeLibraryCardData({ isFavorite: true })} />);
    expect(screen.getByRole('button', { name: 'Toggle favorite: Synthetic Quest' })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
  });

  it('heart click invokes onToggleFavorite', async () => {
    const user = userEvent.setup();
    const onToggleFavorite = vi.fn();
    const profile = makeLibraryCardData({ isFavorite: true });
    renderWithMocks(<LibraryCard {...defaultProps} onToggleFavorite={onToggleFavorite} profile={profile} />);
    await user.click(screen.getByRole('button', { name: 'Toggle favorite: Synthetic Quest' }));
    expect(onToggleFavorite).toHaveBeenCalledWith('Synthetic Quest', true);
  });

  it('selects on single hitbox click when onSelect is set (after double-click guard)', async () => {
    const user = userEvent.setup();
    const onOpenDetails = vi.fn();
    const onSelect = vi.fn();

    renderWithMocks(<LibraryCard {...defaultProps} onOpenDetails={onOpenDetails} onSelect={onSelect} />);

    const hitbox = screen.getByRole('button', { name: 'Select Synthetic Quest' });
    await user.click(hitbox);

    await waitFor(() => {
      expect(onSelect).toHaveBeenCalledTimes(1);
    });
    expect(onOpenDetails).not.toHaveBeenCalled();
  });

  it('opens details on double-clicking the hitbox when onSelect is set without selecting twice', async () => {
    const user = userEvent.setup();
    const onOpenDetails = vi.fn();
    const onSelect = vi.fn();

    renderWithMocks(<LibraryCard {...defaultProps} onOpenDetails={onOpenDetails} onSelect={onSelect} />);

    const hitbox = screen.getByRole('button', { name: 'Select Synthetic Quest' });
    await user.dblClick(hitbox);

    expect(onSelect).not.toHaveBeenCalled();
    expect(onOpenDetails).toHaveBeenCalledTimes(1);
    expect(onOpenDetails).toHaveBeenCalledWith('Synthetic Quest');
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
