import { useCallback, useEffect, useRef } from 'react';
import { launchOptimizationsAutosaveDelayMs } from '@/hooks/profile/constants';
import type { PersistProfileDraft } from '@/hooks/profile/useProfileCrud';
import type { GameProfile } from '@/types/profile';

export interface UseHeroLaunchHooksAutosaveOptions {
  hasSavedSelectedProfile: boolean;
  profile: GameProfile;
  profileName: string;
  persistProfileDraft: PersistProfileDraft;
}

export interface HeroLaunchHooksAutosave {
  scheduleHookAutosave: (nextProfile: GameProfile) => void;
}

function hooksSignature(profile: GameProfile): string {
  return JSON.stringify({
    pre_launch_hooks: profile.pre_launch_hooks ?? [],
    post_exit_hooks: profile.post_exit_hooks ?? [],
  });
}

export function useHeroLaunchHooksAutosave({
  hasSavedSelectedProfile,
  profile,
  profileName,
  persistProfileDraft,
}: UseHeroLaunchHooksAutosaveOptions): HeroLaunchHooksAutosave {
  const autosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const persistProfileDraftRef = useRef(persistProfileDraft);
  const latestProfileRef = useRef(profile);
  const latestProfileNameRef = useRef(profileName);
  const lastScheduledProfileRef = useRef<GameProfile | null>(null);

  useEffect(() => {
    persistProfileDraftRef.current = persistProfileDraft;
    latestProfileRef.current = profile;
    latestProfileNameRef.current = profileName;
  }, [persistProfileDraft, profile, profileName]);

  useEffect(() => {
    return () => {
      if (autosaveTimerRef.current !== null) {
        clearTimeout(autosaveTimerRef.current);
        autosaveTimerRef.current = null;
      }
    };
  }, []);

  const scheduleHookAutosave = useCallback(
    (nextProfile: GameProfile) => {
      if (!hasSavedSelectedProfile) {
        return;
      }

      lastScheduledProfileRef.current = nextProfile;
      const scheduledProfileName = latestProfileNameRef.current;
      const scheduledSignature = hooksSignature(nextProfile);

      if (autosaveTimerRef.current !== null) {
        clearTimeout(autosaveTimerRef.current);
      }

      autosaveTimerRef.current = setTimeout(() => {
        if (latestProfileNameRef.current !== scheduledProfileName) {
          return;
        }

        const profileForSave = lastScheduledProfileRef.current ?? latestProfileRef.current;
        if (hooksSignature(profileForSave) !== scheduledSignature) {
          return;
        }

        void persistProfileDraftRef.current(scheduledProfileName, profileForSave);
      }, launchOptimizationsAutosaveDelayMs);
    },
    [hasSavedSelectedProfile]
  );

  return { scheduleHookAutosave };
}

export default useHeroLaunchHooksAutosave;
