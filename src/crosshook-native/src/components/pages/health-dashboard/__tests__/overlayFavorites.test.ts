import { describe, expect, it } from 'vitest';
import type { EnrichedProfileHealthReport, ProfileHealthMetadata } from '@/types/health';
import { overlayFavorites } from '../useHealthDashboardState';

function metadata(isFavorite: boolean | undefined): ProfileHealthMetadata {
  return {
    profile_id: null,
    last_success: null,
    failure_count_30d: 0,
    total_launches: 0,
    launcher_drift_state: null,
    is_community_import: false,
    is_favorite: isFavorite,
  };
}

function report(name: string, meta: ProfileHealthMetadata | null): EnrichedProfileHealthReport {
  return {
    name,
    status: 'broken',
    launch_method: 'proton_run',
    issues: [
      {
        field: 'game.executable_path',
        path: '/mock/missing.exe',
        message: 'Missing',
        remediation: 'Fix',
        severity: 'error',
      },
    ],
    checked_at: '2020-01-01T00:00:00.000Z',
    metadata: meta,
  };
}

describe('overlayFavorites', () => {
  it('sets is_favorite from the live favorites list', () => {
    const profiles = [report('Alpha', metadata(false)), report('Beta', metadata(true))];

    const result = overlayFavorites(profiles, ['Alpha']);

    expect(result.map((r) => [r.name, r.metadata?.is_favorite])).toEqual([
      ['Alpha', true],
      ['Beta', false],
    ]);
  });

  it('treats a missing is_favorite as not favorite', () => {
    const unset = report('Alpha', metadata(undefined));

    expect(overlayFavorites([unset], [])[0]).toBe(unset);
    expect(overlayFavorites([unset], ['Alpha'])[0]?.metadata?.is_favorite).toBe(true);
  });

  it('keeps report identity when the favorite flag already matches', () => {
    const alpha = report('Alpha', metadata(true));
    const beta = report('Beta', metadata(false));

    const result = overlayFavorites([alpha, beta], ['Alpha']);

    expect(result[0]).toBe(alpha);
    expect(result[1]).toBe(beta);
  });

  it('leaves reports without metadata untouched', () => {
    const bare = report('Alpha', null);

    expect(overlayFavorites([bare], ['Alpha'])[0]).toBe(bare);
  });

  it('changes only the favorite flag, never health status, issues or metadata fields', () => {
    const alpha = report('Alpha', {
      ...metadata(false),
      failure_count_30d: 3,
      last_success: '2020-01-02T00:00:00.000Z',
    });

    const [result] = overlayFavorites([alpha], ['Alpha']);

    expect(result).toEqual({ ...alpha, metadata: { ...alpha.metadata, is_favorite: true } });
    expect(alpha.metadata?.is_favorite).toBe(false);
  });
});
