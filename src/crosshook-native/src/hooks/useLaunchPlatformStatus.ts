import { useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';

export interface LaunchPlatformCapabilities {
  isFlatpak: boolean;
  unshareNetAvailable: boolean;
}

/**
 * One-shot fetch of Flatpak / host capability flags for launch UI badges (not persisted).
 */
export function useLaunchPlatformStatus(): LaunchPlatformCapabilities | null {
  const [caps, setCaps] = useState<LaunchPlatformCapabilities | null>(null);

  useEffect(() => {
    let active = true;
    void callCommand<LaunchPlatformCapabilities>('launch_platform_status')
      .then((c) => {
        if (active) {
          setCaps(c);
        }
      })
      .catch(() => {
        if (active) {
          setCaps(null);
        }
      });
    return () => {
      active = false;
    };
  }, []);

  return caps;
}
