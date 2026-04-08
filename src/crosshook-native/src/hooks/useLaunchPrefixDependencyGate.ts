import { useCallback } from 'react';
import { callCommand } from '@/lib/ipc';

import type { PrefixDependencyStatus } from '../types/prefix-deps';

export interface UseLaunchPrefixDependencyGateResult {
  getDependencyStatus: (profileName: string, prefixPath: string) => Promise<PrefixDependencyStatus[]>;
  installPrefixDependency: (profileName: string, prefixPath: string, packages: string[]) => Promise<void>;
}

export function useLaunchPrefixDependencyGate(): UseLaunchPrefixDependencyGateResult {
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

  return { getDependencyStatus, installPrefixDependency };
}

export default useLaunchPrefixDependencyGate;
