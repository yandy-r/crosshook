import type { GameProfile } from '../../types';

export function validateProfileForSave(profile: GameProfile): string | null {
  if (!profile.game.executable_path.trim()) {
    return 'Game executable path is required before saving a profile.';
  }

  return null;
}
