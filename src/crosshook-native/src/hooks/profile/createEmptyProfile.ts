import { DEFAULT_INJECTION_SECTION, type GameProfile } from '../../types';

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
    },
    launch: {
      method: 'proton_run',
      optimizations: {
        enabled_option_ids: [],
      },
      presets: {},
      active_preset: '',
      custom_env_vars: {},
    },
  };
}
