import type { GameProfile } from '../types';

/** Structural equality for profile drafts (install review, etc.). */
export function profilesEqual(a: GameProfile, b: GameProfile): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}
