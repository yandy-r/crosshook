import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useReducer } from "react";

import type { HealthCheckSummary, ProfileHealthReport } from "../types";
import { countProfileStatuses } from "../utils/health";

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
    void batchValidate(controller.signal);

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

  return {
    summary: state.summary,
    loading: state.status === "loading",
    error: state.error,
    healthByName,
    batchValidate,
    revalidateSingle,
  };
}
