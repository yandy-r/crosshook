export interface AppSettingsData {
  auto_load_last_profile: boolean;
  last_used_profile: string;
}

export interface RecentFilesData {
  game_paths: string[];
  trainer_paths: string[];
  dll_paths: string[];
}
