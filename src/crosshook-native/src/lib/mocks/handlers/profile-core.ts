// Core profile handlers: list, list_summaries, list_favorites, load.
// See `lib/mocks/README.md`.
// All error messages MUST start with `[dev-mock]` to participate in the
// `.github/workflows/release.yml` "Verify no mock code in production bundle"
// sentinel.

import type { ProfileSummary } from '../../../types/library';
import type { GameProfile } from '../../../types/profile';
import { createDefaultProfile } from '../../../types/profile';
import { getActiveFixture } from '../../fixture';
import { getStore } from '../store';
import { getMockCollectionDefaults } from './collections';
import { applyMockCollectionDefaults, forcedError, neverResolving } from './profile-utils';
import type { Handler } from './types';

// Module-scope state for features not tracked in MockStore
export const profileFavorites = new Set<string>();

/** Seeded demo profiles — 8 entries (two rows × 4 in the library grid). Steam app IDs ≥ 9999001 per README. */
const DEMO_PROFILE_SEEDS: ReadonlyArray<{ name: string; appId: string }> = [
  { name: 'Test Game Alpha', appId: '9999001' },
  { name: 'Dev Game Beta', appId: '9999002' },
  { name: 'Sample Game Gamma', appId: '9999003' },
  { name: 'Sample Game Delta', appId: '9999004' },
  { name: 'Sample Game Epsilon', appId: '9999005' },
  { name: 'Sample Game Zeta', appId: '9999006' },
  { name: 'Sample Game Eta', appId: '9999007' },
  { name: 'Sample Game Theta', appId: '9999008' },
];

export function seedDemoProfiles(): void {
  const store = getStore();
  if (store.profiles.size > 0) return;

  const base = createDefaultProfile();

  for (const { name, appId } of DEMO_PROFILE_SEEDS) {
    const profile: GameProfile = {
      ...base,
      game: {
        ...base.game,
        name,
        custom_cover_art_path: '',
        custom_portrait_art_path: '',
        custom_background_art_path: '',
      },
      steam: {
        ...base.steam,
        app_id: appId,
      },
    };
    store.profiles.set(name, profile);
  }

  store.activeProfileId = DEMO_PROFILE_SEEDS[0].name;
}

export function registerProfileCore(map: Map<string, Handler>): void {
  // profile_list — SHELL-CRITICAL (BR-11): always resolves with populated
  // data (or `empty` returns []) so `<AppShell />` can render under every
  // fixture state, including `error` and `loading`.
  map.set('profile_list', async () => {
    const fixture = getActiveFixture();
    if (fixture === 'empty') return [];
    seedDemoProfiles();
    return Array.from(getStore().profiles.keys());
  });

  // profile_list_summaries — SHELL-CRITICAL (BR-11): same rationale as above.
  map.set('profile_list_summaries', async (args): Promise<ProfileSummary[]> => {
    const fixture = getActiveFixture();
    if (fixture === 'empty') return [];
    seedDemoProfiles();
    const { collectionId } = (args ?? {}) as { collectionId?: string };
    return Array.from(getStore().profiles.entries()).map(([name, p]) => {
      let profile = p;
      if (collectionId !== undefined && collectionId !== null && collectionId.trim() !== '') {
        const defaults = getMockCollectionDefaults(collectionId);
        if (defaults) {
          profile = applyMockCollectionDefaults(p, defaults);
        }
      }
      return {
        name,
        gameName: profile.game.name,
        steamAppId: profile.steam.app_id,
        customCoverArtPath: profile.game.custom_cover_art_path,
        customPortraitArtPath: profile.game.custom_portrait_art_path,
        networkIsolation: profile.launch.network_isolation ?? true,
      };
    });
  });

  map.set('profile_list_favorites', async () => {
    const fixture = getActiveFixture();
    if (fixture === 'empty') return [];
    if (fixture === 'loading') return neverResolving<string[]>();
    // `error` is allowed to resolve here — favorites are non-fatal and the UI
    // can render an empty favorites strip in error state.
    seedDemoProfiles();
    return Array.from(profileFavorites).filter((n) => getStore().profiles.has(n));
  });

  map.set('profile_load', async (args) => {
    const fixture = getActiveFixture();
    if (fixture === 'empty') return null;
    if (fixture === 'loading') return neverResolving<unknown>();
    if (fixture === 'error') throw forcedError('profile_load');
    seedDemoProfiles();
    const { name, collectionId } = args as { name: string; collectionId?: string };
    const profile = getStore().profiles.get(name) ?? null;
    if (profile === null) return null;
    // No collection context → return raw storage profile (matches Rust shim path).
    if (collectionId === undefined || collectionId === null || collectionId.trim() === '') {
      return profile;
    }
    const defaults = getMockCollectionDefaults(collectionId);
    if (!defaults) return profile;
    return applyMockCollectionDefaults(profile, defaults);
  });
}
