import { useCallback, useMemo, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type {
  GameProfile,
  LauncherInfo,
  SerializedDuplicateProfileResult,
  SerializedGameProfile,
} from '../../types';
import type { OptimizationEntry } from '../../utils/optimization-catalog';
import { createEmptyProfile } from './createEmptyProfile';
import { mergeLiveLaunchSections } from './mergeLiveLaunchSections';
import { normalizeProfileForEdit, normalizeProfileForSave } from './profileNormalize';
import { clearLastUsedProfileIfDeleted, syncProfileMetadata } from './profileMetadataSync';
import { validateProfileForSave } from './profileValidation';

export interface PendingDelete {
  name: string;
  launcherInfo: LauncherInfo | null;
}

export type PersistProfileDraftResult = { ok: true } | { ok: false; error: string };
export type PersistProfileDraft = (name: string, profile: GameProfile) => Promise<PersistProfileDraftResult>;

export type EnqueueProfileWrite = <T>(fn: () => Promise<T>) => Promise<T>;
export type FlushPendingLaunchSectionSaves = (nameForSave: string) => Promise<void>;

export interface RenameProfileResult {
  ok: boolean;
  hadLauncher: boolean;
}

export interface ProfileLoadOptions {
  collectionId?: string;
  loadErrorContext?: string;
  throwOnFailure?: boolean;
}

interface UseProfileCrudOptions {
  optionsById: Record<string, OptimizationEntry>;
  /** True once optimization catalog fetch completed successfully (`catalog !== null` in `useProfile`). */
  catalogLoaded: boolean;
  autoSelectFirstProfile: boolean;
  setLastSavedProfileSnapshot: (profile: GameProfile) => void;
  clearAutosaveTimers: () => void;
  enqueueProfileWrite?: EnqueueProfileWrite;
  flushPendingLaunchSectionSaves?: FlushPendingLaunchSectionSaves;
}

export function useProfileCrud({
  optionsById,
  catalogLoaded,
  autoSelectFirstProfile,
  setLastSavedProfileSnapshot,
  clearAutosaveTimers,
  enqueueProfileWrite,
  flushPendingLaunchSectionSaves,
}: UseProfileCrudOptions) {
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

  const profileRef = useRef(profile);
  profileRef.current = profile;
  const selectedProfileRef = useRef(selectedProfile);
  selectedProfileRef.current = selectedProfile;
  // Monotonic request id for `loadProfile`. Only the latest request may change
  // load state, so an out-of-order response (the startup auto-load racing a
  // user pick) cannot overwrite the newer selection.
  const loadSeqRef = useRef(0);
  // Set synchronously at the start of every named `loadProfile` request, before
  // any await. ProfileContext reads it to keep the startup auto-load from
  // racing an in-flight user-initiated load (`selectedProfile` is only written
  // after `profile_load` resolves, so it is too late as a guard).
  const selectionRequestedRef = useRef(false);

  const hasExistingSavedProfile = useMemo(() => {
    const trimmedName = profileName.trim();
    return (
      trimmedName.length > 0 &&
      selectedProfile.trim().length > 0 &&
      trimmedName === selectedProfile &&
      profiles.includes(trimmedName)
    );
  }, [profileName, profiles, selectedProfile]);

  const hasExistingSavedProfileRef = useRef(false);
  hasExistingSavedProfileRef.current = hasExistingSavedProfile;

  const profileExists = useMemo(() => profiles.includes(profileName.trim()), [profileName, profiles]);

  const loadFavorites = useCallback(async () => {
    try {
      const names = await callCommand<string[]>('profile_list_favorites');
      setFavoriteProfiles(names);
    } catch {
      setFavoriteProfiles([]);
    }
  }, []);

  const toggleFavorite = useCallback(
    async (name: string, favorite: boolean) => {
      try {
        await callCommand('profile_set_favorite', { name, favorite });
        await loadFavorites();
      } catch (err) {
        console.error('Failed to update profile favorite state', err);
        throw err;
      }
    },
    [loadFavorites]
  );

  const loadProfile = useCallback(
    async (name: string, loadOptions?: ProfileLoadOptions) => {
      const trimmed = name.trim();
      if (!trimmed) {
        // An explicit deselection also supersedes any in-flight load: it bumps
        // the sequence so a late response stays inert, and completes
        // synchronously, so it owns clearing the loading flag.
        loadSeqRef.current += 1;
        const empty = createEmptyProfile();
        setSelectedProfile('');
        setProfileName('');
        setProfile(empty);
        setDirty(false);
        setLastSavedProfileSnapshot(empty);
        setLoading(false);
        return;
      }

      // Only the newest request may change load state; a slower response that
      // started earlier must not overwrite a newer selection (auto-load racing
      // a user pick, or two rapid user picks). Any named request also marks a
      // selection as requested, which suppresses the startup auto-load.
      const requestId = ++loadSeqRef.current;
      selectionRequestedRef.current = true;
      const isStale = () => requestId !== loadSeqRef.current;

      setLoading(true);
      setError(null);
      const formatLoadError = (err: unknown) => (err instanceof Error ? err.message : String(err));
      const collectionId = loadOptions?.collectionId?.trim() || undefined;

      try {
        const loaded = await callCommand<SerializedGameProfile>('profile_load', {
          name: trimmed,
          collectionId,
        });
        if (isStale()) {
          return;
        }
        const normalized = normalizeProfileForEdit(loaded, optionsById, catalogLoaded);
        setSelectedProfile(trimmed);
        setProfileName(trimmed);
        setProfile(normalized);
        setDirty(false);
        setLastSavedProfileSnapshot(normalized);

        try {
          await syncProfileMetadata(trimmed, normalized);
        } catch (syncErr) {
          console.error('Failed to sync profile metadata (last-used profile, recent files)', syncErr);
          if (isStale()) {
            return;
          }
          setError(
            `Profile loaded, but preferences sync failed: ${syncErr instanceof Error ? syncErr.message : String(syncErr)}`
          );
        }
      } catch (err) {
        if (isStale()) {
          return;
        }
        const msg = formatLoadError(err);
        const fullMsg = loadOptions?.loadErrorContext ? `${loadOptions.loadErrorContext}: ${msg}` : msg;
        setError(fullMsg);
        if (loadOptions?.throwOnFailure) {
          throw fullMsg;
        }
      } finally {
        // A newer request owns the loading flag; leave it untouched so it can
        // set `false` itself when its own response lands.
        if (!isStale()) {
          setLoading(false);
        }
      }
    },
    [catalogLoaded, optionsById, setLastSavedProfileSnapshot]
  );

  const refreshProfiles = useCallback(async () => {
    try {
      const names = await callCommand<string[]>('profile_list');
      setProfiles(names);

      if (names.length === 0) {
        // Preserve a completed auto-load selection when the list is still empty
        // (startup race: `profile_load` from the startup report can resolve before
        // `profile_list` returns). In-flight loads are covered by `loadProfile`'s
        // request counter, which suppresses state changes from stale responses.
        if (selectedProfileRef.current.trim()) {
          return;
        }

        const empty = createEmptyProfile();
        setSelectedProfile('');
        setProfileName('');
        setProfile(empty);
        setDirty(false);
        setLastSavedProfileSnapshot(empty);
        return;
      }

      if (selectedProfile && names.includes(selectedProfile)) {
        return;
      }

      if (autoSelectFirstProfile) {
        await loadProfile(names[0]);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [autoSelectFirstProfile, loadProfile, selectedProfile, setLastSavedProfileSnapshot]);

  const finalizeProfileDeletion = useCallback(
    async (name: string) => {
      const trimmedDeleted = name.trim();
      await clearLastUsedProfileIfDeleted(name);
      const names = await callCommand<string[]>('profile_list');
      setProfiles(names);
      void loadFavorites();

      if (names.length === 0) {
        const empty = createEmptyProfile();
        setSelectedProfile('');
        setProfileName('');
        setProfile(empty);
        setDirty(false);
        setLastSavedProfileSnapshot(empty);
        return;
      }

      const currentSelected = selectedProfile.trim();
      if (currentSelected && names.includes(currentSelected) && currentSelected !== trimmedDeleted) {
        return;
      }

      await loadProfile(names[0], {
        loadErrorContext: 'Profile deleted, but loading the next profile failed',
      });
    },
    [loadFavorites, loadProfile, selectedProfile, setLastSavedProfileSnapshot]
  );

  const hydrateProfile = useCallback(
    (name: string, nextProfile: GameProfile) => {
      const trimmedName = name.trim();
      if (!trimmedName) {
        setError('Profile name is required.');
        return;
      }

      const normalizedProfile = normalizeProfileForEdit(nextProfile, optionsById, catalogLoaded);
      setSelectedProfile(profiles.includes(trimmedName) ? trimmedName : '');
      setProfileName(trimmedName);
      setProfile(normalizedProfile);
      setLastSavedProfileSnapshot(normalizedProfile);
      setDirty(true);
      setError(null);
    },
    [catalogLoaded, optionsById, profiles, setLastSavedProfileSnapshot]
  );

  const updateProfile = useCallback((updater: (current: GameProfile) => GameProfile) => {
    setProfile((current: GameProfile) => updater(current));
    setDirty(true);
  }, []);

  const updateLaunchSetting = useCallback((updater: (current: GameProfile) => GameProfile) => {
    setProfile((current: GameProfile) => updater(current));
    setDirty((currentDirty: boolean) => currentDirty || !hasExistingSavedProfileRef.current);
  }, []);

  const persistProfileDraft = useCallback<PersistProfileDraft>(
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
        if (flushPendingLaunchSectionSaves) {
          await flushPendingLaunchSectionSaves(trimmedName);
        }

        const draftWithLiveLaunchSections = mergeLiveLaunchSections(draftProfile, profileRef.current);
        const normalizedProfile = normalizeProfileForSave(draftWithLiveLaunchSections, optionsById, catalogLoaded);

        const saveProfile = async () => {
          await callCommand('profile_save', { name: trimmedName, data: normalizedProfile });
        };

        if (enqueueProfileWrite) {
          await enqueueProfileWrite(saveProfile);
        } else {
          await saveProfile();
        }

        setLastSavedProfileSnapshot(normalizedProfile);
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
    [
      catalogLoaded,
      enqueueProfileWrite,
      flushPendingLaunchSectionSaves,
      loadProfile,
      optionsById,
      refreshProfiles,
      setLastSavedProfileSnapshot,
    ]
  );

  const saveProfile = useCallback(async () => {
    await persistProfileDraft(profileName, profile);
  }, [persistProfileDraft, profile, profileName]);

  const duplicateProfile = useCallback(
    async (sourceName: string): Promise<void> => {
      if (!sourceName.trim()) return;
      setDuplicating(true);
      setError(null);
      try {
        const result = await callCommand<SerializedDuplicateProfileResult>('profile_duplicate', {
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

  const renameProfile = useCallback(
    async (oldName: string, newName: string): Promise<RenameProfileResult> => {
      if (!oldName.trim() || !newName.trim() || oldName.trim() === newName.trim()) {
        return { ok: false, hadLauncher: false };
      }

      clearAutosaveTimers();
      setRenaming(true);
      setError(null);
      try {
        const hadLauncher = await callCommand<boolean>('profile_rename', {
          oldName: oldName.trim(),
          newName: newName.trim(),
        });
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
    [clearAutosaveTimers, loadFavorites, loadProfile, refreshProfiles]
  );

  const confirmDelete = useCallback(async (name: string) => {
    const trimmed = name.trim();
    if (!trimmed) {
      setError('Select a profile to delete.');
      return;
    }

    setError(null);
    try {
      const launcherInfo = await callCommand<LauncherInfo>('check_launcher_for_profile', {
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
      await callCommand('profile_delete', { name });
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
    hasExistingSavedProfile,
    setProfileName,
    setProfile,
    setDirty,
    setError,
    loadFavorites,
    toggleFavorite,
    loadProfile,
    selectionRequestedRef,
    refreshProfiles,
    hydrateProfile,
    updateProfile,
    updateLaunchSetting,
    saveProfile,
    persistProfileDraft,
    duplicateProfile,
    renameProfile,
    confirmDelete,
    executeDelete,
    cancelDelete,
  };
}
