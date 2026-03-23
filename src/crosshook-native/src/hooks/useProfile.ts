import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { AppSettingsData, GameProfile, RecentFilesData } from '../types';

export interface UseProfileResult {
  profiles: string[];
  selectedProfile: string;
  profileName: string;
  profile: GameProfile;
  dirty: boolean;
  loading: boolean;
  saving: boolean;
  deleting: boolean;
  error: string | null;
  profileExists: boolean;
  setProfileName: (name: string) => void;
  selectProfile: (name: string) => Promise<void>;
  updateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  saveProfile: () => Promise<void>;
  deleteProfile: () => Promise<void>;
  refreshProfiles: () => Promise<void>;
}

export interface UseProfileOptions {
  autoSelectFirstProfile?: boolean;
}

type ResolvedLaunchMethod = Exclude<GameProfile['launch']['method'], ''>;

function looksLikeWindowsExecutable(path: string): boolean {
  return path.trim().toLowerCase().endsWith('.exe');
}

function deriveDisplayNameFromPath(path: string): string {
  const normalized = path.trim();
  if (!normalized) {
    return '';
  }

  const segment = normalized.split(/[\\/]/).pop() ?? '';
  return segment.replace(/\.[^.]+$/, '').trim();
}

function deriveGameName(profile: GameProfile): string {
  return profile.game.name.trim() || deriveDisplayNameFromPath(profile.game.executable_path);
}

function deriveLauncherDisplayName(profile: GameProfile): string {
  return (
    profile.steam.launcher.display_name.trim() ||
    deriveGameName(profile) ||
    deriveDisplayNameFromPath(profile.trainer.path)
  );
}

function resolveLaunchMethod(profile: GameProfile): ResolvedLaunchMethod {
  const method = profile.launch.method.trim();

  if (method === 'steam_applaunch' || method === 'proton_run' || method === 'native') {
    return method;
  }

  if (profile.steam.enabled) {
    return 'steam_applaunch';
  }

  if (looksLikeWindowsExecutable(profile.game.executable_path)) {
    return 'proton_run';
  }

  return 'native';
}

function normalizeProfileForEdit(profile: GameProfile): GameProfile {
  const method = resolveLaunchMethod(profile);
  const runtime = profile.runtime ?? {
    prefix_path: '',
    proton_path: '',
    working_directory: '',
  };

  return {
    ...profile,
    trainer: {
      ...profile.trainer,
      type: profile.trainer.type.trim(),
    },
    steam: {
      ...profile.steam,
      enabled: method === 'steam_applaunch',
      launcher: {
        ...profile.steam.launcher,
      },
    },
    runtime: {
      prefix_path: runtime.prefix_path.trim(),
      proton_path: runtime.proton_path.trim(),
      working_directory: runtime.working_directory.trim(),
    },
    launch: {
      ...profile.launch,
      method,
    },
  };
}

function normalizeProfileForSave(profile: GameProfile): GameProfile {
  const normalized = normalizeProfileForEdit(profile);

  return {
    ...normalized,
    game: {
      ...normalized.game,
      name: deriveGameName(normalized),
    },
    trainer: {
      ...normalized.trainer,
    },
    steam: {
      ...normalized.steam,
      launcher: {
        ...normalized.steam.launcher,
        display_name: deriveLauncherDisplayName(normalized),
      },
    },
  };
}

function mergeRecentPaths(currentPaths: string[], nextPath: string): string[] {
  const trimmed = nextPath.trim();
  if (!trimmed) {
    return currentPaths;
  }

  return [trimmed, ...currentPaths.filter((path) => path !== trimmed)].slice(0, 10);
}

function createEmptyProfile(): GameProfile {
  return {
    game: {
      name: '',
      executable_path: '',
    },
    trainer: {
      path: '',
      type: '',
    },
    injection: {
      dll_paths: [],
      inject_on_launch: [false, false],
    },
    steam: {
      enabled: false,
      app_id: '',
      compatdata_path: '',
      proton_path: '',
      launcher: {
        icon_path: '',
        display_name: '',
      },
    },
    runtime: {
      prefix_path: '',
      proton_path: '',
      working_directory: '',
    },
    launch: {
      method: '',
    },
  };
}

export function useProfile(options: UseProfileOptions = {}): UseProfileResult {
  const [profiles, setProfiles] = useState<string[]>([]);
  const [selectedProfile, setSelectedProfile] = useState('');
  const [profileName, setProfileName] = useState('');
  const [profile, setProfile] = useState<GameProfile>(createEmptyProfile);
  const [dirty, setDirty] = useState(false);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const syncProfileMetadata = useCallback(async (name: string, currentProfile: GameProfile) => {
    const settings = await invoke<AppSettingsData>('settings_load');
    await invoke('settings_save', {
      data: {
        ...settings,
        last_used_profile: name,
      } satisfies AppSettingsData,
    });

    const recentFiles = await invoke<RecentFilesData>('recent_files_load');
    await invoke('recent_files_save', {
      data: {
        game_paths: mergeRecentPaths(recentFiles.game_paths, currentProfile.game.executable_path),
        trainer_paths: mergeRecentPaths(recentFiles.trainer_paths, currentProfile.trainer.path),
        dll_paths: currentProfile.injection.dll_paths.reduce(
          (paths, dllPath) => mergeRecentPaths(paths, dllPath),
          recentFiles.dll_paths
        ),
      } satisfies RecentFilesData,
    });
  }, []);

  const loadProfile = useCallback(async (name: string) => {
    const trimmed = name.trim();
    if (!trimmed) {
      setSelectedProfile('');
      setProfileName('');
      setProfile(createEmptyProfile());
      setDirty(false);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const loaded = await invoke<GameProfile>('profile_load', { name: trimmed });
      const normalized = normalizeProfileForEdit(loaded);
      setSelectedProfile(trimmed);
      setProfileName(trimmed);
      setProfile(normalized);
      setDirty(false);
      await syncProfileMetadata(trimmed, normalized);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  const refreshProfiles = useCallback(async () => {
    const names = await invoke<string[]>('profile_list');
    setProfiles(names);

    if (names.length === 0) {
      setSelectedProfile('');
      setProfileName('');
      setProfile(createEmptyProfile());
      setDirty(false);
      return;
    }

    if (selectedProfile && names.includes(selectedProfile)) {
      return;
    }

    if (options.autoSelectFirstProfile ?? true) {
      await loadProfile(names[0]);
    }
  }, [loadProfile, options.autoSelectFirstProfile, selectedProfile]);

  const selectProfile = useCallback(
    async (name: string) => {
      await loadProfile(name);
    },
    [loadProfile]
  );

  const updateProfile = useCallback((updater: (current: GameProfile) => GameProfile) => {
    setProfile((current) => updater(current));
    setDirty(true);
  }, []);

  const saveProfile = useCallback(async () => {
    const name = profileName.trim();
    if (!name) {
      setError('Profile name is required.');
      return;
    }

    setSaving(true);
    setError(null);

    try {
      const normalizedProfile = normalizeProfileForSave(profile);
      setProfile(normalizedProfile);
      await invoke('profile_save', { name, data: normalizedProfile });
      await syncProfileMetadata(name, normalizedProfile);
      await refreshProfiles();
      await loadProfile(name);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }, [profile, profileName, refreshProfiles]);

  const deleteProfile = useCallback(async () => {
    const name = profileName.trim();
    if (!name) {
      setError('Select a profile to delete.');
      return;
    }

    setDeleting(true);
    setError(null);

    try {
      await invoke('profile_delete', { name });
      const settings = await invoke<AppSettingsData>('settings_load');
      if (settings.last_used_profile === name) {
        await invoke('settings_save', {
          data: {
            ...settings,
            last_used_profile: '',
          } satisfies AppSettingsData,
        });
      }
      const names = await invoke<string[]>('profile_list');
      setProfiles(names);

      if (names.length === 0) {
        setSelectedProfile('');
        setProfileName('');
        setProfile(createEmptyProfile());
        setDirty(false);
        return;
      }

      await loadProfile(names[0]);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeleting(false);
    }
  }, [loadProfile, profileName]);

  useEffect(() => {
    void refreshProfiles().catch((err: unknown) => {
      setError(err instanceof Error ? err.message : String(err));
    });
  }, [refreshProfiles]);

  const profileExists = useMemo(() => profiles.includes(profileName.trim()), [profileName, profiles]);

  return {
    profiles,
    selectedProfile,
    profileName,
    profile,
    dirty,
    loading,
    saving,
    deleting,
    error,
    profileExists,
    setProfileName,
    selectProfile,
    updateProfile,
    saveProfile,
    deleteProfile,
    refreshProfiles,
  };
}
