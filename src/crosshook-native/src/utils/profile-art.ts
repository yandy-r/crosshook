import type { GameProfile } from '../types/profile';

type CustomArtKey = 'custom_portrait_art_path' | 'custom_background_art_path';

/**
 * Effective custom art path: local override wins over base profile game paths.
 */
export function effectiveGameArtPath(profile: GameProfile | null | undefined, key: CustomArtKey): string | undefined {
  if (!profile) {
    return undefined;
  }
  const overrideVal = profile.local_override?.game?.[key];
  const trimmedOverride = typeof overrideVal === 'string' ? overrideVal.trim() : '';
  if (trimmedOverride) {
    return trimmedOverride;
  }
  const baseVal = profile.game[key];
  const trimmedBase = typeof baseVal === 'string' ? baseVal.trim() : '';
  return trimmedBase || undefined;
}
