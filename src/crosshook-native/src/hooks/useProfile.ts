import { useCallback, useEffect, useMemo, useRef } from 'react';
import { subscribeEvent } from '@/lib/events';
import type {
  BundledOptimizationPreset,
  ConfigDiffResult,
  ConfigRevisionSummary,
  ConfigRollbackResult,
  GameProfile,
  LaunchAutoSaveStatus,
} from '../types';
import type { LaunchOptimizationId } from '../types/launch-optimizations';
import {
  buildConflictMatrix,
  buildOptionsById,
  type OptimizationCatalogPayload,
  type OptimizationEntry,
} from '../utils/optimization-catalog';

// Stable empty fallbacks prevent new object references on each render when catalog is null,
// which would otherwise cascade into useProfileCrud / useProfileLaunchAutosave dependency loops.
const EMPTY_OPTIONS_BY_ID: Record<string, OptimizationEntry> = {};
const EMPTY_CONFLICT_MATRIX: Record<string, readonly string[]> = {};

import type { LaunchOptimizationsStatus as LaunchOptimizationsStatusType } from './profile/launchOptimizationStatus';
import {
  type PendingDelete as PendingDeleteType,
  type PersistProfileDraftResult as PersistProfileDraftResultType,
  type PersistProfileDraft as PersistProfileDraftType,
  type RenameProfileResult as RenameProfileResultType,
  useProfileCrud,
} from './profile/useProfileCrud';
import { useProfileHistory } from './profile/useProfileHistory';
import { useProfileLaunchAutosave } from './profile/useProfileLaunchAutosave';
import { useLaunchOptimizationCatalog } from './useLaunchOptimizationCatalog';

export type PendingDelete = PendingDeleteType;
export type PersistProfileDraftResult = PersistProfileDraftResultType;
export type PersistProfileDraft = PersistProfileDraftType;
export type RenameProfileResult = RenameProfileResultType;

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
  gamescopeAutoSaveStatus: LaunchAutoSaveStatus;
  trainerGamescopeAutoSaveStatus: LaunchAutoSaveStatus;
  mangoHudAutoSaveStatus: LaunchAutoSaveStatus;
  /** True while any config-history IPC call is in flight. */
  historyLoading: boolean;
  /** Error message from the most recent config-history operation; null when none. */
  historyError: string | null;
  setProfileName: (name: string) => void;
  /**
   * Loads a profile into the editor / active state. Mirrors `loadProfile`.
   *
   * The optional `loadOptions.collectionId` triggers Rust-side merge of that
   * collection's launch defaults via `effective_profile_with`. **EDITOR SAFETY**:
   * `ProfilesPage` callers MUST NOT pass `collectionId` — the editor must always
   * see the raw storage profile so saves don't persist a merged view.
   */
  selectProfile: (
    name: string,
    loadOptions?: {
      collectionId?: string;
      loadErrorContext?: string;
      throwOnFailure?: boolean;
    }
  ) => Promise<void>;
  hydrateProfile: (name: string, profile: GameProfile) => void;
  updateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  updateLaunchSetting: (updater: (current: GameProfile) => GameProfile) => void;
  toggleLaunchOptimization: (optionId: LaunchOptimizationId, nextEnabled: boolean) => void;
  /** Persists switching the active named optimization preset (requires presets in profile TOML). */
  switchLaunchOptimizationPreset: (presetName: string) => Promise<void>;
  /** GPU vendor presets from the app catalog (metadata DB); empty when metadata is off. */
  bundledOptimizationPresets: BundledOptimizationPreset[];
  /** Applies a bundled preset: writes `launch.presets.bundled/<id>` and activates it. */
  applyBundledOptimizationPreset: (presetId: string) => Promise<void>;
  /** Saves current checkbox selection as a new user preset and activates it. */
  saveManualOptimizationPreset: (presetDisplayName: string) => Promise<void>;
  /** True while apply/save bundled or manual preset IPC is in flight. */
  optimizationPresetActionBusy: boolean;
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
  /** Returns the revision history for the named profile, newest-first. */
  fetchConfigHistory: (profileName: string, limit?: number) => Promise<ConfigRevisionSummary[]>;
  /** Returns a unified diff between a stored revision and the current profile state (or a second revision). */
  fetchConfigDiff: (profileName: string, revisionId: number, rightRevisionId?: number) => Promise<ConfigDiffResult>;
  /** Restores a stored revision, refreshes profile and health state on success. */
  rollbackConfig: (profileName: string, revisionId: number) => Promise<ConfigRollbackResult>;
  /** Marks a stored revision as the last known-good config for the profile. */
  markKnownGood: (profileName: string, revisionId: number) => Promise<void>;
  /** The runtime optimization catalog from the backend, or null while loading. */
  catalog: OptimizationCatalogPayload | null;
  /** True while the optimization catalog is being fetched from the backend. */
  catalogLoading: boolean;
}

export interface UseProfileOptions {
  autoSelectFirstProfile?: boolean;
  /**
   * Optional callback invoked after a successful rollback so the caller can
   * trigger a health revalidation without coupling the profile and health hooks.
   * Called with the profile name that was restored.
   */
  onAfterRollback?: (profileName: string) => void;
}

/** @deprecated Use `LaunchAutoSaveStatus` from `../types` instead. */
export type LaunchOptimizationsStatusTone = LaunchAutoSaveStatus['tone'];

/** @deprecated Use `LaunchAutoSaveStatus` from `../types` instead. */
export type LaunchOptimizationsStatus = LaunchOptimizationsStatusType;

export function useProfile(options: UseProfileOptions = {}): UseProfileResult {
  const { catalog, loading: catalogLoading } = useLaunchOptimizationCatalog();
  const catalogLoaded = catalog !== null;
  const optionsById = useMemo(() => (catalog ? buildOptionsById(catalog.entries) : EMPTY_OPTIONS_BY_ID), [catalog]);
  const conflictMatrix = useMemo(
    () => (catalog ? buildConflictMatrix(catalog.entries) : EMPTY_CONFLICT_MATRIX),
    [catalog]
  );

  const setLastSavedProfileSnapshotRef = useRef<(profile: GameProfile) => void>(() => {});
  const clearAutosaveTimersRef = useRef<() => void>(() => {});

  // Stable wrappers that call through refs. useCallback with [] ensures these never
  // change identity across renders — inline arrows would cause loadProfile to recreate
  // on every render cycle, leading to an infinite setState loop via InspectorSelectionContext.
  const setLastSavedProfileSnapshot = useCallback(
    (nextProfile: GameProfile) => setLastSavedProfileSnapshotRef.current(nextProfile),
    []
  );
  const clearAutosaveTimers = useCallback(() => clearAutosaveTimersRef.current(), []);

  const crud = useProfileCrud({
    optionsById,
    catalogLoaded,
    autoSelectFirstProfile: options.autoSelectFirstProfile ?? true,
    setLastSavedProfileSnapshot,
    clearAutosaveTimers,
  });

  const launchAutosave = useProfileLaunchAutosave({
    profile: crud.profile,
    profileName: crud.profileName,
    selectedProfile: crud.selectedProfile,
    hasExistingSavedProfile: crud.hasExistingSavedProfile,
    optionsById,
    catalogLoaded,
    conflictMatrix,
    setProfile: crud.setProfile,
    setDirty: crud.setDirty,
    setError: crud.setError,
  });

  setLastSavedProfileSnapshotRef.current = launchAutosave.setLastSavedProfileSnapshot;
  clearAutosaveTimersRef.current = launchAutosave.clearAutosaveTimers;

  const history = useProfileHistory({
    loadProfile: crud.loadProfile,
    onAfterRollback: options.onAfterRollback,
  });

  const { refreshProfiles, loadFavorites, setError } = crud;

  useEffect(() => {
    void refreshProfiles().catch((err: unknown) => {
      setError(err instanceof Error ? err.message : String(err));
    });
    void loadFavorites();
  }, [loadFavorites, refreshProfiles, setError]);

  useEffect(() => {
    let active = true;
    const unlistenPromise = subscribeEvent<string>('profiles-changed', () => {
      if (!active) {
        return;
      }

      void refreshProfiles().catch((err: unknown) => {
        setError(err instanceof Error ? err.message : String(err));
      });
      void loadFavorites();
    });

    return () => {
      active = false;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [loadFavorites, refreshProfiles, setError]);

  return {
    profiles: crud.profiles,
    favoriteProfiles: crud.favoriteProfiles,
    selectedProfile: crud.selectedProfile,
    profileName: crud.profileName,
    profile: crud.profile,
    dirty: crud.dirty,
    loading: crud.loading,
    saving: crud.saving,
    deleting: crud.deleting,
    duplicating: crud.duplicating,
    renaming: crud.renaming,
    error: crud.error,
    profileExists: crud.profileExists,
    pendingDelete: crud.pendingDelete,
    launchOptimizationsStatus: launchAutosave.launchOptimizationsStatus,
    gamescopeAutoSaveStatus: launchAutosave.gamescopeAutoSaveStatus,
    trainerGamescopeAutoSaveStatus: launchAutosave.trainerGamescopeAutoSaveStatus,
    mangoHudAutoSaveStatus: launchAutosave.mangoHudAutoSaveStatus,
    historyLoading: history.historyLoading,
    historyError: history.historyError,
    setProfileName: crud.setProfileName,
    selectProfile: crud.loadProfile,
    hydrateProfile: crud.hydrateProfile,
    updateProfile: crud.updateProfile,
    updateLaunchSetting: crud.updateLaunchSetting,
    toggleLaunchOptimization: launchAutosave.toggleLaunchOptimization,
    switchLaunchOptimizationPreset: launchAutosave.switchLaunchOptimizationPreset,
    bundledOptimizationPresets: launchAutosave.bundledOptimizationPresets,
    applyBundledOptimizationPreset: launchAutosave.applyBundledOptimizationPreset,
    saveManualOptimizationPreset: launchAutosave.saveManualOptimizationPreset,
    optimizationPresetActionBusy: launchAutosave.optimizationPresetActionBusy,
    saveProfile: crud.saveProfile,
    duplicateProfile: crud.duplicateProfile,
    renameProfile: crud.renameProfile,
    persistProfileDraft: crud.persistProfileDraft,
    confirmDelete: crud.confirmDelete,
    executeDelete: crud.executeDelete,
    cancelDelete: crud.cancelDelete,
    refreshProfiles: crud.refreshProfiles,
    toggleFavorite: crud.toggleFavorite,
    fetchConfigHistory: history.fetchConfigHistory,
    fetchConfigDiff: history.fetchConfigDiff,
    rollbackConfig: history.rollbackConfig,
    markKnownGood: history.markKnownGood,
    catalog,
    catalogLoading,
  };
}
