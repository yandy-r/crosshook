import { type Dispatch, type MutableRefObject, type SetStateAction, useEffect } from 'react';
import { callCommand } from '@/lib/ipc';
import type { BundledOptimizationPreset, GameProfile, LaunchAutoSaveStatus } from '../../types';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG } from '../../types/profile';
import { resolveLaunchMethod } from '../../utils/launch';
import { launchOptimizationsAutosaveDelayMs } from './constants';
import { formatInvokeError } from './formatInvokeError';
import { areLaunchOptimizationIdsEqual } from './launchOptimizationIds';
import { buildLaunchOptimizationsStatus, type LaunchOptimizationsStatus } from './launchOptimizationStatus';

interface LaunchOptimizationAutosaveEffectOptions {
  enqueueLaunchProfileWrite: <T>(fn: () => Promise<T>) => Promise<T>;
  hasExistingSavedProfile: boolean;
  profile: GameProfile;
  profileName: string;
  setDirty: Dispatch<SetStateAction<boolean>>;
  setLaunchOptimizationsStatus: Dispatch<SetStateAction<LaunchOptimizationsStatus>>;
  launchOptimizationsAutosaveTimerRef: MutableRefObject<ReturnType<typeof setTimeout> | null>;
  lastSavedLaunchOptimizationIdsRef: MutableRefObject<string[]>;
}

interface LaunchConfigAutosaveEffectOptions {
  enqueueLaunchProfileWrite: <T>(fn: () => Promise<T>) => Promise<T>;
  hasExistingSavedProfile: boolean;
  profile: GameProfile;
  profileName: string;
  setDirty: Dispatch<SetStateAction<boolean>>;
}

interface GamescopeEffectOptions extends LaunchConfigAutosaveEffectOptions {
  gamescopeAutosaveTimerRef: MutableRefObject<ReturnType<typeof setTimeout> | null>;
  lastSavedGamescopeJsonRef: MutableRefObject<string>;
  setGamescopeAutoSaveStatus: Dispatch<SetStateAction<LaunchAutoSaveStatus>>;
}

interface TrainerGamescopeEffectOptions extends LaunchConfigAutosaveEffectOptions {
  trainerGamescopeAutosaveTimerRef: MutableRefObject<ReturnType<typeof setTimeout> | null>;
  lastSavedTrainerGamescopeJsonRef: MutableRefObject<string>;
  setTrainerGamescopeAutoSaveStatus: Dispatch<SetStateAction<LaunchAutoSaveStatus>>;
}

interface MangoHudEffectOptions extends LaunchConfigAutosaveEffectOptions {
  mangoHudAutosaveTimerRef: MutableRefObject<ReturnType<typeof setTimeout> | null>;
  lastSavedMangoHudJsonRef: MutableRefObject<string>;
  setMangoHudAutoSaveStatus: Dispatch<SetStateAction<LaunchAutoSaveStatus>>;
}

export function useBundledOptimizationPresetsEffect(
  setBundledOptimizationPresets: Dispatch<SetStateAction<BundledOptimizationPreset[]>>
) {
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const rows = await callCommand<BundledOptimizationPreset[]>('profile_list_bundled_optimization_presets');
        if (!cancelled) {
          setBundledOptimizationPresets(rows);
        }
      } catch {
        if (!cancelled) {
          setBundledOptimizationPresets([]);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [setBundledOptimizationPresets]);
}

export function useLaunchOptimizationsAutosaveEffect({
  enqueueLaunchProfileWrite,
  hasExistingSavedProfile,
  profile,
  profileName,
  setDirty,
  setLaunchOptimizationsStatus,
  launchOptimizationsAutosaveTimerRef,
  lastSavedLaunchOptimizationIdsRef,
}: LaunchOptimizationAutosaveEffectOptions) {
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
          await enqueueLaunchProfileWrite(async () => {
            await callCommand('profile_save_launch_optimizations', {
              name: trimmedName,
              optimizations: {
                enabled_option_ids: normalizedIds,
              },
            });
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
            detail: formatInvokeError(err),
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
  }, [
    enqueueLaunchProfileWrite,
    hasExistingSavedProfile,
    profile,
    profileName,
    setDirty,
    setLaunchOptimizationsStatus,
    launchOptimizationsAutosaveTimerRef,
    lastSavedLaunchOptimizationIdsRef,
  ]);
}

export function useGamescopeAutosaveEffect({
  enqueueLaunchProfileWrite,
  hasExistingSavedProfile,
  profile,
  profileName,
  setDirty,
  gamescopeAutosaveTimerRef,
  lastSavedGamescopeJsonRef,
  setGamescopeAutoSaveStatus,
}: GamescopeEffectOptions) {
  useEffect(() => {
    const currentJson = JSON.stringify(profile.launch.gamescope ?? null);

    if (gamescopeAutosaveTimerRef.current !== null) {
      clearTimeout(gamescopeAutosaveTimerRef.current);
      gamescopeAutosaveTimerRef.current = null;
    }

    if (!hasExistingSavedProfile) {
      if (currentJson !== lastSavedGamescopeJsonRef.current) {
        setGamescopeAutoSaveStatus({ tone: 'warning', label: 'Save profile first' });
      }
      return;
    }

    if (currentJson === lastSavedGamescopeJsonRef.current) {
      setGamescopeAutoSaveStatus({ tone: 'idle', label: 'Ready' });
      return;
    }

    setGamescopeAutoSaveStatus({ tone: 'saving', label: 'Saving...' });
    const trimmedName = profileName.trim();
    let cancelled = false;
    gamescopeAutosaveTimerRef.current = setTimeout(() => {
      void (async () => {
        try {
          await enqueueLaunchProfileWrite(async () => {
            await callCommand('profile_save_gamescope_config', {
              name: trimmedName,
              config: profile.launch.gamescope ?? DEFAULT_GAMESCOPE_CONFIG,
            });
          });
          if (cancelled) return;
          lastSavedGamescopeJsonRef.current = currentJson;
          setGamescopeAutoSaveStatus({ tone: 'success', label: 'Saved automatically' });
        } catch (err) {
          if (cancelled) return;
          setDirty(true);
          setGamescopeAutoSaveStatus({ tone: 'error', label: 'Failed to save', detail: formatInvokeError(err) });
        }
      })();
    }, launchOptimizationsAutosaveDelayMs);

    return () => {
      cancelled = true;
      if (gamescopeAutosaveTimerRef.current !== null) {
        clearTimeout(gamescopeAutosaveTimerRef.current);
        gamescopeAutosaveTimerRef.current = null;
      }
    };
  }, [
    enqueueLaunchProfileWrite,
    hasExistingSavedProfile,
    profile,
    profileName,
    setDirty,
    gamescopeAutosaveTimerRef,
    lastSavedGamescopeJsonRef,
    setGamescopeAutoSaveStatus,
  ]);
}

export function useTrainerGamescopeAutosaveEffect({
  enqueueLaunchProfileWrite,
  hasExistingSavedProfile,
  profile,
  profileName,
  setDirty,
  trainerGamescopeAutosaveTimerRef,
  lastSavedTrainerGamescopeJsonRef,
  setTrainerGamescopeAutoSaveStatus,
}: TrainerGamescopeEffectOptions) {
  useEffect(() => {
    const currentJson = JSON.stringify(profile.launch.trainer_gamescope ?? null);

    if (trainerGamescopeAutosaveTimerRef.current !== null) {
      clearTimeout(trainerGamescopeAutosaveTimerRef.current);
      trainerGamescopeAutosaveTimerRef.current = null;
    }

    if (!hasExistingSavedProfile) {
      if (currentJson !== lastSavedTrainerGamescopeJsonRef.current) {
        setTrainerGamescopeAutoSaveStatus({ tone: 'warning', label: 'Save profile first' });
      }
      return;
    }

    if (currentJson === lastSavedTrainerGamescopeJsonRef.current) {
      setTrainerGamescopeAutoSaveStatus({ tone: 'idle', label: 'Ready' });
      return;
    }

    setTrainerGamescopeAutoSaveStatus({ tone: 'saving', label: 'Saving...' });
    const trimmedName = profileName.trim();
    let cancelled = false;
    trainerGamescopeAutosaveTimerRef.current = setTimeout(() => {
      void (async () => {
        try {
          await enqueueLaunchProfileWrite(async () => {
            await callCommand('profile_save_trainer_gamescope_config', {
              name: trimmedName,
              config: profile.launch.trainer_gamescope ?? DEFAULT_GAMESCOPE_CONFIG,
            });
          });
          if (cancelled) return;
          lastSavedTrainerGamescopeJsonRef.current = currentJson;
          setTrainerGamescopeAutoSaveStatus({ tone: 'success', label: 'Saved automatically' });
        } catch (err) {
          if (cancelled) return;
          setDirty(true);
          setTrainerGamescopeAutoSaveStatus({ tone: 'error', label: 'Failed to save', detail: formatInvokeError(err) });
        }
      })();
    }, launchOptimizationsAutosaveDelayMs);

    return () => {
      cancelled = true;
      if (trainerGamescopeAutosaveTimerRef.current !== null) {
        clearTimeout(trainerGamescopeAutosaveTimerRef.current);
        trainerGamescopeAutosaveTimerRef.current = null;
      }
    };
  }, [
    enqueueLaunchProfileWrite,
    hasExistingSavedProfile,
    profile,
    profileName,
    setDirty,
    trainerGamescopeAutosaveTimerRef,
    lastSavedTrainerGamescopeJsonRef,
    setTrainerGamescopeAutoSaveStatus,
  ]);
}

export function useMangoHudAutosaveEffect({
  enqueueLaunchProfileWrite,
  hasExistingSavedProfile,
  profile,
  profileName,
  setDirty,
  mangoHudAutosaveTimerRef,
  lastSavedMangoHudJsonRef,
  setMangoHudAutoSaveStatus,
}: MangoHudEffectOptions) {
  useEffect(() => {
    const currentJson = JSON.stringify(profile.launch.mangohud ?? null);

    if (mangoHudAutosaveTimerRef.current !== null) {
      clearTimeout(mangoHudAutosaveTimerRef.current);
      mangoHudAutosaveTimerRef.current = null;
    }

    if (!hasExistingSavedProfile) {
      if (currentJson !== lastSavedMangoHudJsonRef.current) {
        setMangoHudAutoSaveStatus({ tone: 'warning', label: 'Save profile first' });
      }
      return;
    }

    if (currentJson === lastSavedMangoHudJsonRef.current) {
      setMangoHudAutoSaveStatus({ tone: 'idle', label: 'Ready' });
      return;
    }

    setMangoHudAutoSaveStatus({ tone: 'saving', label: 'Saving...' });
    const trimmedName = profileName.trim();
    let cancelled = false;
    mangoHudAutosaveTimerRef.current = setTimeout(() => {
      void (async () => {
        try {
          await enqueueLaunchProfileWrite(async () => {
            await callCommand('profile_save_mangohud_config', {
              name: trimmedName,
              config: profile.launch.mangohud ?? DEFAULT_MANGOHUD_CONFIG,
            });
          });
          if (cancelled) return;
          lastSavedMangoHudJsonRef.current = currentJson;
          setMangoHudAutoSaveStatus({ tone: 'success', label: 'Saved automatically' });
        } catch (err) {
          if (cancelled) return;
          setDirty(true);
          setMangoHudAutoSaveStatus({ tone: 'error', label: 'Failed to save', detail: formatInvokeError(err) });
        }
      })();
    }, launchOptimizationsAutosaveDelayMs);

    return () => {
      cancelled = true;
      if (mangoHudAutosaveTimerRef.current !== null) {
        clearTimeout(mangoHudAutosaveTimerRef.current);
        mangoHudAutosaveTimerRef.current = null;
      }
    };
  }, [
    enqueueLaunchProfileWrite,
    hasExistingSavedProfile,
    profile,
    profileName,
    setDirty,
    mangoHudAutosaveTimerRef,
    lastSavedMangoHudJsonRef,
    setMangoHudAutoSaveStatus,
  ]);
}
