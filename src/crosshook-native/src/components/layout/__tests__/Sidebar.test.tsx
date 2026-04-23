import * as Tabs from '@radix-ui/react-tabs';
import { screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { renderWithMocks } from '@/test/render';
import { Sidebar } from '../Sidebar';

function renderSidebar(variant: 'rail' | 'mid' | 'full') {
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
        />
      </Tabs.Root>
    </CollectionsProvider>
  );
}

describe('Sidebar', () => {
  it('renders sections in declared order with Collections formalized between Game and Setup', async () => {
    renderSidebar('full');

    await screen.findByRole('button', { name: /Action \/ Adventure/i });

    const sectionLabels = Array.from(document.querySelectorAll('.crosshook-sidebar__section-label')).map((node) =>
      node.textContent?.trim()
    );

    expect(sectionLabels).toEqual(['Game', 'Collections', 'Setup', 'Dashboards', 'Community']);
    expect(screen.getByLabelText('CrossHook navigation')).toHaveAttribute('data-sidebar-variant', 'full');
    expect(screen.getByLabelText('CrossHook navigation')).toHaveAttribute('data-sidebar-width', '240');
    expect(screen.getByLabelText('CrossHook navigation')).toHaveAttribute('data-collapsed', 'false');
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
});
