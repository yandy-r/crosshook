import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type { GameProfile } from '../types/profile';
import type { UpdateGameRequest, UpdateGameResult, UpdateGameStage, UpdateGameValidationState } from '../types';
import { UPDATE_GAME_VALIDATION_MESSAGES, UPDATE_GAME_VALIDATION_FIELD } from '../types';
import type { UpdateGameValidationError } from '../types/update';

/** Steam / custom art used to show cover hero after a profile is loaded for update. */
export interface UpdateGameProfileCoverSource {
  steamAppId: string | undefined;
  customCoverArtPath: string;
}

export interface UseUpdateGameResult {
  request: UpdateGameRequest;
  validation: UpdateGameValidationState;
  stage: UpdateGameStage;
  result: UpdateGameResult | null;
  error: string | null;
  profiles: string[];
  profilesError: string | null;
  isLoadingProfiles: boolean;
  selectedProfile: string;
  /** Set when `populateFromProfile` succeeds; drives cover art on the Update panel. */
  profileCoverSource: UpdateGameProfileCoverSource | null;
  updateField: <Key extends keyof UpdateGameRequest>(key: Key, value: string) => void;
  statusText: string;
  hintText: string;
  actionLabel: string;
  canStart: boolean;
  isRunning: boolean;
  loadProfiles: () => Promise<void>;
  populateFromProfile: (name: string) => Promise<void>;
  startUpdate: () => Promise<void>;
  cancelUpdate: () => Promise<void>;
  reset: () => void;
}

function createEmptyRequest(): UpdateGameRequest {
  return {
    profile_name: '',
    updater_path: '',
    proton_path: '',
    prefix_path: '',
    steam_client_install_path: '',
  };
}

function createEmptyValidationState(): UpdateGameValidationState {
  return {
    fieldErrors: {},
    generalError: null,
  };
}

function normalizeErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function mapValidationErrorToField(message: string): keyof UpdateGameRequest | null {
  const variants = Object.keys(UPDATE_GAME_VALIDATION_MESSAGES) as UpdateGameValidationError[];
  for (const variant of variants) {
    if (message === UPDATE_GAME_VALIDATION_MESSAGES[variant]) {
      return UPDATE_GAME_VALIDATION_FIELD[variant];
    }
  }

  const normalized = message.toLowerCase();

  if (
    normalized.includes('updater') ||
    normalized.includes('update executable') ||
    normalized.includes('windows .exe')
  ) {
    return 'updater_path';
  }

  if (normalized.includes('proton path')) {
    return 'proton_path';
  }

  if (normalized.includes('prefix path')) {
    return 'prefix_path';
  }

  return null;
}

export function useUpdateGame(): UseUpdateGameResult {
  const [request, setRequest] = useState<UpdateGameRequest>(createEmptyRequest);
  const [validation, setValidation] = useState<UpdateGameValidationState>(createEmptyValidationState);
  const [stage, setStage] = useState<UpdateGameStage>('idle');
  const [result, setResult] = useState<UpdateGameResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [profiles, setProfiles] = useState<string[]>([]);
  const [profilesError, setProfilesError] = useState<string | null>(null);
  const [isLoadingProfiles, setIsLoadingProfiles] = useState(false);
  const [selectedProfile, setSelectedProfile] = useState('');
  const [profileCoverSource, setProfileCoverSource] = useState<UpdateGameProfileCoverSource | null>(null);
  const unlistenRef = useRef<(() => void) | null>(null);

  function cleanupListener() {
    unlistenRef.current?.();
    unlistenRef.current = null;
  }

  const loadProfiles = useCallback(async () => {
    setIsLoadingProfiles(true);
    setProfilesError(null);

    try {
      const allNames = await invoke<string[]>('profile_list');
      const protonRunNames: string[] = [];
      let skippedCount = 0;

      for (const name of allNames) {
        try {
          const profile = await invoke<GameProfile>('profile_load', { name });
          if (profile.launch.method === 'proton_run') {
            protonRunNames.push(name);
          }
        } catch {
          // Track that this profile failed to load but don't block the rest
          skippedCount++;
        }
      }

      setProfiles(protonRunNames);

      if (skippedCount > 0) {
        setProfilesError(`${skippedCount} profile(s) could not be loaded.`);
      } else {
        setProfilesError(null);
      }
    } catch (loadError) {
      setProfiles([]);
      setProfilesError(`Failed to load profiles: ${normalizeErrorMessage(loadError)}`);
    } finally {
      setIsLoadingProfiles(false);
    }
  }, []);

  const populateFromProfile = useCallback(async (name: string) => {
    try {
      const profile = await invoke<GameProfile>('profile_load', { name });

      setRequest((current) => ({
        ...current,
        profile_name: name,
        proton_path: profile.runtime.proton_path,
        prefix_path: profile.runtime.prefix_path,
      }));
      setSelectedProfile(name);
      const rawAppId = profile.steam.app_id?.trim() ?? '';
      setProfileCoverSource({
        steamAppId: rawAppId.length > 0 ? rawAppId : undefined,
        customCoverArtPath: profile.game.custom_cover_art_path?.trim() ?? '',
      });
      setValidation(createEmptyValidationState());
    } catch (invokeError) {
      const message = normalizeErrorMessage(invokeError);
      setValidation({
        fieldErrors: {},
        generalError: `Failed to load profile "${name}": ${message}`,
      });
    }
  }, []);

  const updateField = useCallback(<Key extends keyof UpdateGameRequest>(key: Key, value: string) => {
    setRequest((current) => ({
      ...current,
      [key]: value,
    }));

    setValidation((current) => {
      if (!current.fieldErrors[key]) {
        return current;
      }

      const fieldErrors = { ...current.fieldErrors };
      delete fieldErrors[key];
      return {
        ...current,
        fieldErrors,
      };
    });
  }, []);

  const startUpdate = useCallback(async () => {
    cleanupListener();
    setValidation(createEmptyValidationState());
    setError(null);
    setResult(null);
    setStage('preparing');

    try {
      await invoke<void>('validate_update_request', { request });
    } catch (invokeError) {
      const message = normalizeErrorMessage(invokeError);
      const validationField = mapValidationErrorToField(message);

      setStage('idle');

      if (validationField === null) {
        setValidation({
          fieldErrors: {},
          generalError: message,
        });
      } else {
        setValidation((current) => ({
          ...current,
          fieldErrors: {
            ...current.fieldErrors,
            [validationField]: message,
          },
        }));
      }

      return;
    }

    // Track whether the completion event arrived before invoke resolved,
    // so we don't overwrite a terminal stage with 'running_updater'.
    let completedBeforeInvoke = false;

    try {
      // Subscribe to the completion event BEFORE invoking the command
      // to avoid a race where the process exits before the listener exists.
      const unlisten = await listen<number | null>('update-complete', (event) => {
        completedBeforeInvoke = true;
        const exitCode = event.payload;

        if (exitCode === 0) {
          setStage('complete');
        } else if (exitCode === null) {
          setStage('failed');
          setError('Update process was terminated by a signal.');
        } else {
          setStage('failed');
          setError(`Update process exited with code ${exitCode}.`);
        }

        unlistenRef.current = null;
        unlisten();
      });
      unlistenRef.current = unlisten;

      const updateResult = await invoke<UpdateGameResult>('update_game', { request });
      setResult(updateResult);

      // Only transition to 'running_updater' if the process hasn't already exited.
      if (!completedBeforeInvoke) {
        setStage('running_updater');
      }
    } catch (invokeError) {
      const message = normalizeErrorMessage(invokeError);
      setStage('failed');
      setError(message);
      cleanupListener();
    }
  }, [request]);

  const cancelUpdate = useCallback(async () => {
    try {
      await invoke<void>('cancel_update');
    } catch {
      // Best-effort cancellation
    }
  }, []);

  const reset = useCallback(() => {
    if (stage === 'running_updater') {
      void cancelUpdate();
    }
    cleanupListener();
    setRequest(createEmptyRequest());
    setValidation(createEmptyValidationState());
    setStage('idle');
    setResult(null);
    setError(null);
    setProfilesError(null);
    setSelectedProfile('');
    setProfileCoverSource(null);
  }, [stage, cancelUpdate]);

  useEffect(() => {
    void loadProfiles();
  }, [loadProfiles]);

  useEffect(() => {
    return () => cleanupListener();
  }, []);

  const statusText = (() => {
    switch (stage) {
      case 'preparing':
        return 'Validating...';
      case 'running_updater':
        return 'Running update...';
      case 'complete':
        return result?.message || 'Update complete.';
      case 'failed':
        return error || 'Update failed.';
      case 'idle':
      default:
        return '';
    }
  })();

  const hintText = (() => {
    switch (stage) {
      case 'running_updater':
        return 'Check the console for live output.';
      case 'complete':
        return 'The update was applied successfully.';
      case 'failed':
        return 'Check the console log for details.';
      case 'idle':
      default:
        return 'Select a profile and update executable.';
    }
  })();

  const actionLabel = (() => {
    switch (stage) {
      case 'preparing':
        return 'Validating...';
      case 'running_updater':
        return 'Running...';
      default:
        return 'Apply Update';
    }
  })();

  const canStart =
    (stage === 'idle' || stage === 'complete' || stage === 'failed') &&
    request.updater_path.trim().length > 0 &&
    selectedProfile.length > 0;
  const isRunning = stage === 'preparing' || stage === 'running_updater';

  return {
    request,
    validation,
    stage,
    result,
    error,
    profiles,
    profilesError,
    isLoadingProfiles,
    selectedProfile,
    profileCoverSource,
    updateField,
    statusText,
    hintText,
    actionLabel,
    canStart,
    isRunning,
    loadProfiles,
    populateFromProfile,
    startUpdate,
    cancelUpdate,
    reset,
  };
}

export default useUpdateGame;
