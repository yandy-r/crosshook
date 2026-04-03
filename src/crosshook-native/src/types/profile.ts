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

export type GamescopeFilter = 'fsr' | 'nis' | 'linear' | 'nearest' | 'pixel';

export interface GamescopeConfig {
  enabled: boolean;
  internal_width?: number;
  internal_height?: number;
  output_width?: number;
  output_height?: number;
  frame_rate_limit?: number;
  fsr_sharpness?: number;
  upscale_filter?: GamescopeFilter;
  fullscreen: boolean;
  borderless: boolean;
  grab_cursor: boolean;
  force_grab_cursor: boolean;
  hdr_enabled: boolean;
  allow_nested: boolean;
  extra_args: string[];
}

export const DEFAULT_GAMESCOPE_CONFIG: GamescopeConfig = {
  enabled: false,
  fullscreen: false,
  borderless: false,
  grab_cursor: false,
  force_grab_cursor: false,
  hdr_enabled: false,
  allow_nested: false,
  extra_args: [],
};

export type MangoHudPosition =
  | 'top-left'
  | 'top-right'
  | 'bottom-left'
  | 'bottom-right'
  | 'top-center'
  | 'bottom-center';

export interface MangoHudConfig {
  enabled: boolean;
  fps_limit?: number;
  gpu_stats: boolean;
  cpu_stats: boolean;
  ram: boolean;
  frametime: boolean;
  battery: boolean;
  watt: boolean;
  position?: MangoHudPosition;
}

export const DEFAULT_MANGOHUD_CONFIG: MangoHudConfig = {
  enabled: false,
  gpu_stats: false,
  cpu_stats: false,
  ram: false,
  frametime: false,
  battery: false,
  watt: false,
};

/** IPC DTO from `profile_list_bundled_optimization_presets`. */
export interface BundledOptimizationPreset {
  preset_id: string;
  display_name: string;
  vendor: string;
  mode: string;
  enabled_option_ids: string[];
  catalog_version: number;
}
export type TrainerLoadingMode = 'source_directory' | 'copy_to_prefix';

export interface GameProfile {
  game: {
    name: string;
    executable_path: string;
    custom_cover_art_path?: string;
    custom_portrait_art_path?: string;
    custom_background_art_path?: string;
  };
  trainer: {
    path: string;
    type: string;
    /** Catalog id (`standalone`, `aurora`, …); omitted in TOML when `unknown`. */
    trainer_type?: string;
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
    steam_app_id?: string;
  };
  launch: {
    method: LaunchMethod;
    optimizations: LaunchOptimizations;
    /** Named optimization bundles (`[launch.presets.<name>]` in profile TOML). */
    presets?: Record<string, LaunchOptimizations>;
    /** When set and present in `presets`, optimizations are kept in sync with that entry. */
    active_preset?: string;
    custom_env_vars: Record<string, string>;
    gamescope?: GamescopeConfig;
    trainer_gamescope?: GamescopeConfig;
    mangohud?: MangoHudConfig;
  };
  local_override?: {
    game: {
      executable_path: string;
      custom_cover_art_path?: string;
      custom_portrait_art_path?: string;
      custom_background_art_path?: string;
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
