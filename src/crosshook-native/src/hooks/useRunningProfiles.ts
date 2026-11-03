import { useRunningGameProfileNames } from './runningSessionsStore';

/**
 * Names of saved profiles with a running game session, pushed by the
 * `launch-session-changed` event stream (no polling). Anonymous and trainer
 * sessions are excluded. All callers share one subscription.
 */
export function useRunningProfiles(): ReadonlySet<string> {
  return useRunningGameProfileNames();
}
