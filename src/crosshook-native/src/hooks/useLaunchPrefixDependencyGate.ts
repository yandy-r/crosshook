import { useCallback, useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type { PrefixDependencyStatus } from '../types/prefix-deps';

export interface UseLaunchPrefixDependencyGateResult {
  getDependencyStatus: (profileName: string, prefixPath: string) => Promise<PrefixDependencyStatus[]>;
  installPrefixDependency: (profileName: string, prefixPath: string, packages: string[]) => Promise<void>;
  /** True when the app is running inside an active Gamescope session (from `check_gamescope_session`). */
  isGamescopeRunning: boolean;
}

export function useLaunchPrefixDependencyGate(): UseLaunchPrefixDependencyGateResult {
  const [isGamescopeRunning, setIsGamescopeRunning] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const inside = await callCommand<boolean>('check_gamescope_session');
        if (!cancelled) {
          setIsGamescopeRunning(inside);
        }
      } catch (error) {
        console.warn('check_gamescope_session failed; leaving prior Gamescope session state', error);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const getDependencyStatus = useCallback(async (profileName: string, prefixPath: string) => {
    return callCommand<PrefixDependencyStatus[]>('get_dependency_status', {
      profileName,
      prefixPath,
    });
  }, []);

  const installPrefixDependency = useCallback(async (profileName: string, prefixPath: string, packages: string[]) => {
    await callCommand('install_prefix_dependency', {
      profileName,
      prefixPath,
      packages,
    });
  }, []);

  return { getDependencyStatus, installPrefixDependency, isGamescopeRunning };
}

export default useLaunchPrefixDependencyGate;
