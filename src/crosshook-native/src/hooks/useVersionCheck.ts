import { useCallback } from 'react';
import { callCommand } from '@/lib/ipc';

/**
 * Wraps `check_version_status` so UI code does not call IPC transport directly.
 */
export function useVersionCheck() {
  const checkVersionStatus = useCallback(async (name: string) => {
    await callCommand('check_version_status', { name }).catch(() => {});
  }, []);

  return { checkVersionStatus };
}
