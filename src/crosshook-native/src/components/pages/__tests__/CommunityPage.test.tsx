import { screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithMocks } from '@/test/render';
import { CommunityPage } from '../CommunityPage';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

describe('CommunityPage', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.spyOn(console, 'debug').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders the community route banner and DashboardPanelSection headings', async () => {
    const { container } = renderWithMocks(<CommunityPage />);

    expect(await screen.findByRole('heading', { level: 1, name: 'Browse' })).toBeInTheDocument();
    expect(await screen.findByRole('heading', { level: 2, name: 'Tap Management' })).toBeInTheDocument();
    expect(await screen.findByRole('heading', { level: 2, name: 'Community Profiles' })).toBeInTheDocument();
    expect(container.querySelector('.crosshook-page-scroll-shell--community')).toBeInTheDocument();
    expect(container.querySelector('.crosshook-route-card-scroll')).toBeInTheDocument();
    expect(container.querySelector('.crosshook-dashboard-panel-section')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('renders the Tap Management actions in the DashboardPanelSection header', async () => {
    renderWithMocks(<CommunityPage />);

    expect(await screen.findByRole('button', { name: 'Refresh Index' })).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: 'Sync Taps' })).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('renders an empty community profiles section when no entries exist', async () => {
    const { container } = renderWithMocks(<CommunityPage />, {
      handlerOverrides: {
        community_list_profiles: async () => ({
          entries: [],
          diagnostics: [],
        }),
      },
    });

    expect(await screen.findByRole('heading', { level: 2, name: 'Community Profiles' })).toBeInTheDocument();
    expect(
      await screen.findByText('No community profiles matched the current search. Sync a tap or widen the filter.')
    ).toBeInTheDocument();
    expect(container.querySelector('.crosshook-page-scroll-shell--community')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});
