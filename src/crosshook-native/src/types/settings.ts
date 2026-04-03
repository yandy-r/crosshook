export interface CommunityTapSubscription {
  url: string;
  branch?: string;
  pinned_commit?: string;
}

export interface AppSettingsData {
  auto_load_last_profile: boolean;
  last_used_profile: string;
  community_taps: CommunityTapSubscription[];
  onboarding_completed?: boolean;
  offline_mode?: boolean;
  /** True when a SteamGridDB API key is stored on the backend. The raw key is never sent to the frontend. */
  has_steamgriddb_api_key: boolean;
}

export interface RecentFilesData {
  game_paths: string[];
  trainer_paths: string[];
  dll_paths: string[];
}
