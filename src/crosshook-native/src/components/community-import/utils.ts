import { isLaunchValidationIssue, type LaunchRequest, type LaunchValidationIssue } from '../../types';
import type { GameProfile } from '../../types/profile';
import { buildProfileLaunchRequest, resolveLaunchMethod } from '../../utils/launch';
import type { SteamFieldState } from './types';

/**
 * Local stand-in for the backend `game_path_required` issue (text mirrors
 * `ValidationError::GamePathRequired` in crosshook-core) used when the draft has no
 * game executable, so the wizard never sends a request it knows is invalid.
 */
export const MISSING_GAME_PATH_ISSUE: LaunchValidationIssue = {
  message: 'A game executable path is required.',
  help: "Browse to the game executable before launching. For Steam or Proton launches, this is usually the game's .exe under steamapps/common/.",
  severity: 'fatal',
  code: 'game_path_required',
};

export function normalizeProfile(profile: GameProfile): GameProfile {
  return {
    ...profile,
    trainer: {
      ...profile.trainer,
      loading_mode: profile.trainer?.loading_mode ?? 'source_directory',
    },
    steam: {
      ...profile.steam,
      launcher: {
        icon_path: profile.steam?.launcher?.icon_path ?? '',
        display_name: profile.steam?.launcher?.display_name ?? '',
      },
    },
    runtime: {
      ...profile.runtime,
      prefix_path: profile.runtime?.prefix_path ?? '',
      proton_path: profile.runtime?.proton_path ?? '',
      working_directory: profile.runtime?.working_directory ?? '',
    },
    launch: {
      ...profile.launch,
      method: profile.launch?.method ?? 'proton_run',
      optimizations: {
        enabled_option_ids: profile.launch?.optimizations?.enabled_option_ids ?? [],
      },
      custom_env_vars: { ...(profile.launch?.custom_env_vars ?? {}) },
    },
    local_override: profile.local_override ?? {
      game: { executable_path: '' },
      trainer: { path: '' },
      steam: {
        compatdata_path: '',
        proton_path: '',
      },
      runtime: {
        prefix_path: '',
        proton_path: '',
      },
    },
  };
}

/**
 * Build the wizard's preview/validate request through the shared profile builder.
 * The wizard has no saved profile name yet and uses the backend default umu preference.
 * Returns `null` when the draft has no game executable path.
 */
export function buildLaunchRequest(profile: GameProfile, steamClientInstallPath: string): LaunchRequest | null {
  return buildProfileLaunchRequest(profile, resolveLaunchMethod(profile), steamClientInstallPath, '', 'auto');
}

export function toStatusClass(state: SteamFieldState): string {
  switch (state) {
    case 'Found':
      return 'found';
    case 'NotFound':
      return 'not-found';
    case 'Ambiguous':
      return 'ambiguous';
    case 'Saved':
      return 'saved';
    default:
      return 'idle';
  }
}

export function isStrictLaunchValidationIssue(value: unknown): value is LaunchValidationIssue {
  if (!isLaunchValidationIssue(value) || typeof value !== 'object' || value === null || Array.isArray(value)) {
    return false;
  }

  const allowedKeys = new Set([
    'message',
    'help',
    'severity',
    'code',
    'trainer_hash_stored',
    'trainer_hash_current',
    'trainer_sha256_community',
  ]);
  return Object.keys(value).every((key) => allowedKeys.has(key));
}
