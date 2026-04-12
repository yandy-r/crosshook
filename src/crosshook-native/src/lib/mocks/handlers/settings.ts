import type { RecentFilesData } from '../../../types';
import type { SettingsSaveRequest } from '../../../types/settings';
import { getStore } from '../store';
import type { Handler } from './types';

export function registerSettings(map: Map<string, Handler>): void {
  map.set('settings_load', async () => structuredClone(getStore().settings));

  map.set('settings_save', async (args) => {
    const next = (args as { data: SettingsSaveRequest }).data;
    const merged = { ...getStore().settings, ...next };
    getStore().settings = structuredClone(merged);
    return structuredClone(getStore().settings);
  });

  map.set('settings_save_steamgriddb_key', async (args) => {
    const { key } = args as { key: string | null };
    getStore().settings = {
      ...getStore().settings,
      has_steamgriddb_api_key: key !== null && key.trim().length > 0,
    };
    return null;
  });

  map.set('recent_files_load', async () => structuredClone(getStore().recentFiles));

  map.set('recent_files_save', async (args) => {
    const next = (args as { data: RecentFilesData }).data;
    const copy = structuredClone(next);
    getStore().recentFiles = copy;
    return structuredClone(getStore().recentFiles);
  });

  map.set('default_steam_client_install_path', async () => getStore().defaultSteamClientInstallPath);
}
