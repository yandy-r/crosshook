import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ComponentProps } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { makeLibraryCardData } from '@/test/fixtures';
import { type MockRenderOptions, renderWithMocks } from '@/test/render';
import { GameDetail } from '../GameDetail';
import type { HeroDetailTabsProps } from '../HeroDetailTabs';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

const heroDetailTabsSpy = vi.fn<(props: HeroDetailTabsProps) => null>();

vi.mock('../HeroDetailTabs', () => ({
  HeroDetailTabs: (props: HeroDetailTabsProps) => {
    heroDetailTabsSpy(props);
    return (
      <div data-testid="hero-detail-tabs">
        <button
          type="button"
          role="tab"
          aria-selected={props.activeTab === 'overview'}
          onClick={() => props.onActiveTabChange('overview')}
        >
          Overview
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={props.activeTab === 'history'}
          onClick={() => props.onActiveTabChange('history')}
        >
          History
        </button>
      </div>
    );
  },
}));

type GameDetailProps = ComponentProps<typeof GameDetail>;

function renderGameDetail(
  props: Partial<GameDetailProps> & Pick<GameDetailProps, 'summary'>,
  options: MockRenderOptions = {}
) {
  return renderWithMocks(
    <ProfileProvider>
      <PreferencesProvider>
        <GameDetail
          onBack={vi.fn()}
          healthByName={{}}
          healthLoading={false}
          offlineReportFor={() => undefined}
          offlineError={null}
          onLaunch={vi.fn()}
          onEdit={vi.fn()}
          onToggleFavorite={vi.fn()}
          {...props}
        />
      </PreferencesProvider>
    </ProfileProvider>,
    options
  );
}

describe('GameDetail', () => {
  beforeEach(() => {
    heroDetailTabsSpy.mockClear();
  });

  it('forwards phase-1 panel-contract placeholders through panelProps', () => {
    const summary = makeLibraryCardData();
    renderGameDetail({ summary });
    const latestCall = heroDetailTabsSpy.mock.calls[heroDetailTabsSpy.mock.calls.length - 1];

    expect(heroDetailTabsSpy).toHaveBeenCalled();
    expect(latestCall?.[0]).toEqual(
      expect.objectContaining({
        activeTab: 'overview',
        panelProps: expect.objectContaining({
          summary,
          updateProfile: undefined,
          profileList: undefined,
          onSetActiveTab: undefined,
        }),
      })
    );
  });

  it('renders hero detail shell, Back, and default tab', async () => {
    renderGameDetail({ summary: makeLibraryCardData({ name: 'Synthetic Quest', gameName: 'Synthetic Quest' }) });

    expect(screen.getByTestId('game-detail')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Back' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Overview' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Synthetic Quest', level: 2 })).toBeInTheDocument();
  });

  it('calls onBack when Back is pressed', async () => {
    const user = userEvent.setup();
    const onBack = vi.fn();
    renderGameDetail({ summary: makeLibraryCardData(), onBack });

    await user.click(screen.getByRole('button', { name: 'Back' }));
    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it('switches tabs', async () => {
    const user = userEvent.setup();
    renderGameDetail({ summary: makeLibraryCardData() });

    const historyTab = screen.getByRole('tab', { name: 'History' });
    await user.click(historyTab);
    expect(historyTab).toHaveAttribute('aria-selected', 'true');
  });
});
