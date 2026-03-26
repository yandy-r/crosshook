import { invoke } from "@tauri-apps/api/core";
import { useEffect, useReducer } from "react";

import type {
  LaunchFeedback,
  LaunchMethod,
  LaunchRequest,
  LaunchResult,
  LaunchValidationIssue,
} from "../types";
import { LaunchPhase } from "../types";
import { isLaunchValidationIssue } from "../types";

type LaunchState = {
  phase: LaunchPhase;
  feedback: LaunchFeedback | null;
  helperLogPath: string | null;
};

type LaunchAction =
  | { type: "reset" }
  | { type: "game-start" }
  | { type: "game-success"; helperLogPath: string; nextPhase: LaunchPhase }
  | { type: "trainer-start" }
  | { type: "trainer-success"; helperLogPath: string }
  | { type: "failure"; fallbackPhase: LaunchPhase; feedback: LaunchFeedback };

interface UseLaunchStateArgs {
  profileId: string;
  method: Exclude<LaunchMethod, "">;
  request: LaunchRequest | null;
}

const initialState: LaunchState = {
  phase: LaunchPhase.Idle,
  feedback: null,
  helperLogPath: null,
};

function reducer(state: LaunchState, action: LaunchAction): LaunchState {
  switch (action.type) {
    case "reset":
      return initialState;
    case "game-start":
      return {
        ...state,
        phase: LaunchPhase.GameLaunching,
        feedback: null,
      };
    case "game-success":
      return {
        phase: action.nextPhase,
        feedback: null,
        helperLogPath: action.helperLogPath,
      };
    case "trainer-start":
      return {
        ...state,
        phase: LaunchPhase.TrainerLaunching,
        feedback: null,
      };
    case "trainer-success":
      return {
        phase: LaunchPhase.SessionActive,
        feedback: null,
        helperLogPath: action.helperLogPath,
      };
    case "failure":
      return {
        ...state,
        phase: action.fallbackPhase,
        feedback: action.feedback,
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
  method,
  request,
}: UseLaunchStateArgs) {
  const [state, dispatch] = useReducer(reducer, initialState);
  const hasLaunchRequest = request !== null;
  const isTwoStepLaunch = method !== "native";

  useEffect(() => {
    dispatch({ type: "reset" });
  }, [method, profileId]);

  async function launchGame() {
    if (!hasLaunchRequest || !request) {
      return;
    }

    const launchRequest = buildLaunchRequest(request, LaunchPhase.GameLaunching);
    dispatch({ type: "game-start" });

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
    dispatch({ type: "trainer-start" });

    try {
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

  return {
    actionLabel,
    canLaunchGame,
    canLaunchTrainer,
    hintText,
    helperLogPath: state.helperLogPath,
    isBusy,
    launchGame,
    launchTrainer,
    phase: state.phase,
    reset,
    statusText,
    feedback: state.feedback,
  };
}
