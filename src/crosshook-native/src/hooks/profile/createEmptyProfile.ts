import { DEFAULT_INJECTION_SECTION, type GameProfile } from '../../types';
import { DEFAULT_LAUNCH_COMMAND_ARGUMENTS } from '../../types/launch-command-arguments';

export function createEmptyProfile(): GameProfile {
  return {
    game: {
      name: '',
      executable_path: '',
    },
    trainer: {
      path: '',
      type: '',
      loading_mode: 'source_directory',
    },
    injection: {
      ...DEFAULT_INJECTION_SECTION,
      dll_paths: [],
      inject_on_launch: [],
      loaded_hooks: [],
    },
    steam: {
      enabled: false,
      app_id: '',
      compatdata_path: '',
      proton_path: '',
      launcher: {
        icon_path: '',
        display_name: '',
      },
    },
    runtime: {
      prefix_path: '',
      proton_path: '',
      working_directory: '',
      steam_app_id: '',
      umu_game_id: '',
      umu_store: '',
      umu_codename: '',
    },
    launch: {
      method: 'proton_run',
      optimizations: {
        enabled_option_ids: [],
      },
      command_arguments: { ...DEFAULT_LAUNCH_COMMAND_ARGUMENTS },
      presets: {},
      active_preset: '',
      custom_env_vars: {},
    },
  };
}
