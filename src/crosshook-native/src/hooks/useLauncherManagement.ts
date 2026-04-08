import { useCallback, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { LauncherDeleteResult, LauncherInfo } from '../types';

interface UseLauncherManagementOptions {
  targetHomePath: string;
  steamClientInstallPath: string;
}

interface UseLauncherManagementResult {
  launchers: LauncherInfo[];
  error: string | null;
  isListing: boolean;
  deletingSlug: string | null;
  reexportingSlug: string | null;
  listLaunchers: () => Promise<void>;
  deleteLauncher: (launcherSlug: string) => Promise<boolean>;
  reexportLauncher: (launcherSlug: string) => Promise<boolean>;
}

export function useLauncherManagement({
  targetHomePath,
  steamClientInstallPath,
}: UseLauncherManagementOptions): UseLauncherManagementResult {
  const [launchers, setLaunchers] = useState<LauncherInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isListing, setIsListing] = useState(false);
  const [deletingSlug, setDeletingSlug] = useState<string | null>(null);
  const [reexportingSlug, setReexportingSlug] = useState<string | null>(null);

  const listLaunchers = useCallback(async () => {
    setIsListing(true);
    try {
      const result = await callCommand<LauncherInfo[]>('list_launchers', {
        targetHomePath,
        steamClientInstallPath,
      });
      setLaunchers(result);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsListing(false);
    }
  }, [targetHomePath, steamClientInstallPath]);

  const deleteLauncher = useCallback(
    async (launcherSlug: string) => {
      setDeletingSlug(launcherSlug);
      setError(null);
      try {
        await callCommand<LauncherDeleteResult>('delete_launcher_by_slug', {
          launcherSlug,
          targetHomePath,
          steamClientInstallPath,
        });
        await listLaunchers();
        return true;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return false;
      } finally {
        setDeletingSlug(null);
      }
    },
    [listLaunchers, targetHomePath, steamClientInstallPath]
  );

  const reexportLauncher = useCallback(
    async (launcherSlug: string) => {
      setReexportingSlug(launcherSlug);
      setError(null);
      try {
        await callCommand<void>('reexport_launcher_by_slug', {
          launcherSlug,
          targetHomePath,
          steamClientInstallPath,
        });
        await listLaunchers();
        return true;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return false;
      } finally {
        setReexportingSlug(null);
      }
    },
    [listLaunchers, targetHomePath, steamClientInstallPath]
  );

  return {
    launchers,
    error,
    isListing,
    deletingSlug,
    reexportingSlug,
    listLaunchers,
    deleteLauncher,
    reexportLauncher,
  };
}

export default useLauncherManagement;
