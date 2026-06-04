import type { GameProfile } from '../../types/profile';

export interface ProfileCreateSeed {
  /** Pre-fills the wizard profile name field (not a GameProfile field). */
  suggestedName?: string;
  /** → game.name */
  gameName?: string;
  /** → steam.app_id + runtime.steam_app_id (numeric digits only, 1–12 chars). */
  steamAppId?: string;
  /** → game.executable_path */
  executablePath?: string;
  /** → game.custom_cover_art_path */
  coverArtPath?: string;
  /** → game.custom_portrait_art_path */
  portraitArtPath?: string;
}

/** Regex that accepts 1–12 digit-only Steam App IDs. */
const NUMERIC_APP_ID_RE = /^\d{1,12}$/;

/**
 * Pure function. Shallow-merges seed fields into a blank GameProfile draft.
 * Only non-empty seed values are applied — empty strings / undefined are skipped.
 * The input profile is never mutated; a new object is always returned.
 */
export function applyCreateSeed(profile: GameProfile, seed: ProfileCreateSeed): GameProfile {
  // game section overrides
  const gamePatch: Partial<GameProfile['game']> = {};
  if (seed.gameName) gamePatch.name = seed.gameName;
  if (seed.executablePath) gamePatch.executable_path = seed.executablePath;
  if (seed.coverArtPath) gamePatch.custom_cover_art_path = seed.coverArtPath;
  if (seed.portraitArtPath) gamePatch.custom_portrait_art_path = seed.portraitArtPath;

  // steam + runtime section overrides (only when appId is numeric 1–12 digits)
  const steamPatch: Partial<GameProfile['steam']> = {};
  const runtimePatch: Partial<GameProfile['runtime']> = {};
  if (seed.steamAppId && NUMERIC_APP_ID_RE.test(seed.steamAppId)) {
    steamPatch.app_id = seed.steamAppId;
    steamPatch.enabled = true;
    runtimePatch.steam_app_id = seed.steamAppId;
  }

  const hasGamePatch = Object.keys(gamePatch).length > 0;
  const hasSteamPatch = Object.keys(steamPatch).length > 0;
  const hasRuntimePatch = Object.keys(runtimePatch).length > 0;

  if (!hasGamePatch && !hasSteamPatch && !hasRuntimePatch) {
    // Empty seed — return a new object to satisfy the "never mutate" contract
    // while avoiding unnecessary deep copies.
    return { ...profile };
  }

  return {
    ...profile,
    game: hasGamePatch ? { ...profile.game, ...gamePatch } : profile.game,
    steam: hasSteamPatch ? { ...profile.steam, ...steamPatch } : profile.steam,
    runtime: hasRuntimePatch ? { ...profile.runtime, ...runtimePatch } : profile.runtime,
  };
}
