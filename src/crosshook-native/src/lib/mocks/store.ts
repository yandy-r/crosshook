import type { AppSettingsData, RecentFilesData } from '../../types';
import type { GameProfile } from '../../types/profile';
import { DEFAULT_APP_SETTINGS } from '../../types/settings';
// createDefaultProfile is imported only when seeding profiles in handlers/profile.ts (Task 1.14)

export interface MockStore {
  settings: AppSettingsData;
  recentFiles: RecentFilesData;
  profiles: Map<string, GameProfile>;
  activeProfileId: string | null;
  defaultSteamClientInstallPath: string;
}

const EMPTY_RECENT_FILES: RecentFilesData = {
  game_paths: [],
  trainer_paths: [],
  dll_paths: [],
};

let store: MockStore | null = null;

export function getStore(): MockStore {
  if (!store) {
    store = {
      settings: { ...DEFAULT_APP_SETTINGS },
      recentFiles: structuredClone(EMPTY_RECENT_FILES),
      profiles: new Map(),
      activeProfileId: null,
      defaultSteamClientInstallPath: '/home/devuser/.steam/steam',
    };
  }
  return store;
}

export function resetStore(): void {
  store = null;
}
