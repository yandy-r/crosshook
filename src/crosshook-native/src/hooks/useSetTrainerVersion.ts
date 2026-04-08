import { callCommand } from '@/lib/ipc';
import { useCallback, useState } from 'react';

export interface UseSetTrainerVersionResult {
  setting: boolean;
  error: string | null;
  success: boolean;
  setVersion: (version: string) => Promise<boolean>;
  clearSuccess: () => void;
}

export function useSetTrainerVersion(
  profileName: string,
  onVersionSet?: () => void
): UseSetTrainerVersionResult {
  const [setting, setSetting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const setVersion = useCallback(
    async (version: string) => {
      const trimmedVersion = version.trim();
      if (!trimmedVersion) {
        return false;
      }

      setSetting(true);
      setError(null);
      setSuccess(false);

      try {
        await callCommand('set_trainer_version', { name: profileName, version: trimmedVersion });
        onVersionSet?.();
        setSuccess(true);
        return true;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return false;
      } finally {
        setSetting(false);
      }
    },
    [onVersionSet, profileName]
  );

  const clearSuccess = useCallback(() => {
    setSuccess(false);
  }, []);

  return {
    setting,
    error,
    success,
    setVersion,
    clearSuccess,
  };
}
