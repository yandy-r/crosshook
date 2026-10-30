import type {
  RunCommandError,
  RunExecutableCompletePayload,
  RunExecutableLogPayload,
  RunExecutableRequest,
  RunExecutableResult,
} from '../../../types/run-executable';
import { emitMockEvent } from '../eventBus';
import type { Handler } from './types';

const LINE_INTERVAL_MS = 250;
const SIGTERM = 15;
const SIGKILL = 9;

/**
 * run_id of the in-flight mock run, null when idle. Like the backend, a second
 * `run_executable` from the same window while a run is active is refused with
 * a `Conflict`, surfaced as a runtime `RunCommandError`.
 */
let activeRunId: string | null = null;
let runCounter = 0;
const pendingRunTimers = new Set<number>();

function scheduleRunTimeout(callback: () => void, delayMs: number): void {
  const id = window.setTimeout(() => {
    pendingRunTimers.delete(id);
    callback();
  }, delayMs);
  pendingRunTimers.add(id);
}

function clearRunTimers(): void {
  for (const timerId of pendingRunTimers) {
    window.clearTimeout(timerId);
  }
  pendingRunTimers.clear();
}

function finishRun(code: number | null, signal: number | null): void {
  if (activeRunId === null) {
    return;
  }
  const payload: RunExecutableCompletePayload = { run_id: activeRunId, code, signal };
  clearRunTimers();
  activeRunId = null;
  emitMockEvent('run-executable-complete', payload);
}

export function registerRunExecutable(map: Map<string, Handler>): void {
  map.set('validate_run_executable_request', async (): Promise<void> => {
    // Browser mode cannot inspect the host filesystem, so every request is accepted.
  });

  map.set('run_executable', async (args): Promise<RunExecutableResult> => {
    const { request } = args as { request: RunExecutableRequest };
    if (activeRunId !== null) {
      const busy: RunCommandError = {
        kind: 'runtime',
        message: '[dev-mock] another run is already active for this window',
      };
      throw busy;
    }

    runCounter += 1;
    const runId = `mock-run-${runCounter}`;
    activeRunId = runId;

    const name = request.executable_path.trim().replace(/\\/g, '/').split('/').pop() || 'executable';
    const lines = [
      `[dev-mock] starting ${name} through Proton`,
      'wine: created the configuration directory',
      `[dev-mock] ${name}: running`,
      `[dev-mock] ${name}: exited cleanly`,
    ];
    lines.forEach((line, index) => {
      scheduleRunTimeout(
        () => {
          const payload: RunExecutableLogPayload = { run_id: runId, line };
          emitMockEvent('run-executable-log', payload);
        },
        LINE_INTERVAL_MS * (index + 1)
      );
    });
    scheduleRunTimeout(() => finishRun(0, null), LINE_INTERVAL_MS * (lines.length + 1));

    const prefixPath = request.prefix_path.trim();
    return {
      succeeded: true,
      message: `[dev-mock] Started ${name}.`,
      helper_log_path: `/mock/logs/run-executable-${runCounter}.log`,
      resolved_prefix_path: prefixPath || '/mock/prefixes/_run-adhoc/mock-slug',
      run_id: runId,
    };
  });

  map.set('cancel_run_executable', async (): Promise<void> => {
    finishRun(null, SIGTERM);
  });

  map.set('stop_run_executable', async (): Promise<void> => {
    finishRun(null, SIGKILL);
  });
}

export function resetRunExecutableMockState(): void {
  clearRunTimers();
  activeRunId = null;
  runCounter = 0;
}
