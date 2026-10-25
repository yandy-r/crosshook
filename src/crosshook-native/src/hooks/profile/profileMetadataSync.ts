import { callCommand } from '@/lib/ipc';
import type { AppSettingsData, GameProfile, RecentFilesData } from '../../types';
import { toSettingsSaveRequest } from '../../types/settings';
import { mergeRecentPaths } from './mergeRecentPaths';

/**
 * Persists the side effects of selecting a profile: `last_used_profile` in
 * settings and the game / trainer / DLL paths in recent files. Canonical
 * injection hooks stay ahead of manually added DLL paths.
 */
export async function syncProfileMetadata(name: string, currentProfile: GameProfile): Promise<void> {
  const settings = await callCommand<AppSettingsData>('settings_load');
  await callCommand('settings_save', {
    data: toSettingsSaveRequest({ ...settings, last_used_profile: name }),
  });

  const recentFiles = await callCommand<RecentFilesData>('recent_files_load');
  const canonicalDllPaths = currentProfile.injection.loaded_hooks.map((hook) => hook.path);
  const dllPaths = [
    ...canonicalDllPaths,
    ...currentProfile.injection.dll_paths.filter((dllPath) => !canonicalDllPaths.includes(dllPath)),
  ];
  await callCommand('recent_files_save', {
    data: {
      game_paths: mergeRecentPaths(recentFiles.game_paths, currentProfile.game.executable_path),
      trainer_paths: mergeRecentPaths(recentFiles.trainer_paths, currentProfile.trainer.path),
      dll_paths: dllPaths.reduce((paths, dllPath) => mergeRecentPaths(paths, dllPath), recentFiles.dll_paths),
    } satisfies RecentFilesData,
  });
}

/**
 * Clears `last_used_profile` when the deleted profile was the last-used one.
 * Only that concern lives here; the caller (`useProfileCrud`) owns refreshing
 * the list and selecting the next profile.
 */
export async function clearLastUsedProfileIfDeleted(name: string): Promise<void> {
  const settings = await callCommand<AppSettingsData>('settings_load');
  if (settings.last_used_profile !== name) {
    return;
  }
  await callCommand('settings_save', {
    data: toSettingsSaveRequest({ ...settings, last_used_profile: '' }),
  });
}
