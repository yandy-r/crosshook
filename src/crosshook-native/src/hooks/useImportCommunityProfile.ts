import { useCallback } from 'react';
import { callCommand } from '@/lib/ipc';

export function useImportCommunityProfile() {
  const importCommunityProfile = useCallback(async (path: string) => {
    await callCommand('community_import_profile', { path });
  }, []);

  return { importCommunityProfile };
}
