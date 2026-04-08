import { useCallback, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';

export type AcknowledgeVersionChangeOutcome =
  | { ok: true }
  | { ok: false; reason: 'busy' }
  | { ok: false; stage: 'acknowledge'; error: unknown }
  | { ok: false; stage: 'revalidate'; error: unknown };

/**
 * Surfaces non-ok outcomes with the shared alert/console pattern used by Launch and Profiles.
 * No-op for success and for `busy` (caller should keep buttons disabled while busy).
 */
export function presentAcknowledgeVersionChangeOutcome(outcome: AcknowledgeVersionChangeOutcome): void {
  if (outcome.ok) {
    return;
  }
  if ('reason' in outcome) {
    return;
  }
  const message = outcome.error instanceof Error ? outcome.error.message : String(outcome.error);
  if (outcome.stage === 'acknowledge') {
    console.error('Failed to acknowledge version change', outcome.error);
    window.alert(`Could not mark profile as verified: ${message}`);
  } else {
    console.error('Failed to refresh profile health after acknowledge_version_change', outcome.error);
    window.alert(`Version change was acknowledged, but health data refresh failed: ${message}`);
  }
}

export function useAcknowledgeVersionChange() {
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);

  const acknowledgeVersionChange = useCallback(
    async (
      profileId: string,
      revalidateSingle: (id: string) => Promise<void>
    ): Promise<AcknowledgeVersionChangeOutcome> => {
      if (busyRef.current) {
        return { ok: false, reason: 'busy' };
      }
      busyRef.current = true;
      setBusy(true);
      try {
        try {
          await callCommand('acknowledge_version_change', { name: profileId });
        } catch (error) {
          return { ok: false, stage: 'acknowledge', error };
        }
        try {
          await revalidateSingle(profileId);
          return { ok: true };
        } catch (error) {
          return { ok: false, stage: 'revalidate', error };
        }
      } finally {
        busyRef.current = false;
        setBusy(false);
      }
    },
    []
  );

  return { acknowledgeVersionChange, busy };
}
