import { useCallback, useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import { isPrefixBusy, type PrefixDependencyStatus, type PrefixVersionRepairStatus } from '../types/prefix-deps';

const NO_REPAIR_REQUIRED: PrefixVersionRepairStatus = {
  required: false,
  state: null,
  last_error: null,
  in_progress: false,
  last_recovery: null,
};

/** Re-poll interval of the repair status while another job holds the prefix. */
export const PREFIX_BUSY_POLL_MS = 2000;

export interface UsePrefixDepsResult {
  deps: PrefixDependencyStatus[];
  repairStatus: PrefixVersionRepairStatus;
  repairStatusKnown: boolean;
  repairStatusError: string | null;
  loading: boolean;
  repairing: boolean;
  error: string | null;
  checkDeps: (packages: string[]) => Promise<void>;
  installDep: (packages: string[]) => Promise<void>;
  repairPrefixVersion: () => Promise<PrefixVersionRepairStatus>;
  reload: () => Promise<void>;
}

function normalizeError(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export function usePrefixDeps(profileName: string, prefixPath: string): UsePrefixDepsResult {
  const [deps, setDeps] = useState<PrefixDependencyStatus[]>([]);
  const [repairStatus, setRepairStatus] = useState<PrefixVersionRepairStatus>(NO_REPAIR_REQUIRED);
  const [repairStatusKnown, setRepairStatusKnown] = useState(false);
  const [repairStatusError, setRepairStatusError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [repairing, setRepairing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Resolves to the loaded repair status, or null when its lookup failed or
  // the load was superseded (`isActive` turned false).
  const load = useCallback(
    async (isActive: () => boolean = () => true): Promise<PrefixVersionRepairStatus | null> => {
      if (!profileName) {
        if (isActive()) {
          setDeps([]);
          setRepairStatus(NO_REPAIR_REQUIRED);
          setRepairStatusKnown(false);
          setRepairStatusError(null);
          setError(null);
          setLoading(false);
        }
        return NO_REPAIR_REQUIRED;
      }

      setLoading(true);
      try {
        const args = { profileName, prefixPath };
        const [dependencyResult, repairResult] = await Promise.allSettled([
          callCommand<PrefixDependencyStatus[]>('get_dependency_status', args),
          callCommand<PrefixVersionRepairStatus>('get_prefix_version_repair_status', args),
        ]);

        if (!isActive()) return null;
        const loadErrors: string[] = [];
        if (dependencyResult.status === 'fulfilled') {
          setDeps(dependencyResult.value);
        } else {
          setDeps([]);
          loadErrors.push(normalizeError(dependencyResult.reason));
        }
        let loadedStatus: PrefixVersionRepairStatus | null = null;
        if (repairResult.status === 'fulfilled') {
          loadedStatus = repairResult.value;
          setRepairStatus(loadedStatus);
          setRepairStatusKnown(true);
          setRepairStatusError(null);
        } else {
          const repairError = normalizeError(repairResult.reason);
          setRepairStatusError(repairError);
          loadErrors.push(repairError);
        }
        setError(loadErrors.length > 0 ? loadErrors.join(' ') : null);
        return loadedStatus;
      } finally {
        if (isActive()) setLoading(false);
      }
    },
    [profileName, prefixPath]
  );

  useEffect(() => {
    let active = true;
    void load(() => active);

    return () => {
      active = false;
    };
  }, [load]);

  // While a job holds the prefix over a repair row, the status is provisional:
  // re-poll until the job releases it, then reload everything so a finished
  // recovery or a new repair requirement shows up. The poll re-arms itself
  // after that reload while the prefix is still busy or the lookup failed, as
  // `prefixBusy` (the effect's trigger) may not change in either case.
  const prefixBusy = isPrefixBusy(repairStatus);
  useEffect(() => {
    if (!profileName || !prefixBusy) return;
    let active = true;
    let timer: number | undefined;
    const poll = async () => {
      try {
        const next = await callCommand<PrefixVersionRepairStatus>('get_prefix_version_repair_status', {
          profileName,
          prefixPath,
        });
        if (!active) return;
        if (isPrefixBusy(next)) {
          setRepairStatus(next);
          setRepairStatusError(null);
          timer = window.setTimeout(() => void poll(), PREFIX_BUSY_POLL_MS);
          return;
        }
        const loaded = await load(() => active);
        if (!active) return;
        if (loaded === null || isPrefixBusy(loaded)) {
          timer = window.setTimeout(() => void poll(), PREFIX_BUSY_POLL_MS);
        }
      } catch (err) {
        if (!active) return;
        // Keep the last known (busy) status and keep polling; dependency
        // actions stay blocked while the error is set.
        setRepairStatusError(normalizeError(err));
        timer = window.setTimeout(() => void poll(), PREFIX_BUSY_POLL_MS);
      }
    };
    timer = window.setTimeout(() => void poll(), PREFIX_BUSY_POLL_MS);

    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [load, prefixBusy, prefixPath, profileName]);

  const checkDeps = useCallback(
    async (packages: string[]) => {
      setLoading(true);
      try {
        const result = await callCommand<PrefixDependencyStatus[]>('check_prefix_dependencies', {
          profileName,
          prefixPath,
          packages,
        });
        setDeps(result);
        setError(null);
      } catch (err) {
        setError(normalizeError(err));
      } finally {
        setLoading(false);
      }
    },
    [profileName, prefixPath]
  );

  const installDep = useCallback(
    async (packages: string[]) => {
      try {
        await callCommand('install_prefix_dependency', {
          profileName,
          prefixPath,
          packages,
        });
        // After install starts, the backend streams events.
        // Reload status after a short delay to pick up any immediate changes.
      } catch (err) {
        setError(normalizeError(err));
        throw err;
      }
    },
    [profileName, prefixPath]
  );

  const repairPrefixVersion = useCallback(async () => {
    setRepairing(true);
    try {
      const result = await callCommand<PrefixVersionRepairStatus>('repair_prefix_windows_version', {
        profileName,
        prefixPath,
      });
      setRepairStatus(result);
      setRepairStatusKnown(true);
      setRepairStatusError(null);
      setError(null);
      await load();
      return result;
    } catch (err) {
      const repairError = normalizeError(err);
      setRepairStatusError(repairError);
      setError(repairError);
      throw err;
    } finally {
      setRepairing(false);
    }
  }, [load, prefixPath, profileName]);

  const reload = useCallback(async () => {
    await load();
  }, [load]);

  return {
    deps,
    repairStatus,
    repairStatusKnown,
    repairStatusError,
    loading,
    repairing,
    error,
    checkDeps,
    installDep,
    repairPrefixVersion,
    reload,
  };
}

export default usePrefixDeps;
