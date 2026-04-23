import { useCallback, useEffect, useRef } from 'react';
import type { GameProfile } from '../../types/profile';
import type { PersistProfileDraft } from './useProfileCrud';

interface UseLaunchEnvironmentAutosaveOptions {
  hasSavedSelectedProfile: boolean;
  profile: GameProfile;
  profileName: string;
  persistProfileDraft: PersistProfileDraft;
}

export interface LaunchEnvironmentAutosave {
  handleEnvironmentBlurAutoSave: (
    trigger: 'key' | 'value',
    row: Readonly<{ key: string; value: string }>,
    nextEnvVars: Readonly<Record<string, string>>
  ) => void;
}

export function useLaunchEnvironmentAutosave({
  hasSavedSelectedProfile,
  profile,
  profileName,
  persistProfileDraft,
}: UseLaunchEnvironmentAutosaveOptions): LaunchEnvironmentAutosave {
  const environmentAutosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const persistProfileDraftRef = useRef(persistProfileDraft);
  const latestProfileRef = useRef(profile);
  const latestProfileNameRef = useRef(profileName);
  const latestNextEnvVarsRef = useRef<Readonly<Record<string, string>>>({});

  // Keep refs in sync with latest values
  useEffect(() => {
    persistProfileDraftRef.current = persistProfileDraft;
    latestProfileRef.current = profile;
    latestProfileNameRef.current = profileName;
  }, [persistProfileDraft, profile, profileName]);

  // Clear timer on unmount
  useEffect(() => {
    return () => {
      if (environmentAutosaveTimerRef.current !== null) {
        clearTimeout(environmentAutosaveTimerRef.current);
        environmentAutosaveTimerRef.current = null;
      }
    };
  }, []);

  const handleEnvironmentBlurAutoSave = useCallback(
    (
      trigger: 'key' | 'value',
      row: Readonly<{ key: string; value: string }>,
      nextEnvVars: Readonly<Record<string, string>>
    ) => {
      if (!hasSavedSelectedProfile) {
        return;
      }
      if (trigger === 'value' && row.key.trim().length === 0) {
        return;
      }
      latestNextEnvVarsRef.current = { ...nextEnvVars };
      if (environmentAutosaveTimerRef.current !== null) {
        clearTimeout(environmentAutosaveTimerRef.current);
      }
      environmentAutosaveTimerRef.current = setTimeout(() => {
        const latestProfile = latestProfileRef.current;
        const latestProfileName = latestProfileNameRef.current;
        void persistProfileDraftRef.current(latestProfileName, {
          ...latestProfile,
          launch: {
            ...latestProfile.launch,
            custom_env_vars: { ...latestNextEnvVarsRef.current },
          },
        });
      }, 400);
    },
    [hasSavedSelectedProfile]
  );

  return { handleEnvironmentBlurAutoSave };
}
