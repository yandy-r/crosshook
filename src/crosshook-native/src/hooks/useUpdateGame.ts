import { useCallback, useEffect, useRef, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import { callCommand } from '@/lib/ipc';
import type { UpdateGameRequest, UpdateGameResult, UpdateGameStage, UpdateGameValidationState } from '../types';
import { UPDATE_GAME_VALIDATION_FIELD, UPDATE_GAME_VALIDATION_MESSAGES } from '../types';
import type { SerializedGameProfile } from '../types/profile';
import { normalizeSerializedGameProfile } from '../types/profile';
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
  // Completion listeners of started runs. `update-complete` carries no run id,
  // so each listener handles only the first completion after its start and
  // then removes itself; once no run is active nothing listens. Any left are
  // removed on unmount.
  const listenersRef = useRef(new Set<() => void>());
  // Bumped by every start and by reset; a start whose generation is stale
  // detaches from its run (cancelling it) instead of driving this hook's state.
  const generationRef = useRef(0);

  const cleanupListeners = useCallback(() => {
    for (const unlisten of listenersRef.current) unlisten();
    listenersRef.current.clear();
  }, []);

  const loadProfiles = useCallback(async () => {
    setIsLoadingProfiles(true);
    setProfilesError(null);

    try {
      const allNames = await callCommand<string[]>('profile_list');
      const protonRunNames: string[] = [];
      let skippedCount = 0;

      for (const name of allNames) {
        try {
          const profile = normalizeSerializedGameProfile(
            await callCommand<SerializedGameProfile>('profile_load', { name })
          );
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
      const profile = normalizeSerializedGameProfile(
        await callCommand<SerializedGameProfile>('profile_load', { name })
      );

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
    generationRef.current += 1;
    const generation = generationRef.current;
    const isCurrent = () => generationRef.current === generation;
    setValidation(createEmptyValidationState());
    setError(null);
    setResult(null);
    setStage('preparing');

    try {
      await callCommand<void>('validate_update_request', { request });
    } catch (invokeError) {
      if (!isCurrent()) return;
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
    if (!isCurrent()) return;

    let sawCompletion = false;
    let unlisten: (() => void) | null = null;
    const detach = () => {
      if (unlisten !== null) {
        listenersRef.current.delete(unlisten);
        unlisten();
        unlisten = null;
      }
    };
    try {
      // Subscribe to the completion event BEFORE invoking the command
      // to avoid a race where the process exits before the listener exists.
      // The listener outlives a reset so a detached start still learns
      // whether its run already ended before cancelling it.
      unlisten = await subscribeEvent<number | null>('update-complete', (event) => {
        sawCompletion = true;
        detach();
        if (!isCurrent()) {
          return;
        }
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
      });
      listenersRef.current.add(unlisten);
      if (!isCurrent()) {
        detach();
        return;
      }

      const updateResult = await callCommand<UpdateGameResult>('update_game', { request });
      if (!isCurrent()) {
        // Reset or a newer start happened while this start was in flight:
        // stop the run it started unless a completion already ended it.
        if (!sawCompletion) {
          detach();
          void callCommand<void>('cancel_update').catch(() => undefined);
        }
        return;
      }
      setResult(updateResult);

      // The process may already have exited; keep a terminal stage.
      setStage((current) => (current === 'preparing' ? 'running_updater' : current));
    } catch (invokeError) {
      detach();
      if (!isCurrent()) return;
      const message = normalizeErrorMessage(invokeError);
      setStage('failed');
      setError(message);
    }
  }, [request]);

  const cancelUpdate = useCallback(async () => {
    try {
      await callCommand<void>('cancel_update');
    } catch {
      // Best-effort cancellation
    }
  }, []);

  const reset = useCallback(() => {
    // The backend claims the window's run slot at the start of `update_game`,
    // so a cancel while preparing aborts the pending start before it spawns.
    if (stage === 'preparing' || stage === 'running_updater') {
      void cancelUpdate();
    }
    generationRef.current += 1;
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
    return () => cleanupListeners();
  }, [cleanupListeners]);

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
