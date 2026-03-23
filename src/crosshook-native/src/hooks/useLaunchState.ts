import { invoke } from "@tauri-apps/api/core";
import { useEffect, useReducer } from "react";

import type { LaunchResult, SteamLaunchRequest } from "../types";
import { LaunchPhase } from "../types";

type LaunchState = {
  phase: LaunchPhase;
  errorMessage: string | null;
  helperLogPath: string | null;
};

type LaunchAction =
  | { type: "reset" }
  | { type: "game-start" }
  | { type: "game-success"; helperLogPath: string }
  | { type: "trainer-start" }
  | { type: "trainer-success"; helperLogPath: string }
  | { type: "failure"; errorMessage: string; fallbackPhase: LaunchPhase };

interface UseLaunchStateArgs {
  profileId: string;
  steamModeEnabled: boolean;
  request: SteamLaunchRequest | null;
}

const initialState: LaunchState = {
  phase: LaunchPhase.Idle,
  errorMessage: null,
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
        errorMessage: null,
      };
    case "game-success":
      return {
        phase: LaunchPhase.WaitingForTrainer,
        errorMessage: null,
        helperLogPath: action.helperLogPath,
      };
    case "trainer-start":
      return {
        ...state,
        phase: LaunchPhase.TrainerLaunching,
        errorMessage: null,
      };
    case "trainer-success":
      return {
        phase: LaunchPhase.SessionActive,
        errorMessage: null,
        helperLogPath: action.helperLogPath,
      };
    case "failure":
      return {
        ...state,
        phase: action.fallbackPhase,
        errorMessage: action.errorMessage,
      };
    default:
      return state;
  }
}

function buildLaunchRequest(
  request: SteamLaunchRequest,
  phase: LaunchPhase.GameLaunching | LaunchPhase.TrainerLaunching,
): SteamLaunchRequest {
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

export function useLaunchState({
  profileId,
  steamModeEnabled,
  request,
}: UseLaunchStateArgs) {
  const [state, dispatch] = useReducer(reducer, initialState);
  const hasLaunchRequest = steamModeEnabled && request !== null;

  useEffect(() => {
    dispatch({ type: "reset" });
  }, [profileId, steamModeEnabled]);

  async function launchGame() {
    if (!hasLaunchRequest || !request) {
      return;
    }

    dispatch({ type: "game-start" });

    try {
      const result = await invoke<LaunchResult>("launch_game", {
        request: buildLaunchRequest(request, LaunchPhase.GameLaunching),
      });
      dispatch({
        type: "game-success",
        helperLogPath: result.helper_log_path,
      });
    } catch (error) {
      dispatch({
        type: "failure",
        errorMessage: error instanceof Error ? error.message : String(error),
        fallbackPhase: LaunchPhase.Idle,
      });
    }
  }

  async function launchTrainer() {
    if (!hasLaunchRequest || !request) {
      return;
    }

    dispatch({ type: "trainer-start" });

    try {
      const result = await invoke<LaunchResult>("launch_trainer", {
        request: buildLaunchRequest(request, LaunchPhase.TrainerLaunching),
      });
      dispatch({
        type: "trainer-success",
        helperLogPath: result.helper_log_path,
      });
    } catch (error) {
      dispatch({
        type: "failure",
        errorMessage: error instanceof Error ? error.message : String(error),
        fallbackPhase: LaunchPhase.WaitingForTrainer,
      });
    }
  }

  function reset() {
    dispatch({ type: "reset" });
  }

  const statusText = (() => {
    if (!steamModeEnabled) {
      return "Steam mode is disabled for this profile.";
    }

    if (!request) {
      return "Load a Steam profile to start the two-step launch flow.";
    }

    switch (state.phase) {
      case LaunchPhase.GameLaunching:
        return "Launching the game through Steam.";
      case LaunchPhase.WaitingForTrainer:
        return "Game launch is ready. Start the trainer when the game reaches the menu.";
      case LaunchPhase.TrainerLaunching:
        return "Launching the trainer through Proton.";
      case LaunchPhase.SessionActive:
        return "Session is active.";
      case LaunchPhase.Idle:
      default:
        return "Ready to launch the game.";
    }
  })();

  const hintText = (() => {
    if (!steamModeEnabled) {
      return "Enable Steam mode and load a profile to use the launch workflow.";
    }

    if (!request) {
      return "Select a profile that has Steam launch paths configured.";
    }

    if (state.phase === LaunchPhase.WaitingForTrainer) {
      return "Wait for the game to reach the main menu, then click Launch Trainer.";
    }

    if (state.phase === LaunchPhase.SessionActive) {
      return "The trainer is running. Keep this session open until you are done.";
    }

    return "The game will start first. The trainer is launched in the second step.";
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
    errorMessage: state.errorMessage,
  };
}
