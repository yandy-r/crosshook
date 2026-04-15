import type { GameProfile, LaunchMethod, LaunchRequest } from '../types';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG } from '../types/profile';
import type { UmuPreference } from '../types/settings';

export type ResolvedLaunchMethod = Exclude<GameProfile['launch']['method'], ''>;

export function looksLikeWindowsExecutable(path: string): boolean {
  return path.trim().toLowerCase().endsWith('.exe');
}

/**
 * Build a LaunchRequest from a GameProfile for IPC dispatch.
 * Shared by LaunchStateContext (provider) and LaunchPage (preview).
 */
export function buildProfileLaunchRequest(
  profile: GameProfile,
  launchMethod: Exclude<LaunchMethod, ''>,
  steamClientInstallPath: string,
  profileName: string,
  umuPreference: UmuPreference
): LaunchRequest | null {
  if (!profile.game.executable_path.trim()) {
    return null;
  }

  return {
    method: launchMethod,
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
      steam_app_id: profile.runtime.steam_app_id ?? '',
      umu_game_id: profile.runtime.umu_game_id ?? '',
    },
    optimizations: {
      enabled_option_ids: [...profile.launch.optimizations.enabled_option_ids],
    },
    launch_trainer_only: false,
    launch_game_only: false,
    profile_name: profileName || undefined,
    // Profile runtime override wins over the global default.
    umu_preference: profile.runtime.umu_preference ?? umuPreference,
    custom_env_vars: { ...profile.launch.custom_env_vars },
    network_isolation: profile.launch.network_isolation ?? true,
    gamescope: profile.launch.gamescope ?? DEFAULT_GAMESCOPE_CONFIG,
    trainer_gamescope: profile.launch.trainer_gamescope ?? DEFAULT_GAMESCOPE_CONFIG,
    mangohud: profile.launch.mangohud ?? DEFAULT_MANGOHUD_CONFIG,
  };
}

export function resolveLaunchMethod(profile: GameProfile): ResolvedLaunchMethod {
  const method = profile.launch.method.trim();

  if (method === 'steam_applaunch' || method === 'proton_run' || method === 'native') {
    return method;
  }

  if (profile.steam.enabled) {
    return 'steam_applaunch';
  }

  if (looksLikeWindowsExecutable(profile.game.executable_path)) {
    return 'proton_run';
  }

  return 'native';
}
