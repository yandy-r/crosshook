import { useCallback, useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type { PrefixDependencyStatus } from '../types/prefix-deps';

export interface UseLaunchPrefixDependencyGateResult {
  getDependencyStatus: (profileName: string, prefixPath: string) => Promise<PrefixDependencyStatus[]>;
  installPrefixDependency: (profileName: string, prefixPath: string, packages: string[]) => Promise<void>;
  /** True when the app is running inside an active Gamescope session (from `check_gamescope_session`). */
  isGamescopeRunning: boolean;
  checkGamescope: () => Promise<void>;
}

export function useLaunchPrefixDependencyGate(): UseLaunchPrefixDependencyGateResult {
  const [isGamescopeRunning, setIsGamescopeRunning] = useState(false);

  const checkGamescope = useCallback(async () => {
    try {
      const inside = await callCommand<boolean>('check_gamescope_session');
      setIsGamescopeRunning(inside);
    } catch {
      // Leave prior value on IPC failure
    }
  }, []);

  useEffect(() => {
    void checkGamescope();
  }, [checkGamescope]);

  const getDependencyStatus = useCallback(async (profileName: string, prefixPath: string) => {
    return callCommand<PrefixDependencyStatus[]>('get_dependency_status', {
      profileName,
      prefixPath,
    });
  }, []);

  const installPrefixDependency = useCallback(
    async (profileName: string, prefixPath: string, packages: string[]) => {
      await callCommand('install_prefix_dependency', {
        profileName,
        prefixPath,
        packages,
      });
    },
    []
  );

  return { getDependencyStatus, installPrefixDependency, isGamescopeRunning, checkGamescope };
}

export default useLaunchPrefixDependencyGate;
