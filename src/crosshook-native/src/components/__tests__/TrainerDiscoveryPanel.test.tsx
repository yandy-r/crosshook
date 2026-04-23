import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_APP_SETTINGS } from '@/types/settings';
import { TrainerDiscoveryPanel } from '../TrainerDiscoveryPanel';

// ---------------------------------------------------------------------------
// Module mocks — keep IPC and heavy child sections out of scope
// ---------------------------------------------------------------------------

const usePreferencesContextMock = vi.fn();
const useTrainerDiscoveryMock = vi.fn();
const useExternalTrainerSearchMock = vi.fn();
const useImportCommunityProfileMock = vi.fn();

vi.mock('@/context/PreferencesContext', () => ({
  usePreferencesContext: () => usePreferencesContextMock(),
}));

vi.mock('@/hooks/useTrainerDiscovery', () => ({
  useTrainerDiscovery: () => useTrainerDiscoveryMock(),
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

describe('TrainerDiscoveryPanel', () => {
  beforeEach(() => {
    usePreferencesContextMock.mockReturnValue(buildPreferencesState());
    useTrainerDiscoveryMock.mockReturnValue({
      data: null,
      loading: false,
      error: null,
      refresh: vi.fn().mockResolvedValue(undefined),
    });
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

  // (e) Search sections are hidden when discovery is disabled
  it('does not render the search or results sections when discovery is disabled', () => {
    render(<TrainerDiscoveryPanel />);

    expect(screen.queryByRole('heading', { name: 'Find trainers' })).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Matching trainers' })).not.toBeInTheDocument();
  });

  // (e) Consent gate absent and search visible when discovery is enabled
  it('does not show the consent gate and renders search sections when discovery is enabled', () => {
    usePreferencesContextMock.mockReturnValue(buildPreferencesState({ discovery_enabled: true }));

    render(<TrainerDiscoveryPanel />);

    expect(screen.queryByRole('button', { name: 'Enable Trainer Discovery' })).not.toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Find trainers' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Matching trainers' })).toBeInTheDocument();
  });

  it('shows an alert banner when importCommunityProfile rejects', async () => {
    usePreferencesContextMock.mockReturnValue(buildPreferencesState({ discovery_enabled: true }));
    useTrainerDiscoveryMock.mockReturnValue({
      data: {
        results: [
          {
            id: 1,
            gameName: 'Test Game',
            sourceName: 'Community',
            sourceUrl: 'https://example.com',
            relativePath: 'test-game',
            tapUrl: 'https://tap.example.com',
            tapLocalPath: '/tmp/tap',
            relevanceScore: 1.0,
          },
        ],
        totalCount: 1,
      },
      loading: false,
      error: null,
      refresh: vi.fn().mockResolvedValue(undefined),
    });
    useImportCommunityProfileMock.mockReturnValue({
      importCommunityProfile: vi.fn().mockRejectedValue(new Error('Import failed')),
    });

    render(<TrainerDiscoveryPanel />);

    await userEvent.click(screen.getByRole('button', { name: 'Import Profile' }));

    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument();
    });
  });
});
