import type { Handler } from '../index';
import type { SteamMetadataLookupResult } from '../../../types/game-metadata';

// Synthetic Steam auto-populate result shapes (BR-10 / W-3)
type SteamFieldState = 'Idle' | 'Saved' | 'NotFound' | 'Found' | 'Ambiguous';

interface SteamAutoPopulateResult {
  app_id_state: SteamFieldState;
  app_id: string;
  compatdata_state: SteamFieldState;
  compatdata_path: string;
  proton_state: SteamFieldState;
  proton_path: string;
  diagnostics: string[];
  manual_hints: string[];
}

// Synthetic metadata for the seeded "Test Game Alpha" profile (app_id 9999001).
// Any other app_id returns an unavailable result — cover art fetches return null.
const SEEDED_APP_ID = '9999001';

const SEEDED_METADATA: SteamMetadataLookupResult = {
  app_id: SEEDED_APP_ID,
  state: 'ready',
  app_details: {
    name: 'Test Game Alpha',
    short_description:
      'A synthetic action-RPG used exclusively for CrossHook browser-mode development.',
    header_image: null,
    genres: [
      { id: '1', description: 'Action' },
      { id: '25', description: 'Adventure' },
    ],
  },
  from_cache: true,
  is_stale: false,
};

function unavailableMetadata(appId: string): SteamMetadataLookupResult {
  return {
    app_id: appId,
    state: 'unavailable',
    app_details: null,
    from_cache: false,
    is_stale: false,
  };
}

export function registerLibrary(map: Map<string, Handler>): void {
  // fetch_game_cover_art(appId, imageType) → string | null
  // convertFileSrc passthrough means callers fall back to placeholder — returning null is correct.
  map.set('fetch_game_cover_art', async (_args): Promise<string | null> => {
    return null;
  });

  // fetch_game_metadata(appId, forceRefresh) → SteamMetadataLookupResult
  map.set('fetch_game_metadata', async (args): Promise<SteamMetadataLookupResult> => {
    const { appId } = args as { appId: string; forceRefresh?: boolean };
    if (!appId || typeof appId !== 'string') {
      throw new Error('[dev-mock] fetch_game_metadata: appId is required');
    }
    const normalizedId = appId.trim();
    if (normalizedId === SEEDED_APP_ID) {
      return { ...SEEDED_METADATA };
    }
    return unavailableMetadata(normalizedId);
  });

  // import_custom_cover_art(sourcePath) → string (cached path)
  // In browser mode there is no real filesystem — return the source path unchanged.
  map.set('import_custom_cover_art', async (args): Promise<string> => {
    const { sourcePath } = args as { sourcePath: string };
    if (!sourcePath || typeof sourcePath !== 'string') {
      throw new Error('[dev-mock] import_custom_cover_art: sourcePath is required');
    }
    return sourcePath;
  });

  // import_custom_art(sourcePath, artType) → string (cached path)
  // Same passthrough behaviour as import_custom_cover_art.
  map.set('import_custom_art', async (args): Promise<string> => {
    const { sourcePath, artType } = args as { sourcePath: string; artType?: string };
    if (!sourcePath || typeof sourcePath !== 'string') {
      throw new Error('[dev-mock] import_custom_art: sourcePath is required');
    }
    const allowed = ['cover', 'portrait', 'background'];
    const resolvedType = artType ?? 'cover';
    if (!allowed.includes(resolvedType)) {
      throw new Error(`[dev-mock] import_custom_art: unknown art type: ${resolvedType}`);
    }
    return sourcePath;
  });

  // auto_populate_steam(request) → SteamAutoPopulateResult
  // Returns a synthetic "Found" result for the seeded game path; "NotFound" otherwise.
  map.set('auto_populate_steam', async (args): Promise<SteamAutoPopulateResult> => {
    const { request } = args as {
      request: { game_path: string; steam_client_install_path: string };
    };
    if (!request || typeof request !== 'object') {
      throw new Error('[dev-mock] auto_populate_steam: request is required');
    }

    const isSeededGame =
      typeof request.game_path === 'string' &&
      request.game_path.trim().length > 0;

    if (isSeededGame) {
      return {
        app_id_state: 'Found',
        app_id: SEEDED_APP_ID,
        compatdata_state: 'Found',
        compatdata_path: '/home/devuser/.steam/steam/steamapps/compatdata/9999001',
        proton_state: 'Found',
        proton_path: '/home/devuser/.steam/steam/steamapps/common/Proton 9.0',
        diagnostics: ['[dev-mock] synthetic auto-populate result'],
        manual_hints: [],
      };
    }

    return {
      app_id_state: 'NotFound',
      app_id: '',
      compatdata_state: 'NotFound',
      compatdata_path: '',
      proton_state: 'NotFound',
      proton_path: '',
      diagnostics: ['[dev-mock] no game path provided'],
      manual_hints: ['Provide a game executable path to trigger auto-populate.'],
    };
  });
}
