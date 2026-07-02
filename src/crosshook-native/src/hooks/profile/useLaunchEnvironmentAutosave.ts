import { useCallback, useEffect, useRef } from 'react';
import type { GameProfile } from '../../types/profile';
import { envVarSignature } from '../../utils/envVarSignature';
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
  // Signature of the env vars this hook considers already persisted for the
  // current profile: either the last signature this hook successfully
  // persisted, or — immediately after a profile switch, before any persist
  // has happened yet — the signature of the freshly loaded profile's saved
  // env vars. Dedupe must compare against this baseline, never live editor
  // state, which already mirrors the pending edit before blur fires and
  // would suppress every persist.
  const lastPersistedEnvSignatureRef = useRef<string | null>(null);
  // Monotonic counter identifying the most recently scheduled persist. Used
  // to discard stale resolutions from overlapping persists (or persists
  // belonging to a profile that has since been switched away from) so an
  // out-of-order resolve can't stomp a newer signature back to a stale one.
  const persistRequestSeqRef = useRef(0);

  // Keep refs in sync with latest values
  useEffect(() => {
    persistProfileDraftRef.current = persistProfileDraft;
    latestProfileRef.current = profile;
    latestProfileNameRef.current = profileName;
  }, [persistProfileDraft, profile, profileName]);

  // Seed the dedupe baseline with the newly selected profile's saved env vars
  // so the first blur after a switch only persists when the edit actually
  // changes something — not unconditionally, which would round-trip the
  // whole (possibly dirty) profile draft to disk for a no-op blur. Also
  // invalidate any in-flight persist from the previous profile so its
  // resolution can't overwrite this seed.
  // biome-ignore lint/correctness/useExhaustiveDependencies: trigger-only dep — reseed the persisted signature when the selected profile changes
  useEffect(() => {
    lastPersistedEnvSignatureRef.current = envVarSignature(latestProfileRef.current.launch.custom_env_vars);
    persistRequestSeqRef.current += 1;
  }, [profileName]);

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
      const scheduledProfileName = latestProfileNameRef.current;
      const scheduledEnvVars = { ...latestNextEnvVarsRef.current };
      const scheduledEnvSignature = envVarSignature(scheduledEnvVars);
      if (environmentAutosaveTimerRef.current !== null) {
        clearTimeout(environmentAutosaveTimerRef.current);
      }
      environmentAutosaveTimerRef.current = setTimeout(() => {
        if (latestProfileNameRef.current !== scheduledProfileName) {
          return;
        }
        if (scheduledEnvSignature === lastPersistedEnvSignatureRef.current) {
          return;
        }
        const latestProfile = latestProfileRef.current;
        const requestSeq = ++persistRequestSeqRef.current;
        void persistProfileDraftRef
          .current(scheduledProfileName, {
            ...latestProfile,
            launch: {
              ...latestProfile.launch,
              custom_env_vars: scheduledEnvVars,
            },
          })
          .then(
            (result) => {
              // On failure leave the ref unchanged so the next blur retries.
              // Guard against out-of-order resolutions: only the most
              // recently scheduled persist for the still-current profile may
              // update the dedupe baseline. A stale (earlier) resolution
              // arriving after a newer persist has already started must not
              // stomp the baseline back to an older signature.
              if (
                result.ok &&
                requestSeq === persistRequestSeqRef.current &&
                latestProfileNameRef.current === scheduledProfileName
              ) {
                lastPersistedEnvSignatureRef.current = scheduledEnvSignature;
              }
            },
            () => {
              // Rejection also leaves the ref unchanged so the next blur retries.
            }
          );
      }, 400);
    },
    [hasSavedSelectedProfile]
  );

  return { handleEnvironmentBlurAutoSave };
}
