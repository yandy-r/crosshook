import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { LibraryList } from '../LibraryList';

vi.mock('../LibraryListRow', () => ({
  LibraryListRow: ({ profile }: { profile: { name: string } }) => (
    <li data-testid={`row-${profile.name}`}>{profile.name}</li>
  ),
}));

describe('LibraryList', () => {
  it('renders empty state and invokes onAddGame from the CTA', async () => {
    const onAddGame = vi.fn();

    render(
      <LibraryList
        profiles={[]}
        hasNoProfiles
        onAddGame={onAddGame}
        onOpenDetails={vi.fn()}
        onLaunch={vi.fn()}
        onEdit={vi.fn()}
        onToggleFavorite={vi.fn()}
      />
    );

    expect(screen.getByText('Add your first game')).toBeInTheDocument();
    await screen.getByRole('button', { name: 'Add game' }).click();
    expect(onAddGame).toHaveBeenCalledTimes(1);
  });

  it('renders rows when profiles exist', () => {
    render(
      <LibraryList
        profiles={[
          {
            name: 'Synthetic Quest',
            gameName: 'Synthetic Quest',
            steamAppId: '1',
            networkIsolation: false,
            isFavorite: false,
          },
        ]}
        onOpenDetails={vi.fn()}
        onLaunch={vi.fn()}
        onEdit={vi.fn()}
        onToggleFavorite={vi.fn()}
      />
    );

    expect(screen.getByTestId('row-Synthetic Quest')).toBeInTheDocument();
  });
});
