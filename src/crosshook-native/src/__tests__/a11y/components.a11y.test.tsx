import { TooltipProvider } from '@radix-ui/react-tooltip';
import { screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { ContextRail } from '@/components/layout/ContextRail';
import { Inspector } from '@/components/layout/Inspector';
import { GameDetail } from '@/components/library/GameDetail';
import { HeroDetailHeader } from '@/components/library/HeroDetailHeader';
import { HeroDetailTabs } from '@/components/library/HeroDetailTabs';
import { LibraryListRow } from '@/components/library/LibraryListRow';
import { CommandPalette } from '@/components/palette/CommandPalette';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { InspectorSelectionProvider } from '@/context/InspectorSelectionContext';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider } from '@/context/ProfileContext';
import type { UseGameMetadataResult } from '@/hooks/useGameMetadata';
import { makeLibraryCardData } from '@/test/fixtures';
import { renderWithMocks } from '@/test/render';
import { axe } from '@/test/setup';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

const noop = () => {};

const META_STUB: UseGameMetadataResult = {
  appId: '',
  state: 'idle',
  loading: false,
  result: { app_id: '', state: 'idle', app_details: null, from_cache: false, is_stale: false },
  appDetails: null,
  fromCache: false,
  isStale: false,
  isUnavailable: false,
  refresh: async () => {},
};

function ContextRailProviders({ children }: { children: ReactNode }) {
  return (
    <TooltipProvider>
      <ProfileProvider>
        <HostReadinessProvider>
          <InspectorSelectionProvider>{children}</InspectorSelectionProvider>
        </HostReadinessProvider>
      </ProfileProvider>
    </TooltipProvider>
  );
}

function GameDetailProviders({ children }: { children: ReactNode }) {
  return (
    <ProfileProvider>
      <PreferencesProvider>{children}</PreferencesProvider>
    </ProfileProvider>
  );
}

// ---------------------------------------------------------------------------
// CommandPalette
// ---------------------------------------------------------------------------

describe('CommandPalette accessibility', () => {
  it('has no axe violations when open', async () => {
    const { container } = renderWithMocks(
      <CommandPalette
        open={true}
        query=""
        commands={[]}
        activeId={null}
        onClose={noop}
        onQueryChange={noop}
        onMoveActive={noop}
        onExecuteCommand={noop}
      />
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

// ---------------------------------------------------------------------------
// Inspector
// ---------------------------------------------------------------------------

describe('Inspector accessibility', () => {
  it('has no axe violations for a route without inspector body', async () => {
    const { container } = renderWithMocks(<Inspector route="settings" width={320} />);
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

// ---------------------------------------------------------------------------
// ContextRail
// ---------------------------------------------------------------------------

describe('ContextRail accessibility', () => {
  it('has no axe violations', async () => {
    const { container } = renderWithMocks(
      <ContextRailProviders>
        <ContextRail />
      </ContextRailProviders>
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

// ---------------------------------------------------------------------------
// GameDetail
// ---------------------------------------------------------------------------

describe('GameDetail accessibility', () => {
  it('has no axe violations', async () => {
    const summary = makeLibraryCardData();
    const { container } = renderWithMocks(
      <GameDetailProviders>
        <GameDetail
          summary={summary}
          onBack={noop}
          healthByName={{}}
          healthLoading={false}
          offlineReportFor={() => undefined}
          offlineError={null}
          onLaunch={noop}
          onEdit={noop}
          onToggleFavorite={noop}
        />
      </GameDetailProviders>
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

// ---------------------------------------------------------------------------
// HeroDetailHeader
// ---------------------------------------------------------------------------

describe('HeroDetailHeader accessibility', () => {
  it('has no axe violations', async () => {
    const summary = makeLibraryCardData();
    const { container } = renderWithMocks(
      <HeroDetailHeader
        summary={summary}
        displayName={summary.gameName}
        profile={null}
        loadState="idle"
        profileError={null}
        methodLabel={null}
        heroResolved={{ url: null, showSkeleton: false }}
        portraitArt={{ coverArtUrl: null, loading: false }}
        heroImgBroken={false}
        setHeroImgBroken={noop}
        portraitImgBroken={false}
        setPortraitImgBroken={noop}
        onBack={noop}
        onLaunch={noop}
        onEdit={noop}
        onToggleFavorite={noop}
      />
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

// ---------------------------------------------------------------------------
// HeroDetailTabs
// ---------------------------------------------------------------------------

describe('HeroDetailTabs accessibility', () => {
  it('has no axe violations', async () => {
    const summary = makeLibraryCardData();
    const { container } = renderWithMocks(
      <HeroDetailTabs
        activeTab="overview"
        onActiveTabChange={noop}
        panelProps={{
          summary,
          steamAppId: summary.steamAppId ?? '',
          meta: META_STUB,
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
        }}
      />
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });

  it('renders profiles tab content with correct data-testid', () => {
    const summary = makeLibraryCardData();
    renderWithMocks(
      <ProfileProvider>
        <HeroDetailTabs
          activeTab="profiles"
          onActiveTabChange={noop}
          panelProps={{
            summary,
            steamAppId: summary.steamAppId ?? '',
            meta: META_STUB,
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
          }}
        />
      </ProfileProvider>
    );
    expect(screen.getByTestId('hero-detail-profiles-tab')).toBeInTheDocument();
  });

  it('renders launch-options tab content with correct data-testid', () => {
    const summary = makeLibraryCardData();
    renderWithMocks(
      <ProfileProvider>
        <HeroDetailTabs
          activeTab="launch-options"
          onActiveTabChange={noop}
          panelProps={{
            summary,
            steamAppId: summary.steamAppId ?? '',
            meta: META_STUB,
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
          }}
        />
      </ProfileProvider>
    );
    expect(screen.getByTestId('hero-detail-launch-tab')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// LibraryListRow
// ---------------------------------------------------------------------------

describe('LibraryListRow accessibility', () => {
  it('has no axe violations', async () => {
    const profile = makeLibraryCardData();
    const { container } = renderWithMocks(
      <ul>
        <LibraryListRow profile={profile} onOpenDetails={noop} onLaunch={noop} onEdit={noop} onToggleFavorite={noop} />
      </ul>
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});
