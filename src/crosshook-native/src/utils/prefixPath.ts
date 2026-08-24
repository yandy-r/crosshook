import type { GameProfile } from '@/types/profile';

/** Resolve the effective Wine prefix while treating blank override paths as absent. */
export function resolveEffectivePrefixPath(profile: Pick<GameProfile, 'runtime' | 'steam'>): string {
  return profile.runtime?.prefix_path?.trim() || profile.steam?.compatdata_path?.trim() || '';
}
