import * as Tabs from '@radix-ui/react-tabs';
import { act, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { renderWithMocks } from '@/test/render';
import type { LibraryFilterKey } from '@/types/library';
import { Sidebar } from '../Sidebar';

function renderSidebar(
  variant: 'rail' | 'mid' | 'full',
  libraryFilterBadges?: Partial<Record<LibraryFilterKey, string | number>>
) {
  return renderWithMocks(
    <CollectionsProvider>
      <Tabs.Root orientation="vertical" value="library">
        <Sidebar
          activeRoute="library"
          onNavigate={() => undefined}
          controllerMode={false}
          lastProfile="Deck Test Profile"
          onOpenCollection={vi.fn()}
          variant={variant}
          activeLibraryFilter="all"
          libraryFilterBadges={libraryFilterBadges}
        />
      </Tabs.Root>
    </CollectionsProvider>
  );
}

describe('Sidebar', () => {
  it('renders route sections in the tablist and Collections as a sibling after it', async () => {
    renderSidebar('full');

    await screen.findByRole('button', { name: /Action \/ Adventure/i });

    const sectionLabels = Array.from(document.querySelectorAll('.crosshook-sidebar__section-label')).map((node) =>
      node.textContent?.trim()
    );

    expect(sectionLabels).toEqual(['Game', 'Setup', 'Dashboards', 'Community', 'Collections']);

    // Collections holds non-tab buttons, so it must not live inside the tablist.
    const tablist = screen.getByRole('tablist', { name: 'CrossHook sections' });
    const collectionsGroup = screen.getByRole('group', { name: 'Library filters and collections' });
    expect(tablist.contains(collectionsGroup)).toBe(false);
    expect(screen.getByLabelText('CrossHook navigation')).toHaveAttribute('data-sidebar-variant', 'full');
    expect(screen.getByLabelText('CrossHook navigation')).toHaveAttribute('data-sidebar-width', '264');
    expect(screen.getByLabelText('CrossHook navigation')).toHaveAttribute('data-collapsed', 'false');
    expect(screen.getByRole('tab', { name: 'Library' })).toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: 'Profiles' })).not.toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: 'Launch' })).not.toBeInTheDocument();
  });

  it('renders fixed library-filter entries with optional badge text', async () => {
    renderSidebar('full', { favorites: 2, currentlyRunning: 1 });

    await screen.findByRole('button', { name: /Action \/ Adventure/i });

    const favorites = screen.getByRole('button', { name: /Favorites/i });
    const currentlyPlaying = screen.getByRole('button', { name: /Currently Playing/i });

    expect(favorites).toBeInTheDocument();
    expect(currentlyPlaying).toBeInTheDocument();
    expect(within(favorites).getByText('2')).toBeInTheDocument();
    expect(within(currentlyPlaying).getByText('1')).toBeInTheDocument();
  });

  it('marks the mid variant as collapsed while preserving the declared section structure', async () => {
    renderSidebar('mid');

    await screen.findByRole('button', { name: /Action \/ Adventure/i });

    const nav = screen.getByLabelText('CrossHook navigation');
    expect(nav).toHaveAttribute('data-sidebar-variant', 'mid');
    expect(nav).toHaveAttribute('data-sidebar-width', '68');
    expect(nav).toHaveAttribute('data-collapsed', 'true');
    expect(Array.from(document.querySelectorAll('.crosshook-sidebar__section')).length).toBe(5);
    expect(screen.getByRole('button', { name: 'New Collection' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Import Preset' })).toBeInTheDocument();
  });

  it('collections block is a single tab stop with arrow navigation', async () => {
    renderSidebar('full');

    await screen.findByRole('button', { name: /Action \/ Adventure/i });

    const group = screen.getByRole('group', { name: 'Library filters and collections' });
    const items = Array.from(group.querySelectorAll<HTMLElement>('[data-roving-item]'));
    expect(items.length).toBeGreaterThan(1);
    expect(items.filter((item) => item.tabIndex === 0)).toHaveLength(1);

    const [first, second] = items;
    act(() => first.focus());
    act(() => {
      first.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
    });

    expect(document.activeElement).toBe(second);
    expect(second.tabIndex).toBe(0);
    expect(first.tabIndex).toBe(-1);
  });
});
