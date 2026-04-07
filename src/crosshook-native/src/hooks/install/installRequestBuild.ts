import type { GameProfile, LaunchMethod } from '../../types/profile';
import type { InstallGameRequest } from '../../types/install';

export function buildInstallGameRequest(
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
