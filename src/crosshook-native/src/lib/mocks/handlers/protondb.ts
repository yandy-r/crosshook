import type { Handler } from '../index';
import { getStore } from '../store';
import type {
  AcceptSuggestionRequest,
  AcceptSuggestionResult,
  ProtonDbLookupResult,
  ProtonDbSuggestionSet,
} from '../../../types/protondb';
import type { GameProfile } from '../../../types/profile';

// Tracks dismissed suggestion keys across handler calls within a browser session.
// Key format: `${profileName}:${appId}:${suggestionKey}`
const dismissedSuggestions = new Set<string>();

// Synthetic ProtonDB tier data keyed by Steam app ID (BR-10/W-3: IDs >= 9999001).
function buildLookupResult(appId: string): ProtonDbLookupResult {
  const now = new Date().toISOString();
  const expires = new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString();

  const tierMap: Record<string, { tier: 'platinum' | 'gold' | 'silver' | 'bronze' | 'borked'; score: number; totalReports: number }> = {
    '9999001': { tier: 'platinum', score: 0.97, totalReports: 284 },
    '9999002': { tier: 'gold',     score: 0.82, totalReports: 142 },
  };

  const data = tierMap[appId] ?? { tier: 'silver', score: 0.65, totalReports: 58 };

  return {
    app_id: appId,
    state: 'ready',
    cache: {
      cache_key: `protondb:${appId}`,
      fetched_at: now,
      expires_at: expires,
      from_cache: true,
      is_stale: false,
      is_offline: false,
    },
    snapshot: {
      app_id: appId,
      tier: data.tier,
      best_reported_tier: data.tier,
      trending_tier: data.tier,
      score: data.score,
      confidence: 'high',
      total_reports: data.totalReports,
      recommendation_groups: [
        {
          group_id: 'envvars',
          title: 'Environment Variables',
          summary: 'Suggested env vars from community reports',
          notes: [],
          env_vars: [
            {
              key: 'PROTON_USE_WINED3D',
              value: '0',
              source_label: 'ProtonDB community',
              supporting_report_count: 12,
            },
          ],
          launch_options: [],
        },
      ],
      source_url: `https://www.protondb.com/app/${appId}`,
      fetched_at: now,
    },
  };
}

function buildSuggestionSet(appId: string, profileName: string): ProtonDbSuggestionSet {
  const tierMap: Record<string, 'platinum' | 'gold' | 'silver' | 'bronze' | 'borked'> = {
    '9999001': 'platinum',
    '9999002': 'gold',
  };
  const tier = tierMap[appId] ?? 'silver';
  const totalReports = appId === '9999001' ? 284 : appId === '9999002' ? 142 : 58;

  const envKey = 'PROTON_USE_WINED3D';
  const envValue = '0';
  const suggestionKey = envKey;
  const dismissed = dismissedSuggestions.has(`${profileName}:${appId}:${suggestionKey}`);

  const store = getStore();
  const profile = store.profiles.get(profileName);
  const customEnv = profile?.launch?.custom_env_vars ?? {};
  const alreadyApplied = customEnv[envKey] === envValue;

  let envVarSuggestions: ProtonDbSuggestionSet['envVarSuggestions'];
  if (dismissed) {
    envVarSuggestions = [];
  } else if (alreadyApplied) {
    envVarSuggestions = [
      {
        key: envKey,
        value: envValue,
        status: 'already_applied',
        supportingReportCount: 12,
      },
    ];
  } else {
    envVarSuggestions = [
      {
        key: envKey,
        value: envValue,
        status: 'new',
        supportingReportCount: 12,
      },
    ];
  }

  return {
    catalogSuggestions: [],
    envVarSuggestions,
    launchOptionSuggestions: [
      {
        rawText: 'PROTON_NO_ESYNC=1 %command%',
        supportingReportCount: 7,
      },
    ],
    tier,
    totalReports,
    isStale: false,
  };
}

export function registerProtonDb(map: Map<string, Handler>): void {
  map.set('protondb_lookup', (args): ProtonDbLookupResult => {
    const { appId } = args as { appId: string; forceRefresh?: boolean };
    if (!appId) {
      throw new Error('[dev-mock] protondb_lookup: appId is required');
    }
    return buildLookupResult(appId);
  });

  map.set('protondb_get_suggestions', (args): ProtonDbSuggestionSet => {
    const { appId, profileName } = args as {
      appId: string;
      profileName: string;
      forceRefresh?: boolean;
    };
    if (!appId) {
      throw new Error('[dev-mock] protondb_get_suggestions: appId is required');
    }
    if (!profileName) {
      throw new Error('[dev-mock] protondb_get_suggestions: profileName is required');
    }
    const store = getStore();
    if (!store.profiles.has(profileName)) {
      throw new Error(`[dev-mock] protondb_get_suggestions: profile not found: ${profileName}`);
    }
    return buildSuggestionSet(appId, profileName);
  });

  map.set('protondb_accept_suggestion', (args): AcceptSuggestionResult => {
    const { request } = args as { request: AcceptSuggestionRequest };
    if (!request) {
      throw new Error('[dev-mock] protondb_accept_suggestion: request is required');
    }

    const store = getStore();
    const profileName = request.profileName;
    const profile = store.profiles.get(profileName);
    if (!profile) {
      throw new Error(
        `[dev-mock] protondb_accept_suggestion: profile not found: ${profileName}`,
      );
    }

    if (request.kind === 'catalog') {
      const { catalogEntryId } = request;
      return {
        updatedProfile: profile,
        appliedKeys: [],
        toggledOptionIds: [catalogEntryId],
      };
    }

    if (request.kind === 'env_var') {
      const { envKey, envValue } = request;
      const updated: GameProfile = structuredClone(profile);
      const nextEnv = { ...(updated.launch.custom_env_vars ?? {}) };
      nextEnv[envKey] = envValue;
      updated.launch = { ...updated.launch, custom_env_vars: nextEnv };
      store.profiles.set(profileName, updated);
      return {
        updatedProfile: updated,
        appliedKeys: [envKey],
        toggledOptionIds: [],
      };
    }

    throw new Error('[dev-mock] protondb_accept_suggestion: unknown request kind');
  });

  map.set('protondb_dismiss_suggestion', (args): null => {
    const { profileName, appId, suggestionKey } = args as {
      profileName: string;
      appId: string;
      suggestionKey: string;
    };
    if (!profileName) {
      throw new Error('[dev-mock] protondb_dismiss_suggestion: profileName is required');
    }
    if (!appId) {
      throw new Error('[dev-mock] protondb_dismiss_suggestion: appId is required');
    }
    if (!suggestionKey) {
      throw new Error('[dev-mock] protondb_dismiss_suggestion: suggestionKey is required');
    }
    dismissedSuggestions.add(`${profileName}:${appId}:${suggestionKey}`);
    return null;
  });
}
