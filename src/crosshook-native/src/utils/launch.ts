import type { GameProfile } from '../types';

export type ResolvedLaunchMethod = Exclude<GameProfile['launch']['method'], ''>;

export function looksLikeWindowsExecutable(path: string): boolean {
  return path.trim().toLowerCase().endsWith('.exe');
}

export function resolveLaunchMethod(profile: GameProfile): ResolvedLaunchMethod {
  const method = profile.launch.method.trim();

  if (method === 'steam_applaunch' || method === 'proton_run' || method === 'native') {
    return method;
  }

  if (profile.steam.enabled) {
    return 'steam_applaunch';
  }

  if (looksLikeWindowsExecutable(profile.game.executable_path)) {
    return 'proton_run';
  }

  return 'native';
}
