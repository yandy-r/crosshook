import { useEffect, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import { callCommand } from '@/lib/ipc';

function setsEqualString(a: Set<string>, b: Set<string>): boolean {
  if (a.size !== b.size) {
    return false;
  }
  for (const x of a) {
    if (!b.has(x)) {
      return false;
    }
  }
  return true;
}

export function useRunningProfiles(options?: { enabled?: boolean }): Set<string> {
  const enabled = options?.enabled ?? true;
  const [runningProfiles, setRunningProfiles] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    if (!enabled) {
      return;
    }

    let cancelled = false;

    const refresh = () => {
      void callCommand<string[]>('list_running_profiles')
        .then((profileNames) => {
          if (cancelled) {
            return;
          }
          const next = new Set(profileNames);
          setRunningProfiles((prev) => (setsEqualString(prev, next) ? prev : next));
        })
        .catch(() => {
          if (cancelled) {
            return;
          }
          setRunningProfiles((prev) => (prev.size === 0 ? prev : new Set()));
        });
    };

    refresh();
    const intervalId = window.setInterval(refresh, 3000);
    const unlistenComplete = subscribeEvent('launch-complete', () => {
      if (!cancelled) refresh();
    }).catch(() => undefined);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
      void unlistenComplete.then((unlisten) => unlisten?.());
    };
  }, [enabled]);

  return runningProfiles;
}
