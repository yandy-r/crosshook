import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { AppSettingsData, DuplicateProfileResult, GameProfile, LauncherInfo, RecentFilesData } from '../types';
import {
  LAUNCH_OPTIMIZATION_OPTIONS_BY_ID,
  getConflictingLaunchOptimizationIds,
  type LaunchOptimizationId,
} from '../types/launch-optimizations';
import { resolveLaunchMethod, type ResolvedLaunchMethod } from '../utils/launch';

export interface PendingDelete {
  name: string;
  launcherInfo: LauncherInfo | null;
}

export type PersistProfileDraftResult = { ok: true } | { ok: false; error: string };

export type PersistProfileDraft = (name: string, profile: GameProfile) => Promise<PersistProfileDraftResult>;

export interface UseProfileResult {
  profiles: string[];
  favoriteProfiles: string[];
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
  launchOptimizationsStatus: LaunchOptimizationsStatus;
  setProfileName: (name: string) => void;
  selectProfile: (name: string) => Promise<void>;
  hydrateProfile: (name: string, profile: GameProfile) => void;
  updateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  toggleLaunchOptimization: (optionId: LaunchOptimizationId, nextEnabled: boolean) => void;
  saveProfile: () => Promise<void>;
  /** Duplicates the named profile on the backend and auto-selects the new copy. */
  duplicateProfile: (sourceName: string) => Promise<void>;
  /** True while a duplication IPC call is in-flight. */
  duplicating: boolean;
  /** Renames an existing profile on the backend, refreshes the list, and selects the new name. */
  renameProfile: (oldName: string, newName: string) => Promise<RenameProfileResult>;
  /** True while a rename IPC call is in-flight. */
  renaming: boolean;
  persistProfileDraft: PersistProfileDraft;
  confirmDelete: (name: string) => Promise<void>;
  executeDelete: () => Promise<void>;
  cancelDelete: () => void;
  refreshProfiles: () => Promise<void>;
  toggleFavorite: (name: string, favorite: boolean) => Promise<void>;
}

export interface UseProfileOptions {
  autoSelectFirstProfile?: boolean;
}

export interface RenameProfileResult {
  ok: boolean;
  hadLauncher: boolean;
}

const automaticLauncherSuffix = ' - Trainer';
const launchOptimizationsAutosaveDelayMs = 350;

export type LaunchOptimizationsStatusTone = 'idle' | 'saving' | 'success' | 'warning' | 'error';

export interface LaunchOptimizationsStatus {
  tone: LaunchOptimizationsStatusTone;
  label: string;
  detail?: string;
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

function normalizeLaunchOptimizationIds(
  ids: readonly string[] | undefined
): LaunchOptimizationId[] {
  if (ids === undefined) {
    return [];
  }

  const normalized: LaunchOptimizationId[] = [];
  const seenIds = new Set<LaunchOptimizationId>();

  for (const optionId of ids) {
    if (!(optionId in LAUNCH_OPTIMIZATION_OPTIONS_BY_ID)) {
      continue;
    }

    const typedOptionId = optionId as LaunchOptimizationId;
    if (seenIds.has(typedOptionId)) {
      continue;
    }

    seenIds.add(typedOptionId);
    normalized.push(typedOptionId);
  }

  return normalized;
}

type ApplyLaunchOptimizationToggleResult =
  | { ok: true; profile: GameProfile }
  | { ok: false; conflictLabels: string[] };

function applyLaunchOptimizationToggle(
  current: GameProfile,
  optionId: LaunchOptimizationId,
  nextEnabled: boolean
): ApplyLaunchOptimizationToggleResult {
  const currentIds = current.launch.optimizations.enabled_option_ids;
  const conflictingIds = nextEnabled
    ? getConflictingLaunchOptimizationIds(optionId, currentIds)
    : [];

  if (conflictingIds.length > 0) {
    const conflictLabels = conflictingIds.map(
      (conflictingId) => LAUNCH_OPTIMIZATION_OPTIONS_BY_ID[conflictingId].label
    );
    return { ok: false, conflictLabels };
  }

  const nextIds = nextEnabled
    ? normalizeLaunchOptimizationIds([...currentIds, optionId])
    : currentIds.filter((currentOptionId) => currentOptionId !== optionId);

  return {
    ok: true,
    profile: {
      ...current,
      launch: {
        ...current.launch,
        optimizations: {
          enabled_option_ids: nextIds,
        },
      },
    },
  };
}

function areLaunchOptimizationIdsEqual(
  left: readonly LaunchOptimizationId[],
  right: readonly LaunchOptimizationId[]
): boolean {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((optionId, index) => optionId === right[index]);
}

function buildLaunchOptimizationsStatus(
  method: ResolvedLaunchMethod,
  hasExistingSavedProfile: boolean
): LaunchOptimizationsStatus {
  if (method !== 'proton_run' && method !== 'steam_applaunch') {
    return {
      tone: 'warning',
      label: 'Unavailable for current method',
      detail: 'Launch optimizations are only editable when the profile method is proton_run or steam_applaunch.',
    };
  }

  if (!hasExistingSavedProfile) {
    return {
      tone: 'warning',
      label: 'Save profile first to enable autosave',
      detail: 'Optimization changes stay local until the profile has been saved once.',
    };
  }

  return {
    tone: 'idle',
    label: 'Ready to autosave',
    detail:
      method === 'steam_applaunch'
        ? 'Only launch.optimizations will be written automatically; paste the generated line into Steam yourself.'
        : 'Only launch.optimizations will be written automatically for this saved profile.',
  };
}

function normalizeProfileForEdit(profile: GameProfile): GameProfile {
  const method = resolveLaunchMethod(profile);
  const runtime = profile.runtime ?? {
    prefix_path: '',
    proton_path: '',
    working_directory: '',
  };
  const enabledOptionIds = normalizeLaunchOptimizationIds(
    profile.launch.optimizations?.enabled_option_ids
  );

  return {
    ...profile,
    trainer: {
      ...profile.trainer,
      type: profile.trainer.type.trim(),
      loading_mode: profile.trainer.loading_mode ?? 'source_directory',
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
      optimizations: {
        enabled_option_ids: enabledOptionIds,
      },
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
      loading_mode: 'source_directory',
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
      optimizations: {
        enabled_option_ids: [],
      },
    },
  };
}

export function useProfile(options: UseProfileOptions = {}): UseProfileResult {
  const [profiles, setProfiles] = useState<string[]>([]);
  const [favoriteProfiles, setFavoriteProfiles] = useState<string[]>([]);
  const [selectedProfile, setSelectedProfile] = useState('');
  const [profileName, setProfileName] = useState('');
  const [profile, setProfile] = useState<GameProfile>(createEmptyProfile);
  const [dirty, setDirty] = useState(false);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [duplicating, setDuplicating] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<PendingDelete | null>(null);
  const [launchOptimizationsStatus, setLaunchOptimizationsStatus] = useState<LaunchOptimizationsStatus>(
    buildLaunchOptimizationsStatus('proton_run', false)
  );
  const launchOptimizationsAutosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastSavedLaunchOptimizationIdsRef = useRef<LaunchOptimizationId[]>([]);
  const profileRef = useRef(profile);
  profileRef.current = profile;

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

  const loadFavorites = useCallback(async () => {
    try {
      const names = await invoke<string[]>('profile_list_favorites');
      setFavoriteProfiles(names);
    } catch {
      setFavoriteProfiles([]);
    }
  }, []);

  const toggleFavorite = useCallback(async (name: string, favorite: boolean) => {
    try {
      await invoke('profile_set_favorite', { name, favorite });
      await loadFavorites();
    } catch (err) {
      console.error('Failed to update profile favorite state', err);
      throw err;
    }
  }, [loadFavorites]);

  const loadProfile = useCallback(
    async (name: string, loadOptions?: { loadErrorContext?: string }) => {
      const trimmed = name.trim();
      if (!trimmed) {
        setSelectedProfile('');
        setProfileName('');
        setProfile(createEmptyProfile());
        setDirty(false);
        lastSavedLaunchOptimizationIdsRef.current = [];
        return;
      }

      setLoading(true);
      setError(null);

      const formatLoadError = (err: unknown) =>
        err instanceof Error ? err.message : String(err);

      try {
        const loaded = await invoke<GameProfile>('profile_load', { name: trimmed });
        const normalized = normalizeProfileForEdit(loaded);
        setSelectedProfile(trimmed);
        setProfileName(trimmed);
        setProfile(normalized);
        setDirty(false);
        lastSavedLaunchOptimizationIdsRef.current = normalized.launch.optimizations.enabled_option_ids;

        try {
          await syncProfileMetadata(trimmed, normalized);
        } catch (syncErr) {
          console.error('Failed to sync profile metadata (last-used profile, recent files)', syncErr);
          setError(`Profile loaded, but preferences sync failed: ${syncErr instanceof Error ? syncErr.message : String(syncErr)}`);
        }
      } catch (err) {
        const msg = formatLoadError(err);
        setError(loadOptions?.loadErrorContext ? `${loadOptions.loadErrorContext}: ${msg}` : msg);
      } finally {
        setLoading(false);
      }
    },
    [syncProfileMetadata]
  );

  const refreshProfiles = useCallback(async () => {
    try {
      const names = await invoke<string[]>('profile_list');
      setProfiles(names);

      if (names.length === 0) {
        setSelectedProfile('');
        setProfileName('');
        setProfile(createEmptyProfile());
        setDirty(false);
        lastSavedLaunchOptimizationIdsRef.current = [];
        return;
      }

      if (selectedProfile && names.includes(selectedProfile)) {
        return;
      }

      if (options.autoSelectFirstProfile ?? true) {
        await loadProfile(names[0]);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [loadProfile, options.autoSelectFirstProfile, selectedProfile]);

  const selectProfile = loadProfile;

  const finalizeProfileDeletion = useCallback(
    async (name: string) => {
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
      void loadFavorites();

      if (names.length === 0) {
        setSelectedProfile('');
        setProfileName('');
        setProfile(createEmptyProfile());
        setDirty(false);
        lastSavedLaunchOptimizationIdsRef.current = [];
        return;
      }

      await loadProfile(names[0], {
        loadErrorContext: 'Profile deleted, but loading the next profile failed',
      });
    },
    [loadFavorites, loadProfile]
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
      lastSavedLaunchOptimizationIdsRef.current =
        normalizedProfile.launch.optimizations.enabled_option_ids;
      setDirty(true);
      setError(null);
    },
    [profiles]
  );

  const updateProfile = useCallback((updater: (current: GameProfile) => GameProfile) => {
    setProfile((current) => updater(current));
    setDirty(true);
  }, []);

  const hasExistingSavedProfile = useMemo(() => {
    const trimmedName = profileName.trim();
    return (
      trimmedName.length > 0 &&
      selectedProfile.trim().length > 0 &&
      trimmedName === selectedProfile &&
      profiles.includes(trimmedName)
    );
  }, [profileName, profiles, selectedProfile]);

  const toggleLaunchOptimization = useCallback(
    (optionId: LaunchOptimizationId, nextEnabled: boolean) => {
      const result = applyLaunchOptimizationToggle(profileRef.current, optionId, nextEnabled);
      if (!result.ok) {
        setLaunchOptimizationsStatus({
          tone: 'warning',
          label: 'Conflicting option blocked',
          detail: `Disable ${result.conflictLabels.join(' or ')} before enabling ${LAUNCH_OPTIMIZATION_OPTIONS_BY_ID[optionId].label}.`,
        });
        return;
      }

      setProfile(result.profile);
      setDirty((currentDirty) => currentDirty || !hasExistingSavedProfile);
    },
    [hasExistingSavedProfile]
  );

  const persistProfileDraft = useCallback(
    async (name: string, draftProfile: GameProfile): Promise<PersistProfileDraftResult> => {
      const trimmedName = name.trim();
      if (!trimmedName) {
        const message = 'Profile name is required.';
        setError(message);
        return { ok: false, error: message };
      }

      const validationError = validateProfileForSave(draftProfile);
      if (validationError !== null) {
        setError(validationError);
        return { ok: false, error: validationError };
      }

      setSaving(true);
      setError(null);

      try {
        const normalizedProfile = normalizeProfileForSave(draftProfile);
        await invoke('profile_save', { name: trimmedName, data: normalizedProfile });
        lastSavedLaunchOptimizationIdsRef.current =
          normalizedProfile.launch.optimizations.enabled_option_ids;
        await syncProfileMetadata(trimmedName, normalizedProfile);
        await refreshProfiles();
        await loadProfile(trimmedName);
        return { ok: true };
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
        return { ok: false, error: message };
      } finally {
        setSaving(false);
      }
    },
    [loadProfile, refreshProfiles, syncProfileMetadata]
  );

  const saveProfile = useCallback(async () => {
    await persistProfileDraft(profileName, profile);
  }, [persistProfileDraft, profile, profileName]);

  /**
   * Duplicates the profile identified by `sourceName` via the backend, then
   * refreshes the profile list and auto-selects the newly created copy.
   *
   * The backend generates a unique name (e.g. "MyGame (Copy)") so the caller
   * does not need to supply one. The `duplicating` state flag is true for
   * the duration of the async operation, allowing the UI to show a spinner.
   *
   * @param sourceName - Name of the existing profile to duplicate.
   */
  const duplicateProfile = useCallback(
    async (sourceName: string): Promise<void> => {
      if (!sourceName.trim()) return;
      setDuplicating(true);
      setError(null);
      try {
        const result = await invoke<DuplicateProfileResult>('profile_duplicate', {
          name: sourceName,
        });
        await refreshProfiles();
        await loadProfile(result.name);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
      } finally {
        setDuplicating(false);
      }
    },
    [loadProfile, refreshProfiles]
  );

  /**
   * Renames the profile identified by `oldName` to `newName` via the backend,
   * then refreshes the profile list and auto-selects the renamed profile.
   *
   * The autosave timer is cancelled before the rename to prevent a debounced
   * save from writing to the old filename while the rename is in progress.
   *
   * @param oldName - Current name of the profile to rename.
   * @param newName - Desired new name for the profile.
   */
  const renameProfile = useCallback(
    async (oldName: string, newName: string): Promise<RenameProfileResult> => {
      if (!oldName.trim() || !newName.trim() || oldName.trim() === newName.trim()) {
        return { ok: false, hadLauncher: false };
      }

      if (launchOptimizationsAutosaveTimerRef.current !== null) {
        clearTimeout(launchOptimizationsAutosaveTimerRef.current);
        launchOptimizationsAutosaveTimerRef.current = null;
      }

      setRenaming(true);
      setError(null);
      try {
        const hadLauncher = await invoke<boolean>('profile_rename', { oldName: oldName.trim(), newName: newName.trim() });
        await refreshProfiles();
        await loadProfile(newName.trim());
        await loadFavorites();
        return { ok: true, hadLauncher };
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return { ok: false, hadLauncher: false };
      } finally {
        setRenaming(false);
      }
    },
    [loadFavorites, loadProfile, refreshProfiles]
  );

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
      setError(err instanceof Error ? err.message : String(err));
      return;
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
      await finalizeProfileDeletion(name);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeleting(false);
    }
  }, [finalizeProfileDeletion, pendingDelete]);

  const cancelDelete = useCallback(() => {
    setPendingDelete(null);
  }, []);

  useEffect(() => {
    void refreshProfiles().catch((err: unknown) => {
      setError(err instanceof Error ? err.message : String(err));
    });
    void loadFavorites();
  }, [loadFavorites, refreshProfiles]);

  useEffect(() => {
    const method = resolveLaunchMethod(profile);
    const currentIds = profile.launch.optimizations.enabled_option_ids;

    if (launchOptimizationsAutosaveTimerRef.current !== null) {
      clearTimeout(launchOptimizationsAutosaveTimerRef.current);
      launchOptimizationsAutosaveTimerRef.current = null;
    }

    if ((method !== 'proton_run' && method !== 'steam_applaunch') || !hasExistingSavedProfile) {
      setLaunchOptimizationsStatus(buildLaunchOptimizationsStatus(method, hasExistingSavedProfile));
      return;
    }

    if (areLaunchOptimizationIdsEqual(currentIds, lastSavedLaunchOptimizationIdsRef.current)) {
      setLaunchOptimizationsStatus(buildLaunchOptimizationsStatus(method, true));
      return;
    }

    setLaunchOptimizationsStatus({
      tone: 'saving',
      label: 'Saving...',
      detail: 'Persisting only launch.optimizations for the current saved profile.',
    });

    const trimmedName = profileName.trim();
    const normalizedIds = [...currentIds];
    let cancelled = false;

    launchOptimizationsAutosaveTimerRef.current = setTimeout(() => {
      void (async () => {
        try {
          await invoke('profile_save_launch_optimizations', {
            name: trimmedName,
            optimizations: {
              enabled_option_ids: normalizedIds,
            },
          });

          if (cancelled) {
            return;
          }

          lastSavedLaunchOptimizationIdsRef.current = normalizedIds;
          setLaunchOptimizationsStatus({
            tone: 'success',
            label: 'Saved automatically',
            detail: 'Only the Launch Optimizations section was written to disk.',
          });
        } catch (err) {
          if (cancelled) {
            return;
          }

          setDirty(true);
          setLaunchOptimizationsStatus({
            tone: 'error',
            label: 'Failed to save',
            detail: err instanceof Error ? err.message : String(err),
          });
        }
      })();
    }, launchOptimizationsAutosaveDelayMs);

    return () => {
      cancelled = true;
      if (launchOptimizationsAutosaveTimerRef.current !== null) {
        clearTimeout(launchOptimizationsAutosaveTimerRef.current);
        launchOptimizationsAutosaveTimerRef.current = null;
      }
    };
  }, [hasExistingSavedProfile, profile, profileName]);

  const profileExists = useMemo(() => profiles.includes(profileName.trim()), [profileName, profiles]);

  return {
    profiles,
    favoriteProfiles,
    selectedProfile,
    profileName,
    profile,
    dirty,
    loading,
    saving,
    deleting,
    duplicating,
    renaming,
    error,
    profileExists,
    pendingDelete,
    launchOptimizationsStatus,
    setProfileName,
    selectProfile,
    hydrateProfile,
    updateProfile,
    toggleLaunchOptimization,
    saveProfile,
    duplicateProfile,
    renameProfile,
    persistProfileDraft,
    confirmDelete,
    executeDelete,
    cancelDelete,
    refreshProfiles,
    toggleFavorite,
  };
}
