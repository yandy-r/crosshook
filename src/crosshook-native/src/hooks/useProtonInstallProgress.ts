import { useEffect, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import type { ProtonInstallProgress } from '../types/protonup';

/**
 * Subscribes to `protonup:install:progress` events and tracks progress for a
 * specific install operation identified by `opId`.
 *
 * Returns `null` for both `progress` and `percent` while no `opId` is active.
 */
export function useProtonInstallProgress(opId: string | null) {
  const [progress, setProgress] = useState<ProtonInstallProgress | null>(null);

  useEffect(() => {
    if (!opId) {
      setProgress(null);
      return;
    }

    let cancelled = false;

    const unlistenPromise = subscribeEvent<ProtonInstallProgress>('protonup:install:progress', (event) => {
      if (cancelled) return;
      const payload = event.payload;
      if (payload.op_id === opId) {
        setProgress(payload);
      }
    });

    return () => {
      cancelled = true;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [opId]);

  const percent =
    progress?.bytes_total && progress.bytes_total > 0
      ? Math.min(100, Math.floor(((progress.bytes_done ?? 0) / progress.bytes_total) * 100))
      : null;

  return { progress, percent };
}
