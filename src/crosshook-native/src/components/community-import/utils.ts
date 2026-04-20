import { isLaunchValidationIssue, type LaunchRequest, type LaunchValidationIssue } from '../../types';
import {
  DEFAULT_GAMESCOPE_CONFIG,
  DEFAULT_MANGOHUD_CONFIG,
  type GameProfile,
  type LaunchMethod,
} from '../../types/profile';
import type { SteamFieldState } from './types';

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

export function resolveLaunchMethod(profile: GameProfile): Exclude<LaunchMethod, ''> {
  if (
    profile.launch?.method === 'steam_applaunch' ||
    profile.launch?.method === 'proton_run' ||
    profile.launch?.method === 'native'
  ) {
    return profile.launch.method;
  }

  if (profile.steam.enabled) {
    return 'steam_applaunch';
  }

  if (profile.game.executable_path.trim().toLowerCase().endsWith('.exe')) {
    return 'proton_run';
  }

  return 'native';
}

export function buildLaunchRequest(profile: GameProfile, steamClientInstallPath: string): LaunchRequest {
  const method = resolveLaunchMethod(profile);
  return {
    method,
    game_path: profile.game.executable_path,
    trainer_path: profile.trainer.path,
    trainer_host_path: profile.trainer.path,
    trainer_loading_mode: profile.trainer.loading_mode,
    steam: {
      app_id: profile.steam.app_id,
      compatdata_path: profile.steam.compatdata_path,
      proton_path: profile.steam.proton_path,
      steam_client_install_path: steamClientInstallPath,
    },
    runtime: {
      prefix_path: profile.runtime.prefix_path,
      proton_path: profile.runtime.proton_path,
      working_directory: profile.runtime.working_directory,
    },
    optimizations: {
      enabled_option_ids: [...profile.launch.optimizations.enabled_option_ids],
    },
    launch_game_only: false,
    launch_trainer_only: false,
    custom_env_vars: { ...profile.launch.custom_env_vars },
    network_isolation: profile.launch.network_isolation ?? true,
    gamescope: profile.launch.gamescope ?? DEFAULT_GAMESCOPE_CONFIG,
    mangohud: profile.launch.mangohud ?? DEFAULT_MANGOHUD_CONFIG,
  };
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
