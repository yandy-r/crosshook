import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useReducer, useState } from "react";

import type { CachedHealthSnapshot, HealthCheckSummary, HealthStatus, ProfileHealthReport } from "../types";
import { countProfileStatuses } from "../utils/health";

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

type HookStatus = "idle" | "loading" | "loaded" | "error";

type ProfileHealthState = {
  status: HookStatus;
  summary: HealthCheckSummary | null;
  error: string | null;
};

type ProfileHealthAction =
  | { type: "batch-loading" }
  | { type: "batch-complete"; summary: HealthCheckSummary }
  | { type: "single-loading" }
  | { type: "single-complete"; report: ProfileHealthReport }
  | { type: "error"; message: string }
  | { type: "reset" };

const initialState: ProfileHealthState = {
  status: "idle",
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
    case "batch-loading":
      return { ...state, status: "loading", error: null };
    case "batch-complete":
      return { status: "loaded", summary: action.summary, error: null };
    case "single-loading":
      return { ...state, error: null };
    case "single-complete": {
      if (!state.summary) {
        return state;
      }

      const existingIndex = state.summary.profiles.findIndex(
        (p) => p.name === action.report.name
      );
      const updatedProfiles =
        existingIndex === -1
          ? [...state.summary.profiles, action.report]
          : state.summary.profiles.map((p) =>
              p.name === action.report.name ? action.report : p
            );

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
    case "error":
      return { ...state, status: "error", error: action.message };
    case "reset":
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

  const batchValidate = useCallback(async (signal?: AbortSignal) => {
    if (signal?.aborted) {
      return;
    }
    dispatch({ type: "batch-loading" });
    try {
      const summary = await invoke<HealthCheckSummary>("batch_validate_profiles");
      if (signal?.aborted) {
        return;
      }
      dispatch({ type: "batch-complete", summary });
    } catch (error) {
      if (signal?.aborted) {
        return;
      }
      dispatch({ type: "error", message: normalizeError(error) });
    }
  }, []);

  const revalidateSingle = useCallback(async (name: string) => {
    dispatch({ type: "single-loading" });
    try {
      const report = await invoke<ProfileHealthReport>("get_profile_health", { name });
      dispatch({ type: "single-complete", report });
    } catch (error) {
      dispatch({ type: "error", message: normalizeError(error) });
    }
  }, []);

  useEffect(() => {
    let active = true;
    const controller = new AbortController();

    const run = async () => {
      try {
        const snapshots = await invoke<CachedHealthSnapshot[]>('get_cached_health_snapshots');
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
      void batchValidate(controller.signal);
    };

    void run();

    const unlistenBatchComplete = listen<HealthCheckSummary>(
      "profile-health-batch-complete",
      (event) => {
        if (active) {
          dispatch({ type: "batch-complete", summary: event.payload });
        }
      }
    );

    return () => {
      active = false;
      controller.abort();
      void unlistenBatchComplete.then((unlisten) => unlisten());
    };
  }, [batchValidate]);

  const healthByName = useMemo<Record<string, ProfileHealthReport>>(() => {
    if (!state.summary) {
      return {};
    }

    return Object.fromEntries(
      state.summary.profiles.map((p) => [p.name, p])
    );
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
    loading: state.status === "loading",
    error: state.error,
    healthByName,
    cachedSnapshots,
    trendByName,
    staleInfoByName,
    batchValidate,
    revalidateSingle,
  };
}
