import { useEffect, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import type { ProtonInstallPhase, ProtonInstallProgress } from '../types/protonup';

const KNOWN_PHASES: ReadonlySet<ProtonInstallPhase> = new Set([
  'resolving',
  'downloading',
  'verifying',
  'extracting',
  'finalizing',
  'done',
  'failed',
  'cancelled',
]);

function isKnownPhase(value: string): value is ProtonInstallPhase {
  return KNOWN_PHASES.has(value as ProtonInstallPhase);
}

/**
 * Subscribes to `protonup:install:progress` events and tracks progress for a
 * specific install operation identified by `opId`.
 *
 * Unknown phase values are dropped rather than assigned to state — this
 * defensively guards against future Rust-side sentinels leaking through and
 * clobbering a valid terminal state.
 *
 * Returns `null` for both `progress` and `percent` while no `opId` is active.
 */
export function useProtonInstallProgress(opId: string | null) {
  const [progress, setProgress] = useState<ProtonInstallProgress | null>(null);

  useEffect(() => {
    setProgress(null);
    if (!opId) {
      return;
    }

    let cancelled = false;

    const unlistenPromise = subscribeEvent<ProtonInstallProgress>('protonup:install:progress', (event) => {
      if (cancelled) return;
      const payload = event.payload;
      if (payload.op_id !== opId) return;
      if (!isKnownPhase(payload.phase)) return;
      setProgress(payload);
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
