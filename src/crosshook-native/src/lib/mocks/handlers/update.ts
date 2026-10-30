import type { UpdateGameRequest, UpdateGameResult } from '../../../types/update';
import { emitMockEvent } from '../eventBus';
import type { Handler } from './types';

/** Tracks the profile name of the in-flight update, or null when idle. */
let updateInFlight: string | null = null;
/** Set to true when cancel_update is called during a running update. */
let updateCancelled = false;
const pendingUpdateTimers = new Set<number>();
/** Monotonic counter for synthetic `run_id` values. */
let updateRunCounter = 0;
const MOCK_UPDATE_LOG_LINES = [
  'Starting updater under Proton...',
  'Extracting update archive...',
  'Patching game files...',
  'Updater finished.',
];

function validateRequest(request: UpdateGameRequest): void {
  if (!request.updater_path || request.updater_path.trim().length === 0) {
    throw new Error('[dev-mock] The updater executable path is required.');
  }
  if (!request.proton_path || request.proton_path.trim().length === 0) {
    throw new Error('[dev-mock] A Proton path is required.');
  }
  if (!request.prefix_path || request.prefix_path.trim().length === 0) {
    throw new Error('[dev-mock] A prefix path is required.');
  }
}

function scheduleUpdateTimeout(callback: () => void, delayMs: number): void {
  const id = window.setTimeout(() => {
    pendingUpdateTimers.delete(id);
    callback();
  }, delayMs);
  pendingUpdateTimers.add(id);
}

export function registerUpdate(map: Map<string, Handler>): void {
  map.set('validate_update_request', async (args) => {
    const { request } = args as { request: UpdateGameRequest };
    validateRequest(request);
    // Returns void on success; throws on validation failure
  });

  map.set('update_game', async (args): Promise<UpdateGameResult> => {
    const { request } = args as { request: UpdateGameRequest };

    // Guard: reject if already running
    if (updateInFlight !== null) {
      throw new Error(`[dev-mock] update already in flight for profile "${updateInFlight}"`);
    }

    validateRequest(request);

    updateInFlight = request.profile_name;
    updateCancelled = false;

    const profileName = request.profile_name;
    updateRunCounter += 1;
    const runId = `mock-update-${updateRunCounter}`;

    // Stream a few updater lines on `update-log` (bare strings, like the relay),
    // then `update-complete` with the exit code (null = terminated by a signal).
    MOCK_UPDATE_LOG_LINES.forEach((line, index) => {
      scheduleUpdateTimeout(
        () => {
          if (!updateCancelled) {
            emitMockEvent('update-log', `[dev-mock] ${line}`);
          }
        },
        300 * (index + 1)
      );
    });
    scheduleUpdateTimeout(() => {
      const exitCode: number | null = updateCancelled ? null : 0;

      emitMockEvent('update-complete', exitCode);

      updateInFlight = null;
      updateCancelled = false;
    }, 1500);

    return {
      succeeded: true,
      message: `[dev-mock] Update process started for "${profileName}".`,
      helper_log_path: `/mock/logs/update-${profileName.toLowerCase().replace(/\s+/g, '-')}.log`,
      run_id: runId,
    };
  });

  map.set('cancel_update', async (): Promise<void> => {
    if (updateInFlight !== null) {
      updateCancelled = true;
    }
    // Best-effort; no error if nothing is running
  });
}

export function resetUpdateMockState(): void {
  updateInFlight = null;
  updateCancelled = false;
  updateRunCounter = 0;
  for (const timerId of pendingUpdateTimers) {
    window.clearTimeout(timerId);
  }
  pendingUpdateTimers.clear();
}
