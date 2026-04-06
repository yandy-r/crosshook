import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AppSettingsData, GameProfile } from '../types';
import { toSettingsSaveRequest } from '../types/settings';

export type CommunityCompatibilityRating = 'unknown' | 'broken' | 'partial' | 'working' | 'platinum';

export interface CommunityTapSubscription {
  url: string;
  branch?: string;
  pinned_commit?: string;
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
  /** Optional SHA-256 of trainer executable (from manifest). */
  trainer_sha256?: string | null;
}

export interface CommunityProfileManifest {
  schema_version: number;
  metadata: CommunityProfileMetadata;
  profile: GameProfile;
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
  status: 'cloned' | 'updated' | 'cached_fallback';
  head_commit: string;
  index: CommunityProfileIndex;
  /** True when git fetch failed but a local clone was used for the index. */
  from_cache?: boolean;
  /** ISO timestamp of last successful network sync for this tap, if known. */
  last_sync_at?: string | null;
}

export interface CommunityImportResult {
  profile_name: string;
  source_path: string;
  profile_path: string;
  profile: GameProfile;
  manifest: CommunityProfileManifest;
}

export interface CommunityImportPreview {
  profile_name: string;
  source_path: string;
  profile: GameProfile;
  manifest: CommunityProfileManifest;
  required_prefix_deps: string[];
}

export interface CommunityExportResult {
  profile_name: string;
  output_path: string;
  manifest: CommunityProfileManifest;
}

export interface UseCommunityProfilesOptions {
  profilesDirectoryPath?: string;
}

export interface UseCommunityProfilesResult {
  taps: CommunityTapSubscription[];
  index: CommunityProfileIndex;
  lastSyncedCommits: Record<string, string>;
  /** Results from the most recent successful `community_sync` (for offline/cache UI). */
  lastTapSyncResults: CommunityTapSyncResult[];
  importedProfileNames: Set<string>;
  loading: boolean;
  syncing: boolean;
  importing: boolean;
  error: string | null;
  refreshProfiles: () => Promise<void>;
  syncTaps: () => Promise<void>;
  addTap: (tap: CommunityTapSubscription) => Promise<CommunityTapSubscription[]>;
  removeTap: (tap: CommunityTapSubscription) => Promise<void>;
  pinTapToCurrentVersion: (tap: CommunityTapSubscription) => Promise<void>;
  unpinTap: (tap: CommunityTapSubscription) => Promise<void>;
  getTapHeadCommit: (tap: CommunityTapSubscription) => string | undefined;
  prepareCommunityImport: (jsonPath: string) => Promise<CommunityImportPreview>;
  saveImportedProfile: (name: string, profile: GameProfile) => Promise<void>;
  setError: (message: string | null) => void;
}

function sanitizeProfileName(name: string): string {
  let slug = '';
  let lastWasSeparator = false;
  for (const ch of name.trim()) {
    if (/^[a-zA-Z0-9]$/.test(ch)) {
      slug += ch.toLowerCase();
      lastWasSeparator = false;
    } else if (!lastWasSeparator) {
      slug += '-';
      lastWasSeparator = true;
    }
  }

  const trimmed = slug.replace(/^-+|-+$/g, '');
  return trimmed.length === 0 ? 'community-profile' : trimmed;
}

function basename(value: string): string {
  const normalized = value.replace(/\\/g, '/').replace(/\/+$/, '');
  const parts = normalized.split('/');
  return parts[parts.length - 1] ?? '';
}

export function deriveCommunityImportProfileName(entry: CommunityProfileIndexEntry): string {
  const gameName = entry.manifest.metadata.game_name.trim();
  if (gameName.length > 0) {
    return sanitizeProfileName(gameName);
  }

  const normalizedRelativePath = entry.relative_path.replace(/\\/g, '/');
  const segments = normalizedRelativePath.split('/').filter((segment) => segment.length > 0);
  const parent = segments.length > 1 ? segments[segments.length - 2] : basename(normalizedRelativePath);
  if (parent.length > 0) {
    return sanitizeProfileName(parent);
  }

  return 'community-profile';
}

function tapIdentityKey(tap: CommunityTapSubscription): string {
  return `${tap.url}::${tap.branch ?? ''}::${tap.pinned_commit ?? ''}`;
}

function tapSyncKey(tap: CommunityTapSubscription): string {
  return `${tap.url}::${tap.branch ?? ''}`;
}

function isSameTapIdentity(left: CommunityTapSubscription, right: CommunityTapSubscription): boolean {
  return (
    left.url === right.url &&
    (left.branch ?? '') === (right.branch ?? '') &&
    (left.pinned_commit ?? '') === (right.pinned_commit ?? '')
  );
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

  const pinnedCommit = tap.pinned_commit?.trim();
  if (pinnedCommit) {
    normalized.pinned_commit = pinnedCommit;
  }

  return normalized;
}

function dedupeTaps(taps: CommunityTapSubscription[]): CommunityTapSubscription[] {
  const seen = new Set<string>();
  const unique: CommunityTapSubscription[] = [];

  for (const tap of taps) {
    const normalized = normalizeTap(tap);
    const key = tapIdentityKey(normalized);
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
  const [lastSyncedCommits, setLastSyncedCommits] = useState<Record<string, string>>({});
  const [lastTapSyncResults, setLastTapSyncResults] = useState<CommunityTapSyncResult[]>([]);
  const [importedProfileNames, setImportedProfileNames] = useState<Set<string>>(new Set());
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
      data: toSettingsSaveRequest({ ...settings, community_taps: nextTaps }),
    });
  }, []);

  const refreshProfiles = useCallback(async () => {
    const response = await invoke<CommunityProfileIndex>('community_list_profiles');
    setIndex(response);
  }, []);

  const refreshImportedProfileNames = useCallback(async () => {
    const names = await invoke<string[]>('profile_list');
    setImportedProfileNames(new Set(names.map((name) => sanitizeProfileName(name)).filter((name) => name.length > 0)));
  }, []);

  const syncTaps = useCallback(async () => {
    setSyncing(true);
    setError(null);

    try {
      const results = await invoke<CommunityTapSyncResult[]>('community_sync');
      setLastTapSyncResults(results);
      setLastSyncedCommits((previous) => {
        const next = { ...previous };
        for (const result of results) {
          next[tapSyncKey(result.workspace.subscription)] = result.head_commit;
        }
        return next;
      });
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
      const nextTaps = taps.filter((entry) => tapIdentityKey(normalizeTap(entry)) !== tapIdentityKey(normalized));
      const deduped = dedupeTaps(nextTaps);
      await saveSettingsTaps(deduped);
      setTaps(deduped);
      setLastTapSyncResults((prev) =>
        prev.filter((r) => tapIdentityKey(normalizeTap(r.workspace.subscription)) !== tapIdentityKey(normalized))
      );
      await refreshProfiles();
    },
    [refreshProfiles, saveSettingsTaps, taps]
  );

  const getTapHeadCommit = useCallback(
    (tap: CommunityTapSubscription) => {
      const normalized = normalizeTap(tap);
      return lastSyncedCommits[tapSyncKey(normalized)];
    },
    [lastSyncedCommits]
  );

  const pinTapToCurrentVersion = useCallback(
    async (tap: CommunityTapSubscription) => {
      setError(null);
      const normalized = normalizeTap(tap);
      const headCommit = lastSyncedCommits[tapSyncKey(normalized)];
      if (!headCommit) {
        throw new Error('No synced commit found for this tap. Sync taps first, then pin.');
      }

      const nextTaps = taps.map((entry) =>
        isSameTapIdentity(normalizeTap(entry), normalized)
          ? normalizeTap({ ...entry, pinned_commit: headCommit })
          : normalizeTap(entry)
      );
      const deduped = dedupeTaps(nextTaps);
      await saveSettingsTaps(deduped);
      setTaps(deduped);
      await refreshProfiles();
    },
    [lastSyncedCommits, refreshProfiles, saveSettingsTaps, taps]
  );

  const unpinTap = useCallback(
    async (tap: CommunityTapSubscription) => {
      setError(null);
      const normalized = normalizeTap(tap);
      const nextTaps = taps.map((entry) =>
        isSameTapIdentity(normalizeTap(entry), normalized)
          ? normalizeTap({ ...entry, pinned_commit: undefined })
          : normalizeTap(entry)
      );
      const deduped = dedupeTaps(nextTaps);
      await saveSettingsTaps(deduped);
      setTaps(deduped);
      await refreshProfiles();
    },
    [refreshProfiles, saveSettingsTaps, taps]
  );

  const prepareCommunityImport = useCallback(async (jsonPath: string) => {
    setImporting(true);
    setError(null);

    try {
      return await invoke<CommunityImportPreview>('community_prepare_import', {
        path: jsonPath,
      });
    } catch (importError) {
      setError(importError instanceof Error ? importError.message : String(importError));
      throw importError;
    } finally {
      setImporting(false);
    }
  }, []);

  const saveImportedProfile = useCallback(
    async (name: string, profile: GameProfile) => {
      setImporting(true);
      setError(null);

      try {
        await invoke('profile_save', {
          name,
          data: profile,
        });
        await refreshImportedProfileNames();
      } catch (saveError) {
        setError(saveError instanceof Error ? saveError.message : String(saveError));
        throw saveError;
      } finally {
        setImporting(false);
      }
    },
    [refreshImportedProfileNames]
  );

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
    void refreshImportedProfileNames().catch((loadError) => {
      if (active) {
        setError(loadError instanceof Error ? loadError.message : String(loadError));
      }
    });

    return () => {
      active = false;
    };
  }, [refreshImportedProfileNames]);

  useEffect(() => {
    let active = true;
    const unlistenPromise = listen<string>('profiles-changed', () => {
      if (!active) {
        return;
      }

      void refreshImportedProfileNames().catch((syncError) => {
        setError(syncError instanceof Error ? syncError.message : String(syncError));
      });
    });

    return () => {
      active = false;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refreshImportedProfileNames]);

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
    lastSyncedCommits,
    lastTapSyncResults,
    importedProfileNames,
    loading,
    syncing,
    importing,
    error,
    refreshProfiles,
    syncTaps,
    addTap,
    removeTap,
    pinTapToCurrentVersion,
    unpinTap,
    getTapHeadCommit,
    prepareCommunityImport,
    saveImportedProfile,
    setError,
  };
}
