import type { LaunchRequest } from '../types/launch';
import { DEFAULT_LAUNCH_COMMAND_ARGUMENTS } from '../types/launch-command-arguments';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG } from '../types/profile';
import type { RunExecutableRequest } from '../types/run-executable';
import type { UmuPreference } from '../types/settings';

/**
 * Maps an ad-hoc Run EXE/MSI form state onto a `preview_launch` request.
 * Returns null while the form lacks the two required inputs (executable + Proton path).
 */
export function buildRunExecutablePreviewRequest(
  request: RunExecutableRequest,
  umuPreference: UmuPreference
): LaunchRequest | null {
  const executablePath = request.executable_path.trim();
  const protonPath = request.proton_path.trim();
  if (executablePath === '' || protonPath === '') {
    return null;
  }
  return {
    method: 'proton_run',
    game_path: executablePath,
    trainer_path: '',
    trainer_host_path: '',
    trainer_loading_mode: 'source_directory',
    steam: {
      app_id: '',
      compatdata_path: '',
      proton_path: '',
      steam_client_install_path: request.steam_client_install_path.trim(),
    },
    runtime: {
      prefix_path: request.prefix_path.trim(),
      proton_path: protonPath,
      working_directory: request.working_directory.trim(),
    },
    optimizations: { enabled_option_ids: [] },
    command_arguments: { ...DEFAULT_LAUNCH_COMMAND_ARGUMENTS },
    launch_trainer_only: false,
    launch_game_only: true,
    custom_env_vars: {},
    network_isolation: false,
    umu_preference: umuPreference,
    gamescope: { ...DEFAULT_GAMESCOPE_CONFIG, extra_args: [] },
    mangohud: { ...DEFAULT_MANGOHUD_CONFIG },
  };
}
