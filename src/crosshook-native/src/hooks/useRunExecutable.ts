import { useCallback, useEffect, useRef, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import { callCommand } from '@/lib/ipc';

import {
  isRunCommandError,
  isRunExecutableCompletePayload,
  isRunExecutableLogPayload,
  type RunCommandError,
  type RunExecutableCompletePayload,
  type RunExecutableRequest,
  type RunExecutableResult,
  type RunExecutableStage,
  type RunExecutableValidationState,
} from '../types/run-executable';

/** Maximum number of output lines kept for the live log region. */
export const RUN_LOG_LINE_CAP = 500;

/**
 * Conflict text of `run_executable` while this window's earlier run is still
 * active (`MSG_OWNER_BUSY` in `crosshook-app` `services/process/owner.rs`).
 */
const WINDOW_BUSY_MESSAGE = 'another run is already active for this window';

export interface UseRunExecutableResult {
  request: RunExecutableRequest;
  validation: RunExecutableValidationState;
  stage: RunExecutableStage;
  result: RunExecutableResult | null;
  error: string | null;
  updateField: <Key extends keyof RunExecutableRequest>(key: Key, value: string) => void;
  statusText: string;
  hintText: string;
  actionLabel: string;
  canStart: boolean;
  isRunning: boolean;
  /** Cancel/Stop apply: this hook's run is in flight, or the window's earlier run is still active (`busy`). */
  canCancel: boolean;
  /** Live output lines of the current run, oldest first, capped at {@link RUN_LOG_LINE_CAP}. */
  logLines: string[];
  startRun: () => Promise<void>;
  cancelRun: () => Promise<void>;
  stopRun: () => Promise<void>;
  reset: () => void;
}

function createEmptyRequest(): RunExecutableRequest {
  return {
    executable_path: '',
    proton_path: '',
    prefix_path: '',
    working_directory: '',
    steam_client_install_path: '',
  };
}

function createEmptyValidationState(): RunExecutableValidationState {
  return {
    fieldErrors: {},
    generalError: null,
  };
}

function normalizeUnknownError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === 'string') {
    return error;
  }
  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === 'string') {
      return message;
    }
  }
  return 'An unexpected error occurred.';
}

/** Events of one run that arrived before its `run_id` was known. */
interface BufferedRun {
  lines: string[];
  completion: RunExecutableCompletePayload | null;
}

/**
 * Which run's events the hook accepts. `starting` covers the window between
 * calling `run_executable` and learning its `run_id`. Events are global, so a
 * late event of an earlier (cancelled) run or another window's run can arrive
 * then: they are buffered per `run_id`, and only the returned run's buffer is
 * replayed. The backend refuses a second run per window (`Conflict`), which
 * `startRun` surfaces as the `busy` stage.
 */
type StartingTracking = { kind: 'starting'; buffered: Map<string, BufferedRun> };
type RunTracking = { kind: 'idle' } | StartingTracking | { kind: 'active'; runId: string };

/** How many recently completed run_ids are remembered for detached starts. */
const RECENT_COMPLETIONS_CAP = 16;

function bufferFor(tracking: StartingTracking, runId: string): BufferedRun {
  let entry = tracking.buffered.get(runId);
  if (entry === undefined) {
    entry = { lines: [], completion: null };
    tracking.buffered.set(runId, entry);
  }
  return entry;
}

function appendCapped(lines: string[], line: string): string[] {
  const next = [...lines, line];
  return next.length > RUN_LOG_LINE_CAP ? next.slice(next.length - RUN_LOG_LINE_CAP) : next;
}

function completionError(payload: RunExecutableCompletePayload): string | null {
  if (payload.code === 0) {
    return null;
  }
  if (payload.code !== null) {
    return `Run process exited with code ${payload.code}.`;
  }
  if (payload.signal !== null) {
    return `Run process was terminated by signal ${payload.signal}.`;
  }
  return 'Run process ended without an exit status.';
}

export function useRunExecutable(): UseRunExecutableResult {
  const [request, setRequest] = useState<RunExecutableRequest>(createEmptyRequest);
  const [validation, setValidation] = useState<RunExecutableValidationState>(createEmptyValidationState);
  const [stage, setStage] = useState<RunExecutableStage>('idle');
  const [result, setResult] = useState<RunExecutableResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [logLines, setLogLines] = useState<string[]>([]);
  const trackingRef = useRef<RunTracking>({ kind: 'idle' });
  // Bumped by every start and by reset; an in-flight start whose generation is
  // stale detaches (and cancels the run it started) instead of taking over.
  const generationRef = useRef(0);
  // run_ids whose completion was seen, so a detached start does not cancel the
  // window's slot after its own run already ended (it may hold a newer run).
  const recentCompletionsRef = useRef<string[]>([]);

  const applyCompletion = useCallback((payload: RunExecutableCompletePayload) => {
    const failure = completionError(payload);
    setStage(failure === null ? 'complete' : 'failed');
    setError(failure);
  }, []);

  // One listener pair for the hook's lifetime, filtered by the tracked run.
  // While starting, events are buffered per run_id until `run_executable`
  // returns this run's id; a short-lived process may complete before that.
  useEffect(() => {
    const unlistenLog = subscribeEvent<unknown>('run-executable-log', (event) => {
      const payload = event.payload;
      if (!isRunExecutableLogPayload(payload)) {
        return;
      }
      const tracking = trackingRef.current;
      if (tracking.kind === 'starting') {
        const buffered = bufferFor(tracking, payload.run_id);
        buffered.lines = appendCapped(buffered.lines, payload.line);
      } else if (tracking.kind === 'active' && tracking.runId === payload.run_id) {
        setLogLines((lines) => appendCapped(lines, payload.line));
      }
    });

    const unlistenComplete = subscribeEvent<unknown>('run-executable-complete', (event) => {
      const payload = event.payload;
      if (!isRunExecutableCompletePayload(payload)) {
        return;
      }
      const recent = recentCompletionsRef.current;
      recent.push(payload.run_id);
      if (recent.length > RECENT_COMPLETIONS_CAP) recent.shift();
      const tracking = trackingRef.current;
      if (tracking.kind === 'starting') {
        bufferFor(tracking, payload.run_id).completion = payload;
      } else if (tracking.kind === 'active' && tracking.runId === payload.run_id) {
        applyCompletion(payload);
      }
    });

    return () => {
      void unlistenLog.then((unlisten) => unlisten());
      void unlistenComplete.then((unlisten) => unlisten());
    };
  }, [applyCompletion]);
  const applyCommandError = useCallback((commandError: RunCommandError) => {
    if (commandError.kind === 'validation') {
      setValidation((current) => ({
        ...current,
        generalError: null,
        fieldErrors: {
          ...current.fieldErrors,
          [commandError.field]: commandError.message,
        },
      }));
      return;
    }

    setValidation({
      fieldErrors: {},
      generalError: commandError.message,
    });
  }, []);

  const updateField = useCallback(<Key extends keyof RunExecutableRequest>(key: Key, value: string) => {
    setRequest((current) => ({
      ...current,
      [key]: value,
    }));

    setValidation((current) => {
      if (!current.fieldErrors[key]) {
        return current;
      }

      const fieldErrors = { ...current.fieldErrors };
      delete fieldErrors[key];
      return {
        ...current,
        fieldErrors,
      };
    });
  }, []);

  // Cancel/Stop act on the window's active run, so they also end a run this
  // hook never tracked (`busy`); afterwards a new start is allowed.
  const cancelRun = useCallback(async () => {
    try {
      await callCommand<void>('cancel_run_executable');
    } catch {
      // Best-effort cancellation — surface failures to the user via the next state transition.
    }
    setStage((current) => (current === 'busy' ? 'idle' : current));
  }, []);

  const stopRun = useCallback(async () => {
    try {
      await callCommand<void>('stop_run_executable');
    } catch {
      // Best-effort forceful stop.
    }
    setStage((current) => (current === 'busy' ? 'idle' : current));
  }, []);

  const startRun = useCallback(async () => {
    generationRef.current += 1;
    const generation = generationRef.current;
    const isCurrent = () => generationRef.current === generation;
    trackingRef.current = { kind: 'idle' };
    setLogLines([]);
    setValidation(createEmptyValidationState());
    setError(null);
    setResult(null);
    setStage('preparing');

    try {
      await callCommand<void>('validate_run_executable_request', { request });
    } catch (invokeError) {
      if (!isCurrent()) return;
      setStage('idle');
      if (isRunCommandError(invokeError)) {
        applyCommandError(invokeError);
      } else {
        setValidation({
          fieldErrors: {},
          generalError: normalizeUnknownError(invokeError),
        });
      }
      return;
    }
    if (!isCurrent()) return;

    const starting: StartingTracking = { kind: 'starting', buffered: new Map() };
    trackingRef.current = starting;
    try {
      const runResult = await callCommand<RunExecutableResult>('run_executable', { request });
      if (!isCurrent()) {
        // Reset or a newer start happened while this start was in flight:
        // detach from the run and stop it, unless it already finished (then
        // the window's run slot may belong to the newer run).
        if (!recentCompletionsRef.current.includes(runResult.run_id)) {
          void cancelRun();
        }
        return;
      }
      trackingRef.current = { kind: 'active', runId: runResult.run_id };
      setResult(runResult);
      // Replay only this run's events that beat the invoke response.
      const buffered = starting.buffered.get(runResult.run_id);
      if (buffered !== undefined) {
        setLogLines(buffered.lines);
      }
      if (buffered?.completion) {
        applyCompletion(buffered.completion);
      } else {
        setStage('running');
      }
    } catch (invokeError) {
      if (!isCurrent()) return;
      trackingRef.current = { kind: 'idle' };
      setStage('failed');
      if (isRunCommandError(invokeError)) {
        if (invokeError.kind === 'validation') {
          // Backend re-validated during the run path and rejected. Treat
          // it as a field-level validation error and roll the stage back
          // so the user can fix the input.
          setStage('idle');
          applyCommandError(invokeError);
        } else if (invokeError.message.includes(WINDOW_BUSY_MESSAGE)) {
          // A run started earlier in this window (e.g. before this panel
          // was remounted) still holds the window's run slot.
          setStage('busy');
        } else {
          setError(invokeError.message);
        }
      } else {
        setError(normalizeUnknownError(invokeError));
      }
    }
  }, [applyCommandError, applyCompletion, cancelRun, request]);

  const reset = useCallback(() => {
    if (stage === 'running' || stage === 'preparing') {
      void cancelRun();
    }
    generationRef.current += 1;
    trackingRef.current = { kind: 'idle' };
    setRequest(createEmptyRequest());
    setValidation(createEmptyValidationState());
    setStage('idle');
    setResult(null);
    setError(null);
    setLogLines([]);
  }, [stage, cancelRun]);

  const statusText = (() => {
    switch (stage) {
      case 'preparing':
        return 'Validating...';
      case 'running':
        return 'Running executable...';
      case 'busy':
        return 'A run started earlier in this window is still active.';
      case 'complete':
        return result?.message || 'Executable run complete.';
      case 'failed':
        // Failure detail lives in the danger banner; do not duplicate it here.
        return '';
      default:
        return '';
    }
  })();

  const hintText = (() => {
    switch (stage) {
      case 'running':
        return 'Live output is shown below.';
      case 'busy':
        return 'Cancel or Stop it to start a new run.';
      case 'complete':
        return 'The executable finished. Review the log for details.';
      case 'failed':
        return 'Review the output below and the run log for details.';
      default:
        return '';
    }
  })();

  const actionLabel = (() => {
    switch (stage) {
      case 'preparing':
        return 'Validating...';
      case 'running':
        return 'Running...';
      default:
        return 'Run';
    }
  })();

  const canStart =
    (stage === 'idle' || stage === 'complete' || stage === 'failed') &&
    request.executable_path.trim().length > 0 &&
    request.proton_path.trim().length > 0;
  const isRunning = stage === 'preparing' || stage === 'running';
  const canCancel = isRunning || stage === 'busy';

  return {
    request,
    validation,
    stage,
    result,
    error,
    updateField,
    statusText,
    hintText,
    actionLabel,
    canStart,
    isRunning,
    canCancel,
    logLines,
    startRun,
    cancelRun,
    stopRun,
    reset,
  };
}

export default useRunExecutable;
