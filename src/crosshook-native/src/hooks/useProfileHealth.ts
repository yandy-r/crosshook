import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import { callCommand } from '@/lib/ipc';
import { parseProfilesChanged, profileChangeAffectsHealth } from '@/lib/profileEvents';
import { fetchStartupReport } from '@/lib/startupReport';
import type { ProfilesChangedPayload } from '@/types/profile-events';
import type { StartupJobFinished } from '@/types/startup';

import type { CachedHealthSnapshot, EnrichedHealthSummary, EnrichedProfileHealthReport, HealthStatus } from '../types';
import { countProfileStatuses } from '../utils/health';

export type TrendDirection = 'got_worse' | 'got_better' | 'unchanged' | null;

export function computeTrend(currentStatus: HealthStatus, cachedStatus: string | undefined): TrendDirection {
  if (!cachedStatus) return null;

  const statusRank: Record<string, number> = { healthy: 0, stale: 1, broken: 2 };
  const currentRank = statusRank[currentStatus] ?? 0;
  const cachedRank = statusRank[cachedStatus] ?? 0;

  if (currentRank > cachedRank) return 'got_worse';
  if (currentRank < cachedRank) return 'got_better';
  return 'unchanged';
}

type HookStatus = 'idle' | 'loading' | 'loaded' | 'error';

type ProfileHealthState = {
  status: HookStatus;
  summary: EnrichedHealthSummary | null;
  error: string | null;
};

type ProfileHealthAction =
  | { type: 'batch-loading' }
  | { type: 'batch-complete'; summary: EnrichedHealthSummary }
  | { type: 'single-loading' }
  | { type: 'single-complete'; report: EnrichedProfileHealthReport }
  | { type: 'error'; message: string }
  | { type: 'reset' };

const initialState: ProfileHealthState = {
  status: 'idle',
  summary: null,
  error: null,
};

const STALE_THRESHOLD_DAYS = 7;

function isSnapshotStale(checkedAt: string): boolean {
  const checkedDate = new Date(checkedAt);
  if (Number.isNaN(checkedDate.getTime())) {
    console.warn('Invalid profile health snapshot date encountered:', checkedAt);
    return true;
  }
  const now = new Date();
  const diffMs = now.getTime() - checkedDate.getTime();
  const diffDays = diffMs / (1000 * 60 * 60 * 24);
  return diffDays > STALE_THRESHOLD_DAYS;
}

function daysAgo(checkedAt: string): number {
  const checkedDate = new Date(checkedAt);
  if (Number.isNaN(checkedDate.getTime())) {
    console.warn('Invalid profile health snapshot date encountered:', checkedAt);
    return STALE_THRESHOLD_DAYS + 1;
  }
  const now = new Date();
  const diffMs = now.getTime() - checkedDate.getTime();
  return Math.floor(diffMs / (1000 * 60 * 60 * 24));
}

function reducer(state: ProfileHealthState, action: ProfileHealthAction): ProfileHealthState {
  switch (action.type) {
    case 'batch-loading':
      return { ...state, status: 'loading', error: null };
    case 'batch-complete':
      return { status: 'loaded', summary: action.summary, error: null };
    case 'single-loading':
      return { ...state, error: null };
    case 'single-complete': {
      if (!state.summary) {
        return state;
      }

      const existingIndex = state.summary.profiles.findIndex((p) => p.name === action.report.name);
      const updatedProfiles =
        existingIndex === -1
          ? [...state.summary.profiles, action.report]
          : state.summary.profiles.map((p) => (p.name === action.report.name ? action.report : p));

      const counts = countProfileStatuses(updatedProfiles);

      return {
        ...state,
        summary: {
          ...state.summary,
          ...counts,
          profiles: updatedProfiles,
        },
      };
    }
    case 'error':
      return { ...state, status: 'error', error: action.message };
    case 'reset':
      return initialState;
    default:
      return state;
  }
}

function normalizeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useProfileHealth() {
  const [state, dispatch] = useReducer(reducer, initialState);
  const [cachedSnapshots, setCachedSnapshots] = useState<Record<string, CachedHealthSnapshot>>({});
  // Every batch run takes a sequence number; only the newest run may apply its
  // result, so an older batch that finishes late never overwrites a newer one.
  const batchSeqRef = useRef(0);
  // Set once the startup summary was applied, superseded by a fallback batch,
  // or superseded by any batch result. The startup summary is older than every
  // batch this hook runs, so once a batch applied it must never overwrite it.
  const startupHandledRef = useRef(false);
  // `profiles-changed` refreshes are single-flight with one trailing re-run.
  const backgroundBatchRef = useRef({ inFlight: false, pending: false });

  const runBatch = useCallback(async (showLoading: boolean, signal?: AbortSignal) => {
    if (signal?.aborted) {
      return;
    }
    const seq = ++batchSeqRef.current;
    if (showLoading) {
      dispatch({ type: 'batch-loading' });
    }
    try {
      const summary = await callCommand<EnrichedHealthSummary>('batch_validate_profiles');
      if (signal?.aborted || seq !== batchSeqRef.current) {
        return;
      }
      startupHandledRef.current = true;
      dispatch({ type: 'batch-complete', summary });
    } catch (error) {
      if (signal?.aborted || seq !== batchSeqRef.current) {
        return;
      }
      startupHandledRef.current = true;
      dispatch({ type: 'error', message: normalizeError(error) });
    }
  }, []);

  const batchValidate = useCallback((signal?: AbortSignal) => runBatch(true, signal), [runBatch]);

  /**
   * Background revalidation for profile writes (autosaves arrive in bursts).
   * At most one batch is in flight; events during it collapse into a single
   * trailing re-run. The retained summary stays on screen (no `loading`
   * status), so the dashboard does not flicker on every autosave.
   */
  const scheduleBackgroundBatch = useCallback(() => {
    const background = backgroundBatchRef.current;
    if (background.inFlight) {
      background.pending = true;
      return;
    }
    background.inFlight = true;
    void (async () => {
      try {
        do {
          background.pending = false;
          await runBatch(false);
        } while (background.pending);
      } finally {
        background.inFlight = false;
      }
    })();
  }, [runBatch]);

  const revalidateSingle = useCallback(async (name: string) => {
    dispatch({ type: 'single-loading' });
    try {
      const report = await callCommand<EnrichedProfileHealthReport>('get_profile_health', { name });
      dispatch({ type: 'single-complete', report });
    } catch (error) {
      dispatch({ type: 'error', message: normalizeError(error) });
      throw error;
    }
  }, []);

  useEffect(() => {
    let active = true;
    const controller = new AbortController();
    const startup = startupHandledRef;
    const fallback = () => {
      if (!active || startup.current) return;
      startup.current = true;
      void batchValidate(controller.signal);
    };
    const unlistenBatchComplete = subscribeEvent<EnrichedHealthSummary>('profile-health-batch-complete', (event) => {
      // Only the startup bridge emits this; a Resync replay after startup data
      // or a newer batch was applied must not overwrite a newer summary.
      if (!active || startup.current) return;
      startup.current = true;
      dispatch({ type: 'batch-complete', summary: event.payload });
    });
    const unlistenFinished = subscribeEvent<StartupJobFinished>('startup-job-finished', ({ payload }) => {
      if (payload.job !== 'health_batch') return;
      if (payload.state === 'failed' || payload.state === 'skipped') {
        fallback();
        return;
      }
      if (startup.current) return;
      // The legacy payload event may have been lost; re-read a fresh report and
      // apply the job data. Re-check after the await so duplicate or delayed
      // signals, or a batch that finished meanwhile, dispatch at most once.
      void fetchStartupReport({ fresh: true }).then(
        (report) => {
          if (!active || startup.current) return;
          const health = report.jobs.health;
          if (health.state === 'complete') {
            startup.current = true;
            dispatch({ type: 'batch-complete', summary: health.data });
          } else if (health.state === 'failed' || health.state === 'skipped') {
            fallback();
          }
          // pending/running: the snapshot cannot regress behind a terminal
          // signal, so wait for the next terminal event.
        },
        () => fallback()
      );
    });

    const run = async () => {
      try {
        const snapshots = await callCommand<CachedHealthSnapshot[]>('get_cached_health_snapshots');
        if (active) {
          const byName: Record<string, CachedHealthSnapshot> = {};
          for (const snap of snapshots) {
            byName[snap.profile_name] = snap;
          }
          setCachedSnapshots(byName);
        }
      } catch {
        // Cached snapshots are advisory — ignore failures
      }
      try {
        await Promise.all([unlistenBatchComplete, unlistenFinished]);
        if (!active) return;
        const report = await fetchStartupReport({ fresh: true });
        if (!active || startup.current) {
          return;
        }
        const health = report.jobs.health;
        if (health.state === 'complete') {
          startup.current = true;
          dispatch({ type: 'batch-complete', summary: health.data });
        } else if (health.state === 'failed' || health.state === 'skipped') {
          fallback();
        }
        // pending/running: either success data or a terminal failure will arrive.
      } catch {
        fallback();
      }
    };

    void run();

    return () => {
      active = false;
      controller.abort();
      void unlistenBatchComplete.then(
        (unlisten) => unlisten(),
        () => {}
      );
      void unlistenFinished.then(
        (unlisten) => unlisten(),
        () => {}
      );
    };
  }, [batchValidate]);

  useEffect(() => {
    let active = true;
    const unlistenProfilesChanged = subscribeEvent<ProfilesChangedPayload | null>('profiles-changed', (event) => {
      if (!active) return;
      const change = parseProfilesChanged(event.payload);
      // Malformed payloads (null) are a conservative resync: revalidate.
      if (change && !profileChangeAffectsHealth(change.reason)) return;
      scheduleBackgroundBatch();
    });
    const unlistenLaunchComplete = subscribeEvent<unknown>('launch-complete', () => {
      if (!active) return;
      void batchValidate();
    });
    const unlistenVersionScan = subscribeEvent<unknown>('version-scan-complete', () => {
      if (!active) return;
      void batchValidate();
    });

    return () => {
      active = false;
      void unlistenProfilesChanged.then((unlisten) => unlisten());
      void unlistenLaunchComplete.then((unlisten) => unlisten());
      void unlistenVersionScan.then((unlisten) => unlisten());
    };
  }, [batchValidate, scheduleBackgroundBatch]);

  const healthByName = useMemo<Record<string, EnrichedProfileHealthReport>>(() => {
    if (!state.summary) {
      return {};
    }

    return Object.fromEntries(state.summary.profiles.map((p) => [p.name, p]));
  }, [state.summary]);

  const trendByName = useMemo<Record<string, TrendDirection>>(() => {
    if (!state.summary || Object.keys(cachedSnapshots).length === 0) {
      return {};
    }

    const result: Record<string, TrendDirection> = {};
    for (const profile of state.summary.profiles) {
      const cached = cachedSnapshots[profile.name];
      result[profile.name] = computeTrend(profile.status, cached?.status);
    }
    return result;
  }, [state.summary, cachedSnapshots]);

  const staleInfoByName = useMemo<Record<string, { isStale: boolean; daysAgo: number }>>(() => {
    if (Object.keys(cachedSnapshots).length === 0) {
      return {};
    }

    const result: Record<string, { isStale: boolean; daysAgo: number }> = {};
    for (const [name, snap] of Object.entries(cachedSnapshots)) {
      result[name] = {
        isStale: isSnapshotStale(snap.checked_at),
        daysAgo: daysAgo(snap.checked_at),
      };
    }
    return result;
  }, [cachedSnapshots]);

  return {
    summary: state.summary,
    loading: state.status === 'loading',
    error: state.error,
    healthByName,
    cachedSnapshots,
    trendByName,
    staleInfoByName,
    batchValidate,
    revalidateSingle,
  };
}
