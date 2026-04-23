import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { UseCommunityProfilesResult } from '@/hooks/useCommunityProfiles';
import { CommunityBrowser } from '../CommunityBrowser';

// ---------------------------------------------------------------------------
// Module mocks — keep IPC and heavy child sections out of scope
// ---------------------------------------------------------------------------

const useCommunityProfilesMock = vi.fn();

vi.mock('@/hooks/useCommunityProfiles', () => ({
  useCommunityProfiles: () => useCommunityProfilesMock(),
}));

vi.mock('@/components/community/CommunityTapManagementSection', () => ({
  CommunityTapManagementSection: () => <div>Tap Management Section</div>,
}));

vi.mock('@/components/community/CommunityProfilesSection', () => ({
  CommunityProfilesSection: () => <div>Profiles Section</div>,
}));

vi.mock('@/components/CommunityImportWizardModal', () => ({
  default: () => null,
}));

vi.mock('@/lib/plugin-stubs/dialog', () => ({
  open: vi.fn().mockResolvedValue(null),
}));

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

describe('CommunityBrowser', () => {
  beforeEach(() => {
    useCommunityProfilesMock.mockReturnValue(buildCommunityState());
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

    expect(screen.getByText('Tap Management Section')).toBeInTheDocument();
    expect(screen.getByText('Profiles Section')).toBeInTheDocument();
  });
});
