import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { launchOptimizationsAutosaveDelayMs } from '@/hooks/profile/constants';
import type { PersistProfileDraft } from '@/hooks/profile/useProfileCrud';
import type { LaunchAutoSaveStatus } from '@/types/launch';
import type { GameProfile } from '@/types/profile';

const idleStatus: LaunchAutoSaveStatus = { tone: 'idle', label: 'Saved' };
const savingStatus: LaunchAutoSaveStatus = { tone: 'saving', label: 'Saving profile…' };
const savedStatus: LaunchAutoSaveStatus = { tone: 'success', label: 'Profile saved' };

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
  // True once an autosave for the current profile has succeeded; reset on
  // profile switch. "Profile saved" is only shown while the draft is clean.
  const [savedOnce, setSavedOnce] = useState(false);

  const selectedTrimmed = selectedProfile.trim();
  const profileNameTrimmed = profileName.trim();
  const profileExists = selectedTrimmed.length > 0 && profiles.includes(selectedTrimmed);
  const hasSavedSelectedProfile = profileExists && profileNameTrimmed === selectedTrimmed;

  const latestProfileNameRef = useRef(selectedTrimmed);
  const latestProfileRef = useRef(profile);
  const dirtyRef = useRef(dirty);
  const persistProfileDraftRef = useRef(persistProfileDraft);
  const timerRef = useRef<number | null>(null);
  // At most one autosave runs at a time; a debounce that fires during it is
  // queued and re-armed once it settles, so the newest draft is saved after it.
  const saveInFlightRef = useRef(false);
  const saveQueuedRef = useRef(false);

  latestProfileNameRef.current = selectedTrimmed;
  latestProfileRef.current = profile;
  dirtyRef.current = dirty;
  persistProfileDraftRef.current = persistProfileDraft;

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const scheduleAutosave = useCallback(
    (profileNameToSave: string) => {
      clearTimer();
      timerRef.current = window.setTimeout(() => {
        timerRef.current = null;
        if (latestProfileNameRef.current !== profileNameToSave || !dirtyRef.current) {
          return;
        }
        if (saveInFlightRef.current) {
          saveQueuedRef.current = true;
          return;
        }

        saveInFlightRef.current = true;
        void persistProfileDraftRef
          .current(profileNameToSave, latestProfileRef.current)
          .then((result) => {
            if (result.ok && latestProfileNameRef.current === profileNameToSave) {
              setSavedOnce(true);
            }
          })
          .finally(() => {
            saveInFlightRef.current = false;
            if (saveQueuedRef.current) {
              saveQueuedRef.current = false;
              scheduleAutosave(profileNameToSave);
            }
          });
      }, launchOptimizationsAutosaveDelayMs);
    },
    [clearTimer]
  );

  // biome-ignore lint/correctness/useExhaustiveDependencies: trigger-only dep — reset save state when the selected profile changes
  useEffect(() => {
    setSavedOnce(false);
    saveQueuedRef.current = false;
  }, [selectedTrimmed]);

  // Debounced autosave: every draft change while dirty restarts the timer.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `profile` is a trigger-only dep — each edit re-arms the debounce
  useEffect(() => {
    if (!dirty || !hasSavedSelectedProfile) {
      clearTimer();
      return;
    }
    scheduleAutosave(selectedTrimmed);
  }, [clearTimer, dirty, hasSavedSelectedProfile, profile, scheduleAutosave, selectedTrimmed]);

  useEffect(() => clearTimer, [clearTimer]);

  const autoSaveStatus = useMemo<LaunchAutoSaveStatus>(() => {
    if (saving) {
      return savingStatus;
    }
    if (error) {
      return { tone: 'error', label: 'Profile save failed', detail: error };
    }
    if (dirty) {
      // A save is pending (debounce armed or queued); never claim "saved".
      return hasSavedSelectedProfile ? savingStatus : idleStatus;
    }
    return savedOnce ? savedStatus : idleStatus;
  }, [dirty, error, hasSavedSelectedProfile, savedOnce, saving]);

  // Flush dirty draft before switching profile cards, then select the new card
  const selectCard = useCallback(
    async (cardName: string) => {
      if (cardName === selectedTrimmed) {
        return;
      }

      if (dirty && hasSavedSelectedProfile) {
        clearTimer();
        saveQueuedRef.current = false;
        const result = await persistProfileDraft(selectedTrimmed, latestProfileRef.current);
        if (!result.ok) {
          return;
        }
      }

      await selectProfile(cardName);
    },
    [clearTimer, dirty, hasSavedSelectedProfile, persistProfileDraft, selectProfile, selectedTrimmed]
  );

  return { autoSaveStatus, selectCard };
}
