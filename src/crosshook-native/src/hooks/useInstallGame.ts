import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import type { GameProfile, LaunchMethod } from '../types/profile';
import { createDefaultProfile } from '../types/profile';
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
import { resolveLaunchMethod } from '../utils/launch';

export interface InstallerInputs {
  installer_path: string;
}

export interface UseInstallGameResult {
  /** @deprecated Prefer `draftProfile` + `buildInstallGameRequest` — derived snapshot for legacy callers. */
  request: InstallGameRequest;
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
  /** @deprecated Use `updateDraftProfile` / `updateInstallerInputs`. */
  setRequest: (request: InstallGameRequest) => void;
  /** @deprecated Use `updateDraftProfile` / `updateInstallerInputs`. */
  updateRequest: <Key extends keyof InstallGameRequest>(key: Key, value: InstallGameRequest[Key]) => void;
  /** @deprecated Use `updateDraftProfile` / `updateInstallerInputs`. */
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

  if (normalized.includes('custom portrait art path')) {
    return 'custom_portrait_art_path';
  }

  if (normalized.includes('custom background art path')) {
    return 'custom_background_art_path';
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

function buildInstallGameRequest(
  profileName: string,
  draftProfile: GameProfile,
  installerPath: string
): InstallGameRequest {
  const launchMethod = (draftProfile.launch.method?.trim() || 'proton_run') as LaunchMethod;
  const protonPath =
    launchMethod === 'steam_applaunch' ? draftProfile.steam.proton_path : draftProfile.runtime.proton_path;
  const prefixPath =
    launchMethod === 'steam_applaunch' ? draftProfile.steam.compatdata_path : draftProfile.runtime.prefix_path;
  const steamAppId =
    launchMethod === 'steam_applaunch' ? draftProfile.steam.app_id : (draftProfile.runtime.steam_app_id ?? '');
  return {
    profile_name: profileName,
    display_name: draftProfile.game.name,
    installer_path: installerPath,
    trainer_path: draftProfile.trainer.path,
    proton_path: protonPath,
    prefix_path: prefixPath,
    installed_game_executable_path: draftProfile.game.executable_path,
    launcher_icon_path: draftProfile.steam.launcher.icon_path,
    custom_cover_art_path: draftProfile.game.custom_cover_art_path ?? '',
    runner_method: launchMethod,
    steam_app_id: steamAppId,
    custom_portrait_art_path: draftProfile.game.custom_portrait_art_path ?? '',
    custom_background_art_path: draftProfile.game.custom_background_art_path ?? '',
    working_directory: draftProfile.runtime.working_directory ?? '',
  };
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

  const request = useMemo(
    () => buildInstallGameRequest(profileName, draftProfile, installerInputs.installer_path),
    [profileName, draftProfile, installerInputs.installer_path]
  );

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

  const setRequest = useCallback((nextRequest: InstallGameRequest) => {
    setProfileNameState(nextRequest.profile_name);
    setInstallerInputs({ installer_path: nextRequest.installer_path });
    setDraftProfileState((current) => {
      const launchMethod = (nextRequest.runner_method || current.launch.method || 'proton_run') as LaunchMethod;
      return {
        ...current,
        game: {
          ...current.game,
          name: nextRequest.display_name,
          executable_path: nextRequest.installed_game_executable_path,
          custom_cover_art_path: nextRequest.custom_cover_art_path,
          custom_portrait_art_path: nextRequest.custom_portrait_art_path,
          custom_background_art_path: nextRequest.custom_background_art_path,
        },
        trainer: { ...current.trainer, path: nextRequest.trainer_path },
        steam: {
          ...current.steam,
          app_id: launchMethod === 'steam_applaunch' ? nextRequest.steam_app_id : current.steam.app_id,
          compatdata_path: launchMethod === 'steam_applaunch' ? nextRequest.prefix_path : current.steam.compatdata_path,
          proton_path: launchMethod === 'steam_applaunch' ? nextRequest.proton_path : current.steam.proton_path,
          launcher: { ...current.steam.launcher, icon_path: nextRequest.launcher_icon_path },
        },
        runtime: {
          ...current.runtime,
          prefix_path: launchMethod !== 'steam_applaunch' ? nextRequest.prefix_path : current.runtime.prefix_path,
          proton_path: launchMethod !== 'steam_applaunch' ? nextRequest.proton_path : current.runtime.proton_path,
          working_directory: nextRequest.working_directory,
          steam_app_id:
            launchMethod === 'proton_run' || launchMethod === 'native' ? nextRequest.steam_app_id : current.runtime.steam_app_id,
        },
        launch: { ...current.launch, method: launchMethod },
      };
    });
  }, []);

  const updateRequest = useCallback(
    <Key extends keyof InstallGameRequest>(key: Key, value: InstallGameRequest[Key]) => {
      const v = value as string;
      switch (key) {
        case 'profile_name':
          setProfileNameState(v);
          break;
        case 'installer_path':
          setInstallerInputs((prev) => ({ ...prev, installer_path: v }));
          break;
        case 'display_name':
          setDraftProfileState((c) => ({ ...c, game: { ...c.game, name: v } }));
          break;
        case 'trainer_path':
          setDraftProfileState((c) => ({ ...c, trainer: { ...c.trainer, path: v } }));
          break;
        case 'proton_path':
          setDraftProfileState((c) => {
            const m = resolveLaunchMethod(c);
            if (m === 'steam_applaunch') {
              return { ...c, steam: { ...c.steam, proton_path: v } };
            }
            return { ...c, runtime: { ...c.runtime, proton_path: v } };
          });
          break;
        case 'prefix_path':
          setDraftProfileState((c) => {
            const m = resolveLaunchMethod(c);
            if (m === 'steam_applaunch') {
              return { ...c, steam: { ...c.steam, compatdata_path: v } };
            }
            return { ...c, runtime: { ...c.runtime, prefix_path: v } };
          });
          break;
        case 'installed_game_executable_path':
          setDraftProfileState((c) => ({
            ...c,
            game: { ...c.game, executable_path: v },
          }));
          break;
        case 'launcher_icon_path':
          setDraftProfileState((c) => ({
            ...c,
            steam: { ...c.steam, launcher: { ...c.steam.launcher, icon_path: v } },
          }));
          break;
        case 'custom_cover_art_path':
          setDraftProfileState((c) => ({ ...c, game: { ...c.game, custom_cover_art_path: v } }));
          break;
        case 'runner_method':
          setDraftProfileState((c) => ({ ...c, launch: { ...c.launch, method: v as LaunchMethod } }));
          break;
        case 'steam_app_id':
          setDraftProfileState((c) => {
            const m = resolveLaunchMethod(c);
            if (m === 'steam_applaunch') {
              return { ...c, steam: { ...c.steam, app_id: v } };
            }
            return { ...c, runtime: { ...c.runtime, steam_app_id: v } };
          });
          break;
        case 'custom_portrait_art_path':
          setDraftProfileState((c) => ({ ...c, game: { ...c.game, custom_portrait_art_path: v } }));
          break;
        case 'custom_background_art_path':
          setDraftProfileState((c) => ({ ...c, game: { ...c.game, custom_background_art_path: v } }));
          break;
        case 'working_directory':
          setDraftProfileState((c) => ({ ...c, runtime: { ...c.runtime, working_directory: v } }));
          break;
        default:
          break;
      }
    },
    []
  );

  const patchRequest = useCallback((patch: Partial<InstallGameRequest>) => {
    (Object.keys(patch) as (keyof InstallGameRequest)[]).forEach((key) => {
      if (Object.prototype.hasOwnProperty.call(patch, key)) {
        updateRequest(key, patch[key] as InstallGameRequest[typeof key]);
      }
    });
  }, [updateRequest]);

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

    setDraftProfileState((current) => ({
      ...nextResult.profile,
      game: {
        ...nextResult.profile.game,
        custom_cover_art_path: current.game.custom_cover_art_path?.trim()
          ? current.game.custom_cover_art_path
          : nextResult.profile.game.custom_cover_art_path,
        custom_portrait_art_path: current.game.custom_portrait_art_path?.trim()
          ? current.game.custom_portrait_art_path
          : nextResult.profile.game.custom_portrait_art_path,
        custom_background_art_path: current.game.custom_background_art_path?.trim()
          ? current.game.custom_background_art_path
          : nextResult.profile.game.custom_background_art_path,
      },
    }));

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
          working_directory: trimmedPath
            ? parentDirectory(trimmedPath)
            : current.runtime.working_directory,
        },
      }));

      if (result?.succeeded) {
        setStageState(trimmedPath.length > 0 ? 'ready_to_save' : 'review_required');
      }
    },
    [result]
  );

  const resolveDefaultPrefixPath = useCallback(
    async (resolvedProfileName: string, requestId?: number) => {
      const trimmedProfileName = resolvedProfileName.trim();
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

        setDraftProfileState((current) => {
          const launchMethod = resolveLaunchMethod(current);
          const currentPrefixPath =
            launchMethod === 'steam_applaunch'
              ? current.steam.compatdata_path.trim()
              : current.runtime.prefix_path.trim();
          const previousAutoPrefixPath = defaultPrefixPath.trim();
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
        setDefaultPrefixPathState('failed');
        setDefaultPrefixPathError(message);
        return '';
      }
    },
    [defaultPrefixPath]
  );

  const startInstall = useCallback(async () => {
    const trimmedProfileName = profileName.trim();
    clearValidation();
    setErrorState(null);
    setStageState('preparing');

    let finalRequest = buildInstallGameRequest(
      trimmedProfileName,
      draftProfile,
      installerInputs.installer_path
    );

    if (finalRequest.prefix_path.trim().length === 0) {
      const resolved = (await resolveDefaultPrefixPath(trimmedProfileName)).trim();
      finalRequest = { ...finalRequest, prefix_path: resolved };
    }

    try {
      await invoke<void>('validate_install_request', {
        request: finalRequest,
      });

      setStageState('running_installer');

      const installResult = await invoke<InstallGameResult>('install_game', {
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
    setProfileNameState('');
    setDraftProfileState(createDefaultProfile());
    setInstallerInputs({ installer_path: '' });
    setValidationState(createEmptyValidationState());
    setStageState('idle');
    setResultState(null);
    setErrorState(null);
    setDefaultPrefixPath('');
    setDefaultPrefixPathState('idle');
    setDefaultPrefixPathError(null);
  }, []);

  useEffect(() => {
    const trimmedProfileName = profileName.trim();
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
    request,
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
