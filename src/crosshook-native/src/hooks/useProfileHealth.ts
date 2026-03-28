import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useReducer } from "react";

import type { HealthCheckSummary, ProfileHealthReport } from "../types";

type HealthStatus = "idle" | "loading" | "loaded" | "error";

type ProfileHealthState = {
  status: HealthStatus;
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

function recomputeCounts(profiles: ProfileHealthReport[]): Pick<
  HealthCheckSummary,
  "healthy_count" | "stale_count" | "broken_count" | "total_count"
> {
  let healthy_count = 0;
  let stale_count = 0;
  let broken_count = 0;

  for (const profile of profiles) {
    if (profile.status === "healthy") {
      healthy_count++;
    } else if (profile.status === "stale") {
      stale_count++;
    } else if (profile.status === "broken") {
      broken_count++;
    }
  }

  return { healthy_count, stale_count, broken_count, total_count: profiles.length };
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

      const counts = recomputeCounts(updatedProfiles);

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
    const controller = new AbortController();
    void batchValidate(controller.signal);

    // Cleanup: abort signal guards against dispatch after unmount.
    // Phase C will add listen("profile-health-batch-complete") here
    // with the unlisten() cleanup pattern from useLaunchState.
    return () => {
      controller.abort();
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

  return {
    summary: state.summary,
    loading: state.status === "loading",
    error: state.error,
    healthByName,
    batchValidate,
    revalidateSingle,
  };
}
