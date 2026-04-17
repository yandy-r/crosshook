// Proton manager mocks for browser dev mode (see #274).
import type {
  ProtonUpAvailableVersion,
  ProtonUpCatalogResponse,
  ProtonUpInstallRequest,
  ProtonUpInstallResult,
  ProtonUpSuggestion,
} from '../../../types/protonup';
import { emitMockEvent } from '../eventBus';
import type { Handler } from './types';

const GE_PROTON_VERSIONS: ProtonUpAvailableVersion[] = [
  {
    provider: 'ge-proton',
    version: 'GE-Proton9-27',
    release_url: 'https://example.invalid/GE-Proton9-27',
    download_url: 'https://example.invalid/GE-Proton9-27.tar.gz',
    asset_size: 530_000_000,
    published_at: '2026-03-12T18:00:00Z',
  },
  {
    provider: 'ge-proton',
    version: 'GE-Proton9-26',
    release_url: 'https://example.invalid/GE-Proton9-26',
    download_url: 'https://example.invalid/GE-Proton9-26.tar.gz',
    asset_size: 528_000_000,
    published_at: '2026-01-20T12:00:00Z',
  },
  {
    provider: 'ge-proton',
    version: 'GE-Proton9-25',
    release_url: 'https://example.invalid/GE-Proton9-25',
    download_url: 'https://example.invalid/GE-Proton9-25.tar.gz',
    asset_size: 524_288_000,
    published_at: '2025-11-05T09:00:00Z',
  },
];

const CACHYOS_VERSIONS: ProtonUpAvailableVersion[] = [
  {
    provider: 'proton-cachyos',
    version: 'proton-cachyos-9.0-1',
    release_url: 'https://example.invalid/proton-cachyos-9.0-1',
    download_url: 'https://example.invalid/proton-cachyos-9.0-1.tar.zst',
    asset_size: 540_000_000,
    published_at: '2026-04-01T10:00:00Z',
  },
  {
    provider: 'proton-cachyos',
    version: 'proton-cachyos-8.0-2',
    release_url: 'https://example.invalid/proton-cachyos-8.0-2',
    download_url: 'https://example.invalid/proton-cachyos-8.0-2.tar.zst',
    asset_size: 536_000_000,
    published_at: '2025-12-10T14:30:00Z',
  },
];

const PROTON_EM_VERSIONS: ProtonUpAvailableVersion[] = [
  {
    provider: 'proton-em',
    version: 'proton-em-10-rev2',
    release_url: 'https://example.invalid/proton-em-10-rev2',
    download_url: 'https://example.invalid/proton-em-10-rev2.tar.gz',
    asset_size: 510_000_000,
    published_at: '2026-02-28T16:00:00Z',
  },
  {
    provider: 'proton-em',
    version: 'proton-em-10',
    release_url: 'https://example.invalid/proton-em-10',
    download_url: 'https://example.invalid/proton-em-10.tar.gz',
    asset_size: 504_000_000,
    published_at: '2025-10-15T08:00:00Z',
  },
];

function mockCatalogFor(provider: string | undefined): ProtonUpAvailableVersion[] {
  switch (provider) {
    case 'proton-cachyos':
      return CACHYOS_VERSIONS;
    case 'proton-em':
      return PROTON_EM_VERSIONS;
    case 'ge-proton':
      return GE_PROTON_VERSIONS;
    default:
      return [];
  }
}

/** Tracks in-flight mock installs so protonup_cancel_install can drop remaining emissions. */
const activeInstalls = new Set<string>();

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
    const { provider } = (args ?? {}) as { provider?: string; force_refresh?: boolean };

    return {
      versions: mockCatalogFor(provider),
      cache: getMockCacheMeta(),
    };
  });

  map.set('protonup_install_version', async (args): Promise<ProtonUpInstallResult> => {
    const { request } = (args ?? {}) as { request: ProtonUpInstallRequest };

    if (!request?.version || !request?.provider) {
      throw new Error('[dev-mock] protonup_install_version: missing required request fields');
    }

    const catalog = mockCatalogFor(request.provider);

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

  // ── Batch 4 commands ──────────────────────────────────────────────────────

  map.set('protonup_list_providers', (_args) => {
    return [
      { id: 'ge-proton', display_name: 'GE-Proton', supports_install: true, checksum_kind: 'sha512-sidecar' },
      { id: 'proton-cachyos', display_name: 'Proton-CachyOS', supports_install: true, checksum_kind: 'sha512-sidecar' },
      { id: 'proton-em', display_name: 'Proton-EM', supports_install: true, checksum_kind: 'none' },
    ];
  });

  map.set('protonup_resolve_install_roots', (_args) => {
    return [
      {
        kind: 'native-steam',
        path: '/home/devuser/.local/share/Steam/compatibilitytools.d',
        writable: true,
        reason: null,
      },
      {
        kind: 'flatpak-steam',
        path: '/home/devuser/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d',
        writable: false,
        reason: 'flatpak-steam-path-read-only',
      },
    ];
  });

  map.set('protonup_install_version_async', async (_args) => {
    const opId = `mock-op-${Date.now()}`;
    activeInstalls.add(opId);

    const schedule = (delayMs: number, payload: Record<string, unknown>) => {
      window.setTimeout(() => {
        if (!activeInstalls.has(opId)) return;
        emitMockEvent('protonup:install:progress', { op_id: opId, ...payload });
      }, delayMs);
    };

    schedule(0, { phase: 'resolving', bytes_done: 0, bytes_total: null });
    schedule(100, { phase: 'downloading', bytes_done: 0, bytes_total: 100_000_000 });
    schedule(200, { phase: 'downloading', bytes_done: 25_000_000, bytes_total: 100_000_000 });
    schedule(400, { phase: 'downloading', bytes_done: 75_000_000, bytes_total: 100_000_000 });
    schedule(600, { phase: 'downloading', bytes_done: 100_000_000, bytes_total: 100_000_000 });
    schedule(700, { phase: 'verifying', bytes_done: 100_000_000, bytes_total: 100_000_000 });
    schedule(900, { phase: 'extracting', bytes_done: 100_000_000, bytes_total: 100_000_000 });
    schedule(1100, { phase: 'finalizing', bytes_done: 100_000_000, bytes_total: 100_000_000 });

    window.setTimeout(() => {
      if (!activeInstalls.has(opId)) return;
      activeInstalls.delete(opId);
      emitMockEvent('protonup:install:progress', {
        op_id: opId,
        phase: 'done',
        bytes_done: 100_000_000,
        bytes_total: 100_000_000,
      });
    }, 1200);

    return { op_id: opId };
  });

  map.set('protonup_cancel_install', (args) => {
    const { opId } = (args ?? {}) as { opId: string };
    if (!activeInstalls.has(opId)) return false;
    activeInstalls.delete(opId);
    emitMockEvent('protonup:install:progress', { op_id: opId, phase: 'cancelled' });
    return true;
  });

  map.set('protonup_plan_uninstall_version', (args) => {
    const { toolPath } = (args ?? {}) as { toolPath: string };

    if (toolPath.includes('usr/share') || toolPath.startsWith('/usr/')) {
      return { success: false, conflicting_app_ids: [], error_message: 'refusing to delete system path' };
    }

    if (toolPath.includes('conflict')) {
      return { success: true, conflicting_app_ids: ['12345', '67890'], error_message: null };
    }

    return { success: true, conflicting_app_ids: [], error_message: null };
  });

  map.set('protonup_uninstall_version', (args) => {
    const { toolPath } = (args ?? {}) as { toolPath: string };

    if (toolPath.includes('usr/share') || toolPath.startsWith('/usr/')) {
      return { success: false, conflicting_app_ids: [], error_message: 'refusing to delete system path' };
    }

    return { success: true, conflicting_app_ids: [], error_message: null };
  });
}
