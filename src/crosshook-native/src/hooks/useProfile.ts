import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { AppSettingsData, GameProfile, LauncherInfo, RecentFilesData } from '../types';

export interface PendingDelete {
  name: string;
  launcherInfo: LauncherInfo | null;
}

export type PersistProfileDraft = (name: string, profile: GameProfile) => Promise<void>;

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
  pendingDelete: PendingDelete | null;
  setProfileName: (name: string) => void;
  selectProfile: (name: string) => Promise<void>;
  hydrateProfile: (name: string, profile: GameProfile) => void;
  updateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  saveProfile: () => Promise<void>;
  persistProfileDraft: PersistProfileDraft;
  deleteProfile: () => Promise<void>;
  confirmDelete: (name: string) => Promise<void>;
  executeDelete: () => Promise<void>;
  cancelDelete: () => void;
  refreshProfiles: () => Promise<void>;
}

export interface UseProfileOptions {
  autoSelectFirstProfile?: boolean;
}

type ResolvedLaunchMethod = Exclude<GameProfile['launch']['method'], ''>;
const automaticLauncherSuffix = ' - Trainer';

function looksLikeWindowsExecutable(path: string): boolean {
  return path.trim().toLowerCase().endsWith('.exe');
}

function stripAutomaticLauncherSuffix(value: string): string {
  const trimmed = value.trim();
  return trimmed.endsWith(automaticLauncherSuffix)
    ? trimmed.slice(0, -automaticLauncherSuffix.length).trimEnd()
    : trimmed;
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
    stripAutomaticLauncherSuffix(profile.steam.launcher.display_name) ||
    deriveGameName(profile) ||
    stripAutomaticLauncherSuffix(deriveDisplayNameFromPath(profile.trainer.path))
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
        display_name: stripAutomaticLauncherSuffix(profile.steam.launcher.display_name),
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

function validateProfileForSave(profile: GameProfile): string | null {
  if (!profile.game.executable_path.trim()) {
    return 'Game executable path is required before saving a profile.';
  }

  return null;
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
      method: 'proton_run',
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
  const [pendingDelete, setPendingDelete] = useState<PendingDelete | null>(null);

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

  const hydrateProfile = useCallback(
    (name: string, nextProfile: GameProfile) => {
      const trimmedName = name.trim();
      if (!trimmedName) {
        setError('Profile name is required.');
        return;
      }

      const normalizedProfile = normalizeProfileForEdit(nextProfile);

      setSelectedProfile(profiles.includes(trimmedName) ? trimmedName : '');
      setProfileName(trimmedName);
      setProfile(normalizedProfile);
      setDirty(true);
      setError(null);
    },
    [profiles]
  );

  const updateProfile = useCallback((updater: (current: GameProfile) => GameProfile) => {
    setProfile((current) => updater(current));
    setDirty(true);
  }, []);

  const persistProfileDraft = useCallback(
    async (name: string, draftProfile: GameProfile) => {
      const trimmedName = name.trim();
      if (!trimmedName) {
        setError('Profile name is required.');
        return;
      }

      const validationError = validateProfileForSave(draftProfile);
      if (validationError !== null) {
        setError(validationError);
        return;
      }

      setSaving(true);
      setError(null);

      try {
        const normalizedProfile = normalizeProfileForSave(draftProfile);
        await invoke('profile_save', { name: trimmedName, data: normalizedProfile });
        await syncProfileMetadata(trimmedName, normalizedProfile);
        await refreshProfiles();
        await loadProfile(trimmedName);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setSaving(false);
      }
    },
    [loadProfile, refreshProfiles, syncProfileMetadata]
  );

  const saveProfile = useCallback(async () => {
    await persistProfileDraft(profileName, profile);
  }, [persistProfileDraft, profile, profileName]);

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

  const confirmDelete = useCallback(async (name: string) => {
    const trimmed = name.trim();
    if (!trimmed) {
      setError('Select a profile to delete.');
      return;
    }

    setError(null);

    try {
      const launcherInfo = await invoke<LauncherInfo>('check_launcher_for_profile', {
        name: trimmed,
      });

      if (launcherInfo.script_exists || launcherInfo.desktop_entry_exists) {
        setPendingDelete({ name: trimmed, launcherInfo });
        return;
      }
    } catch (err) {
      console.error('Failed to inspect launcher state before profile delete.', err);
    }

    setPendingDelete({ name: trimmed, launcherInfo: null });
  }, []);

  const executeDelete = useCallback(async () => {
    if (!pendingDelete) {
      return;
    }

    const { name } = pendingDelete;
    setPendingDelete(null);
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
  }, [loadProfile, pendingDelete]);

  const cancelDelete = useCallback(() => {
    setPendingDelete(null);
  }, []);

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
    pendingDelete,
    setProfileName,
    selectProfile,
    hydrateProfile,
    updateProfile,
    saveProfile,
    persistProfileDraft,
    deleteProfile,
    confirmDelete,
    executeDelete,
    cancelDelete,
    refreshProfiles,
  };
}
