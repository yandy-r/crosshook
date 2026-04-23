import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { makeLibraryCardData } from '@/test/fixtures';
import { renderWithMocks } from '@/test/render';
import GameInspector from '../GameInspector';

function mount(ui: ReactElement) {
  return renderWithMocks(
    <ProfileProvider>
      <ProfileHealthProvider>{ui}</ProfileHealthProvider>
    </ProfileProvider>
  );
}

describe('GameInspector', () => {
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
});
