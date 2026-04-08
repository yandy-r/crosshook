import { useCallback, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';

export type AcknowledgeVersionChangeOutcome =
  | { ok: true }
  | { ok: false; reason: 'busy' }
  | { ok: false; stage: 'acknowledge'; error: unknown }
  | { ok: false; stage: 'revalidate'; error: unknown };

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
