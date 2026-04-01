import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from 'react';

import type {
  CachedOfflineReadinessSnapshot,
  OfflineReadinessReport,
  OfflineReadinessScanCompletePayload,
} from '../types';

type HookStatus = 'idle' | 'loading' | 'loaded' | 'error' | 'single-complete';

type OfflineReadinessState = {
  status: HookStatus;
  reports: OfflineReadinessReport[];
  error: string | null;
};

type OfflineReadinessAction =
  | { type: 'batch-loading' }
  | { type: 'batch-complete'; reports: OfflineReadinessReport[] }
  | { type: 'single-loading' }
  | { type: 'single-complete'; report: OfflineReadinessReport }
  | { type: 'error'; message: string }
  | { type: 'reset' };

const initialState: OfflineReadinessState = {
  status: 'idle',
  reports: [],
  error: null,
};

function reducer(state: OfflineReadinessState, action: OfflineReadinessAction): OfflineReadinessState {
  switch (action.type) {
    case 'batch-loading':
      return { ...state, status: 'loading', error: null };
    case 'batch-complete':
      return { status: 'loaded', reports: action.reports, error: null };
    case 'single-loading':
      return { ...state, error: null };
    case 'single-complete': {
      const idx = state.reports.findIndex((r) => r.profile_name === action.report.profile_name);
      const next =
        idx === -1 ? [...state.reports, action.report] : state.reports.map((r, i) => (i === idx ? action.report : r));
      return { ...state, status: 'single-complete', reports: next, error: null };
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

function snapshotToReport(row: CachedOfflineReadinessSnapshot): OfflineReadinessReport {
  const blocking = row.blocking_reasons
    ? row.blocking_reasons
        .split(';')
        .map((s) => s.trim())
        .filter(Boolean)
    : [];
  return {
    profile_name: row.profile_name,
    score: Math.min(100, Math.max(0, Number(row.readiness_score))),
    readiness_state: row.readiness_state,
    trainer_type: row.trainer_type,
    checks: [],
    blocking_reasons: blocking,
    checked_at: row.checked_at,
  };
}

export function useOfflineReadiness() {
  const [state, dispatch] = useReducer(reducer, initialState);
  const [cachedByProfileName, setCachedByProfileName] = useState<Record<string, CachedOfflineReadinessSnapshot>>({});
  const startupEventReceivedRef = useRef(false);
  /** True after a live `batch_offline_readiness` run completed successfully; blocks stale cache hydration. */
  const liveBatchCompletedRef = useRef(false);

  const batchCheck = useCallback(async (signal?: AbortSignal) => {
    if (signal?.aborted) {
      return;
    }
    dispatch({ type: 'batch-loading' });
    try {
      const reports = await invoke<OfflineReadinessReport[]>('batch_offline_readiness');
      if (signal?.aborted) {
        return;
      }
      liveBatchCompletedRef.current = true;
      dispatch({ type: 'batch-complete', reports });
    } catch (error) {
      if (signal?.aborted) {
        return;
      }
      dispatch({ type: 'error', message: normalizeError(error) });
    }
  }, []);

  const checkSingle = useCallback(async (name: string) => {
    dispatch({ type: 'single-loading' });
    try {
      const report = await invoke<OfflineReadinessReport>('check_offline_readiness', { name });
      dispatch({ type: 'single-complete', report });
    } catch (error) {
      dispatch({ type: 'error', message: normalizeError(error) });
      throw error;
    }
  }, []);

  useEffect(() => {
    let active = true;
    const controller = new AbortController();
    let fallbackTimer: ReturnType<typeof setTimeout> | null = null;

    const unlistenScan = listen<OfflineReadinessScanCompletePayload>('offline-readiness-scan-complete', () => {
      startupEventReceivedRef.current = true;
      if (active) {
        void batchCheck(controller.signal);
      }
    });

    const run = async () => {
      try {
        const rows = await invoke<CachedOfflineReadinessSnapshot[]>('get_cached_offline_readiness_snapshots');
        if (!active) {
          return;
        }
        const byName: Record<string, CachedOfflineReadinessSnapshot> = {};
        for (const row of rows) {
          byName[row.profile_name] = row;
        }
        setCachedByProfileName(byName);
        const synthetic = rows.map(snapshotToReport);
        if (synthetic.length > 0 && !liveBatchCompletedRef.current) {
          dispatch({ type: 'batch-complete', reports: synthetic });
        }
      } catch {
        // cache is advisory
      }
      fallbackTimer = setTimeout(() => {
        if (!active || startupEventReceivedRef.current) {
          return;
        }
        void batchCheck(controller.signal);
      }, 900);
    };

    void run();

    return () => {
      active = false;
      controller.abort();
      if (fallbackTimer !== null) {
        clearTimeout(fallbackTimer);
      }
      void unlistenScan.then((u) => u());
    };
  }, [batchCheck]);

  const reportForProfile = useCallback(
    (profileName: string): OfflineReadinessReport | undefined => {
      const live = state.reports.find((r) => r.profile_name === profileName);
      if (live) {
        return live;
      }
      const snap = cachedByProfileName[profileName];
      return snap ? snapshotToReport(snap) : undefined;
    },
    [cachedByProfileName, state.reports]
  );

  return useMemo(
    () => ({
      status: state.status,
      reports: state.reports,
      error: state.error,
      cachedByProfileName,
      batchCheck,
      checkSingle,
      reportForProfile,
    }),
    [state.status, state.reports, state.error, cachedByProfileName, batchCheck, checkSingle, reportForProfile]
  );
}
