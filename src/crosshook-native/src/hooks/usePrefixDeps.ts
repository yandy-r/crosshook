import { useCallback, useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type { PrefixDependencyStatus } from '../types/prefix-deps';

export interface UsePrefixDepsResult {
  deps: PrefixDependencyStatus[];
  loading: boolean;
  error: string | null;
  checkDeps: (packages: string[]) => Promise<void>;
  installDep: (packages: string[]) => Promise<void>;
  reload: () => void;
}

function normalizeError(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export function usePrefixDeps(profileName: string, prefixPath: string): UsePrefixDepsResult {
  const [deps, setDeps] = useState<PrefixDependencyStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [reloadVersion, setReloadVersion] = useState(0);

  useEffect(() => {
    let active = true;

    async function load() {
      if (!profileName) {
        setDeps([]);
        setLoading(false);
        return;
      }

      setLoading(true);
      try {
        const result = await callCommand<PrefixDependencyStatus[]>('get_dependency_status', {
          profileName,
          prefixPath,
        });

        if (!active) return;
        setDeps(result);
        setError(null);
      } catch (loadError) {
        if (!active) return;
        setDeps([]);
        setError(normalizeError(loadError));
      } finally {
        if (active) setLoading(false);
      }
    }

    void load();

    return () => {
      active = false;
    };
  }, [profileName, prefixPath, reloadVersion]);

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

  const reload = useCallback(() => {
    setReloadVersion((v) => v + 1);
  }, []);

  return { deps, loading, error, checkDeps, installDep, reload };
}

export default usePrefixDeps;
