import type { ExternalTrainerSourceSubscription } from './discovery';

export type { ExternalTrainerSourceSubscription } from './discovery';

export interface CommunityTapSubscription {
  url: string;
  branch?: string;
  pinned_commit?: string;
}

/** Payload for `settings_save` (matches Rust `SettingsSaveRequest`). */
export interface SettingsSaveRequest {
  auto_load_last_profile: boolean;
  last_used_profile: string;
  community_taps: CommunityTapSubscription[];
  onboarding_completed: boolean;
  offline_mode: boolean;
  default_proton_path: string;
  default_launch_method: string;
  default_bundled_optimization_preset_id: string;
  default_trainer_loading_mode: string;
  log_filter: string;
  console_drawer_collapsed_default: boolean;
  recent_files_limit: number;
  profiles_directory: string;
  protontricks_binary_path: string;
  auto_install_prefix_deps: boolean;
  discovery_enabled: boolean;
  external_trainer_sources?: ExternalTrainerSourceSubscription[];
}

export interface AppSettingsData extends SettingsSaveRequest {
  /** True when a SteamGridDB API key is stored on the backend. The raw key is never sent to the frontend. */
  has_steamgriddb_api_key: boolean;
  /** Resolved from settings (may differ from active until restart). */
  resolved_profiles_directory: string;
  active_profiles_directory: string;
  profiles_directory_requires_restart: boolean;
}

export function toSettingsSaveRequest(s: AppSettingsData): SettingsSaveRequest {
  return {
    auto_load_last_profile: s.auto_load_last_profile,
    last_used_profile: s.last_used_profile,
    community_taps: s.community_taps,
    onboarding_completed: s.onboarding_completed,
    offline_mode: s.offline_mode,
    default_proton_path: s.default_proton_path,
    default_launch_method: s.default_launch_method,
    default_bundled_optimization_preset_id: s.default_bundled_optimization_preset_id,
    default_trainer_loading_mode: s.default_trainer_loading_mode,
    log_filter: s.log_filter,
    console_drawer_collapsed_default: s.console_drawer_collapsed_default,
    recent_files_limit: s.recent_files_limit,
    profiles_directory: s.profiles_directory,
    protontricks_binary_path: s.protontricks_binary_path,
    auto_install_prefix_deps: s.auto_install_prefix_deps,
    discovery_enabled: s.discovery_enabled,
    external_trainer_sources: s.external_trainer_sources,
  };
}

export const DEFAULT_APP_SETTINGS: AppSettingsData = {
  auto_load_last_profile: false,
  last_used_profile: '',
  community_taps: [],
  onboarding_completed: false,
  offline_mode: false,
  has_steamgriddb_api_key: false,
  default_proton_path: '',
  default_launch_method: '',
  default_bundled_optimization_preset_id: '',
  default_trainer_loading_mode: 'source_directory',
  log_filter: 'info',
  console_drawer_collapsed_default: true,
  recent_files_limit: 10,
  profiles_directory: '',
  resolved_profiles_directory: '',
  active_profiles_directory: '',
  profiles_directory_requires_restart: false,
  protontricks_binary_path: '',
  auto_install_prefix_deps: false,
  discovery_enabled: false,
  external_trainer_sources: [],
};

export interface RecentFilesData {
  game_paths: string[];
  trainer_paths: string[];
  dll_paths: string[];
}
