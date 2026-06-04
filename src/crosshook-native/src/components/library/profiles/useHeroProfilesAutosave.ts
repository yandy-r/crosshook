import { useCallback, useEffect, useRef, useState } from 'react';
import { launchOptimizationsAutosaveDelayMs } from '@/hooks/profile/constants';
import type { PersistProfileDraft } from '@/hooks/profile/useProfileCrud';
import type { LaunchAutoSaveStatus } from '@/types/launch';
import type { GameProfile } from '@/types/profile';

const idleStatus: LaunchAutoSaveStatus = {
  tone: 'idle',
  label: 'Saved',
};

export interface UseHeroProfilesAutosaveOptions {
  profile: GameProfile;
  profileName: string;
  selectedProfile: string;
  profiles: string[];
  dirty: boolean;
  saving: boolean;
  error: string | null;
  persistProfileDraft: PersistProfileDraft;
  selectProfile: (name: string) => Promise<void>;
}

export interface UseHeroProfilesAutosaveResult {
  autoSaveStatus: LaunchAutoSaveStatus;
  selectCard: (cardName: string) => Promise<void>;
}

/**
 * Manages the 350ms draft autosave and flush-before-switch logic for the
 * Hero Detail profiles tab. This is the single full-draft autosave owner for
 * Hero Detail — do not add a second autosave writer for this context.
 */
export function useHeroProfilesAutosave({
  profile,
  profileName,
  selectedProfile,
  profiles,
  dirty,
  saving,
  error,
  persistProfileDraft,
  selectProfile,
}: UseHeroProfilesAutosaveOptions): UseHeroProfilesAutosaveResult {
  const [autoSaveStatus, setAutoSaveStatus] = useState<LaunchAutoSaveStatus>(idleStatus);

  const selectedTrimmed = selectedProfile.trim();
  const profileNameTrimmed = profileName.trim();
  const profileExists = selectedTrimmed.length > 0 && profiles.includes(selectedTrimmed);
  const hasSavedSelectedProfile = profileExists && profileNameTrimmed === selectedTrimmed;

  const latestProfileNameRef = useRef(selectedTrimmed);
  const latestProfileRef = useRef(profile);

  useEffect(() => {
    latestProfileNameRef.current = selectedTrimmed;
  }, [selectedTrimmed]);

  useEffect(() => {
    latestProfileRef.current = profile;
  }, [profile]);

  // Reflect save state in the autosave chip
  useEffect(() => {
    if (saving) {
      setAutoSaveStatus({ tone: 'saving', label: 'Saving profile…' });
      return;
    }

    if (error) {
      setAutoSaveStatus({ tone: 'error', label: 'Profile save failed', detail: error });
      return;
    }

    if (!dirty) {
      setAutoSaveStatus(idleStatus);
    }
  }, [dirty, error, saving]);

  // Debounced autosave: fires after launchOptimizationsAutosaveDelayMs when dirty
  useEffect(() => {
    if (!dirty || !hasSavedSelectedProfile) {
      return;
    }

    const scheduledProfileName = selectedTrimmed;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (cancelled || latestProfileNameRef.current !== scheduledProfileName) {
        return;
      }

      setAutoSaveStatus({ tone: 'saving', label: 'Saving profile…' });
      void persistProfileDraft(scheduledProfileName, latestProfileRef.current).then((result) => {
        if (cancelled || latestProfileNameRef.current !== scheduledProfileName) {
          return;
        }

        setAutoSaveStatus(
          result.ok
            ? { tone: 'success', label: 'Profile saved' }
            : { tone: 'error', label: 'Profile save failed', detail: result.error }
        );
      });
    }, launchOptimizationsAutosaveDelayMs);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [dirty, hasSavedSelectedProfile, persistProfileDraft, selectedTrimmed]);

  // Flush dirty draft before switching profile cards, then select the new card
  const selectCard = useCallback(
    async (cardName: string) => {
      if (cardName === selectedTrimmed) {
        return;
      }

      if (dirty && hasSavedSelectedProfile) {
        const result = await persistProfileDraft(selectedTrimmed, profile);
        if (!result.ok) {
          setAutoSaveStatus({ tone: 'error', label: 'Profile save failed', detail: result.error });
          return;
        }
      }

      await selectProfile(cardName);
    },
    [dirty, hasSavedSelectedProfile, persistProfileDraft, profile, selectProfile, selectedTrimmed]
  );

  return { autoSaveStatus, selectCard };
}
