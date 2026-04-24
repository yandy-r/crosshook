import { useEffect, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import { callCommand } from '@/lib/ipc';

export function useRunningProfiles(): Set<string> {
  const [runningProfiles, setRunningProfiles] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    let cancelled = false;

    const refresh = () => {
      void callCommand<string[]>('list_running_profiles')
        .then((profileNames) => {
          if (!cancelled) setRunningProfiles(new Set(profileNames));
        })
        .catch(() => {
          if (!cancelled) setRunningProfiles(new Set());
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
  }, []);

  return runningProfiles;
}
