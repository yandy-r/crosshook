import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { axe } from '@/test/setup';
import type { CatalogPage } from '@/types/discovery';
import { DEFAULT_APP_SETTINGS } from '@/types/settings';
import { TrainerDiscoveryPanel } from '../TrainerDiscoveryPanel';

// ---------------------------------------------------------------------------
// Module mocks — keep IPC and heavy child sections out of scope
// ---------------------------------------------------------------------------

const usePreferencesContextMock = vi.fn();
const useCommunityCatalogMock = vi.fn();
const useExternalTrainerSearchMock = vi.fn();
const useImportCommunityProfileMock = vi.fn();

vi.mock('@/context/PreferencesContext', () => ({
  usePreferencesContext: () => usePreferencesContextMock(),
}));

vi.mock('@/hooks/useCommunityCatalog', () => ({
  useCommunityCatalog: (options?: { enabled?: boolean }) => useCommunityCatalogMock(options),
}));

vi.mock('@/hooks/useExternalTrainerSearch', () => ({
  useExternalTrainerSearch: () => useExternalTrainerSearchMock(),
}));

vi.mock('@/hooks/useImportCommunityProfile', () => ({
  useImportCommunityProfile: () => useImportCommunityProfileMock(),
}));

vi.mock('@/components/ExternalResultsSection', () => ({
  ExternalResultsSection: () => <div>External Results Section</div>,
}));

vi.mock('@/lib/plugin-stubs/shell', () => ({
  open: vi.fn().mockResolvedValue(undefined),
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function buildPreferencesState(overrides: Partial<typeof DEFAULT_APP_SETTINGS> = {}) {
  return {
    settings: { ...DEFAULT_APP_SETTINGS, ...overrides },
    recentFiles: { game_paths: [], trainer_paths: [], dll_paths: [] },
    settingsError: null,
    defaultSteamClientInstallPath: '',
    refreshPreferences: vi.fn().mockResolvedValue(undefined),
    persistSettings: vi.fn().mockResolvedValue(undefined),
    handleAutoLoadChange: vi.fn().mockResolvedValue(undefined),
    handleSteamGridDbApiKeyChange: vi.fn().mockResolvedValue(undefined),
    clearRecentFiles: vi.fn().mockResolvedValue(undefined),
  };
}

function buildCatalogReturn(data: CatalogPage | null = null) {
  return {
    data,
    loading: false,
    error: null,
    query: {},
    searchText: '',
    setSearchText: vi.fn(),
    setFacet: vi.fn(),
    clearFilters: vi.fn(),
    loadMore: vi.fn().mockResolvedValue(undefined),
    refresh: vi.fn().mockResolvedValue(undefined),
  };
}

function buildCatalogPage(overrides: Partial<CatalogPage> = {}): CatalogPage {
  return {
    entries: [],
    facets: { gameTitles: [], loadingModes: [], compatibilityBands: [], taps: [] },
    totalCount: 0,
    tapCount: 0,
    degraded: false,
    ...overrides,
  };
}

const ELDEN_RING_ENTRY = {
  id: 1,
  tapUrl: 'https://tap.example.com',
  tapLocalPath: '/tmp/tap',
  relativePath: 'elden-ring/community-profile.json',
  manifestPath: '/tmp/tap/elden-ring/community-profile.json',
  gameName: 'Elden Ring',
  compatibilityRating: 'working',
  trainerLoadingMode: 'source_directory',
  schemaVersion: 1,
  sources: [
    {
      sourceName: 'Community',
      sourceUrl: 'https://example.com',
      sha256: 'a'.repeat(64),
    },
  ],
};

describe('TrainerDiscoveryPanel', () => {
  beforeEach(() => {
    usePreferencesContextMock.mockReturnValue(buildPreferencesState());
    useCommunityCatalogMock.mockReturnValue(buildCatalogReturn());
    useExternalTrainerSearchMock.mockReturnValue({
      data: null,
      loading: false,
      error: null,
      search: vi.fn().mockResolvedValue(undefined),
    });
    useImportCommunityProfileMock.mockReturnValue({
      importCommunityProfile: vi.fn().mockResolvedValue(undefined),
    });
  });

  // (a) Shell chrome: DashboardPanelSection heading renders for the panel
  it('renders the Trainer Discovery dashboard panel section heading', () => {
    render(<TrainerDiscoveryPanel />);

    expect(screen.getByRole('heading', { name: 'Trainer Discovery' })).toBeInTheDocument();
  });

  // (b) Error-banner regression: no alert role in the happy path
  it('does not show an error banner in the happy path', () => {
    render(<TrainerDiscoveryPanel />);

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  // (e) Consent gate shows when settings.discovery_enabled === false
  it('shows the Enable Trainer Discovery gate when discovery is disabled', () => {
    // DEFAULT_APP_SETTINGS has discovery_enabled: false
    render(<TrainerDiscoveryPanel />);

    expect(screen.getByRole('button', { name: 'Enable Trainer Discovery' })).toBeInTheDocument();
    expect(
      screen.getByText('Trainer Discovery is disabled. Enable it to search community trainer sources.')
    ).toBeInTheDocument();
  });

  // (e) Catalog sections are hidden when discovery is disabled
  it('does not render the catalog or external sections when discovery is disabled', () => {
    render(<TrainerDiscoveryPanel />);

    expect(screen.queryByRole('heading', { name: 'Trainer Catalog' })).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'External results' })).not.toBeInTheDocument();
  });

  // (e) Consent disabled keeps the catalog hook idle
  it('passes enabled false to the catalog hook when discovery is disabled', () => {
    render(<TrainerDiscoveryPanel />);

    expect(useCommunityCatalogMock).toHaveBeenCalledWith({ enabled: false });
  });

  // (e) Consent gate absent and catalog visible when discovery is enabled
  it('does not show the consent gate and renders catalog sections when discovery is enabled', () => {
    usePreferencesContextMock.mockReturnValue(buildPreferencesState({ discovery_enabled: true }));

    render(<TrainerDiscoveryPanel />);

    expect(screen.queryByRole('button', { name: 'Enable Trainer Discovery' })).not.toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Trainer Catalog' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'External results' })).toBeInTheDocument();
    expect(useCommunityCatalogMock).toHaveBeenCalledWith({ enabled: true });
  });

  it('shows an alert banner when importCommunityProfile rejects', async () => {
    usePreferencesContextMock.mockReturnValue(buildPreferencesState({ discovery_enabled: true }));
    useCommunityCatalogMock.mockReturnValue(
      buildCatalogReturn(buildCatalogPage({ entries: [ELDEN_RING_ENTRY], totalCount: 1, tapCount: 1 }))
    );
    const importCommunityProfile = vi.fn().mockRejectedValue(new Error('Import failed'));
    useImportCommunityProfileMock.mockReturnValue({ importCommunityProfile });

    render(<TrainerDiscoveryPanel />);

    await userEvent.click(screen.getByRole('button', { name: 'Import Profile' }));

    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument();
    });
    expect(importCommunityProfile).toHaveBeenCalledWith('/tmp/tap/elden-ring/community-profile.json');
  });

  it('external section renders independent of facet state', () => {
    usePreferencesContextMock.mockReturnValue(buildPreferencesState({ discovery_enabled: true }));
    // Catalog fully filtered down to nothing with active facets — the
    // consent-gated external section must still render unchanged.
    useCommunityCatalogMock.mockReturnValue({
      ...buildCatalogReturn(buildCatalogPage({ totalCount: 0, tapCount: 1 })),
      query: { compatibilityBands: ['working'], gameTitles: ['Elden Ring'] },
    });

    render(<TrainerDiscoveryPanel />);

    expect(screen.getByText('External Results Section')).toBeInTheDocument();
  });

  it('copy button names the game and passes axe button-name', async () => {
    usePreferencesContextMock.mockReturnValue(buildPreferencesState({ discovery_enabled: true }));
    useCommunityCatalogMock.mockReturnValue(
      buildCatalogReturn(buildCatalogPage({ entries: [ELDEN_RING_ENTRY], totalCount: 1, tapCount: 1 }))
    );

    const { container } = render(<TrainerDiscoveryPanel />);

    const copy = screen.getByRole('button', { name: 'Copy SHA-256 checksum for Elden Ring' });
    expect(copy).toHaveAttribute('title', 'Copy full SHA-256');

    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});
