import type { Handler } from '../index';
import { getStore } from '../store';
import type { AppSettingsData, RecentFilesData } from '../../../types';
import type { SettingsSaveRequest } from '../../../types/settings';

export function registerSettings(map: Map<string, Handler>): void {
  map.set('settings_load', async () => getStore().settings);

  map.set('settings_save', async (args) => {
    const next = (args as { data: SettingsSaveRequest }).data;
    getStore().settings = { ...getStore().settings, ...next };
    return getStore().settings;
  });

  map.set('settings_save_steamgriddb_key', async (args) => {
    const { key } = args as { key: string | null };
    getStore().settings = {
      ...getStore().settings,
      has_steamgriddb_api_key: key !== null && key.trim().length > 0,
    };
    return null;
  });

  map.set('recent_files_load', async () => getStore().recentFiles);

  map.set('recent_files_save', async (args) => {
    const next = (args as { data: RecentFilesData }).data;
    getStore().recentFiles = next;
    return next;
  });

  map.set('default_steam_client_install_path', async () => getStore().defaultSteamClientInstallPath);
}
