import { useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { UmuCsvCoverage } from '../types/launch';
import type { UmuPreference } from '../types/settings';

/**
 * Queries the umu CSV coverage for a given app id when the effective runner
 * preference is `'umu'`. Returns `'unknown'` immediately when the preference
 * is not `'umu'` or the app id is empty, avoiding an unnecessary IPC round
 * trip.
 */
export function useUmuCoverage(effectivePreference: UmuPreference, appId: string): UmuCsvCoverage {
  const [coverage, setCoverage] = useState<UmuCsvCoverage>('unknown');

  useEffect(() => {
    if (effectivePreference !== 'umu' || appId === '') {
      setCoverage('unknown');
      return;
    }
    let cancelled = false;
    // Tauri auto-converts snake_case Rust params to camelCase on the JS boundary.
    void callCommand<UmuCsvCoverage>('check_umu_coverage', { appId })
      .then((result) => {
        if (!cancelled) setCoverage(result);
      })
      .catch(() => {
        if (!cancelled) setCoverage('unknown');
      });
    return () => {
      cancelled = true;
    };
  }, [effectivePreference, appId]);

  return coverage;
}
