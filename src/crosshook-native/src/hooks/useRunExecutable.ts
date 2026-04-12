import { useCallback, useEffect, useRef, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import { callCommand } from '@/lib/ipc';

import {
  isRunCommandError,
  type RunCommandError,
  type RunExecutableRequest,
  type RunExecutableResult,
  type RunExecutableStage,
  type RunExecutableValidationState,
} from '../types/run-executable';

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

/**
 * Sentinel value distinct from `number | null` so the listener-coordinator
 * holder can tell "completion event has not arrived yet" apart from
 * "completion event arrived with a `null` exit code".
 */
const NO_EXIT_RECEIVED = Symbol('NO_EXIT_RECEIVED');
type ExitHolderValue = number | null | typeof NO_EXIT_RECEIVED;

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

export function useRunExecutable(): UseRunExecutableResult {
  const [request, setRequest] = useState<RunExecutableRequest>(createEmptyRequest);
  const [validation, setValidation] = useState<RunExecutableValidationState>(createEmptyValidationState);
  const [stage, setStage] = useState<RunExecutableStage>('idle');
  const [result, setResult] = useState<RunExecutableResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const unlistenRef = useRef<(() => void) | null>(null);

  function cleanupListener() {
    unlistenRef.current?.();
    unlistenRef.current = null;
  }

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
    cleanupListener();
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

    // Holders coordinate the listener and the invoke resolution. The
    // backend may emit `run-executable-complete` BEFORE `invoke` resolves
    // for very short-lived processes; without this dance the listener
    // would transition to a terminal stage while `result` is still null,
    // dropping the log path / resolved prefix path from the UI.
    const exitHolder: { value: ExitHolderValue } = { value: NO_EXIT_RECEIVED };
    let invokeResolved = false;

    const finalizeFromExitCode = (exitCode: number | null) => {
      if (exitCode === 0) {
        setStage('complete');
      } else if (exitCode === null) {
        setStage('failed');
        setError('Run process was terminated by a signal.');
      } else {
        setStage('failed');
        setError(`Run process exited with code ${exitCode}.`);
      }
    };

    try {
      // Subscribe to the completion event BEFORE invoking the command
      // to avoid a race where the process exits before the listener exists.
      const unlisten = await subscribeEvent<number | null>('run-executable-complete', (event) => {
        const exitCode = event.payload;
        exitHolder.value = exitCode;
        unlistenRef.current = null;
        unlisten();

        // Defer the terminal-stage transition until invoke has set
        // `result`. Otherwise the UI shows "Complete" with no log path.
        if (invokeResolved) {
          finalizeFromExitCode(exitCode);
        }
      });
      unlistenRef.current = unlisten;

      const runResult = await callCommand<RunExecutableResult>('run_executable', { request });
      setResult(runResult);
      invokeResolved = true;

      if (exitHolder.value !== NO_EXIT_RECEIVED) {
        finalizeFromExitCode(exitHolder.value);
      } else {
        setStage('running');
      }
    } catch (invokeError) {
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
      cleanupListener();
    }
  }, [applyCommandError, request, cleanupListener]);

  const reset = useCallback(() => {
    if (stage === 'running' || stage === 'preparing') {
      void cancelRun();
    }
    cleanupListener();
    setRequest(createEmptyRequest());
    setValidation(createEmptyValidationState());
    setStage('idle');
    setResult(null);
    setError(null);
  }, [stage, cancelRun, cleanupListener]);

  useEffect(() => {
    return () => cleanupListener();
  }, [cleanupListener]);

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
        return 'Check the console drawer for live output.';
      case 'complete':
        return 'The executable finished. Review the log for details.';
      case 'failed':
        return 'Check the console drawer log for details.';
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
    startRun,
    cancelRun,
    stopRun,
    reset,
  };
}

export default useRunExecutable;
