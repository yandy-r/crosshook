import { useCallback, useEffect, useRef, useState } from 'react';
import { launchOptimizationsAutosaveDelayMs } from '@/hooks/profile/constants';
import type { PersistProfileDraft } from '@/hooks/profile/useProfileCrud';
import type { LaunchAutoSaveStatus } from '@/types/launch';
import type { GameProfile } from '@/types/profile';

const idleStatus: LaunchAutoSaveStatus = {
  tone: 'idle',
  label: 'Trainer settings saved',
};

export interface UseHeroTrainerAutosaveOptions {
  hasSavedSelectedProfile: boolean;
  profile: GameProfile;
  profileName: string;
  persistProfileDraft: PersistProfileDraft;
}

export interface HeroTrainerAutosave {
  trainerAutoSaveStatus: LaunchAutoSaveStatus;
  scheduleTrainerAutosave: (nextProfile: GameProfile) => void;
}

function trainerInjectionSignature(profile: GameProfile): string {
  return JSON.stringify({
    injection: {
      loaded_hooks: profile.injection.loaded_hooks,
      dll_paths: profile.injection.dll_paths,
      inject_on_launch: profile.injection.inject_on_launch,
      method: profile.injection.method,
      stage: profile.injection.stage,
      timeout_ms: profile.injection.timeout_ms,
      fallback: profile.injection.fallback,
    },
  });
}

export function useHeroTrainerAutosave({
  hasSavedSelectedProfile,
  profile,
  profileName,
  persistProfileDraft,
}: UseHeroTrainerAutosaveOptions): HeroTrainerAutosave {
  const [trainerAutoSaveStatus, setTrainerAutoSaveStatus] = useState<LaunchAutoSaveStatus>(idleStatus);

  const autosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const persistProfileDraftRef = useRef(persistProfileDraft);
  const latestProfileRef = useRef(profile);
  const latestProfileNameRef = useRef(profileName);
  const lastScheduledProfileRef = useRef<GameProfile | null>(null);
  const latestScheduledSignatureRef = useRef<string | null>(null);
  const saveSequenceRef = useRef(0);
  const mountedRef = useRef(true);

  useEffect(() => {
    persistProfileDraftRef.current = persistProfileDraft;
    latestProfileRef.current = profile;
    latestProfileNameRef.current = profileName;
  }, [persistProfileDraft, profile, profileName]);

  useEffect(() => {
    return () => {
      mountedRef.current = false;
      saveSequenceRef.current += 1;
      if (autosaveTimerRef.current !== null) {
        clearTimeout(autosaveTimerRef.current);
        autosaveTimerRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    if (!hasSavedSelectedProfile) {
      if (autosaveTimerRef.current !== null) {
        clearTimeout(autosaveTimerRef.current);
        autosaveTimerRef.current = null;
      }
      lastScheduledProfileRef.current = null;
      latestScheduledSignatureRef.current = null;
      saveSequenceRef.current += 1;
      setTrainerAutoSaveStatus(idleStatus);
    }
  }, [hasSavedSelectedProfile]);

  const scheduleTrainerAutosave = useCallback(
    (nextProfile: GameProfile) => {
      if (!hasSavedSelectedProfile) {
        return;
      }

      lastScheduledProfileRef.current = nextProfile;
      const scheduledProfileName = latestProfileNameRef.current;
      const scheduledSignature = trainerInjectionSignature(nextProfile);
      latestScheduledSignatureRef.current = scheduledSignature;

      if (autosaveTimerRef.current !== null) {
        clearTimeout(autosaveTimerRef.current);
      }

      autosaveTimerRef.current = setTimeout(() => {
        autosaveTimerRef.current = null;

        if (
          latestProfileNameRef.current !== scheduledProfileName ||
          latestScheduledSignatureRef.current !== scheduledSignature
        ) {
          return;
        }

        const profileForSave = lastScheduledProfileRef.current ?? latestProfileRef.current;
        if (trainerInjectionSignature(profileForSave) !== scheduledSignature) {
          return;
        }

        const saveSequence = saveSequenceRef.current + 1;
        saveSequenceRef.current = saveSequence;
        setTrainerAutoSaveStatus({ tone: 'saving', label: 'Saving trainer settings...' });

        void persistProfileDraftRef
          .current(scheduledProfileName, profileForSave)
          .then((result) => {
            if (
              !mountedRef.current ||
              saveSequenceRef.current !== saveSequence ||
              latestProfileNameRef.current !== scheduledProfileName ||
              latestScheduledSignatureRef.current !== scheduledSignature
            ) {
              return;
            }

            setTrainerAutoSaveStatus(
              result.ok
                ? { tone: 'success', label: 'Trainer settings saved' }
                : { tone: 'error', label: 'Trainer settings failed to save', detail: result.error }
            );
          })
          .catch((err: unknown) => {
            if (
              !mountedRef.current ||
              saveSequenceRef.current !== saveSequence ||
              latestProfileNameRef.current !== scheduledProfileName ||
              latestScheduledSignatureRef.current !== scheduledSignature
            ) {
              return;
            }

            setTrainerAutoSaveStatus({
              tone: 'error',
              label: 'Trainer settings failed to save',
              detail: err instanceof Error ? err.message : String(err),
            });
          });
      }, launchOptimizationsAutosaveDelayMs);
    },
    [hasSavedSelectedProfile]
  );

  return { trainerAutoSaveStatus, scheduleTrainerAutosave };
}

export default useHeroTrainerAutosave;
