import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import type { GameProfile } from '../types/profile';
import {
  INSTALL_GAME_VALIDATION_FIELD,
  INSTALL_GAME_VALIDATION_MESSAGES,
  type InstallGameExecutableCandidate,
  type InstallGamePrefixPathState,
  type InstallGameRequest,
  type InstallGameResult,
  type InstallGameStage,
  type InstallGameValidationError,
  type InstallGameValidationState,
} from '../types/install';

type PrefixPathSource = 'auto' | 'manual';

export interface UseInstallGameResult {
  request: InstallGameRequest;
  validation: InstallGameValidationState;
  stage: InstallGameStage;
  result: InstallGameResult | null;
  reviewProfile: GameProfile | null;
  error: string | null;
  defaultPrefixPath: string;
  defaultPrefixPathState: InstallGamePrefixPathState;
  defaultPrefixPathError: string | null;
  candidateOptions: InstallGameExecutableCandidate[];
  actionLabel: string;
  statusText: string;
  hintText: string;
  isIdle: boolean;
  isPreparing: boolean;
  isRunningInstaller: boolean;
  isReviewRequired: boolean;
  isReadyToSave: boolean;
  hasFailed: boolean;
  isResolvingDefaultPrefixPath: boolean;
  setRequest: (request: InstallGameRequest) => void;
  updateRequest: <Key extends keyof InstallGameRequest>(key: Key, value: InstallGameRequest[Key]) => void;
  patchRequest: (patch: Partial<InstallGameRequest>) => void;
  setFieldError: <Key extends keyof InstallGameRequest>(key: Key, error: string | null) => void;
  setGeneralError: (error: string | null) => void;
  clearValidation: () => void;
  setStage: (stage: InstallGameStage) => void;
  setResult: (result: InstallGameResult | null) => void;
  setError: (error: string | null) => void;
  setInstalledExecutablePath: (path: string) => void;
  startInstall: () => Promise<void>;
  reset: () => void;
}

function createEmptyInstallGameRequest(): InstallGameRequest {
  return {
    profile_name: '',
    display_name: '',
    installer_path: '',
    trainer_path: '',
    proton_path: '',
    prefix_path: '',
    installed_game_executable_path: '',
    launcher_icon_path: '',
    custom_cover_art_path: '',
  };
}

function createEmptyValidationState(): InstallGameValidationState {
  return {
    fieldErrors: {},
    generalError: null,
  };
}

function parentDirectory(path: string): string {
  const normalized = path.trim().replace(/\\/g, '/');
  const separatorIndex = normalized.lastIndexOf('/');

  if (separatorIndex <= 0) {
    return '';
  }

  return normalized.slice(0, separatorIndex);
}

function normalizeErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function mapValidationErrorToField(message: string): keyof InstallGameRequest | null {
  const variants = Object.keys(INSTALL_GAME_VALIDATION_MESSAGES) as InstallGameValidationError[];
  for (const variant of variants) {
    if (message === INSTALL_GAME_VALIDATION_MESSAGES[variant]) {
      return INSTALL_GAME_VALIDATION_FIELD[variant];
    }
  }

  const normalized = message.toLowerCase();

  if (
    normalized.includes('profile name') ||
    normalized.includes('invalid profile name') ||
    normalized.includes('invalid characters')
  ) {
    return 'profile_name';
  }

  if (
    normalized.includes('installer path') ||
    normalized.includes('windows .exe') ||
    normalized.includes('installer media')
  ) {
    return 'installer_path';
  }

  if (normalized.includes('trainer path')) {
    return 'trainer_path';
  }

  if (normalized.includes('custom cover art path')) {
    return 'custom_cover_art_path';
  }

  if (normalized.includes('proton path')) {
    return 'proton_path';
  }

  if (normalized.includes('prefix path')) {
    return 'prefix_path';
  }

  if (normalized.includes('final game executable path')) {
    return 'installed_game_executable_path';
  }

  return null;
}

function deriveReviewProfile(profile: GameProfile | null, executablePath: string): GameProfile | null {
  if (!profile) {
    return null;
  }

  const trimmedExecutablePath = executablePath.trim();

  return {
    ...profile,
    game: {
      ...profile.game,
      executable_path: trimmedExecutablePath,
    },
    runtime: {
      ...profile.runtime,
      working_directory: trimmedExecutablePath
        ? parentDirectory(trimmedExecutablePath)
        : profile.runtime.working_directory,
    },
  };
}

function createCandidateOptions(result: InstallGameResult | null): InstallGameExecutableCandidate[] {
  if (!result) {
    return [];
  }

  return result.discovered_game_executable_candidates.map((path, index) => ({
    path,
    index,
    is_recommended: index === 0,
  }));
}

function deriveResultStage(result: InstallGameResult | null): InstallGameStage {
  if (result === null) {
    return 'idle';
  }

  if (!result.succeeded) {
    return 'failed';
  }

  return result.needs_executable_confirmation ? 'review_required' : 'ready_to_save';
}

function deriveStatusText(
  stage: InstallGameStage,
  defaultPrefixPathState: InstallGamePrefixPathState,
  defaultPrefixPath: string,
  result: InstallGameResult | null
): string {
  if (defaultPrefixPathState === 'loading') {
    return 'Resolving the default prefix path from the current profile name.';
  }

  switch (stage) {
    case 'preparing':
      return 'Validating install inputs and preparing to launch the installer.';
    case 'running_installer':
      return 'Installer execution is in progress through Proton.';
    case 'review_required':
      return 'Installer finished. Confirm the final executable before the profile can be handed off.';
    case 'ready_to_save':
      return 'Final executable confirmed. The profile is ready for the later save handoff.';
    case 'failed':
      return result?.message || 'Install failed. Review the errors and try again.';
    case 'idle':
    default:
      return defaultPrefixPath.trim().length > 0
        ? 'Install fields are ready. CrossHook will use the suggested default prefix unless you override it.'
        : 'Fill the install form to resolve a default prefix and launch the installer.';
  }
}

function deriveHintText(
  stage: InstallGameStage,
  result: InstallGameResult | null,
  defaultPrefixPath: string,
  defaultPrefixPathError: string | null
): string {
  if (defaultPrefixPathError) {
    return defaultPrefixPathError;
  }

  switch (stage) {
    case 'preparing':
      return 'The backend will validate the request, create the prefix if needed, and then launch the installer.';
    case 'running_installer':
      return 'The installer log path appears after the process completes. The resulting profile stays editable.';
    case 'review_required':
      return 'Pick a candidate or browse for the installed executable. The field stays editable after selection.';
    case 'ready_to_save':
      return 'The install result is ready to hand off to the save flow in the next task.';
    case 'failed':
      return 'The install request failed. Review the error message and adjust the inputs before retrying.';
    case 'idle':
    default:
      return defaultPrefixPath.trim().length > 0
        ? 'CrossHook keeps the suggested prefix path editable so you can override it before running the installer.'
        : 'As you type the profile name, CrossHook will resolve a default prefix under your local data directory.';
  }
}

export function useInstallGame(): UseInstallGameResult {
  const [request, setRequestState] = useState<InstallGameRequest>(createEmptyInstallGameRequest);
  const [validation, setValidationState] = useState<InstallGameValidationState>(createEmptyValidationState);
  const [stage, setStageState] = useState<InstallGameStage>('idle');
  const [result, setResultState] = useState<InstallGameResult | null>(null);
  const [reviewProfile, setReviewProfileState] = useState<GameProfile | null>(null);
  const [error, setErrorState] = useState<string | null>(null);
  const [defaultPrefixPath, setDefaultPrefixPath] = useState('');
  const [defaultPrefixPathState, setDefaultPrefixPathState] = useState<InstallGamePrefixPathState>('idle');
  const [defaultPrefixPathError, setDefaultPrefixPathError] = useState<string | null>(null);
  const [prefixPathSource, setPrefixPathSource] = useState<PrefixPathSource>('auto');
  const prefixResolutionRequestIdRef = useRef(0);

  const candidateOptions = useMemo(() => createCandidateOptions(result), [result]);
  const isResolvingDefaultPrefixPath = defaultPrefixPathState === 'loading';

  const setRequest = useCallback((nextRequest: InstallGameRequest) => {
    setRequestState(nextRequest);
    setPrefixPathSource(nextRequest.prefix_path.trim().length > 0 ? 'manual' : 'auto');
  }, []);

  const updateRequest = useCallback(
    <Key extends keyof InstallGameRequest>(key: Key, value: InstallGameRequest[Key]) => {
      setRequestState((current) => ({
        ...current,
        [key]: value,
      }));

      if (key === 'prefix_path') {
        const trimmedValue = String(value).trim();
        setPrefixPathSource(trimmedValue.length > 0 ? 'manual' : 'auto');
      }
    },
    []
  );

  const patchRequest = useCallback((patch: Partial<InstallGameRequest>) => {
    setRequestState((current) => ({
      ...current,
      ...patch,
    }));

    if (Object.prototype.hasOwnProperty.call(patch, 'prefix_path')) {
      const trimmedValue = patch.prefix_path?.trim() ?? '';
      setPrefixPathSource(trimmedValue.length > 0 ? 'manual' : 'auto');
    }
  }, []);

  const setFieldError = useCallback(<Key extends keyof InstallGameRequest>(key: Key, nextError: string | null) => {
    setValidationState((current) => {
      const fieldErrors = { ...current.fieldErrors };
      if (nextError === null) {
        delete fieldErrors[key];
      } else {
        fieldErrors[key] = nextError;
      }

      return {
        ...current,
        fieldErrors,
      };
    });
  }, []);

  const setGeneralError = useCallback((nextError: string | null) => {
    setValidationState((current) => ({
      ...current,
      generalError: nextError,
    }));
  }, []);

  const clearValidation = useCallback(() => {
    setValidationState(createEmptyValidationState());
  }, []);

  const setStage = useCallback((nextStage: InstallGameStage) => {
    setStageState(nextStage);
  }, []);

  const setError = useCallback((nextError: string | null) => {
    setErrorState(nextError);
  }, []);

  const setResult = useCallback((nextResult: InstallGameResult | null) => {
    setResultState(nextResult);

    if (nextResult === null) {
      setReviewProfileState(null);
      setStageState('idle');
      setErrorState(null);
      return;
    }

    setReviewProfileState(nextResult.profile);
    setRequestState((current) => ({
      ...current,
      prefix_path: nextResult.profile.runtime.prefix_path.trim() || current.prefix_path,
      installed_game_executable_path: nextResult.profile.game.executable_path.trim(),
    }));
    setStageState(deriveResultStage(nextResult));
    setErrorState(nextResult.succeeded ? null : nextResult.message);
  }, []);

  const setInstalledExecutablePath = useCallback(
    (path: string) => {
      const trimmedPath = path.trim();

      setRequestState((current) => ({
        ...current,
        installed_game_executable_path: path,
      }));

      setReviewProfileState((currentReviewProfile) =>
        currentReviewProfile === null
          ? null
          : {
              ...currentReviewProfile,
              game: {
                ...currentReviewProfile.game,
                executable_path: trimmedPath,
              },
              runtime: {
                ...currentReviewProfile.runtime,
                working_directory: trimmedPath
                  ? parentDirectory(trimmedPath)
                  : currentReviewProfile.runtime.working_directory,
              },
            }
      );

      if (result?.succeeded) {
        setStageState(trimmedPath.length > 0 ? 'ready_to_save' : 'review_required');
      }
    },
    [result]
  );

  const resolveDefaultPrefixPath = useCallback(
    async (profileName: string, requestId?: number) => {
      const trimmedProfileName = profileName.trim();
      if (!trimmedProfileName) {
        setDefaultPrefixPath('');
        setDefaultPrefixPathState('idle');
        setDefaultPrefixPathError(null);
        return '';
      }

      setDefaultPrefixPathState('loading');
      setDefaultPrefixPathError(null);

      try {
        const resolvedPrefixPath = await invoke<string>('install_default_prefix_path', {
          profileName: trimmedProfileName,
        });

        if (requestId !== undefined && requestId !== prefixResolutionRequestIdRef.current) {
          return resolvedPrefixPath;
        }

        setDefaultPrefixPath(resolvedPrefixPath);
        setDefaultPrefixPathState('ready');

        setRequestState((current) => {
          const currentPrefixPath = current.prefix_path.trim();
          const previousAutoPrefixPath = defaultPrefixPath.trim();
          const shouldApplyResolvedPrefix =
            currentPrefixPath.length === 0 ||
            (previousAutoPrefixPath.length > 0 && currentPrefixPath === previousAutoPrefixPath);

          if (shouldApplyResolvedPrefix) {
            return {
              ...current,
              prefix_path: resolvedPrefixPath,
            };
          }

          return current;
        });

        return resolvedPrefixPath;
      } catch (invokeError) {
        const message = normalizeErrorMessage(invokeError);
        setDefaultPrefixPath('');
        setDefaultPrefixPathState('failed');
        setDefaultPrefixPathError(message);
        return '';
      }
    },
    [defaultPrefixPath]
  );

  const startInstall = useCallback(async () => {
    const profileName = request.profile_name.trim();
    clearValidation();
    setErrorState(null);
    setStageState('preparing');

    let prefixPath = request.prefix_path.trim();

    try {
      if (prefixPath.length === 0) {
        prefixPath = (await resolveDefaultPrefixPath(profileName)).trim();
      }

      const installRequest: InstallGameRequest = {
        ...request,
        prefix_path: prefixPath,
      };

      await invoke<void>('validate_install_request', {
        request: installRequest,
      });

      setRequestState((current) => ({
        ...current,
        prefix_path: prefixPath,
      }));
      setStageState('running_installer');

      const installResult = await invoke<InstallGameResult>('install_game', {
        request: installRequest,
      });

      setResult(installResult);
    } catch (invokeError) {
      const message = normalizeErrorMessage(invokeError);
      const validationField = mapValidationErrorToField(message);

      setStageState('failed');
      setErrorState(message);

      if (validationField === null) {
        setGeneralError(message);
      } else {
        setFieldError(validationField, message);
      }
    }
  }, [clearValidation, resolveDefaultPrefixPath, request, setFieldError, setGeneralError]);

  const reset = useCallback(() => {
    setRequestState(createEmptyInstallGameRequest());
    setValidationState(createEmptyValidationState());
    setStageState('idle');
    setResultState(null);
    setReviewProfileState(null);
    setErrorState(null);
    setDefaultPrefixPath('');
    setDefaultPrefixPathState('idle');
    setDefaultPrefixPathError(null);
    setPrefixPathSource('auto');
  }, []);

  useEffect(() => {
    const trimmedProfileName = request.profile_name.trim();
    let active = true;

    if (!trimmedProfileName) {
      prefixResolutionRequestIdRef.current += 1;
      setDefaultPrefixPath('');
      setDefaultPrefixPathState('idle');
      setDefaultPrefixPathError(null);
      return () => {
        active = false;
      };
    }

    const requestId = ++prefixResolutionRequestIdRef.current;
    const timeout = window.setTimeout(() => {
      void resolveDefaultPrefixPath(trimmedProfileName, requestId).then((resolvedPrefixPath) => {
        if (!active || resolvedPrefixPath.length === 0) {
          return;
        }
      });
    }, 250);

    return () => {
      active = false;
      window.clearTimeout(timeout);
    };
  }, [request.profile_name, resolveDefaultPrefixPath]);

  const statusText = deriveStatusText(stage, defaultPrefixPathState, defaultPrefixPath, result);
  const hintText = deriveHintText(stage, result, defaultPrefixPath, defaultPrefixPathError);

  const actionLabel = (() => {
    switch (stage) {
      case 'running_installer':
        return 'Installing...';
      case 'review_required':
        return 'Retry Install';
      case 'ready_to_save':
        return 'Retry Install';
      case 'failed':
        return 'Retry Install';
      case 'preparing':
      case 'idle':
      default:
        return 'Install Game';
    }
  })();

  return {
    request,
    validation,
    stage,
    result,
    reviewProfile,
    error,
    defaultPrefixPath,
    defaultPrefixPathState,
    defaultPrefixPathError,
    candidateOptions,
    isIdle: stage === 'idle',
    isPreparing: stage === 'preparing',
    isRunningInstaller: stage === 'running_installer',
    isReviewRequired: stage === 'review_required',
    isReadyToSave: stage === 'ready_to_save',
    hasFailed: stage === 'failed',
    isResolvingDefaultPrefixPath,
    setRequest,
    updateRequest,
    patchRequest,
    setFieldError,
    setGeneralError,
    clearValidation,
    setStage,
    setResult,
    setError,
    setInstalledExecutablePath,
    startInstall,
    reset,
    actionLabel,
    statusText,
    hintText,
  };
}
