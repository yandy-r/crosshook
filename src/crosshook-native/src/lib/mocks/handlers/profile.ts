import type { Handler } from '../index';
import { getStore } from '../store';
import type { ProfileSummary } from '../../../types/library';
import type { GameProfile } from '../../../types/profile';
import { createDefaultProfile } from '../../../types/profile';

function seedDemoProfiles(): void {
  const store = getStore();
  if (store.profiles.size > 0) return;

  const base = createDefaultProfile();

  const alpha: GameProfile = {
    ...base,
    game: {
      ...base.game,
      name: 'Test Game Alpha',
      custom_cover_art_path: '',
      custom_portrait_art_path: '',
      custom_background_art_path: '',
    },
    steam: {
      ...base.steam,
      app_id: '9999001',
    },
  };

  const beta: GameProfile = {
    ...base,
    game: {
      ...base.game,
      name: 'Dev Game Beta',
      custom_cover_art_path: '',
      custom_portrait_art_path: '',
      custom_background_art_path: '',
    },
    steam: {
      ...base.steam,
      app_id: '9999002',
    },
  };

  store.profiles.set(alpha.game.name, alpha);
  store.profiles.set(beta.game.name, beta);
  store.activeProfileId = alpha.game.name;
}

export function registerProfile(map: Map<string, Handler>): void {
  map.set('profile_list', async () => {
    seedDemoProfiles();
    return Array.from(getStore().profiles.keys());
  });

  map.set('profile_list_summaries', async (): Promise<ProfileSummary[]> => {
    seedDemoProfiles();
    return Array.from(getStore().profiles.values()).map((p) => ({
      name: p.game.name,
      gameName: p.game.name,
      steamAppId: p.steam.app_id,
      customCoverArtPath: p.game.custom_cover_art_path,
      customPortraitArtPath: p.game.custom_portrait_art_path,
    }));
  });

  map.set('profile_list_favorites', async () => {
    seedDemoProfiles();
    return [];
  });

  map.set('profile_load', async (args) => {
    seedDemoProfiles();
    const { name } = args as { name: string };
    return getStore().profiles.get(name) ?? null;
  });
}
