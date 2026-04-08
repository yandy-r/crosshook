import { useCallback, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';

export function useAcknowledgeVersionChange() {
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);

  const acknowledgeVersionChange = useCallback(
    async (profileId: string, revalidateSingle: (id: string) => Promise<void>) => {
      if (busyRef.current) return;
      busyRef.current = true;
      setBusy(true);
      try {
        await callCommand('acknowledge_version_change', { name: profileId });
        await revalidateSingle(profileId);
      } catch {
        // silently ignore — user can retry
      } finally {
        busyRef.current = false;
        setBusy(false);
      }
    },
    []
  );

  return { acknowledgeVersionChange, busy };
}
