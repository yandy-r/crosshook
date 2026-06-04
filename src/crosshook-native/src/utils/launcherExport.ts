import type { SteamExternalLauncherExportRequest } from '@/hooks/useLauncherExport';
import type { GameProfile, LaunchMethod, UmuPreference } from '@/types';

export const automaticLauncherSuffix = ' - Trainer';

export function safeTrim(value: string | undefined | null): string {
  return value?.trim() ?? '';
}

export function stripAutomaticLauncherSuffix(value: string): string {
  const trimmed = value.trim();
  return trimmed.endsWith(automaticLauncherSuffix)
    ? trimmed.slice(0, -automaticLauncherSuffix.length).trimEnd()
    : trimmed;
}

export function deriveLauncherName(profile: GameProfile): string {
  const explicitName = stripAutomaticLauncherSuffix(safeTrim(profile.steam.launcher.display_name));
  if (explicitName) {
    return explicitName;
  }

  const gameName = safeTrim(profile.game.name);
  if (gameName) {
    return gameName;
  }

  const trainerStem = stripAutomaticLauncherSuffix(
    safeTrim(profile.trainer.path)
      .split(/[\\/]/)
      .pop()
      ?.replace(/\.[^.]+$/, '')
      .trim() ?? ''
  );
  if (trainerStem) {
    return trainerStem;
  }

  const steamAppId = safeTrim(profile.steam.app_id);
  if (steamAppId) {
    return `steam-${steamAppId}-trainer`;
  }

  return 'crosshook-trainer';
}

/** Only call from UI paths guarded by `profileCanExport`; Rust `validate_launcher_export` enforces trainer_path. */
export function buildLauncherExportRequest(
  profile: GameProfile,
  profileName: string,
  method: Exclude<LaunchMethod, ''>,
  launcherName: string,
  launcherIconPath: string,
  steamClientInstallPath: string,
  targetHomePath: string,
  globalUmuPreference: UmuPreference
): SteamExternalLauncherExportRequest {
  return {
    method,
    launcher_name: launcherName.trim(),
    trainer_path: profile.trainer.path.trim(),
    trainer_loading_mode: profile.trainer.loading_mode,
    launcher_icon_path: launcherIconPath.trim(),
    prefix_path:
      method === 'steam_applaunch' ? profile.steam.compatdata_path.trim() : profile.runtime.prefix_path.trim(),
    proton_path: method === 'steam_applaunch' ? profile.steam.proton_path.trim() : profile.runtime.proton_path.trim(),
    steam_app_id: profile.steam.app_id.trim(),
    steam_client_install_path: steamClientInstallPath.trim(),
    target_home_path: targetHomePath.trim(),
    profile_name: profileName.trim() || undefined,
    runtime_steam_app_id: profile.runtime.steam_app_id?.trim() ?? '',
    umu_game_id: profile.runtime.umu_game_id?.trim() ?? '',
    umu_preference: profile.runtime.umu_preference ?? globalUmuPreference,
    network_isolation: profile.launch.network_isolation ?? true,
    gamescope: profile.launch?.trainer_gamescope,
  };
}
