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
    /** Winetricks/protontricks verbs required by this trainer. */
    required_protontricks?: string[];
    /** Optional digest from community profile manifest (advisory at launch). */
    community_trainer_sha256?: string;
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
    /** When true, trainer processes are launched in an isolated network namespace via `unshare --net`. */
    network_isolation?: boolean;
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
      extra_protontricks?: string[];
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

export interface SerializedLocalOverrideSection {
  game?: Partial<NonNullable<GameProfile['local_override']>['game']>;
  trainer?: Partial<NonNullable<GameProfile['local_override']>['trainer']>;
  steam?: Partial<NonNullable<GameProfile['local_override']>['steam']>;
  runtime?: Partial<NonNullable<GameProfile['local_override']>['runtime']>;
}

export interface SerializedGameProfile extends Omit<GameProfile, 'runtime' | 'local_override'> {
  runtime?: Partial<GameProfile['runtime']>;
  local_override?: SerializedLocalOverrideSection;
}

const DEFAULT_RUNTIME_SECTION: GameProfile['runtime'] = {
  prefix_path: '',
  proton_path: '',
  working_directory: '',
  steam_app_id: '',
};

const DEFAULT_LOCAL_OVERRIDE_SECTION: NonNullable<GameProfile['local_override']> = {
  game: {
    executable_path: '',
    custom_cover_art_path: '',
    custom_portrait_art_path: '',
    custom_background_art_path: '',
  },
  trainer: {
    path: '',
    extra_protontricks: [],
  },
  steam: {
    compatdata_path: '',
    proton_path: '',
  },
  runtime: {
    prefix_path: '',
    proton_path: '',
  },
};

const DEFAULT_LAUNCH_SECTION: GameProfile['launch'] = {
  method: '',
  optimizations: {
    enabled_option_ids: [],
  },
  presets: {},
  active_preset: '',
  custom_env_vars: {},
  network_isolation: true,
};

export function normalizeSerializedGameProfile(profile: SerializedGameProfile): GameProfile {
  return {
    ...profile,
    game: {
      ...profile.game,
      custom_cover_art_path: profile.game.custom_cover_art_path ?? '',
      custom_portrait_art_path: profile.game.custom_portrait_art_path ?? '',
      custom_background_art_path: profile.game.custom_background_art_path ?? '',
    },
    trainer: {
      ...profile.trainer,
      trainer_type: profile.trainer.trainer_type ?? 'unknown',
      required_protontricks: [...(profile.trainer.required_protontricks ?? [])],
      community_trainer_sha256: profile.trainer.community_trainer_sha256 ?? '',
    },
    injection: {
      ...profile.injection,
      dll_paths: [...profile.injection.dll_paths],
      inject_on_launch: [...profile.injection.inject_on_launch],
    },
    steam: {
      ...profile.steam,
      launcher: {
        icon_path: profile.steam.launcher?.icon_path ?? '',
        display_name: profile.steam.launcher?.display_name ?? '',
      },
    },
    runtime: {
      ...DEFAULT_RUNTIME_SECTION,
      ...(profile.runtime ?? {}),
    },
    launch: {
      ...DEFAULT_LAUNCH_SECTION,
      ...profile.launch,
      optimizations: {
        enabled_option_ids: [...(profile.launch.optimizations?.enabled_option_ids ?? [])],
      },
      presets: { ...(profile.launch.presets ?? {}) },
      custom_env_vars: { ...(profile.launch.custom_env_vars ?? {}) },
    },
    local_override: {
      game: {
        ...DEFAULT_LOCAL_OVERRIDE_SECTION.game,
        ...(profile.local_override?.game ?? {}),
      },
      trainer: {
        ...DEFAULT_LOCAL_OVERRIDE_SECTION.trainer,
        ...(profile.local_override?.trainer ?? {}),
        extra_protontricks: [...(profile.local_override?.trainer?.extra_protontricks ?? [])],
      },
      steam: {
        ...DEFAULT_LOCAL_OVERRIDE_SECTION.steam,
        ...(profile.local_override?.steam ?? {}),
      },
      runtime: {
        ...DEFAULT_LOCAL_OVERRIDE_SECTION.runtime,
        ...(profile.local_override?.runtime ?? {}),
      },
    },
  };
}

/** Empty profile for install / wizard drafts; matches normalized editor shape. */
export function createDefaultProfile(): GameProfile {
  return normalizeSerializedGameProfile({
    game: { name: '', executable_path: '' },
    trainer: {
      path: '',
      type: '',
      loading_mode: 'source_directory',
    },
    injection: { dll_paths: [], inject_on_launch: [] },
    steam: {
      enabled: false,
      app_id: '',
      compatdata_path: '',
      proton_path: '',
      launcher: { icon_path: '', display_name: '' },
    },
    launch: {
      method: 'proton_run',
      optimizations: { enabled_option_ids: [] },
      custom_env_vars: {},
    },
  });
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

export interface SerializedDuplicateProfileResult extends Omit<DuplicateProfileResult, 'profile'> {
  profile: SerializedGameProfile;
}

/**
 * Collection-scoped overrides for the editable subset of `LaunchSection`.
 *
 * Mirrors the Rust `CollectionDefaultsSection` struct in
 * `crates/crosshook-core/src/profile/models.rs`. The field set matches what the
 * inline editor inside `<CollectionViewModal>` exposes; users wanting to edit
 * `presets` / `active_preset` use the "Open in Profiles page →" link-out.
 *
 * Semantics:
 * - `undefined` (or absent) for any optional field means "inherit from the
 *   profile's base value" — the merge layer leaves it untouched.
 * - `custom_env_vars` is an **additive merge**: collection entries union with
 *   the profile's `launch.custom_env_vars` and the collection key wins on
 *   collision.
 *
 * Tauri serializes Rust `Option<T>` to `undefined`/missing JSON fields, so use
 * `?` (optional) here, never `| null`.
 */
export interface CollectionDefaults {
  method?: LaunchMethod;
  optimizations?: LaunchOptimizations;
  custom_env_vars?: Record<string, string>;
  network_isolation?: boolean;
  gamescope?: GamescopeConfig;
  trainer_gamescope?: GamescopeConfig;
  mangohud?: MangoHudConfig;
}

/** Returns true when no field of `d` would influence the merge layer. */
export function isCollectionDefaultsEmpty(d: CollectionDefaults | null | undefined): boolean {
  if (!d) return true;
  const methodUnset = d.method === undefined || (typeof d.method === 'string' && d.method.trim() === '');
  return (
    methodUnset &&
    d.optimizations === undefined &&
    (d.custom_env_vars === undefined || Object.keys(d.custom_env_vars).length === 0) &&
    d.network_isolation === undefined &&
    d.gamescope === undefined &&
    d.trainer_gamescope === undefined &&
    d.mangohud === undefined
  );
}
