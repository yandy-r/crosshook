import { useCallback, useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { CapabilityState } from '../types/onboarding';
import type { PrefixDependencyStatus, PrefixVersionRepairStatus } from '../types/prefix-deps';
import { useCapabilityGate } from './useCapabilityGate';

export interface UseLaunchPrefixDependencyGateResult {
  getDependencyStatus: (profileName: string, prefixPath: string) => Promise<PrefixDependencyStatus[]>;
  getPrefixVersionRepairStatus: (profileName: string, prefixPath: string) => Promise<PrefixVersionRepairStatus>;
  installPrefixDependency: (profileName: string, prefixPath: string, packages: string[]) => Promise<void>;
  repairPrefixWindowsVersion: (profileName: string, prefixPath: string) => Promise<PrefixVersionRepairStatus>;
  /** True when the app is running inside an active Gamescope session (from `check_gamescope_session`). */
  isGamescopeRunning: boolean;
  prefixToolsCapabilityState: CapabilityState;
  prefixToolsRationale: string | null;
}

export function useLaunchPrefixDependencyGate(): UseLaunchPrefixDependencyGateResult {
  const [isGamescopeRunning, setIsGamescopeRunning] = useState(false);
  const { state: prefixToolsCapabilityState, rationale: prefixToolsRationale } = useCapabilityGate('prefix_tools');

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const inside = await callCommand<boolean>('check_gamescope_session');
        if (!cancelled) {
          setIsGamescopeRunning(inside);
        }
      } catch {
        // check_gamescope_session failed; leave prior Gamescope session state unchanged
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

  const getPrefixVersionRepairStatus = useCallback(async (profileName: string, prefixPath: string) => {
    return callCommand<PrefixVersionRepairStatus>('get_prefix_version_repair_status', {
      profileName,
      prefixPath,
    });
  }, []);

  const repairPrefixWindowsVersion = useCallback(async (profileName: string, prefixPath: string) => {
    return callCommand<PrefixVersionRepairStatus>('repair_prefix_windows_version', {
      profileName,
      prefixPath,
    });
  }, []);

  return {
    getDependencyStatus,
    getPrefixVersionRepairStatus,
    installPrefixDependency,
    repairPrefixWindowsVersion,
    isGamescopeRunning,
    prefixToolsCapabilityState,
    prefixToolsRationale,
  };
}

export default useLaunchPrefixDependencyGate;
