import type { GameProfile } from '../../types';
import { automaticLauncherSuffix } from './constants';

export function stripAutomaticLauncherSuffix(value: string): string {
  const trimmed = value.trim();
  return trimmed.endsWith(automaticLauncherSuffix)
    ? trimmed.slice(0, -automaticLauncherSuffix.length).trimEnd()
    : trimmed;
}

function deriveDisplayNameFromPath(path: string): string {
  const normalized = path.trim();
  if (!normalized) {
    return '';
  }

  const segment = normalized.split(/[\\/]/).pop() ?? '';
  return segment.replace(/\.[^.]+$/, '').trim();
}

export function deriveGameName(profile: GameProfile): string {
  return profile.game.name.trim() || deriveDisplayNameFromPath(profile.game.executable_path);
}

export function deriveLauncherDisplayName(profile: GameProfile): string {
  return (
    stripAutomaticLauncherSuffix(profile.steam.launcher.display_name) ||
    deriveGameName(profile) ||
    stripAutomaticLauncherSuffix(deriveDisplayNameFromPath(profile.trainer.path))
  );
}
