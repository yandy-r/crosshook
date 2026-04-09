import type { Handler } from './types';
import type {
  ProtonUpAvailableVersion,
  ProtonUpCatalogResponse,
  ProtonUpInstallRequest,
  ProtonUpInstallResult,
  ProtonUpSuggestion,
} from '../../../types/protonup';

const GE_PROTON_VERSIONS: ProtonUpAvailableVersion[] = [
  {
    provider: 'ge-proton',
    version: 'GE-Proton9-27',
    release_url: 'https://example.invalid/GE-Proton9-27',
    download_url: 'https://example.invalid/GE-Proton9-27.tar.gz',
    asset_size: 530_000_000,
  },
  {
    provider: 'ge-proton',
    version: 'GE-Proton9-26',
    release_url: 'https://example.invalid/GE-Proton9-26',
    download_url: 'https://example.invalid/GE-Proton9-26.tar.gz',
    asset_size: 528_000_000,
  },
  {
    provider: 'ge-proton',
    version: 'GE-Proton9-25',
    release_url: 'https://example.invalid/GE-Proton9-25',
    download_url: 'https://example.invalid/GE-Proton9-25.tar.gz',
    asset_size: 524_288_000,
  },
];

const CACHYOS_VERSIONS: ProtonUpAvailableVersion[] = [
  {
    provider: 'proton-cachyos',
    version: 'proton-cachyos-9.0-1',
    release_url: 'https://example.invalid/proton-cachyos-9.0-1',
    download_url: 'https://example.invalid/proton-cachyos-9.0-1.tar.zst',
    asset_size: 540_000_000,
  },
  {
    provider: 'proton-cachyos',
    version: 'proton-cachyos-8.0-2',
    release_url: 'https://example.invalid/proton-cachyos-8.0-2',
    download_url: 'https://example.invalid/proton-cachyos-8.0-2.tar.zst',
    asset_size: 536_000_000,
  },
];

function getMockCacheMeta(): {
  stale: boolean;
  offline: boolean;
  fetched_at: string;
  expires_at: string;
} {
  return {
    stale: false,
    offline: false,
    fetched_at: new Date().toISOString(),
    expires_at: new Date(Date.now() + 3_600_000).toISOString(),
  };
}

export function registerProtonUp(map: Map<string, Handler>): void {
  map.set('protonup_list_available_versions', async (args): Promise<ProtonUpCatalogResponse> => {
    const { provider } = (args ?? {}) as { provider?: string; forceRefresh?: boolean };

    const versions = provider === 'proton-cachyos' ? CACHYOS_VERSIONS : GE_PROTON_VERSIONS;

    return {
      versions,
      cache: getMockCacheMeta(),
    };
  });

  map.set('protonup_install_version', async (args): Promise<ProtonUpInstallResult> => {
    const { request } = (args ?? {}) as { request: ProtonUpInstallRequest };

    if (!request?.version || !request?.provider) {
      throw new Error('[dev-mock] protonup_install_version: missing required request fields');
    }

    const catalog = request.provider === 'proton-cachyos' ? CACHYOS_VERSIONS : GE_PROTON_VERSIONS;

    const found = catalog.some((v) => v.version === request.version && v.provider === request.provider);

    if (!found) {
      return {
        success: false,
        error_kind: 'unknown',
        error_message: `[dev-mock] version ${request.version} not found in catalog`,
      };
    }

    const targetRoot = request.target_root || '/home/devuser/.steam/root/compatibilitytools.d';
    const installedPath = `${targetRoot}/${request.version}`;

    return {
      success: true,
      installed_path: installedPath,
    };
  });

  map.set('protonup_get_suggestion', (_args): ProtonUpSuggestion => {
    const { communityVersion } = (_args ?? {}) as {
      communityVersion?: string;
      steamClientInstallPath?: string;
    };

    const version = communityVersion?.trim() ?? '';

    if (version.length === 0) {
      return {
        status: 'unknown',
      };
    }

    // Simulate a match against the GE-Proton synthetic catalog
    const lowerVersion = version.toLowerCase();
    const matched = GE_PROTON_VERSIONS.find(
      (v) => v.version.toLowerCase().includes(lowerVersion) || lowerVersion.includes(v.version.toLowerCase())
    );

    if (matched) {
      return {
        status: 'matched',
        community_version: version,
        matched_install_name: matched.version,
        recommended_version: matched.version,
      };
    }

    return {
      status: 'missing',
      community_version: version,
      recommended_version: GE_PROTON_VERSIONS[0]?.version,
    };
  });
}
