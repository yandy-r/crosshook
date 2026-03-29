export interface CommunityTapSubscription {
  url: string;
  branch?: string;
  pinned_commit?: string;
}

export interface AppSettingsData {
  auto_load_last_profile: boolean;
  last_used_profile: string;
  community_taps: CommunityTapSubscription[];
}

export interface RecentFilesData {
  game_paths: string[];
  trainer_paths: string[];
  dll_paths: string[];
}
