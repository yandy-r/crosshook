import { useEffect, useReducer, useRef, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import { callCommand } from '@/lib/ipc';
import { MIN_OFFLINE_READINESS_SCORE } from '../constants/offline';
import type {
  DiagnosticReport,
  HashVerifyResult,
  LaunchDiagnosticEvent,
  LaunchFeedback,
  LaunchMethod,
  LaunchRequest,
  LaunchResult,
  LaunchValidationIssue,
  OfflineReadinessReport,
} from '../types';
import { isDiagnosticReport, isLaunchValidationIssue, LaunchPhase } from '../types';
import type { RunningSession } from '../types/launch-sessions';
import { useRunningSessions, useSessionExited } from './runningSessionsStore';

type LaunchState = {
  phase: LaunchPhase;
  feedback: LaunchFeedback | null;
  diagnosticReport: DiagnosticReport | null;
  helperLogPath: string | null;
};

type LaunchAction =
  | { type: 'reset' }
  | { type: 'diagnostic-received'; report: DiagnosticReport }
  | { type: 'game-start' }
  | { type: 'game-stopped' }
  | { type: 'game-success'; helperLogPath: string; nextPhase: LaunchPhase }
  | { type: 'launch-complete' }
  | { type: 'trainer-start' }
  | { type: 'trainer-success'; helperLogPath: string }
  | { type: 'failure'; fallbackPhase: LaunchPhase; feedback: LaunchFeedback };

interface UseLaunchStateArgs {
  profileId: string;
  /** Selected profile name for `check_offline_readiness` / metadata (may differ from draft `profileId`). */
  profileName: string;
  method: Exclude<LaunchMethod, ''>;
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
    case 'reset':
      return initialState;
    case 'diagnostic-received':
      return {
        ...state,
        diagnosticReport: action.report,
        feedback: {
          kind: 'diagnostic',
          report: action.report,
        },
      };
    case 'game-start':
      return {
        ...state,
        phase: LaunchPhase.GameLaunching,
        feedback: null,
        diagnosticReport: null,
      };
    case 'game-success':
      return {
        phase: action.nextPhase,
        feedback: null,
        diagnosticReport: null,
        helperLogPath: action.helperLogPath,
      };
    case 'game-stopped':
      return {
        ...state,
        phase: LaunchPhase.Idle,
      };
    case 'launch-complete':
      return state;
    case 'trainer-start':
      return {
        ...state,
        phase: LaunchPhase.TrainerLaunching,
        feedback: null,
        diagnosticReport: null,
      };
    case 'trainer-success':
      return {
        phase: LaunchPhase.SessionActive,
        feedback: null,
        diagnosticReport: null,
        helperLogPath: action.helperLogPath,
      };
    case 'failure':
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

/** Set the game-only / trainer-only flags for a launch phase; every other field is kept as-is. */
export function applyLaunchPhase(
  request: LaunchRequest,
  phase: LaunchPhase.GameLaunching | LaunchPhase.TrainerLaunching
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

async function validateLaunchRequest(request: LaunchRequest): Promise<LaunchValidationIssue | null> {
  try {
    await callCommand<void>('validate_launch', { request });
    return null;
  } catch (error) {
    if (isLaunchValidationIssue(error)) {
      return error;
    }

    throw error;
  }
}

/** True when the registry holds a game session for the saved profile `profileName` (already trimmed). */
function hasProfileGameSession(sessions: ReadonlyMap<string, RunningSession>, profileName: string): boolean {
  if (!profileName) {
    return false;
  }
  for (const session of sessions.values()) {
    if (session.session_kind === 'game' && session.profile_name === profileName) {
      return true;
    }
  }
  return false;
}

export function useLaunchState({ profileId, profileName, method, request }: UseLaunchStateArgs) {
  const [state, dispatch] = useReducer(reducer, initialState);
  /** Registry id of this hook's own game launch (`LaunchResult.session_id`); `null` when none is tracked. */
  const [gameSessionId, setGameSessionId] = useState<string | null>(null);
  const runningSessions = useRunningSessions();
  // The own session covers unsaved/anonymous launches; the profile match keeps a
  // game CrossHook already started visible after a profile switch, reset or
  // reload, when the hook no longer tracks its session id (prevents a double launch).
  const ownGameRunning = gameSessionId !== null && runningSessions.has(gameSessionId);
  const profileGameRunning = hasProfileGameSession(runningSessions, profileName.trim());
  const isGameRunning = ownGameRunning || profileGameRunning;
  /** Drives `game-stopped`: the own session when tracked, else the profile's game session. */
  const trackedGameRunning = gameSessionId !== null ? ownGameRunning : profileGameRunning;
  // An `exited` event for the own id ends the launch even when its `started`
  // event was never applied (exit before `launch_game` resolved). `gameSessionId`
  // is only set by the current launch generation, so an old launch's exit never matches.
  const ownGameExited = useSessionExited(gameSessionId);
  const [offlineReadiness, setOfflineReadiness] = useState<OfflineReadinessReport | null>(null);
  const [offlineReadinessLoading, setOfflineReadinessLoading] = useState(false);
  const [offlineReadinessError, setOfflineReadinessError] = useState<string | null>(null);
  const [launchPathWarnings, setLaunchPathWarnings] = useState<LaunchValidationIssue[]>([]);
  const [trainerHashUpdateBusy, setTrainerHashUpdateBusy] = useState(false);
  const activeHelperLogPathRef = useRef<string | null>(null);
  const observedGameProcessRef = useRef(false);
  /**
   * Bumped by `reset()` (and thus `resetLaunchSession`) so an in-flight
   * `launchGame` / `launchTrainer` call can detect that the user reset the
   * session while its backend call was still pending and skip applying its
   * now-stale success/failure state — otherwise a late-resolving launch
   * command could resurrect state the user explicitly cleared.
   */
  const launchGenerationRef = useRef(0);
  const hasLaunchRequest = request !== null;
  const isTwoStepLaunch = method !== 'native';

  useEffect(() => {
    activeHelperLogPathRef.current = null;
    observedGameProcessRef.current = false;
    dispatch({ type: 'reset' });
    setGameSessionId(null);
    setOfflineReadiness(null);
    setOfflineReadinessError(null);
    setLaunchPathWarnings([]);
    setTrainerHashUpdateBusy(false);
    // Tie reset to the active profile without leaving `profileId` unused (Biome exhaustive-deps).
    void profileId;
  }, [profileId]);

  useEffect(() => {
    if (!profileName.trim() || !request?.trainer_host_path?.trim() || method === 'native') {
      setOfflineReadiness(null);
      setOfflineReadinessError(null);
      setOfflineReadinessLoading(false);
      return;
    }

    let cancelled = false;
    setOfflineReadinessLoading(true);
    setOfflineReadinessError(null);
    void callCommand<OfflineReadinessReport>('check_offline_readiness', { name: profileName })
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
  }, [profileName, method, request?.trainer_host_path]);

  useEffect(() => {
    activeHelperLogPathRef.current = state.helperLogPath;
  }, [state.helperLogPath]);

  useEffect(() => {
    if (state.phase === LaunchPhase.Idle) {
      observedGameProcessRef.current = false;
      return;
    }

    if (trackedGameRunning && !ownGameExited) {
      observedGameProcessRef.current = true;
      return;
    }

    const sessionCanEnd = state.phase === LaunchPhase.WaitingForTrainer || state.phase === LaunchPhase.SessionActive;
    if (sessionCanEnd && (observedGameProcessRef.current || ownGameExited)) {
      observedGameProcessRef.current = false;
      dispatch({ type: 'game-stopped' });
      // The own session is gone; fall back to the profile match from here on.
      setGameSessionId(null);
    }
  }, [trackedGameRunning, ownGameExited, state.phase]);

  useEffect(() => {
    let active = true;

    const unlistenDiagnostic = subscribeEvent<LaunchDiagnosticEvent>('launch-diagnostic', (event) => {
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

      dispatch({ type: 'diagnostic-received', report: event.payload });
    });
    const unlistenComplete = subscribeEvent('launch-complete', () => {
      if (active) {
        dispatch({ type: 'launch-complete' });
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

    const generation = launchGenerationRef.current;
    const launchRequest = applyLaunchPhase(request, LaunchPhase.GameLaunching);
    activeHelperLogPathRef.current = null;
    observedGameProcessRef.current = false;
    setGameSessionId(null);
    dispatch({ type: 'game-start' });
    setLaunchPathWarnings([]);

    try {
      const validationIssue = await validateLaunchRequest(launchRequest);
      if (launchGenerationRef.current !== generation) {
        return;
      }
      if (validationIssue) {
        dispatch({
          type: 'failure',
          feedback: {
            kind: 'validation',
            issue: validationIssue,
          },
          fallbackPhase: LaunchPhase.Idle,
        });
        return;
      }

      const result = await callCommand<LaunchResult>('launch_game', {
        request: launchRequest,
      });
      if (launchGenerationRef.current !== generation) {
        // The session was reset while this launch was in flight — do not
        // resurrect tracking state (or the observed-process flag) for a
        // launch the user already asked CrossHook to stop tracking.
        return;
      }
      setLaunchPathWarnings(result.warnings ?? []);
      setGameSessionId(result.session_id);
      activeHelperLogPathRef.current = result.helper_log_path;
      dispatch({
        type: 'game-success',
        helperLogPath: result.helper_log_path,
        nextPhase: isTwoStepLaunch ? LaunchPhase.WaitingForTrainer : LaunchPhase.SessionActive,
      });
    } catch (error) {
      if (launchGenerationRef.current !== generation) {
        return;
      }
      dispatch({
        type: 'failure',
        feedback: {
          kind: 'runtime',
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

    const generation = launchGenerationRef.current;
    const trainerFallbackPhase =
      state.phase === LaunchPhase.WaitingForTrainer ? LaunchPhase.WaitingForTrainer : LaunchPhase.Idle;
    const launchRequest = applyLaunchPhase(request, LaunchPhase.TrainerLaunching);
    activeHelperLogPathRef.current = null;
    dispatch({ type: 'trainer-start' });
    setLaunchPathWarnings([]);

    try {
      if (profileName.trim()) {
        setOfflineReadinessLoading(true);
        try {
          const report = await callCommand<OfflineReadinessReport>('check_offline_readiness', {
            name: profileName,
          });
          if (launchGenerationRef.current !== generation) {
            return;
          }
          setOfflineReadiness(report);
          setOfflineReadinessError(null);
          if (report.blocking_reasons.length > 0) {
            dispatch({
              type: 'failure',
              feedback: {
                kind: 'validation',
                issue: {
                  message: `Offline readiness blocked: ${report.blocking_reasons.join(', ')}`,
                  help: 'Resolve blocking reasons before launching.',
                  severity: 'fatal',
                },
              },
              fallbackPhase: trainerFallbackPhase,
            });
            return;
          }
        } catch (err) {
          if (launchGenerationRef.current !== generation) {
            return;
          }
          setOfflineReadinessError(normalizeRuntimeError(err));
        } finally {
          if (launchGenerationRef.current === generation) {
            setOfflineReadinessLoading(false);
          }
        }
      }

      const validationIssue = await validateLaunchRequest(launchRequest);
      if (launchGenerationRef.current !== generation) {
        return;
      }
      if (validationIssue) {
        dispatch({
          type: 'failure',
          feedback: {
            kind: 'validation',
            issue: validationIssue,
          },
          fallbackPhase: trainerFallbackPhase,
        });
        return;
      }

      const result = await callCommand<LaunchResult>('launch_trainer', {
        request: launchRequest,
      });
      if (launchGenerationRef.current !== generation) {
        // The session was reset while this launch was in flight — do not
        // resurrect tracking state for a launch the user already asked
        // CrossHook to stop tracking.
        return;
      }
      setLaunchPathWarnings(result.warnings ?? []);
      activeHelperLogPathRef.current = result.helper_log_path;
      dispatch({
        type: 'trainer-success',
        helperLogPath: result.helper_log_path,
      });
    } catch (error) {
      if (launchGenerationRef.current !== generation) {
        return;
      }
      dispatch({
        type: 'failure',
        feedback: {
          kind: 'runtime',
          message: normalizeRuntimeError(error),
        },
        fallbackPhase: trainerFallbackPhase,
      });
    }
  }

  async function updateStoredTrainerHash(): Promise<void> {
    if (!profileName.trim()) {
      return;
    }
    setTrainerHashUpdateBusy(true);
    try {
      await callCommand<HashVerifyResult>('verify_trainer_hash', { name: profileName });
      setLaunchPathWarnings((prev) =>
        prev.filter((i) => i.code !== 'trainer_hash_mismatch' && i.code !== 'trainer_hash_verify_failed')
      );
    } catch (error) {
      const message = normalizeRuntimeError(error);
      setLaunchPathWarnings((prev) => {
        const rest = prev.filter((i) => i.code !== 'trainer_hash_verify_failed');
        return [
          ...rest,
          {
            message: `Could not update stored trainer hash: ${message}`,
            help: 'Check that the trainer file exists, the profile is saved, and try again.',
            severity: 'warning' as const,
            code: 'trainer_hash_verify_failed',
          },
        ];
      });
    } finally {
      setTrainerHashUpdateBusy(false);
    }
  }

  function dismissTrainerHashCommunityWarning(): void {
    setLaunchPathWarnings((prev) => prev.filter((i) => i.code !== 'trainer_hash_community_mismatch'));
  }

  function reset() {
    // Invalidate any in-flight launchGame/launchTrainer call so its eventual
    // success/failure resolution does not resurrect state this reset just
    // cleared (see launchGenerationRef).
    launchGenerationRef.current += 1;
    dispatch({ type: 'reset' });
    setGameSessionId(null);
    observedGameProcessRef.current = false;
    setOfflineReadiness(null);
    setOfflineReadinessError(null);
    setLaunchPathWarnings([]);
    setTrainerHashUpdateBusy(false);
  }

  async function resetLaunchSession(): Promise<void> {
    try {
      if (profileName.trim()) {
        await callCommand<number>('launch_reset_sessions', { profileName });
      }
    } catch {
      // Best-effort backend teardown — local reset must still recover the UI
      // even when the session registry is unreachable.
    }
    reset();
  }

  const statusText = (() => {
    if (!request) {
      switch (method) {
        case 'steam_applaunch':
          return 'Select a game executable and Steam metadata to start the Steam launch flow.';
        case 'proton_run':
          return 'Select a game executable and Proton runtime details to start the Proton launch flow.';
        default:
          return 'Select a Linux-native game executable to enable launch.';
      }
    }

    if (isGameRunning && state.phase === LaunchPhase.Idle) {
      return method === 'native'
        ? 'Native game detected. Trainer unavailable.'
        : 'Game process detected. Launch Trainer when ready.';
    }

    switch (state.phase) {
      case LaunchPhase.GameLaunching:
        return method === 'native'
          ? 'Launching the native game executable.'
          : `Launching the game through ${method === 'steam_applaunch' ? 'Steam' : 'Proton'}.`;
      case LaunchPhase.WaitingForTrainer:
        return `Game launch is ready. Start the trainer when the game reaches the ${method === 'steam_applaunch' ? 'menu' : 'desired in-game state'}.`;
      case LaunchPhase.TrainerLaunching:
        return 'Launching the trainer through Proton.';
      case LaunchPhase.SessionActive:
        return method === 'native' ? 'Native game session is active.' : 'Session is active.';
      default:
        return method === 'native' ? 'Ready to launch the native game.' : 'Ready to launch the game.';
    }
  })();

  const hintText = (() => {
    if (!request) {
      switch (method) {
        case 'steam_applaunch':
          return 'Steam launch needs App ID, compatdata, Proton, and a game path before it can run.';
        case 'proton_run':
          return 'Proton launch needs a game path, prefix path, Proton path, and trainer path for the second step.';
        default:
          return 'Native launch only supports Linux executables and does not use the trainer runner flow.';
      }
    }

    if (isGameRunning && state.phase === LaunchPhase.Idle) {
      return method === 'native'
        ? 'The game is already running. Native launch does not support the trainer flow.'
        : 'The game is already running. You can launch the trainer directly.';
    }

    if (state.phase === LaunchPhase.WaitingForTrainer) {
      return method === 'steam_applaunch'
        ? 'Wait for the game to reach the main menu, then click Launch Trainer.'
        : 'Wait for the game to be ready, then click Launch Trainer to inject into the same prefix.';
    }

    if (state.phase === LaunchPhase.SessionActive) {
      return method === 'native'
        ? 'The native game process has been started. Keep this session open while you monitor logs.'
        : 'The trainer is running. Keep this session open until you are done.';
    }

    return method === 'native'
      ? 'CrossHook will start the Linux-native executable directly.'
      : 'The game starts first. The trainer is launched in the second step.';
  })();

  const isBusy = state.phase === LaunchPhase.GameLaunching || state.phase === LaunchPhase.TrainerLaunching;

  const canLaunchGame = hasLaunchRequest && state.phase === LaunchPhase.Idle && !isBusy && !isGameRunning;
  const canLaunchTrainer =
    hasLaunchRequest &&
    isTwoStepLaunch &&
    (state.phase === LaunchPhase.Idle || state.phase === LaunchPhase.WaitingForTrainer);

  const offlineWarning =
    offlineReadiness !== null &&
    (offlineReadiness.score < MIN_OFFLINE_READINESS_SCORE || (offlineReadiness.blocking_reasons?.length ?? 0) > 0);

  return {
    canLaunchGame,
    canLaunchTrainer,
    diagnosticReport: state.diagnosticReport,
    hintText,
    helperLogPath: state.helperLogPath,
    isBusy,
    isGameRunning,
    launchGame,
    launchTrainer,
    launchPathWarnings,
    trainerHashUpdateBusy,
    updateStoredTrainerHash,
    dismissTrainerHashCommunityWarning,
    offlineReadiness,
    offlineReadinessError,
    offlineReadinessLoading,
    offlineWarning,
    phase: state.phase,
    reset,
    resetLaunchSession,
    statusText,
    feedback: state.feedback,
  };
}
