import type { GameProfile } from '../types/profile';

/**
 * Returns the effective app ID for art/metadata resolution.
 * Prefers steam.app_id, falls back to runtime.steam_app_id.
 */
export function resolveArtAppId(profile: GameProfile): string {
  const steamAppId = profile.steam?.app_id?.trim();
  if (steamAppId) return steamAppId;
  return profile.runtime?.steam_app_id?.trim() ?? '';
}

/**
 * Validates a Steam App ID value.
 * Must be empty (not set) or 1-12 ASCII decimal digits.
 */
export function validateSteamAppId(value: string): boolean {
  if (value === '') return true;
  return /^\d{1,12}$/.test(value);
}
