import type { GameProfile } from '../../types/profile';

/**
 * Merges backend install result into the user's draft: keeps launch, injection,
 * local overrides, and user-edited identity/art; applies authoritative install
 * fields from `next`.
 */
export function mergeInstallGameResultIntoDraft(current: GameProfile, next: GameProfile): GameProfile {
  return {
    ...next,
    launch: current.launch,
    injection: current.injection,
    local_override: current.local_override,
    game: {
      ...next.game,
      name: current.game.name.trim() ? current.game.name : next.game.name,
      custom_cover_art_path: current.game.custom_cover_art_path?.trim()
        ? current.game.custom_cover_art_path
        : next.game.custom_cover_art_path,
      custom_portrait_art_path: current.game.custom_portrait_art_path?.trim()
        ? current.game.custom_portrait_art_path
        : next.game.custom_portrait_art_path,
      custom_background_art_path: current.game.custom_background_art_path?.trim()
        ? current.game.custom_background_art_path
        : next.game.custom_background_art_path,
    },
    trainer: {
      ...current.trainer,
      path: next.trainer.path.trim() ? next.trainer.path : current.trainer.path,
    },
  };
}
