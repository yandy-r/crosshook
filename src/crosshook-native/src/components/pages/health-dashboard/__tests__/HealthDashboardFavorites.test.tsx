import { TooltipProvider } from '@radix-ui/react-tooltip';
import { act, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider, useProfileContext } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { emitMockEvent } from '@/lib/events';
import { renderWithMocks } from '@/test/render';
import { makeStartupReport } from '@/test/startupReportFixture';
import type { EnrichedHealthSummary, EnrichedProfileHealthReport } from '@/types/health';
import type { ProfilesChangedPayload } from '@/types/profile-events';
import { HealthDashboardPage } from '../../HealthDashboardPage';

// `vi.mock` factories are hoisted above static imports, so the shared mock
// bridge must be loaded inside the factory.
vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

function healthReport(name: string): EnrichedProfileHealthReport {
  return {
    name,
    status: 'healthy',
    launch_method: 'proton_run',
    issues: [],
    checked_at: '2020-01-01T00:00:00.000Z',
    metadata: {
      profile_id: `pid-${name}`,
      last_success: null,
      failure_count_30d: 0,
      total_launches: 0,
      launcher_drift_state: null,
      is_community_import: false,
      is_favorite: false,
    },
  };
}

// Equal statuses tie-break by name, so the default (status, desc) order is
// Beta, Alpha; only a favorite pin can move Alpha first.
const summary: EnrichedHealthSummary = {
  profiles: [healthReport('Alpha'), healthReport('Beta')],
  healthy_count: 2,
  stale_count: 0,
  broken_count: 0,
  total_count: 2,
  validated_at: '2020-01-01T00:00:00.000Z',
};

function FavoriteProbe() {
  const { toggleFavorite } = useProfileContext();
  return (
    <button type="button" data-testid="favorite-alpha" onClick={() => void toggleFavorite('Alpha', true)}>
      favorite
    </button>
  );
}

function Providers({ children }: { children: ReactNode }) {
  return (
    <TooltipProvider>
      <ProfileProvider>
        <PreferencesProvider>
          <ProfileHealthProvider>
            <HostReadinessProvider>
              <CollectionsProvider>{children}</CollectionsProvider>
            </HostReadinessProvider>
          </ProfileHealthProvider>
        </PreferencesProvider>
      </ProfileProvider>
    </TooltipProvider>
  );
}

function rowNames(): string[] {
  return Array.from(document.querySelectorAll('tr.crosshook-health-dashboard-row td:nth-child(2)')).map(
    (cell) => cell.textContent ?? ''
  );
}

function favoriteCell(name: string): string {
  const row = Array.from(document.querySelectorAll('tr.crosshook-health-dashboard-row')).find(
    (tr) => tr.querySelector('td:nth-child(2)')?.textContent === name
  );
  return row?.querySelector('.crosshook-health-dashboard-td--favorite')?.textContent ?? '<missing>';
}

/** Crosses several macrotask boundaries so every queued refresh settles. */
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i += 1) {
    const { promise, resolve } = Promise.withResolvers<void>();
    setTimeout(resolve, 0);
    await act(async () => {
      await promise;
    });
  }
}

describe('HealthDashboardPage favorites (D13)', () => {
  let batchValidate: Mock<() => Promise<EnrichedHealthSummary>>;

  beforeEach(() => {
    vi.spyOn(console, 'debug').mockImplementation(() => {});
    const favorites = new Set<string>();
    batchValidate = vi.fn(async () => summary);
    const report = makeStartupReport();
    renderWithMocks(
      <Providers>
        <FavoriteProbe />
        <HealthDashboardPage />
      </Providers>,
      {
        handlerOverrides: {
          profile_list: async () => ['Alpha', 'Beta'],
          profile_list_favorites: async () => Array.from(favorites),
          profile_set_favorite: async (args) => {
            const { name, favorite } = args as { name: string; favorite: boolean };
            if (favorite) favorites.add(name);
            else favorites.delete(name);
            const payload: ProfilesChangedPayload = { reason: { kind: 'favorite_toggled' }, names: [name] };
            emitMockEvent('profiles-changed', payload);
            return null;
          },
          get_cached_health_snapshots: async () => [],
          get_cached_offline_readiness_snapshots: async () => [],
          get_startup_report: async () => ({
            ...report,
            jobs: { ...report.jobs, health: { state: 'complete' as const, data: summary } },
          }),
          batch_validate_profiles: batchValidate,
        },
      }
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('updates the star and favorite-first order without a health batch', async () => {
    await waitFor(() => expect(rowNames()).toEqual(['Beta', 'Alpha']));
    await settle();
    expect(favoriteCell('Alpha')).toBe('');
    expect(batchValidate).not.toHaveBeenCalled();

    await act(async () => {
      screen.getByTestId('favorite-alpha').click();
    });

    await waitFor(() => expect(favoriteCell('Alpha')).toBe('★'));
    expect(rowNames()).toEqual(['Alpha', 'Beta']);
    await settle();
    expect(batchValidate).not.toHaveBeenCalled();
  });

  it('runs a health batch for a saved event (positive control)', async () => {
    await waitFor(() => expect(rowNames()).toEqual(['Beta', 'Alpha']));
    await settle();
    expect(batchValidate).not.toHaveBeenCalled();

    const saved: ProfilesChangedPayload = { reason: { kind: 'saved' }, names: ['Alpha'] };
    await act(async () => {
      emitMockEvent('profiles-changed', saved);
    });

    await waitFor(() => expect(batchValidate).toHaveBeenCalledTimes(1));
  });
});
