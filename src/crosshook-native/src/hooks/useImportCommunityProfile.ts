import { useCallback } from 'react';
import { callCommand } from '@/lib/ipc';
import type { CommunityImportResult } from './useCommunityProfiles';

export interface ImportCommunityProfileApi {
  /**
   * Imports the community manifest at `path` as a new profile. The backend never overwrites an
   * existing profile: `profile_name` is the name actually written (possibly `<name>-N`).
   */
  importCommunityProfile: (path: string) => Promise<CommunityImportResult>;
}

export function useImportCommunityProfile(): ImportCommunityProfileApi {
  const importCommunityProfile = useCallback(
    (path: string) => callCommand<CommunityImportResult>('community_import_profile', { path }),
    []
  );

  return { importCommunityProfile };
}
