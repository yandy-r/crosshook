import { callCommand } from '@/lib/ipc';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import type { GameProfile } from '../types/profile';
import { createDefaultProfile } from '../types/profile';
import {
  type InstallGameExecutableCandidate,
  type InstallGamePrefixPathState,
  type InstallGameRequest,
  type InstallGameResult,
  type InstallGameStage,
  type InstallGameValidationState,
} from '../types/install';
import { resolveLaunchMethod } from '../utils/launch';
import { mergeInstallGameResultIntoDraft } from '../components/install/mergeInstallGameResultIntoDraft';
import { buildInstallGameRequest } from './install/installRequestBuild';
import { deriveHintText, deriveResultStage, deriveStatusText } from './install/installStatusText';
import { mapValidationErrorToField } from './install/installValidationMapping';

export interface InstallerInputs {
  installer_path: string;
}

export interface UseInstallGameResult {
  profileName: string;
  setProfileName: (value: string) => void;
  draftProfile: GameProfile;
  updateDraftProfile: (updater: (current: GameProfile) => GameProfile) => void;
  installerInputs: InstallerInputs;
  updateInstallerInputs: <Key extends keyof InstallerInputs>(key: Key, value: InstallerInputs[Key]) => void;
  validation: InstallGameValidationState;
  stage: InstallGameStage;
  result: InstallGameResult | null;
  /** Present after a successful install — mirrors merged `draftProfile`. */
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

export function useInstallGame(): UseInstallGameResult {
  const [profileName, setProfileNameState] = useState('');
  const [draftProfile, setDraftProfileState] = useState<GameProfile>(createDefaultProfile);
  const [installerInputs, setInstallerInputs] = useState<InstallerInputs>({ installer_path: '' });
  const [validation, setValidationState] = useState<InstallGameValidationState>(createEmptyValidationState);
  const [stage, setStageState] = useState<InstallGameStage>('idle');
  const [result, setResultState] = useState<InstallGameResult | null>(null);
  const [error, setErrorState] = useState<string | null>(null);
  const [defaultPrefixPath, setDefaultPrefixPath] = useState('');
  const [defaultPrefixPathState, setDefaultPrefixPathState] = useState<InstallGamePrefixPathState>('idle');
  const [defaultPrefixPathError, setDefaultPrefixPathError] = useState<string | null>(null);
  const prefixResolutionRequestIdRef = useRef(0);
  /** Last prefix value suggested by IPC; avoids stale closures and redundant effect churn. */
  const lastAutoSuggestedPrefixRef = useRef('');

  const reviewProfile = useMemo(() => (result?.succeeded ? draftProfile : null), [result, draftProfile]);

  const candidateOptions = useMemo(() => createCandidateOptions(result), [result]);
  const isResolvingDefaultPrefixPath = defaultPrefixPathState === 'loading';

  const setProfileName = useCallback((value: string) => {
    setProfileNameState(value);
  }, []);

  const updateDraftProfile = useCallback((updater: (current: GameProfile) => GameProfile) => {
    setDraftProfileState((current) => updater(current));
  }, []);

  const updateInstallerInputs = useCallback(
    <Key extends keyof InstallerInputs>(key: Key, value: InstallerInputs[Key]) => {
      setInstallerInputs((current) => ({ ...current, [key]: value }));
    },
    []
  );

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
      setStageState('idle');
      setErrorState(null);
      return;
    }

    setDraftProfileState((current) => mergeInstallGameResultIntoDraft(current, nextResult.profile));

    setStageState(deriveResultStage(nextResult));
    setErrorState(nextResult.succeeded ? null : nextResult.message);
  }, []);

  const setInstalledExecutablePath = useCallback(
    (path: string) => {
      const trimmedPath = path.trim();

      setDraftProfileState((current) => ({
        ...current,
        game: {
          ...current.game,
          executable_path: path,
        },
        runtime: {
          ...current.runtime,
          working_directory: trimmedPath ? parentDirectory(trimmedPath) : current.runtime.working_directory,
        },
      }));

      if (result?.succeeded) {
        setStageState(trimmedPath.length > 0 ? 'ready_to_save' : 'review_required');
      }
    },
    [result]
  );

  const resolveDefaultPrefixPath = useCallback(async (resolvedProfileName: string, requestId?: number) => {
    const trimmedProfileName = resolvedProfileName.trim();
    if (!trimmedProfileName) {
      setDefaultPrefixPath('');
      lastAutoSuggestedPrefixRef.current = '';
      setDefaultPrefixPathState('idle');
      setDefaultPrefixPathError(null);
      return '';
    }

    setDefaultPrefixPathState('loading');
    setDefaultPrefixPathError(null);

    try {
      const resolvedPrefixPath = await callCommand<string>('install_default_prefix_path', {
        profileName: trimmedProfileName,
      });

      if (requestId !== undefined && requestId !== prefixResolutionRequestIdRef.current) {
        return resolvedPrefixPath;
      }

      const previousAutoPrefixPath = lastAutoSuggestedPrefixRef.current.trim();
      setDefaultPrefixPath(resolvedPrefixPath);
      lastAutoSuggestedPrefixRef.current = resolvedPrefixPath;
      setDefaultPrefixPathState('ready');

      setDraftProfileState((current) => {
        const launchMethod = resolveLaunchMethod(current);
        const currentPrefixPath =
          launchMethod === 'steam_applaunch'
            ? current.steam.compatdata_path.trim()
            : current.runtime.prefix_path.trim();
        const shouldApplyResolvedPrefix =
          currentPrefixPath.length === 0 ||
          (previousAutoPrefixPath.length > 0 && currentPrefixPath === previousAutoPrefixPath);

        if (!shouldApplyResolvedPrefix) {
          return current;
        }

        if (launchMethod === 'steam_applaunch') {
          return {
            ...current,
            steam: { ...current.steam, compatdata_path: resolvedPrefixPath },
          };
        }

        return {
          ...current,
          runtime: { ...current.runtime, prefix_path: resolvedPrefixPath },
        };
      });

      return resolvedPrefixPath;
    } catch (invokeError) {
      const message = normalizeErrorMessage(invokeError);
      setDefaultPrefixPath('');
      lastAutoSuggestedPrefixRef.current = '';
      setDefaultPrefixPathState('failed');
      setDefaultPrefixPathError(message);
      return '';
    }
  }, []);

  const startInstall = useCallback(async () => {
    const trimmedProfileName = profileName.trim();
    clearValidation();
    setErrorState(null);
    setStageState('preparing');

    let finalRequest = buildInstallGameRequest(trimmedProfileName, draftProfile, installerInputs.installer_path);

    if (finalRequest.prefix_path.trim().length === 0) {
      const resolved = (await resolveDefaultPrefixPath(trimmedProfileName)).trim();
      finalRequest = { ...finalRequest, prefix_path: resolved };
    }

    try {
      await callCommand<void>('validate_install_request', {
        request: finalRequest,
      });

      setStageState('running_installer');

      const installResult = await callCommand<InstallGameResult>('install_game', {
        request: finalRequest,
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
  }, [
    clearValidation,
    draftProfile,
    installerInputs.installer_path,
    profileName,
    resolveDefaultPrefixPath,
    setFieldError,
    setGeneralError,
    setResult,
  ]);

  const reset = useCallback(() => {
    prefixResolutionRequestIdRef.current += 1;
    setProfileNameState('');
    setDraftProfileState(createDefaultProfile());
    setInstallerInputs({ installer_path: '' });
    setValidationState(createEmptyValidationState());
    setStageState('idle');
    setResultState(null);
    setErrorState(null);
    setDefaultPrefixPath('');
    lastAutoSuggestedPrefixRef.current = '';
    setDefaultPrefixPathState('idle');
    setDefaultPrefixPathError(null);
  }, []);

  useEffect(() => {
    const trimmedProfileName = profileName.trim();
    let active = true;

    if (!trimmedProfileName) {
      prefixResolutionRequestIdRef.current += 1;
      setDefaultPrefixPath('');
      lastAutoSuggestedPrefixRef.current = '';
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
  }, [profileName, resolveDefaultPrefixPath]);

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
    profileName,
    setProfileName,
    draftProfile,
    updateDraftProfile,
    installerInputs,
    updateInstallerInputs,
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
