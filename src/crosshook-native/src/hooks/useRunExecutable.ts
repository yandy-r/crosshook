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

/**
 * Which run's events the hook accepts. `starting` covers the window between
 * calling `run_executable` and learning its `run_id`: the backend runs at most
 * one executable at a time, so any event seen then belongs to this run.
 */
type RunTracking = { kind: 'idle' } | { kind: 'starting' } | { kind: 'active'; runId: string };

function acceptsRun(tracking: RunTracking, runId: string): boolean {
  return tracking.kind === 'starting' || (tracking.kind === 'active' && tracking.runId === runId);
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

  // One listener pair for the hook's lifetime, filtered by the tracked run.
  // `run-executable-complete` may arrive before `run_executable` resolves for
  // a short-lived process; the terminal stage it sets is kept by the
  // functional update in `startRun`.
  useEffect(() => {
    const unlistenLog = subscribeEvent<unknown>('run-executable-log', (event) => {
      const payload = event.payload;
      if (!isRunExecutableLogPayload(payload) || !acceptsRun(trackingRef.current, payload.run_id)) {
        return;
      }
      setLogLines((lines) => appendCapped(lines, payload.line));
    });

    const unlistenComplete = subscribeEvent<unknown>('run-executable-complete', (event) => {
      const payload = event.payload;
      if (!isRunExecutableCompletePayload(payload) || !acceptsRun(trackingRef.current, payload.run_id)) {
        return;
      }
      const failure = completionError(payload);
      setStage(failure === null ? 'complete' : 'failed');
      setError(failure);
    });

    return () => {
      void unlistenLog.then((unlisten) => unlisten());
      void unlistenComplete.then((unlisten) => unlisten());
    };
  }, []);

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

  const cancelRun = useCallback(async () => {
    try {
      await callCommand<void>('cancel_run_executable');
    } catch {
      // Best-effort cancellation — surface failures to the user via the next state transition.
    }
  }, []);

  const stopRun = useCallback(async () => {
    try {
      await callCommand<void>('stop_run_executable');
    } catch {
      // Best-effort forceful stop.
    }
  }, []);

  const startRun = useCallback(async () => {
    trackingRef.current = { kind: 'idle' };
    setLogLines([]);
    setValidation(createEmptyValidationState());
    setError(null);
    setResult(null);
    setStage('preparing');

    try {
      await callCommand<void>('validate_run_executable_request', { request });
    } catch (invokeError) {
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

    trackingRef.current = { kind: 'starting' };
    try {
      const runResult = await callCommand<RunExecutableResult>('run_executable', { request });
      trackingRef.current = { kind: 'active', runId: runResult.run_id };
      setResult(runResult);
      // Keep a terminal stage set by a completion event that beat the invoke response.
      setStage((current) => (current === 'preparing' ? 'running' : current));
    } catch (invokeError) {
      trackingRef.current = { kind: 'idle' };
      setStage('failed');
      if (isRunCommandError(invokeError)) {
        if (invokeError.kind === 'validation') {
          // Backend re-validated during the run path and rejected. Treat
          // it as a field-level validation error and roll the stage back
          // so the user can fix the input.
          setStage('idle');
          applyCommandError(invokeError);
        } else {
          setError(invokeError.message);
        }
      } else {
        setError(normalizeUnknownError(invokeError));
      }
    }
  }, [applyCommandError, request]);

  const reset = useCallback(() => {
    if (stage === 'running' || stage === 'preparing') {
      void cancelRun();
    }
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
    logLines,
    startRun,
    cancelRun,
    stopRun,
    reset,
  };
}

export default useRunExecutable;
