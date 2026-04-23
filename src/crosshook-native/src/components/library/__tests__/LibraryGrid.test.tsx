import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { makeLibraryCardData } from '@/test/fixtures';
import { LibraryGrid } from '../LibraryGrid';

vi.mock('../LibraryCard', () => ({
  LibraryCard: ({
    profile,
    isSelected,
    isLaunching,
    onSelect,
  }: {
    profile: { name: string };
    isSelected?: boolean;
    isLaunching?: boolean;
    onSelect?: (name: string) => void;
  }) => (
    <li
      data-testid={`card-${profile.name}`}
      data-selected={String(Boolean(isSelected))}
      data-launching={String(Boolean(isLaunching))}
    >
      <button type="button" onClick={() => onSelect?.(profile.name)}>
        select-{profile.name}
      </button>
    </li>
  ),
}));

describe('LibraryGrid', () => {
  it('renders an empty state and routes the CTA to profiles', async () => {
    const onNavigate = vi.fn();

    render(
      <LibraryGrid
        profiles={[]}
        onNavigate={onNavigate}
        onOpenDetails={vi.fn()}
        onLaunch={vi.fn()}
        onEdit={vi.fn()}
        onToggleFavorite={vi.fn()}
      />
    );

    expect(screen.getByText('No game profiles yet')).toBeInTheDocument();
    await screen.getByRole('button', { name: 'Create a profile' }).click();
    expect(onNavigate).toHaveBeenCalledWith('profiles');
  });

  it('renders one card per profile', () => {
    render(
      <LibraryGrid
        profiles={[makeLibraryCardData(), makeLibraryCardData({ name: 'Dev Test Game', steamAppId: '9999002' })]}
        onOpenDetails={vi.fn()}
        onLaunch={vi.fn()}
        onEdit={vi.fn()}
        onToggleFavorite={vi.fn()}
      />
    );

    expect(screen.getByTestId('card-Synthetic Quest')).toBeInTheDocument();
    expect(screen.getByTestId('card-Dev Test Game')).toBeInTheDocument();
  });

  it('calls onSelect when a mocked card button is clicked', async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <LibraryGrid
        profiles={[makeLibraryCardData(), makeLibraryCardData({ name: 'Dev Test Game', steamAppId: '9999002' })]}
        onSelect={onSelect}
        onOpenDetails={vi.fn()}
        onLaunch={vi.fn()}
        onEdit={vi.fn()}
        onToggleFavorite={vi.fn()}
      />
    );

    await user.click(screen.getByRole('button', { name: 'select-Synthetic Quest' }));
    expect(onSelect).toHaveBeenCalledWith('Synthetic Quest');
  });

  it('passes selected and launching state to child cards', () => {
    render(
      <LibraryGrid
        profiles={[makeLibraryCardData(), makeLibraryCardData({ name: 'Dev Test Game', steamAppId: '9999002' })]}
        selectedName="Synthetic Quest"
        launchingName="Dev Test Game"
        onOpenDetails={vi.fn()}
        onLaunch={vi.fn()}
        onEdit={vi.fn()}
        onToggleFavorite={vi.fn()}
      />
    );

    expect(screen.getByTestId('card-Synthetic Quest')).toHaveAttribute('data-selected', 'true');
    expect(screen.getByTestId('card-Dev Test Game')).toHaveAttribute('data-launching', 'true');
  });
});
