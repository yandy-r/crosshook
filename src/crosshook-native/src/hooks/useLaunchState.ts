import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useReducer, useRef, useState } from "react";

import type {
  DiagnosticReport,
  LaunchFeedback,
  LaunchMethod,
  LaunchRequest,
  LaunchResult,
  LaunchValidationIssue,
  OfflineReadinessReport,
} from "../types";
import { LaunchPhase } from "../types";
import { isDiagnosticReport, isLaunchValidationIssue } from "../types";
import { MIN_OFFLINE_READINESS_SCORE } from "../constants/offline";

type LaunchState = {
  phase: LaunchPhase;
  feedback: LaunchFeedback | null;
  diagnosticReport: DiagnosticReport | null;
  helperLogPath: string | null;
};

type LaunchAction =
  | { type: "reset" }
  | { type: "diagnostic-received"; report: DiagnosticReport }
  | { type: "game-start" }
  | { type: "game-success"; helperLogPath: string; nextPhase: LaunchPhase }
  | { type: "launch-complete" }
  | { type: "trainer-start" }
  | { type: "trainer-success"; helperLogPath: string }
  | { type: "failure"; fallbackPhase: LaunchPhase; feedback: LaunchFeedback };

interface UseLaunchStateArgs {
  profileId: string;
  /** Selected profile name for `check_offline_readiness` / metadata (may differ from draft `profileId`). */
  profileName: string;
  method: Exclude<LaunchMethod, "">;
  request: LaunchRequest | null;
}

const initialState: LaunchState = {
  phase: LaunchPhase.Idle,
  feedback: null,
  diagnosticReport: null,
  helperLogPath: null,
};

function reducer(state: LaunchState, action: LaunchAction): LaunchState {
  switch (action.type) {
    case "reset":
      return initialState;
    case "diagnostic-received":
      return {
        ...state,
        diagnosticReport: action.report,
        feedback: {
          kind: "diagnostic",
          report: action.report,
        },
      };
    case "game-start":
      return {
        ...state,
        phase: LaunchPhase.GameLaunching,
        feedback: null,
        diagnosticReport: null,
      };
    case "game-success":
      return {
        phase: action.nextPhase,
        feedback: null,
        diagnosticReport: null,
        helperLogPath: action.helperLogPath,
      };
    case "launch-complete":
      return state;
    case "trainer-start":
      return {
        ...state,
        phase: LaunchPhase.TrainerLaunching,
        feedback: null,
        diagnosticReport: null,
      };
    case "trainer-success":
      return {
        phase: LaunchPhase.SessionActive,
        feedback: null,
        diagnosticReport: null,
        helperLogPath: action.helperLogPath,
      };
    case "failure":
      return {
        ...state,
        phase: action.fallbackPhase,
        feedback: action.feedback,
        diagnosticReport: null,
      };
    default:
      return state;
  }
}

function buildLaunchRequest(
  request: LaunchRequest,
  phase: LaunchPhase.GameLaunching | LaunchPhase.TrainerLaunching,
): LaunchRequest {
  return phase === LaunchPhase.GameLaunching
    ? {
        ...request,
        launch_game_only: true,
        launch_trainer_only: false,
      }
    : {
        ...request,
        launch_game_only: false,
        launch_trainer_only: true,
      };
}

function normalizeRuntimeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function validateLaunchRequest(
  request: LaunchRequest,
): Promise<LaunchValidationIssue | null> {
  try {
    await invoke<void>("validate_launch", { request });
    return null;
  } catch (error) {
    if (isLaunchValidationIssue(error)) {
      return error;
    }

    throw error;
  }
}

export function useLaunchState({
  profileId,
  profileName,
  method,
  request,
}: UseLaunchStateArgs) {
  const [state, dispatch] = useReducer(reducer, initialState);
  const [offlineReadiness, setOfflineReadiness] = useState<OfflineReadinessReport | null>(null);
  const [offlineReadinessLoading, setOfflineReadinessLoading] = useState(false);
  const [offlineReadinessError, setOfflineReadinessError] = useState<string | null>(null);
  const [launchPathWarnings, setLaunchPathWarnings] = useState<LaunchValidationIssue[]>([]);
  const activeHelperLogPathRef = useRef<string | null>(null);
  const hasLaunchRequest = request !== null;
  const isTwoStepLaunch = method !== "native";

  useEffect(() => {
    activeHelperLogPathRef.current = null;
    dispatch({ type: "reset" });
    setOfflineReadiness(null);
    setOfflineReadinessError(null);
    setLaunchPathWarnings([]);
  }, [method, profileId, profileName]);

  useEffect(() => {
    if (!profileName.trim() || !request?.trainer_host_path?.trim() || method === "native") {
      setOfflineReadiness(null);
      setOfflineReadinessError(null);
      setOfflineReadinessLoading(false);
      return;
    }

    let cancelled = false;
    setOfflineReadinessLoading(true);
    setOfflineReadinessError(null);
    void invoke<OfflineReadinessReport>("check_offline_readiness", { name: profileName })
      .then((report) => {
        if (!cancelled) {
          setOfflineReadiness(report);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setOfflineReadiness(null);
          setOfflineReadinessError(normalizeRuntimeError(err));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setOfflineReadinessLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [profileName, method, request?.trainer_host_path, profileId]);

  useEffect(() => {
    activeHelperLogPathRef.current = state.helperLogPath;
  }, [state.helperLogPath]);

  useEffect(() => {
    let active = true;

    const unlistenDiagnostic = listen<DiagnosticReport>("launch-diagnostic", (event) => {
      if (!active || !isDiagnosticReport(event.payload)) {
        return;
      }

      const activeHelperLogPath = activeHelperLogPathRef.current;
      if (
        !activeHelperLogPath ||
        event.payload.log_tail_path === null ||
        event.payload.log_tail_path !== activeHelperLogPath
      ) {
        return;
      }

      dispatch({ type: "diagnostic-received", report: event.payload });
    });
    const unlistenComplete = listen("launch-complete", () => {
      if (active) {
        dispatch({ type: "launch-complete" });
      }
    });

    return () => {
      active = false;
      void unlistenDiagnostic.then((unlisten) => unlisten());
      void unlistenComplete.then((unlisten) => unlisten());
    };
  }, []);

  async function launchGame() {
    if (!hasLaunchRequest || !request) {
      return;
    }

    const launchRequest = buildLaunchRequest(request, LaunchPhase.GameLaunching);
    activeHelperLogPathRef.current = null;
    dispatch({ type: "game-start" });
    setLaunchPathWarnings([]);

    try {
      const validationIssue = await validateLaunchRequest(launchRequest);
      if (validationIssue) {
        dispatch({
          type: "failure",
          feedback: {
            kind: "validation",
            issue: validationIssue,
          },
          fallbackPhase: LaunchPhase.Idle,
        });
        return;
      }

      const result = await invoke<LaunchResult>("launch_game", {
        request: launchRequest,
      });
      setLaunchPathWarnings(result.warnings ?? []);
      activeHelperLogPathRef.current = result.helper_log_path;
      dispatch({
        type: "game-success",
        helperLogPath: result.helper_log_path,
        nextPhase: isTwoStepLaunch ? LaunchPhase.WaitingForTrainer : LaunchPhase.SessionActive,
      });
    } catch (error) {
      dispatch({
        type: "failure",
        feedback: {
          kind: "runtime",
          message: normalizeRuntimeError(error),
        },
        fallbackPhase: LaunchPhase.Idle,
      });
    }
  }

  async function launchTrainer() {
    if (!hasLaunchRequest || !request) {
      return;
    }

    const launchRequest = buildLaunchRequest(request, LaunchPhase.TrainerLaunching);
    activeHelperLogPathRef.current = null;
    dispatch({ type: "trainer-start" });
    setLaunchPathWarnings([]);

    try {
      if (profileName.trim()) {
        setOfflineReadinessLoading(true);
        try {
          const report = await invoke<OfflineReadinessReport>("check_offline_readiness", {
            name: profileName,
          });
          setOfflineReadiness(report);
          setOfflineReadinessError(null);
          if (report.blocking_reasons.length > 0) {
            dispatch({
              type: "failure",
              feedback: {
                kind: "validation",
                issue: {
                  message: `Offline readiness blocked: ${report.blocking_reasons.join(', ')}`,
                  help: 'Resolve blocking reasons before launching.',
                  severity: 'fatal',
                },
              },
              fallbackPhase: LaunchPhase.WaitingForTrainer,
            });
            return;
          }
        } catch (err) {
          setOfflineReadinessError(normalizeRuntimeError(err));
        } finally {
          setOfflineReadinessLoading(false);
        }
      }

      const validationIssue = await validateLaunchRequest(launchRequest);
      if (validationIssue) {
        dispatch({
          type: "failure",
          feedback: {
            kind: "validation",
            issue: validationIssue,
          },
          fallbackPhase: LaunchPhase.WaitingForTrainer,
        });
        return;
      }

      const result = await invoke<LaunchResult>("launch_trainer", {
        request: launchRequest,
      });
      setLaunchPathWarnings(result.warnings ?? []);
      activeHelperLogPathRef.current = result.helper_log_path;
      dispatch({
        type: "trainer-success",
        helperLogPath: result.helper_log_path,
      });
    } catch (error) {
      dispatch({
        type: "failure",
        feedback: {
          kind: "runtime",
          message: normalizeRuntimeError(error),
        },
        fallbackPhase: LaunchPhase.WaitingForTrainer,
      });
    }
  }

  function reset() {
    dispatch({ type: "reset" });
    setOfflineReadiness(null);
    setOfflineReadinessError(null);
    setLaunchPathWarnings([]);
  }

  const statusText = (() => {
    if (!request) {
      switch (method) {
        case "steam_applaunch":
          return "Select a game executable and Steam metadata to start the Steam launch flow.";
        case "proton_run":
          return "Select a game executable and Proton runtime details to start the Proton launch flow.";
        case "native":
        default:
          return "Select a Linux-native game executable to enable launch.";
      }
    }

    switch (state.phase) {
      case LaunchPhase.GameLaunching:
        return method === "native" ? "Launching the native game executable." : `Launching the game through ${method === "steam_applaunch" ? "Steam" : "Proton"}.`;
      case LaunchPhase.WaitingForTrainer:
        return `Game launch is ready. Start the trainer when the game reaches the ${method === "steam_applaunch" ? "menu" : "desired in-game state"}.`;
      case LaunchPhase.TrainerLaunching:
        return "Launching the trainer through Proton.";
      case LaunchPhase.SessionActive:
        return method === "native" ? "Native game session is active." : "Session is active.";
      case LaunchPhase.Idle:
      default:
        return method === "native" ? "Ready to launch the native game." : "Ready to launch the game.";
    }
  })();

  const hintText = (() => {
    if (!request) {
      switch (method) {
        case "steam_applaunch":
          return "Steam launch needs App ID, compatdata, Proton, and a game path before it can run.";
        case "proton_run":
          return "Proton launch needs a game path, prefix path, Proton path, and trainer path for the second step.";
        case "native":
        default:
          return "Native launch only supports Linux executables and does not use the trainer runner flow.";
      }
    }

    if (state.phase === LaunchPhase.WaitingForTrainer) {
      return method === "steam_applaunch"
        ? "Wait for the game to reach the main menu, then click Launch Trainer."
        : "Wait for the game to be ready, then click Launch Trainer to inject into the same prefix.";
    }

    if (state.phase === LaunchPhase.SessionActive) {
      return method === "native"
        ? "The native game process has been started. Keep this session open while you monitor logs."
        : "The trainer is running. Keep this session open until you are done.";
    }

    return method === "native"
      ? "CrossHook will start the Linux-native executable directly."
      : "The game starts first. The trainer is launched in the second step.";
  })();

  const actionLabel =
    state.phase === LaunchPhase.WaitingForTrainer
      ? "Launch Trainer"
      : "Launch Game";

  const isBusy =
    state.phase === LaunchPhase.GameLaunching ||
    state.phase === LaunchPhase.TrainerLaunching;

  const canLaunchGame =
    hasLaunchRequest && state.phase === LaunchPhase.Idle && !isBusy;
  const canLaunchTrainer =
    hasLaunchRequest &&
    isTwoStepLaunch &&
    state.phase === LaunchPhase.WaitingForTrainer &&
    !isBusy;

  const offlineWarning =
    offlineReadiness !== null &&
    (offlineReadiness.score < MIN_OFFLINE_READINESS_SCORE || (offlineReadiness.blocking_reasons?.length ?? 0) > 0);

  return {
    actionLabel,
    canLaunchGame,
    canLaunchTrainer,
    diagnosticReport: state.diagnosticReport,
    hintText,
    helperLogPath: state.helperLogPath,
    isBusy,
    launchGame,
    launchTrainer,
    launchPathWarnings,
    offlineReadiness,
    offlineReadinessError,
    offlineReadinessLoading,
    offlineWarning,
    phase: state.phase,
    reset,
    statusText,
    feedback: state.feedback,
  };
}
