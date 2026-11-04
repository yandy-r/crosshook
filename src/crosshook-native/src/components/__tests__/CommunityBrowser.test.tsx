import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { CommunityTapSubscription, UseCommunityProfilesResult } from '@/hooks/useCommunityProfiles';
import { chooseFile } from '@/utils/dialog';
import { CommunityBrowser } from '../CommunityBrowser';

interface TapManagementMockProps {
  onSync: () => void;
  onRemoveTap: (tap: CommunityTapSubscription) => void;
}

interface ProfilesSectionMockProps {
  onImportFromFile: () => void;
}

// ---------------------------------------------------------------------------
// Module mocks — keep IPC and heavy child sections out of scope
// ---------------------------------------------------------------------------

const useCommunityProfilesMock = vi.fn();
const useCommunityCatalogMock = vi.fn();

vi.mock('@/hooks/useCommunityProfiles', () => ({
  useCommunityProfiles: () => useCommunityProfilesMock(),
}));

vi.mock('@/hooks/useCommunityCatalog', () => ({
  useCommunityCatalog: () => useCommunityCatalogMock(),
}));

vi.mock('@/components/community/CommunityTapManagementSection', () => ({
  CommunityTapManagementSection: ({ onSync, onRemoveTap }: TapManagementMockProps) => (
    <div>
      Tap Management Section
      <button type="button" onClick={onSync}>
        Sync taps now
      </button>
      <button type="button" onClick={() => onRemoveTap({ url: 'https://example.com/tap.git' })}>
        Remove tap now
      </button>
    </div>
  ),
}));

vi.mock('@/components/community/CommunityProfilesSection', () => ({
  CommunityProfilesSection: ({ onImportFromFile }: ProfilesSectionMockProps) => (
    <div>
      Profiles Section
      <button type="button" onClick={onImportFromFile}>
        Import profile file now
      </button>
    </div>
  ),
}));

vi.mock('@/components/CommunityImportWizardModal', () => ({
  default: () => null,
}));

vi.mock('@/utils/dialog', () => ({
  chooseDirectory: vi.fn().mockResolvedValue(null),
  chooseFile: vi.fn().mockResolvedValue(null),
}));

const chooseFileMock = vi.mocked(chooseFile);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function buildCommunityState(overrides: Partial<UseCommunityProfilesResult> = {}): UseCommunityProfilesResult {
  return {
    taps: [],
    index: { entries: [], diagnostics: [] },
    lastSyncedCommits: {},
    lastTapSyncResults: [],
    importedProfileNames: new Set(),
    loading: false,
    syncing: false,
    importing: false,
    error: null,
    refreshProfiles: vi.fn().mockResolvedValue(undefined),
    syncTaps: vi.fn().mockResolvedValue(undefined),
    addTap: vi.fn().mockResolvedValue([]),
    removeTap: vi.fn().mockResolvedValue(undefined),
    pinTapToCurrentVersion: vi.fn().mockResolvedValue(undefined),
    unpinTap: vi.fn().mockResolvedValue(undefined),
    getTapHeadCommit: vi.fn().mockReturnValue(undefined),
    prepareCommunityImport: vi.fn().mockResolvedValue({}),
    saveImportedProfile: vi.fn().mockResolvedValue(undefined),
    setError: vi.fn(),
    ...overrides,
  };
}

function buildCatalogReturn() {
  return {
    data: null,
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

describe('CommunityBrowser', () => {
  beforeEach(() => {
    useCommunityProfilesMock.mockReturnValue(buildCommunityState());
    useCommunityCatalogMock.mockReturnValue(buildCatalogReturn());
  });

  // (a) Shell chrome: root section renders with the community-browser aria-label
  it('renders the community browser root section with aria-label', () => {
    render(<CommunityBrowser />);

    expect(screen.getByRole('region', { name: 'Community profile browser' })).toBeInTheDocument();
  });

  // (b) Error-banner regression: no alert role in the happy path
  it('does not show an error banner in the happy path', () => {
    render(<CommunityBrowser />);

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  // (b) Error-banner present when an error is injected via state prop
  it('does not render an alert banner (error is surfaced inside CommunityProfilesSection)', () => {
    // The CommunityBrowser passes error down to CommunityProfilesSection (mocked here).
    // The shell itself does not render a direct role="alert" banner — the section mock
    // absorbs the prop.  This test guards that the chrome remains clean.
    render(<CommunityBrowser state={buildCommunityState({ error: 'Network failure' })} />);

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  // (d) Cached-fallback role="status" region appears when cachedTapNotices.length > 0
  it('renders the cached-fallback status banner when lastTapSyncResults contains a cache entry', () => {
    const stateWithCache = buildCommunityState({
      lastTapSyncResults: [
        {
          workspace: {
            subscription: {
              url: 'https://github.com/example/taps',
              branch: 'main',
            },
            local_path: '/tmp/taps/example',
          },
          status: 'cached_fallback',
          head_commit: 'abc1234567890',
          index: { entries: [], diagnostics: [] },
          from_cache: true,
          last_sync_at: '2024-01-15T10:00:00.000Z',
        },
      ],
    });

    render(<CommunityBrowser state={stateWithCache} />);

    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.getByText('Cached data')).toBeInTheDocument();
  });

  // (d) No cached-fallback status banner when all results are fresh
  it('does not render the cached-fallback status banner when no cache entries exist', () => {
    render(<CommunityBrowser state={buildCommunityState()} />);

    // The status role here is the cache banner — should not be present
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  // Child sections are rendered
  it('renders the tap management and profiles child sections', () => {
    render(<CommunityBrowser />);

    expect(screen.getByText(/Tap Management Section/)).toBeInTheDocument();
    expect(screen.getByText(/Profiles Section/)).toBeInTheDocument();
  });

  // D6: community profile JSON is read by CrossHook inside the sandbox, so the pick is sandbox-use.
  it('picks a community profile file as sandbox-use and prepares the import from it', async () => {
    const portalPath = '/run/user/1000/doc/abc123/profile.json';
    const state = buildCommunityState();
    useCommunityProfilesMock.mockReturnValue(state);
    chooseFileMock.mockResolvedValueOnce(portalPath);
    render(<CommunityBrowser />);

    await userEvent.click(screen.getByRole('button', { name: 'Import profile file now' }));

    await waitFor(() => {
      expect(state.prepareCommunityImport).toHaveBeenCalledWith(portalPath);
    });
    expect(chooseFileMock).toHaveBeenCalledWith('Select Community Profile JSON', 'sandbox', [
      { name: 'JSON', extensions: ['json'] },
    ]);
  });

  // Mutations refresh the catalog so Browse stays in sync with the backend order
  it('sync triggers catalog refresh', async () => {
    const catalog = buildCatalogReturn();
    useCommunityCatalogMock.mockReturnValue(catalog);
    render(<CommunityBrowser />);

    await userEvent.click(screen.getByRole('button', { name: 'Sync taps now' }));

    await waitFor(() => {
      expect(catalog.refresh).toHaveBeenCalledTimes(1);
    });
  });

  it('tap removal triggers catalog refresh', async () => {
    const catalog = buildCatalogReturn();
    useCommunityCatalogMock.mockReturnValue(catalog);
    render(<CommunityBrowser />);

    await userEvent.click(screen.getByRole('button', { name: 'Remove tap now' }));

    await waitFor(() => {
      expect(catalog.refresh).toHaveBeenCalledTimes(1);
    });
  });
});
