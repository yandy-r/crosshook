import type { Handler } from '../index';
import type { CommunityTapSubscription } from '../../../types/settings';
import type {
  CommunityProfileIndex,
  CommunityProfileIndexEntry,
  CommunityProfileManifest,
  CommunityTapSyncResult,
  CommunityImportPreview,
  CommunityImportResult,
  CommunityExportResult,
} from '../../../hooks/useCommunityProfiles';
import { createDefaultProfile } from '../../../types/profile';

// ---------------------------------------------------------------------------
// Synthetic data fixtures
// ---------------------------------------------------------------------------

const MOCK_TAP_URL = 'https://mock.example.invalid/community-profiles.git';
const MOCK_TAP_BRANCH = 'main';

const MOCK_TAP: CommunityTapSubscription = {
  url: MOCK_TAP_URL,
  branch: MOCK_TAP_BRANCH,
};

function makeMockManifest(gameName: string, author: string, version: string): CommunityProfileManifest {
  const profile = createDefaultProfile();
  return {
    schema_version: 1,
    metadata: {
      game_name: gameName,
      game_version: version,
      trainer_name: `${gameName} Trainer`,
      trainer_version: '1.0.0',
      proton_version: 'GE-Proton9-20',
      platform_tags: ['linux', 'steam-deck'],
      compatibility_rating: 'working',
      author,
      description: `[dev-mock] Synthetic community profile for ${gameName}`,
      trainer_sha256: null,
    },
    profile: {
      ...profile,
      game: {
        ...profile.game,
        name: gameName,
        executable_path: `/mock/games/${gameName.toLowerCase().replace(/ /g, '-')}/game.exe`,
      },
      steam: {
        ...profile.steam,
        enabled: true,
        app_id: gameName === 'Synthetic Quest' ? '9999001' : gameName === 'Dev Test Game' ? '9999002' : '9999003',
      },
    },
  };
}

const MOCK_ENTRIES: CommunityProfileIndexEntry[] = [
  {
    tap_url: MOCK_TAP_URL,
    tap_branch: MOCK_TAP_BRANCH,
    tap_path: '/mock/taps/community-profiles',
    manifest_path: '/mock/taps/community-profiles/synthetic-quest/community.json',
    relative_path: 'synthetic-quest/community.json',
    manifest: makeMockManifest('Synthetic Quest', 'devuser', '2.1.0'),
  },
  {
    tap_url: MOCK_TAP_URL,
    tap_branch: MOCK_TAP_BRANCH,
    tap_path: '/mock/taps/community-profiles',
    manifest_path: '/mock/taps/community-profiles/dev-test-game/community.json',
    relative_path: 'dev-test-game/community.json',
    manifest: makeMockManifest('Dev Test Game', 'mockcontrib', '1.5.3'),
  },
  {
    tap_url: MOCK_TAP_URL,
    tap_branch: MOCK_TAP_BRANCH,
    tap_path: '/mock/taps/community-profiles',
    manifest_path: '/mock/taps/community-profiles/fixture-runner/community.json',
    relative_path: 'fixture-runner/community.json',
    manifest: makeMockManifest('Fixture Runner Pro', 'testdev', '0.9.1'),
  },
];

const MOCK_INDEX: CommunityProfileIndex = {
  entries: MOCK_ENTRIES,
  diagnostics: [],
};

// ---------------------------------------------------------------------------
// Module-scope state
// ---------------------------------------------------------------------------

let taps: CommunityTapSubscription[] = [MOCK_TAP];

// ---------------------------------------------------------------------------
// Register function
// ---------------------------------------------------------------------------

export function registerCommunity(map: Map<string, Handler>): void {
  map.set('community_list_profiles', async (): Promise<CommunityProfileIndex> => {
    return { ...MOCK_INDEX };
  });

  map.set(
    'community_list_indexed_profiles',
    async (): Promise<Array<{ game_name: string | null; proton_version: string | null }>> => {
      return MOCK_ENTRIES.map((entry) => ({
        game_name: entry.manifest.metadata.game_name,
        proton_version: entry.manifest.metadata.proton_version,
      }));
    }
  );

  map.set('community_sync', async (): Promise<CommunityTapSyncResult[]> => {
    const result: CommunityTapSyncResult = {
      workspace: {
        subscription: { ...MOCK_TAP },
        local_path: '/mock/taps/community-profiles',
      },
      status: 'cached_fallback',
      head_commit: 'aabbccdd1122334455667788990011223344556677',
      index: { ...MOCK_INDEX },
      from_cache: true,
      last_sync_at: new Date().toISOString(),
    };
    return [result];
  });

  map.set('community_add_tap', async (args): Promise<CommunityTapSubscription[]> => {
    const { tap } = args as { tap: CommunityTapSubscription };
    if (!tap?.url?.trim()) {
      throw new Error('[dev-mock] community_add_tap: tap URL is required');
    }
    const normalized: CommunityTapSubscription = { url: tap.url.trim() };
    if (tap.branch?.trim()) {
      normalized.branch = tap.branch.trim();
    }
    if (tap.pinned_commit?.trim()) {
      normalized.pinned_commit = tap.pinned_commit.trim();
    }
    const key = `${normalized.url}::${normalized.branch ?? ''}::${normalized.pinned_commit ?? ''}`;
    const alreadyPresent = taps.some(
      (t) => `${t.url}::${t.branch ?? ''}::${t.pinned_commit ?? ''}` === key
    );
    if (!alreadyPresent) {
      taps = [...taps, normalized];
    }
    return taps;
  });

  map.set('community_prepare_import', async (args): Promise<CommunityImportPreview> => {
    const { path } = args as { path: string };
    const entry = MOCK_ENTRIES[0];
    if (!entry) {
      throw new Error('[dev-mock] community_prepare_import: no mock entry available');
    }
    return {
      profile_name: 'synthetic-quest',
      source_path: path ?? entry.manifest_path,
      profile: entry.manifest.profile,
      manifest: entry.manifest,
      required_prefix_deps: [],
    };
  });

  map.set('community_export_profile', async (args): Promise<CommunityExportResult> => {
    const { profile_name, output_path } = args as { profile_name: string; output_path: string };
    const trimmed = (profile_name ?? '').trim();
    if (!trimmed) {
      throw new Error('[dev-mock] community_export_profile: profile_name is required');
    }
    const entry = MOCK_ENTRIES.find(
      (e) => e.manifest.metadata.game_name.toLowerCase() === trimmed.toLowerCase(),
    );
    if (!entry) {
      throw new Error(`[dev-mock] community_export_profile: profile not found: ${profile_name}`);
    }
    return {
      profile_name: trimmed,
      output_path: output_path ?? `/mock/export/${trimmed.replace(/\s+/g, '-')}.json`,
      manifest: entry.manifest,
    };
  });

  map.set('community_import_profile', async (args): Promise<CommunityImportResult> => {
    const { path } = args as { path: string };
    const entry = MOCK_ENTRIES[0];
    if (!entry) {
      throw new Error('[dev-mock] community_import_profile: no mock entry available');
    }
    return {
      profile_name: 'synthetic-quest',
      source_path: path ?? entry.manifest_path,
      profile_path: '/home/devuser/.config/crosshook/profiles/synthetic-quest.toml',
      profile: entry.manifest.profile,
      manifest: entry.manifest,
    };
  });
}
