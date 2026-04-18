import { type Dispatch, type SetStateAction, useCallback, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { BundledOptimizationPreset, GameProfile, LaunchAutoSaveStatus, SerializedGameProfile } from '../../types';
import type { LaunchOptimizationId } from '../../types/launch-optimizations';
import { resolveLaunchMethod } from '../../utils/launch';
import type { OptimizationEntry } from '../../utils/optimization-catalog';
import { formatInvokeError } from './formatInvokeError';
import { areLaunchOptimizationIdsEqual, normalizeLaunchOptimizationIds } from './launchOptimizationIds';
import { buildLaunchOptimizationsStatus, type LaunchOptimizationsStatus } from './launchOptimizationStatus';
import { applyLaunchOptimizationToggle } from './launchOptimizationToggle';
import { normalizeProfileForEdit } from './profileNormalize';
import {
  useBundledOptimizationPresetsEffect,
  useGamescopeAutosaveEffect,
  useLaunchOptimizationsAutosaveEffect,
  useMangoHudAutosaveEffect,
  useTrainerGamescopeAutosaveEffect,
} from './useProfileLaunchAutosaveEffects';

interface UseProfileLaunchAutosaveOptions {
  profile: GameProfile;
  profileName: string;
  selectedProfile: string;
  hasExistingSavedProfile: boolean;
  optionsById: Record<string, OptimizationEntry>;
  catalogLoaded: boolean;
  conflictMatrix: Readonly<Record<string, readonly string[]>>;
  setProfile: Dispatch<SetStateAction<GameProfile>>;
  setDirty: Dispatch<SetStateAction<boolean>>;
  setError: Dispatch<SetStateAction<string | null>>;
}
export function useProfileLaunchAutosave({
  profile,
  profileName,
  selectedProfile,
  hasExistingSavedProfile,
  optionsById,
  catalogLoaded,
  conflictMatrix,
  setProfile,
  setDirty,
  setError,
}: UseProfileLaunchAutosaveOptions) {
  const [launchOptimizationsStatus, setLaunchOptimizationsStatus] = useState<LaunchOptimizationsStatus>(
    buildLaunchOptimizationsStatus('proton_run', false)
  );
  const [bundledOptimizationPresets, setBundledOptimizationPresets] = useState<BundledOptimizationPreset[]>([]);
  const [optimizationPresetActionBusy, setOptimizationPresetActionBusy] = useState(false);
  const [gamescopeAutoSaveStatus, setGamescopeAutoSaveStatus] = useState<LaunchAutoSaveStatus>({
    tone: 'idle',
    label: 'Ready',
  });
  const [trainerGamescopeAutoSaveStatus, setTrainerGamescopeAutoSaveStatus] = useState<LaunchAutoSaveStatus>({
    tone: 'idle',
    label: 'Ready',
  });
  const [mangoHudAutoSaveStatus, setMangoHudAutoSaveStatus] = useState<LaunchAutoSaveStatus>({
    tone: 'idle',
    label: 'Ready',
  });
  const launchOptimizationsAutosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const gamescopeAutosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const trainerGamescopeAutosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mangoHudAutosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastSavedLaunchOptimizationIdsRef = useRef<LaunchOptimizationId[]>([]);
  const lastSavedGamescopeJsonRef = useRef<string>('null');
  const lastSavedTrainerGamescopeJsonRef = useRef<string>('null');
  const lastSavedMangoHudJsonRef = useRef<string>('null');
  const pendingLaunchPresetRef = useRef<string | null>(null);
  const profileRef = useRef(profile);
  profileRef.current = profile;
  const selectedProfileRef = useRef(selectedProfile);
  selectedProfileRef.current = selectedProfile;
  const hasExistingSavedProfileRef = useRef(hasExistingSavedProfile);
  hasExistingSavedProfileRef.current = hasExistingSavedProfile;
  /** Serializes launch-section writes so concurrent autosaves cannot clobber each other. */
  const launchProfileWriteChainRef = useRef<Promise<unknown>>(Promise.resolve());
  const enqueueLaunchProfileWrite = useCallback(<T>(fn: () => Promise<T>): Promise<T> => {
    const run: Promise<T> = launchProfileWriteChainRef.current.then(() => fn());
    launchProfileWriteChainRef.current = run.then(
      () => undefined,
      () => undefined
    );
    return run;
  }, []);
  const setLastSavedProfileSnapshot = useCallback((nextProfile: GameProfile) => {
    lastSavedLaunchOptimizationIdsRef.current = [...nextProfile.launch.optimizations.enabled_option_ids];
    lastSavedGamescopeJsonRef.current = JSON.stringify(nextProfile.launch.gamescope ?? null);
    lastSavedTrainerGamescopeJsonRef.current = JSON.stringify(nextProfile.launch.trainer_gamescope ?? null);
    lastSavedMangoHudJsonRef.current = JSON.stringify(nextProfile.launch.mangohud ?? null);
  }, []);
  const clearAutosaveTimers = useCallback(() => {
    if (launchOptimizationsAutosaveTimerRef.current !== null) {
      clearTimeout(launchOptimizationsAutosaveTimerRef.current);
      launchOptimizationsAutosaveTimerRef.current = null;
    }
    if (gamescopeAutosaveTimerRef.current !== null) {
      clearTimeout(gamescopeAutosaveTimerRef.current);
      gamescopeAutosaveTimerRef.current = null;
    }
    if (trainerGamescopeAutosaveTimerRef.current !== null) {
      clearTimeout(trainerGamescopeAutosaveTimerRef.current);
      trainerGamescopeAutosaveTimerRef.current = null;
    }
    if (mangoHudAutosaveTimerRef.current !== null) {
      clearTimeout(mangoHudAutosaveTimerRef.current);
      mangoHudAutosaveTimerRef.current = null;
    }
  }, []);
  /** Clears pending timer and persists launch.optimizations immediately. */
  const flushPendingLaunchOptimizationsSave = useCallback(
    async (nameForSave: string): Promise<void> => {
      if (launchOptimizationsAutosaveTimerRef.current !== null) {
        clearTimeout(launchOptimizationsAutosaveTimerRef.current);
        launchOptimizationsAutosaveTimerRef.current = null;
      }
      if (!hasExistingSavedProfileRef.current) {
        return;
      }
      const current = profileRef.current;
      const method = resolveLaunchMethod(current);
      if (method !== 'proton_run' && method !== 'steam_applaunch') {
        return;
      }
      const trimmed = nameForSave.trim();
      if (!trimmed) {
        return;
      }
      const currentIds = current.launch.optimizations.enabled_option_ids;
      if (areLaunchOptimizationIdsEqual(currentIds, lastSavedLaunchOptimizationIdsRef.current)) {
        return;
      }
      await enqueueLaunchProfileWrite(async () => {
        await callCommand('profile_save_launch_optimizations', {
          name: trimmed,
          optimizations: {
            enabled_option_ids: [...currentIds],
          },
        });
        lastSavedLaunchOptimizationIdsRef.current = [...currentIds];
      });
    },
    [enqueueLaunchProfileWrite]
  );
  const toggleLaunchOptimization = useCallback(
    (optionId: LaunchOptimizationId, nextEnabled: boolean) => {
      const result = applyLaunchOptimizationToggle(
        profileRef.current,
        optionId,
        nextEnabled,
        optionsById,
        conflictMatrix,
        catalogLoaded
      );
      if (!result.ok) {
        setLaunchOptimizationsStatus({
          tone: 'warning',
          label: 'Conflicting option blocked',
          detail: `Disable ${result.conflictLabels.join(' or ')} before enabling ${optionsById[optionId]?.label ?? optionId}.`,
        });
        return;
      }
      setProfile(result.profile);
      setDirty((currentDirty: boolean) => currentDirty || !hasExistingSavedProfileRef.current);
    },
    [catalogLoaded, conflictMatrix, optionsById, setDirty, setProfile]
  );
  const switchLaunchOptimizationPreset = useCallback(
    async (presetName: string): Promise<void> => {
      const trimmedName = profileName.trim();
      const key = presetName.trim();
      const requestProfileName = trimmedName;
      if (!trimmedName || !hasExistingSavedProfile || !key) {
        return;
      }
      const presets = profileRef.current.launch.presets ?? {};
      if (!presets[key]) {
        setLaunchOptimizationsStatus({
          tone: 'warning',
          label: 'Unknown preset',
          detail: `No launch optimization preset named "${key}" is defined in this profile.`,
        });
        return;
      }
      const method = resolveLaunchMethod(profileRef.current);
      if (method !== 'proton_run' && method !== 'steam_applaunch') {
        return;
      }
      try {
        await flushPendingLaunchOptimizationsSave(trimmedName);
      } catch (err) {
        setDirty(true);
        const message = formatInvokeError(err);
        setError(message);
        setLaunchOptimizationsStatus({
          tone: 'error',
          label: 'Failed to save',
          detail: message,
        });
        return;
      }
      launchOptimizationsAutosaveTimerRef.current = null;
      const currentAfterFlush = profileRef.current;
      const presetsMap = currentAfterFlush.launch.presets ?? {};
      const targetSection = presetsMap[key];
      if (!targetSection) {
        setLaunchOptimizationsStatus({
          tone: 'warning',
          label: 'Unknown preset',
          detail: `No launch optimization preset named "${key}" is defined in this profile.`,
        });
        return;
      }
      const targetPresetIds = normalizeLaunchOptimizationIds(
        targetSection.enabled_option_ids,
        optionsById,
        catalogLoaded
      );
      setError(null);
      pendingLaunchPresetRef.current = key;
      try {
        await enqueueLaunchProfileWrite(async () => {
          await callCommand('profile_save_launch_optimizations', {
            name: trimmedName,
            optimizations: {
              enabled_option_ids: [...targetPresetIds],
              switch_active_preset: key,
            },
          });
        });
        if (pendingLaunchPresetRef.current !== key || selectedProfileRef.current.trim() !== requestProfileName) {
          return;
        }
        setProfile((current: GameProfile) => ({
          ...current,
          launch: {
            ...current.launch,
            active_preset: key,
            optimizations: { enabled_option_ids: targetPresetIds },
          },
        }));
        lastSavedLaunchOptimizationIdsRef.current = targetPresetIds;
        setLaunchOptimizationsStatus({
          tone: 'success',
          label: 'Saved automatically',
          detail: 'Active optimization preset updated.',
        });
      } catch (err) {
        if (pendingLaunchPresetRef.current !== key || selectedProfileRef.current.trim() !== requestProfileName) {
          return;
        }
        setDirty(true);
        const message = formatInvokeError(err);
        setError(message);
        setLaunchOptimizationsStatus({
          tone: 'error',
          label: 'Failed to save',
          detail: message,
        });
      } finally {
        if (pendingLaunchPresetRef.current === key) {
          pendingLaunchPresetRef.current = null;
        }
      }
    },
    [
      enqueueLaunchProfileWrite,
      flushPendingLaunchOptimizationsSave,
      hasExistingSavedProfile,
      catalogLoaded,
      optionsById,
      profileName,
      setDirty,
      setError,
      setProfile,
    ]
  );
  const applyBundledOptimizationPreset = useCallback(
    async (presetId: string): Promise<void> => {
      const trimmedName = profileName.trim();
      const pid = presetId.trim();
      if (!trimmedName || !hasExistingSavedProfile || !pid) {
        return;
      }
      const method = resolveLaunchMethod(profileRef.current);
      if (method !== 'proton_run' && method !== 'steam_applaunch') {
        return;
      }
      try {
        await flushPendingLaunchOptimizationsSave(trimmedName);
      } catch (err) {
        setDirty(true);
        const message = formatInvokeError(err);
        setError(message);
        setLaunchOptimizationsStatus({
          tone: 'error',
          label: 'Failed to save',
          detail: message,
        });
        return;
      }
      launchOptimizationsAutosaveTimerRef.current = null;
      setOptimizationPresetActionBusy(true);
      setError(null);
      try {
        const updated = await enqueueLaunchProfileWrite(() =>
          callCommand<SerializedGameProfile>('profile_apply_bundled_optimization_preset', {
            name: trimmedName,
            presetId: pid,
          })
        );
        if (selectedProfileRef.current.trim() !== trimmedName) {
          return;
        }
        const normalized = normalizeProfileForEdit(updated, optionsById, catalogLoaded);
        setProfile(normalized);
        lastSavedLaunchOptimizationIdsRef.current = normalized.launch.optimizations.enabled_option_ids;
        setLaunchOptimizationsStatus({
          tone: 'success',
          label: 'Bundled preset applied',
          detail: 'Preset saved under launch.presets and activated for this profile.',
        });
      } catch (err) {
        const message = formatInvokeError(err);
        setError(message);
        setLaunchOptimizationsStatus({
          tone: 'error',
          label: 'Failed to apply bundled preset',
          detail: message,
        });
      } finally {
        setOptimizationPresetActionBusy(false);
      }
    },
    [
      enqueueLaunchProfileWrite,
      flushPendingLaunchOptimizationsSave,
      hasExistingSavedProfile,
      catalogLoaded,
      optionsById,
      profileName,
      setDirty,
      setError,
      setProfile,
    ]
  );
  const saveManualOptimizationPreset = useCallback(
    async (presetDisplayName: string): Promise<void> => {
      const trimmedName = profileName.trim();
      const key = presetDisplayName.trim();
      if (!trimmedName || !hasExistingSavedProfile) {
        return;
      }
      if (!key) {
        setLaunchOptimizationsStatus({
          tone: 'warning',
          label: 'Preset name required',
          detail: 'Enter a name for the new preset.',
        });
        throw new Error('Preset name must not be empty');
      }
      if (key.startsWith('bundled/')) {
        setLaunchOptimizationsStatus({
          tone: 'warning',
          label: 'Reserved name',
          detail: 'Names starting with bundled/ are reserved for app-shipped GPU presets.',
        });
        throw new Error('Preset name must not start with bundled/');
      }
      const method = resolveLaunchMethod(profileRef.current);
      if (method !== 'proton_run' && method !== 'steam_applaunch') {
        return;
      }
      try {
        await flushPendingLaunchOptimizationsSave(trimmedName);
      } catch (err) {
        setDirty(true);
        const message = formatInvokeError(err);
        setError(message);
        setLaunchOptimizationsStatus({
          tone: 'error',
          label: 'Failed to save',
          detail: message,
        });
        throw err;
      }
      launchOptimizationsAutosaveTimerRef.current = null;
      const ids = normalizeLaunchOptimizationIds(
        profileRef.current.launch.optimizations.enabled_option_ids,
        optionsById,
        catalogLoaded
      );
      setOptimizationPresetActionBusy(true);
      setError(null);
      try {
        const updated = await enqueueLaunchProfileWrite(() =>
          callCommand<SerializedGameProfile>('profile_save_manual_optimization_preset', {
            name: trimmedName,
            presetName: key,
            enabledOptionIds: ids,
          })
        );
        if (selectedProfileRef.current.trim() !== trimmedName) {
          return;
        }
        const normalized = normalizeProfileForEdit(updated, optionsById, catalogLoaded);
        setProfile(normalized);
        lastSavedLaunchOptimizationIdsRef.current = normalized.launch.optimizations.enabled_option_ids;
        setLaunchOptimizationsStatus({
          tone: 'success',
          label: 'Preset saved',
          detail: `Saved as "${key}" and set as the active preset.`,
        });
      } catch (err) {
        const message = formatInvokeError(err);
        setError(message);
        setLaunchOptimizationsStatus({
          tone: 'error',
          label: 'Failed to save preset',
          detail: message,
        });
        throw err;
      } finally {
        setOptimizationPresetActionBusy(false);
      }
    },
    [
      enqueueLaunchProfileWrite,
      flushPendingLaunchOptimizationsSave,
      hasExistingSavedProfile,
      catalogLoaded,
      optionsById,
      profileName,
      setDirty,
      setError,
      setProfile,
    ]
  );
  useBundledOptimizationPresetsEffect(setBundledOptimizationPresets);
  useLaunchOptimizationsAutosaveEffect({
    enqueueLaunchProfileWrite,
    hasExistingSavedProfile,
    profile,
    profileName,
    setDirty,
    setLaunchOptimizationsStatus,
    launchOptimizationsAutosaveTimerRef,
    lastSavedLaunchOptimizationIdsRef,
  });
  useGamescopeAutosaveEffect({
    enqueueLaunchProfileWrite,
    hasExistingSavedProfile,
    profile,
    profileName,
    setDirty,
    gamescopeAutosaveTimerRef,
    lastSavedGamescopeJsonRef,
    setGamescopeAutoSaveStatus,
  });
  useTrainerGamescopeAutosaveEffect({
    enqueueLaunchProfileWrite,
    hasExistingSavedProfile,
    profile,
    profileName,
    setDirty,
    trainerGamescopeAutosaveTimerRef,
    lastSavedTrainerGamescopeJsonRef,
    setTrainerGamescopeAutoSaveStatus,
  });
  useMangoHudAutosaveEffect({
    enqueueLaunchProfileWrite,
    hasExistingSavedProfile,
    profile,
    profileName,
    setDirty,
    mangoHudAutosaveTimerRef,
    lastSavedMangoHudJsonRef,
    setMangoHudAutoSaveStatus,
  });
  return {
    launchOptimizationsStatus,
    gamescopeAutoSaveStatus,
    trainerGamescopeAutoSaveStatus,
    mangoHudAutoSaveStatus,
    bundledOptimizationPresets,
    optimizationPresetActionBusy,
    toggleLaunchOptimization,
    switchLaunchOptimizationPreset,
    applyBundledOptimizationPreset,
    saveManualOptimizationPreset,
    clearAutosaveTimers,
    setLastSavedProfileSnapshot,
  };
}
