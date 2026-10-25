import type { StartupReport } from '../types/startup';
import { callCommand } from './ipc';
import { type StartupAction, startupReportState } from './startupReportState';

export { resetStartupReportForTests, type StartupAction } from './startupReportState';

/**
 * Memoized pull for once-per-page-load actions. Job consumers request a fresh
 * snapshot after registering listeners. A rejected memoized fetch can retry.
 */
export function fetchStartupReport(options?: { fresh?: boolean }): Promise<StartupReport> {
  if (options?.fresh) {
    return callCommand<StartupReport>('get_startup_report');
  }
  if (startupReportState.memoized === null) {
    const request = callCommand<StartupReport>('get_startup_report').catch((error: unknown) => {
      if (startupReportState.memoized === request) {
        startupReportState.memoized = null;
      }
      throw error;
    });
    startupReportState.memoized = request;
  }
  return startupReportState.memoized;
}

/** Returns true exactly once per page load for each startup action. */
export function claimStartupAction(action: StartupAction): boolean {
  if (startupReportState.claimed.has(action)) {
    return false;
  }
  startupReportState.claimed.add(action);
  return true;
}
