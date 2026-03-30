import type { LaunchOptimizations } from './launch-optimizations';

export interface ProfileData {
  GamePath: string;
  TrainerPath: string;
  Dll1Path: string;
  Dll2Path: string;
  LaunchInject1: boolean;
  LaunchInject2: boolean;
  LaunchMethod: string;
  UseSteamMode: boolean;
  SteamAppId: string;
  SteamCompatDataPath: string;
  SteamProtonPath: string;
  SteamLauncherIconPath: string;
}

export type LaunchMethod = '' | 'steam_applaunch' | 'proton_run' | 'native';
export type TrainerLoadingMode = 'source_directory' | 'copy_to_prefix';

export interface GameProfile {
  game: {
    name: string;
    executable_path: string;
  };
  trainer: {
    path: string;
    type: string;
    loading_mode: TrainerLoadingMode;
  };
  injection: {
    dll_paths: string[];
    inject_on_launch: boolean[];
  };
  steam: {
    enabled: boolean;
    app_id: string;
    compatdata_path: string;
    proton_path: string;
    launcher: {
      icon_path: string;
      display_name: string;
    };
  };
  runtime: {
    prefix_path: string;
    proton_path: string;
    working_directory: string;
  };
  launch: {
    method: LaunchMethod;
    optimizations: LaunchOptimizations;
    /** Named optimization bundles (`[launch.presets.<name>]` in profile TOML). */
    presets?: Record<string, LaunchOptimizations>;
    /** When set and present in `presets`, optimizations are kept in sync with that entry. */
    active_preset?: string;
  };
  local_override?: {
    game: {
      executable_path: string;
    };
    trainer: {
      path: string;
    };
    steam: {
      compatdata_path: string;
      proton_path: string;
    };
    runtime: {
      prefix_path: string;
      proton_path: string;
    };
  };
}

/**
 * IPC result from the `profile_duplicate` Tauri command.
 *
 * Mirrors the Rust `DuplicateProfileResult` struct in
 * `crosshook-core/src/profile/toml_store.rs`. Both sides must stay in sync --
 * field names use snake_case to match serde serialization.
 */
export interface DuplicateProfileResult {
  /** Generated unique name for the duplicate (e.g. "MyGame (Copy)", "MyGame (Copy 2)"). */
  name: string;
  /** Full clone of the source profile's data. */
  profile: GameProfile;
}
