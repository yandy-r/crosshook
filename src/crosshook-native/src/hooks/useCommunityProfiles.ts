import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { AppSettingsData } from '../types';

export type CommunityCompatibilityRating = 'unknown' | 'broken' | 'partial' | 'working' | 'platinum';

export interface CommunityTapSubscription {
  url: string;
  branch?: string;
}

export interface CommunityProfileMetadata {
  game_name: string;
  game_version: string;
  trainer_name: string;
  trainer_version: string;
  proton_version: string;
  platform_tags: string[];
  compatibility_rating: CommunityCompatibilityRating;
  author: string;
  description: string;
}

export interface CommunityProfileManifest {
  schema_version: number;
  metadata: CommunityProfileMetadata;
  profile: {
    game: {
      name: string;
      executable_path: string;
    };
    trainer: {
      path: string;
      type: string;
    };
    injection: {
      dll_paths: string[];
      inject_on_launch: boolean[];
    };
    steam: {
      enabled: boolean;
      app_id: string;
      compatdata_path: string;
      proton_path: string;
      launcher: {
        icon_path: string;
        display_name: string;
      };
    };
    launch: {
      method: string;
    };
  };
}

export interface CommunityProfileIndexEntry {
  tap_url: string;
  tap_branch?: string;
  tap_path: string;
  manifest_path: string;
  relative_path: string;
  manifest: CommunityProfileManifest;
}

export interface CommunityProfileIndex {
  entries: CommunityProfileIndexEntry[];
  diagnostics: string[];
}

export interface CommunityTapWorkspace {
  subscription: CommunityTapSubscription;
  local_path: string;
}

export interface CommunityTapSyncResult {
  workspace: CommunityTapWorkspace;
  status: 'cloned' | 'updated';
  head_commit: string;
  index: CommunityProfileIndex;
}

export interface CommunityImportResult {
  profile_name: string;
  source_path: string;
  profile_path: string;
  manifest: CommunityProfileManifest;
}

export interface UseCommunityProfilesOptions {
  profilesDirectoryPath?: string;
}

export interface UseCommunityProfilesResult {
  taps: CommunityTapSubscription[];
  index: CommunityProfileIndex;
  loading: boolean;
  syncing: boolean;
  importing: boolean;
  error: string | null;
  refreshProfiles: () => Promise<void>;
  syncTaps: () => Promise<void>;
  addTap: (tap: CommunityTapSubscription) => Promise<CommunityTapSubscription[]>;
  removeTap: (tap: CommunityTapSubscription) => Promise<void>;
  importCommunityProfile: (jsonPath: string) => Promise<CommunityImportResult>;
  setError: (message: string | null) => void;
}

function normalizeTap(tap: CommunityTapSubscription): CommunityTapSubscription {
  const url = tap.url.trim();
  if (!url) {
    throw new Error('Tap URL is required.');
  }

  const normalized: CommunityTapSubscription = { url };
  const branch = tap.branch?.trim();
  if (branch) {
    normalized.branch = branch;
  }

  return normalized;
}

function dedupeTaps(taps: CommunityTapSubscription[]): CommunityTapSubscription[] {
  const seen = new Set<string>();
  const unique: CommunityTapSubscription[] = [];

  for (const tap of taps) {
    const normalized = normalizeTap(tap);
    const key = `${normalized.url}::${normalized.branch ?? ''}`;
    if (seen.has(key)) {
      continue;
    }

    seen.add(key);
    unique.push(normalized);
  }

  return unique;
}

export function useCommunityProfiles(options: UseCommunityProfilesOptions): UseCommunityProfilesResult {
  const [taps, setTaps] = useState<CommunityTapSubscription[]>([]);
  const [index, setIndex] = useState<CommunityProfileIndex>({
    entries: [],
    diagnostics: [],
  });
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const persistTaps = useCallback((nextTaps: CommunityTapSubscription[]) => {
    const deduped = dedupeTaps(nextTaps);
    setTaps(deduped);
    return deduped;
  }, []);

  const saveSettingsTaps = useCallback(async (nextTaps: CommunityTapSubscription[]) => {
    const settings = await invoke<AppSettingsData>('settings_load');
    await invoke('settings_save', {
      data: {
        ...settings,
        community_taps: nextTaps,
      } satisfies AppSettingsData,
    });
  }, []);

  const refreshProfiles = useCallback(async () => {
    const response = await invoke<CommunityProfileIndex>('community_list_profiles');
    setIndex(response);
  }, []);

  const syncTaps = useCallback(async () => {
    setSyncing(true);
    setError(null);

    try {
      await invoke<CommunityTapSyncResult[]>('community_sync');
      await refreshProfiles();
    } catch (syncError) {
      setError(syncError instanceof Error ? syncError.message : String(syncError));
      throw syncError;
    } finally {
      setSyncing(false);
    }
  }, [refreshProfiles]);

  const addTap = useCallback(
    async (tap: CommunityTapSubscription) => {
      setError(null);
      const normalized = normalizeTap(tap);

      try {
        const response = await invoke<CommunityTapSubscription[]>('community_add_tap', {
          tap: normalized,
        });
        const updatedTaps = persistTaps(response);
        await refreshProfiles();
        return updatedTaps;
      } catch (addError) {
        setError(addError instanceof Error ? addError.message : String(addError));
        throw addError;
      }
    },
    [persistTaps, refreshProfiles]
  );

  const removeTap = useCallback(
    async (tap: CommunityTapSubscription) => {
      setError(null);
      const normalized = normalizeTap(tap);
      const nextTaps = taps.filter(
        (entry) => !(entry.url === normalized.url && (entry.branch ?? '') === (normalized.branch ?? ''))
      );
      const deduped = dedupeTaps(nextTaps);
      await saveSettingsTaps(deduped);
      setTaps(deduped);
      await refreshProfiles();
    },
    [refreshProfiles, saveSettingsTaps, taps]
  );

  const importCommunityProfile = useCallback(async (jsonPath: string) => {
    setImporting(true);
    setError(null);

    try {
      return await invoke<CommunityImportResult>('community_import_profile', {
        path: jsonPath,
      });
    } catch (importError) {
      setError(importError instanceof Error ? importError.message : String(importError));
      throw importError;
    } finally {
      setImporting(false);
    }
  }, []);

  useEffect(() => {
    let active = true;

    async function loadInitialState() {
      try {
        const settings = await invoke<AppSettingsData>('settings_load');
        if (!active) {
          return;
        }

        setTaps(dedupeTaps(settings.community_taps));
        setError(null);
      } catch (loadError) {
        if (active) {
          setError(loadError instanceof Error ? loadError.message : String(loadError));
          setTaps([]);
        }
      } finally {
        if (active) {
          setLoading(false);
        }
      }
    }

    void loadInitialState();

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (loading) {
      return;
    }

    void refreshProfiles().catch((refreshError) => {
      setError(refreshError instanceof Error ? refreshError.message : String(refreshError));
    });
  }, [loading, refreshProfiles, taps]);

  return {
    taps,
    index,
    loading,
    syncing,
    importing,
    error,
    refreshProfiles,
    syncTaps,
    addTap,
    removeTap,
    importCommunityProfile,
    setError,
  };
}
