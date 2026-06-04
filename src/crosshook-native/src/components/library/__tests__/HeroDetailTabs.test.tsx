import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { UseGameMetadataResult } from '@/hooks/useGameMetadata';
import { makeLibraryCardData } from '@/test/fixtures';
import { HeroDetailTabs, type HeroDetailTabsProps } from '../HeroDetailTabs';

vi.mock('../HeroDetailPanels', () => ({
  HeroDetailPanels: ({ mode }: { mode: string }) => <div>Panel: {mode}</div>,
}));

const metaStub: UseGameMetadataResult = {
  appId: '',
  state: 'idle',
  loading: false,
  result: {
    app_id: '',
    state: 'idle',
    app_details: null,
    from_cache: false,
    is_stale: false,
  },
  appDetails: null,
  fromCache: false,
  isStale: false,
  isUnavailable: false,
  refresh: async () => {},
};

function renderHeroDetailTabs(overrides: Partial<HeroDetailTabsProps> = {}) {
  const props: HeroDetailTabsProps = {
    activeTab: 'profiles',
    onActiveTabChange: vi.fn(),
    panelProps: {
      summary: makeLibraryCardData(),
      steamAppId: '9999001',
      meta: metaStub,
      profile: null,
      loadState: 'idle',
      profileError: null,
      healthReport: undefined,
      healthLoading: false,
      offlineReport: undefined,
      offlineError: null,
      launchRequest: null,
      previewLoading: false,
      preview: null,
      previewError: null,
      updateProfile: undefined,
      profileList: undefined,
      onSetActiveTab: undefined,
    },
    ...overrides,
  };

  return render(<HeroDetailTabs {...props} />);
}

describe('HeroDetailTabs', () => {
  it('uses the registered fill-and-scroll tab structure for game detail content', () => {
    renderHeroDetailTabs();

    const panel = screen.getByTestId('hero-detail-profiles-tab');
    expect(panel).toHaveClass('crosshook-subtab-content--fill');
    expect(panel.querySelector('.crosshook-subtab-content__inner--scroll')).toBeInTheDocument();
  });
});
