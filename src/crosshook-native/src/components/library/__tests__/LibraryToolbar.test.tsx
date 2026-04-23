import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { LibraryToolbar } from '../LibraryToolbar';

describe('LibraryToolbar', () => {
  const base = {
    searchQuery: '',
    onSearchChange: vi.fn(),
    viewMode: 'grid' as const,
    onViewModeChange: vi.fn(),
    sortBy: 'recent' as const,
    onSortChange: vi.fn(),
    filter: 'all' as const,
    onFilterChange: vi.fn(),
  };

  it('emits sort and filter changes from chip presses', async () => {
    const user = userEvent.setup();
    const onSortChange = vi.fn();
    const onFilterChange = vi.fn();

    render(<LibraryToolbar {...base} onSortChange={onSortChange} onFilterChange={onFilterChange} />);

    await user.click(screen.getByRole('button', { name: 'Name' }));
    expect(onSortChange).toHaveBeenCalledWith('name');

    await user.click(screen.getByRole('button', { name: 'Favorites' }));
    expect(onFilterChange).toHaveBeenCalledWith('favorites');
  });

  it('reflects aria-pressed for active sort and filter chips', () => {
    render(<LibraryToolbar {...base} sortBy="name" filter="favorites" />);

    expect(screen.getByRole('button', { name: 'Name' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: 'Recent' })).toHaveAttribute('aria-pressed', 'false');
    expect(screen.getByRole('button', { name: 'Favorites' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('invokes onOpenCommandPalette from the palette trigger', async () => {
    const user = userEvent.setup();
    const onOpenCommandPalette = vi.fn();

    render(<LibraryToolbar {...base} onOpenCommandPalette={onOpenCommandPalette} />);

    await user.click(screen.getByRole('button', { name: 'Open command palette' }));
    expect(onOpenCommandPalette).toHaveBeenCalledTimes(1);
  });

  it('tab order reaches search, chips, view toggle, then palette trigger', async () => {
    const user = userEvent.setup();
    render(<LibraryToolbar {...base} />);

    await user.tab();
    expect(screen.getByRole('searchbox', { name: 'Search games' })).toHaveFocus();

    await user.tab();
    expect(screen.getByRole('button', { name: 'Recent' })).toHaveFocus();

    for (let i = 0; i < 3; i += 1) {
      await user.tab();
    }
    expect(screen.getByRole('button', { name: 'Playtime' })).toHaveFocus();

    await user.tab();
    expect(screen.getByRole('button', { name: 'All' })).toHaveFocus();

    for (let i = 0; i < 3; i += 1) {
      await user.tab();
    }
    expect(screen.getByRole('button', { name: 'Recently Launched' })).toHaveFocus();

    await user.tab();
    expect(screen.getByRole('button', { name: 'Grid view' })).toHaveFocus();

    await user.tab();
    expect(screen.getByRole('button', { name: 'List view' })).toHaveFocus();

    await user.tab();
    expect(screen.getByRole('button', { name: 'Open command palette' })).toHaveFocus();
  });
});
