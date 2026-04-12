import { useCallback, useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type { ProtonInstallOption } from '../types/proton';

export interface UseProtonInstallsOptions {
  steamClientInstallPath?: string;
}

export interface UseProtonInstallsResult {
  installs: ProtonInstallOption[];
  error: string | null;
  reload: () => void;
}

function normalizeLoadError(loadError: unknown): string {
  return loadError instanceof Error ? loadError.message : String(loadError);
}

function sortProtonInstalls(installs: ProtonInstallOption[]): ProtonInstallOption[] {
  return [...installs].sort((left, right) => {
    if (left.is_official !== right.is_official) {
      return left.is_official ? -1 : 1;
    }

    return left.name.localeCompare(right.name) || left.path.localeCompare(right.path);
  });
}

export function useProtonInstalls(options: UseProtonInstallsOptions = {}): UseProtonInstallsResult {
  const [installs, setInstalls] = useState<ProtonInstallOption[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [_reloadVersion, setReloadVersion] = useState(0);
  const steamClientInstallPath = options.steamClientInstallPath?.trim() ?? '';

  const reload = useCallback(() => {
    setReloadVersion((current) => current + 1);
  }, []);

  useEffect(() => {
    let active = true;

    async function loadProtonInstalls() {
      try {
        const resolvedInstalls = await callCommand<ProtonInstallOption[]>('list_proton_installs', {
          steamClientInstallPath: steamClientInstallPath.length > 0 ? steamClientInstallPath : undefined,
        });

        if (!active) {
          return;
        }

        setInstalls(sortProtonInstalls(resolvedInstalls));
        setError(null);
      } catch (loadError) {
        if (!active) {
          return;
        }

        setInstalls([]);
        setError(normalizeLoadError(loadError));
      }
    }

    void loadProtonInstalls();

    return () => {
      active = false;
    };
  }, [steamClientInstallPath]);

  return {
    installs,
    error,
    reload,
  };
}

export default useProtonInstalls;
