import type { DiagnosticBundleResult } from '../../../types/diagnostics';

import type {
  CatalogEntry,
  CatalogFacetValue,
  CatalogPage,
  CatalogQuery,
  ExternalTrainerSearchResponse,
  ExternalTrainerSourceSubscription,
  VersionMatchResult,
} from '../../../types/discovery';
import type { CommandArgumentCatalogPayload, CommandArgumentEntry } from '../../../types/launch-command-arguments';
import type { HashVerifyResult, OfflineReadinessReport, TrainerTypeEntry } from '../../../types/offline';
import type {
  BinaryDetectionResult,
  PrefixDependencyStatus,
  PrefixVersionRepairStatus,
} from '../../../types/prefix-deps';
import type {
  PrefixCleanupResult,
  PrefixCleanupTarget,
  PrefixStorageHistoryResponse,
  PrefixStorageScanResult,
} from '../../../types/prefix-storage';
import type { RunExecutableResult } from '../../../types/run-executable';
import type { OptimizationCatalogPayload, OptimizationEntry } from '../../../utils/optimization-catalog';
import { getActiveFixture } from '../../fixture';
import { getActiveToggles } from '../../toggles';
import { forcedError, neverResolving } from './profile-utils';
import type { Handler } from './types';

// --- Module-scope state ---

const DEFAULT_EXTERNAL_SOURCES: ExternalTrainerSourceSubscription[] = [
  {
    sourceId: 'mock-source-1',
    displayName: 'Mock Trainer Index',
    baseUrl: 'https://mock.example.invalid/trainers',
    sourceType: 'rss',
    enabled: true,
  },
];

let externalSources: ExternalTrainerSourceSubscription[] = structuredClone(DEFAULT_EXTERNAL_SOURCES);

// --- discovery ---

const MOCK_CATALOG_PROFILE_ENTRIES: CatalogEntry[] = [
  {
    id: 1,
    tapUrl: 'https://mock.example.invalid/tap/alpha',
    tapLocalPath: '/mock/tap/alpha',
    relativePath: 'profiles/synthetic-quest/community-profile.json',
    manifestPath: '/mock/tap/alpha/profiles/synthetic-quest/community-profile.json',
    gameName: 'Synthetic Quest',
    gameVersion: '2.0.1',
    trainerName: 'Synthetic Trainer',
    trainerVersion: '1.0.0',
    protonVersion: 'GE-Proton9-21',
    compatibilityRating: 'platinum',
    author: 'Mock Author',
    description: 'Synthetic data — not a real trainer.',
    platformTags: 'linux steam-deck',
    trainerLoadingMode: 'source_directory',
    schemaVersion: 1,
    sources: [
      {
        sourceName: 'Mock Trainer Index',
        sourceUrl: 'https://mock.example.invalid/trainers/synthetic-quest',
        sha256: 'aabbccdd00112233aabbccdd00112233aabbccdd00112233aabbccdd00112233',
        trainerVersion: '1.0.0',
        gameVersion: '2.0.1',
        notes: 'Synthetic data — not a real trainer.',
      },
      {
        sourceName: 'Mock Mirror',
        sourceUrl: 'https://mock.example.invalid/mirror/synthetic-quest',
        sha256: null,
        trainerVersion: null,
        gameVersion: null,
        notes: null,
      },
    ],
  },
  {
    id: 2,
    tapUrl: 'https://mock.example.invalid/tap/alpha',
    tapLocalPath: '/mock/tap/alpha',
    relativePath: 'profiles/dev-test-game/community-profile.json',
    manifestPath: '/mock/tap/alpha/profiles/dev-test-game/community-profile.json',
    gameName: 'Dev Test Game',
    gameVersion: '1.5.0',
    trainerName: 'Dev Trainer',
    trainerVersion: '0.9.0',
    protonVersion: null,
    compatibilityRating: 'working',
    author: 'Mock Author',
    description: null,
    platformTags: 'linux',
    trainerLoadingMode: 'copy_to_prefix',
    schemaVersion: 1,
    sources: [
      {
        sourceName: 'Mock Trainer Index',
        sourceUrl: 'https://mock.example.invalid/trainers/dev-test-game',
        sha256: null,
        trainerVersion: '0.9.0',
        gameVersion: '1.5.0',
        notes: null,
      },
    ],
  },
  {
    id: 3,
    tapUrl: 'https://mock.example.invalid/tap/beta',
    tapLocalPath: '/mock/tap/beta',
    relativePath: 'profiles/orphan-game/community-profile.json',
    manifestPath: '/mock/tap/beta/profiles/orphan-game/community-profile.json',
    gameName: 'Orphan Game',
    gameVersion: null,
    trainerName: null,
    trainerVersion: null,
    protonVersion: null,
    compatibilityRating: null,
    author: null,
    description: 'Sourceless profile — import only.',
    platformTags: null,
    trainerLoadingMode: null,
    schemaVersion: 1,
    sources: [],
  },
  {
    // Source-only entry: trainer sources without a community profile (empty manifestPath hides Import).
    id: null,
    tapUrl: 'https://mock.example.invalid/tap/beta',
    tapLocalPath: '/mock/tap/beta',
    relativePath: 'trainer-sources/source-only-game',
    manifestPath: '',
    gameName: 'Source Only Game',
    gameVersion: null,
    trainerName: null,
    trainerVersion: null,
    protonVersion: null,
    compatibilityRating: null,
    author: null,
    description: null,
    platformTags: null,
    trainerLoadingMode: null,
    schemaVersion: 0,
    sources: [
      {
        sourceName: 'Mock Source Index',
        sourceUrl: 'https://mock.example.invalid/sources/source-only-game',
        sha256: null,
        trainerVersion: '2.4.0',
        gameVersion: null,
        notes: 'Synthetic source-only listing — no community profile.',
      },
    ],
  },
];

type MockFacetKey = 'gameTitles' | 'loadingModes' | 'compatibilityBands' | 'tapUrls';

function normalizeBand(entry: CatalogEntry): string {
  const rating = entry.compatibilityRating;
  return rating === 'platinum' || rating === 'working' || rating === 'partial' || rating === 'broken'
    ? rating
    : 'unknown';
}

function normalizeMode(entry: CatalogEntry): string {
  const mode = entry.trainerLoadingMode;
  return mode === 'source_directory' || mode === 'copy_to_prefix' ? mode : 'unknown';
}

function mockEntryFacetValue(entry: CatalogEntry, key: MockFacetKey): string | null {
  switch (key) {
    case 'gameTitles':
      return entry.gameName ?? null;
    case 'loadingModes':
      return normalizeMode(entry);
    case 'compatibilityBands':
      return normalizeBand(entry);
    case 'tapUrls':
      return entry.tapUrl;
  }
}

function mockMatchesDimension(entry: CatalogEntry, key: MockFacetKey, selections: string[]): boolean {
  if (selections.length === 0) return true;
  const value = mockEntryFacetValue(entry, key);
  return value !== null && selections.includes(value);
}

/** Mirrors the fields Rust `matches_text` searches (names, secondary fields, versions, sources). */
function mockMatchesText(entry: CatalogEntry, query: string): boolean {
  const haystack = [
    entry.gameName,
    entry.trainerName,
    entry.author,
    entry.description,
    entry.platformTags,
    entry.tapUrl,
    entry.gameVersion,
    entry.trainerVersion,
    entry.protonVersion,
    entry.manifestPath,
    ...entry.sources.flatMap((source) => [source.sourceName, source.notes]),
  ]
    .filter((value): value is string => typeof value === 'string' && value.length > 0)
    .join(' ')
    .toLowerCase();
  return haystack.includes(query);
}

function mockFacetCounts(entries: CatalogEntry[], key: MockFacetKey): CatalogFacetValue[] {
  const counts = new Map<string, number>();
  for (const entry of entries) {
    const value = mockEntryFacetValue(entry, key);
    if (value !== null) {
      counts.set(value, (counts.get(value) ?? 0) + 1);
    }
  }
  return [...counts.entries()]
    .map(([value, count]) => ({ value, count }))
    .sort((a, b) => b.count - a.count || a.value.localeCompare(b.value));
}

/** Dev-mode fidelity only; the real filter/facet/rank semantics live in Rust. */
function buildMockCatalogPage(entries: CatalogEntry[], query: CatalogQuery | undefined): CatalogPage {
  const q = (query?.query ?? '').trim().toLowerCase();
  const selections: Record<MockFacetKey, string[]> = {
    gameTitles: query?.gameTitles ?? [],
    loadingModes: query?.loadingModes ?? [],
    compatibilityBands: query?.compatibilityBands ?? [],
    tapUrls: query?.tapUrls ?? [],
  };
  const dimensionKeys: MockFacetKey[] = ['gameTitles', 'loadingModes', 'compatibilityBands', 'tapUrls'];

  const textMatched = entries.filter((entry) => q.length === 0 || mockMatchesText(entry, q));
  const matched = textMatched.filter((entry) =>
    dimensionKeys.every((key) => mockMatchesDimension(entry, key, selections[key]))
  );

  const facetDomain = (facetKey: MockFacetKey) =>
    textMatched.filter((entry) =>
      dimensionKeys.every((key) => key === facetKey || mockMatchesDimension(entry, key, selections[key]))
    );

  const offset = query?.offset ?? 0;
  const limit = query?.limit ?? 50;

  return {
    entries: matched.slice(offset, offset + limit),
    facets: {
      gameTitles: mockFacetCounts(facetDomain('gameTitles'), 'gameTitles'),
      loadingModes: mockFacetCounts(facetDomain('loadingModes'), 'loadingModes'),
      compatibilityBands: mockFacetCounts(facetDomain('compatibilityBands'), 'compatibilityBands'),
      taps: mockFacetCounts(facetDomain('tapUrls'), 'tapUrls'),
    },
    totalCount: matched.length,
    tapCount: new Set(entries.map((entry) => entry.tapUrl)).size,
    degraded: false,
  };
}

const MOCK_EXTERNAL_SEARCH_RESPONSE: ExternalTrainerSearchResponse = {
  results: [
    {
      gameName: 'Synthetic Quest',
      sourceName: 'Mock External Index',
      sourceUrl: 'https://mock.example.invalid/external/synthetic-quest',
      pubDate: new Date().toISOString(),
      source: 'mock',
      relevanceScore: 0.9,
    },
  ],
  source: 'mock',
  cached: false,
  cacheAgeSecs: undefined,
  isStale: false,
  offline: false,
};

// --- prefix storage ---

const MOCK_PREFIX_SCAN: PrefixStorageScanResult = {
  scanned_at: new Date().toISOString(),
  prefixes: [
    {
      resolved_prefix_path: '/home/devuser/.local/share/crosshook/prefixes/synthetic-quest',
      total_bytes: 1_073_741_824, // 1 GiB
      staged_trainers_bytes: 10_485_760, // 10 MiB
      is_orphan: false,
      referenced_by_profiles: ['Test Game Alpha'],
      stale_staged_trainers: [],
    },
  ],
  orphan_targets: [],
  stale_staged_targets: [],
  inventory_incomplete: false,
};

// --- prefix deps ---

const MOCK_BINARY_DETECTION: BinaryDetectionResult = {
  found: true,
  binary_path: '/usr/bin/winetricks',
  binary_name: 'winetricks',
  tool_type: 'winetricks',
  source: 'PATH',
};

const MOCK_PREFIX_VERSION_REPAIR_STATUS: PrefixVersionRepairStatus = {
  required: false,
  state: null,
  last_error: null,
};

// --- optimization catalog ---

const MOCK_CATALOG_ENTRIES: OptimizationEntry[] = [
  {
    id: 'esync',
    applies_to_method: 'proton',
    env: [['PROTON_NO_ESYNC', '0']],
    wrappers: [],
    conflicts_with: ['fsync'],
    required_binary: '',
    label: 'Esync',
    description: 'Enable eventfd-based synchronization for improved performance.',
    help_text: 'Reduces CPU overhead for synchronization-heavy Windows games.',
    category: 'sync',
    target_gpu_vendor: 'any',
    advanced: false,
    community: false,
    applicable_methods: ['proton'],
  },
  {
    id: 'fsync',
    applies_to_method: 'proton',
    env: [['PROTON_NO_FSYNC', '0']],
    wrappers: [],
    conflicts_with: ['esync'],
    required_binary: '',
    label: 'Fsync',
    description: 'Enable futex-based synchronization (requires kernel support).',
    help_text: 'Preferred over esync when the kernel supports futex2.',
    category: 'sync',
    target_gpu_vendor: 'any',
    advanced: false,
    community: false,
    applicable_methods: ['proton'],
  },
  {
    id: 'mangohud',
    applies_to_method: 'any',
    env: [['MANGOHUD', '1']],
    wrappers: ['mangohud'],
    conflicts_with: [],
    required_binary: 'mangohud',
    label: 'MangoHud overlay',
    description: 'Enable the MangoHud performance overlay.',
    help_text: 'Displays GPU/CPU utilization, frametime, and FPS in-game.',
    category: 'overlay',
    target_gpu_vendor: 'any',
    advanced: false,
    community: false,
    applicable_methods: ['proton', 'wine', 'native'],
  },
];

const MOCK_CATALOG: OptimizationCatalogPayload = {
  catalog_version: 1,
  entries: MOCK_CATALOG_ENTRIES,
};

// --- command argument catalog (mirrors default_command_argument_catalog.toml) ---

const MOCK_COMMAND_ARGUMENT_ENTRIES: CommandArgumentEntry[] = [
  {
    id: 'force_vulkan',
    tokens: ['-force-vulkan'],
    label: 'Force Vulkan renderer (Unity)',
    description: 'Pass a Vulkan-forcing switch when the game supports it.',
    help_text:
      'Unity standalone flag (-force-vulkan); ignored by non-Unity engines. Many titles ignore it or crash — verify per game before relying on it.',
    category: 'graphics',
    advanced: false,
    community: false,
    applicable_methods: ['proton_run', 'steam_applaunch'],
    conflicts_with: ['force_dx11', 'force_dx12', 'force_opengl'],
  },
  {
    id: 'force_dx11',
    tokens: ['-dx11'],
    label: 'Force DirectX 11 (Source/Unreal)',
    description: 'Request a DirectX 11 code path when the game reads launch switches.',
    help_text:
      'Common on Source and Unreal titles (-dx11). Unity titles instead use -force-d3d11. Wrong switches can prevent startup or leave you on an unintended renderer.',
    category: 'graphics',
    advanced: false,
    community: false,
    applicable_methods: ['proton_run', 'steam_applaunch'],
    conflicts_with: ['force_vulkan', 'force_dx12', 'force_opengl'],
  },
  {
    id: 'force_dx12',
    tokens: ['-dx12'],
    label: 'Force DirectX 12 (Source/Unreal)',
    description: 'Request a DirectX 12 code path when the game reads launch switches.',
    help_text:
      'Common on Source and Unreal titles (-dx12). Unity titles instead use -force-d3d12. On Proton/Wine paths DX12 support varies by title and driver stack.',
    category: 'graphics',
    advanced: false,
    community: false,
    applicable_methods: ['proton_run', 'steam_applaunch'],
    conflicts_with: ['force_vulkan', 'force_dx11', 'force_opengl'],
  },
  {
    id: 'skip_launcher',
    tokens: ['--skip-launcher'],
    label: 'Skip in-game launcher',
    description: 'Skip a publisher launcher when the game honors the switch.',
    help_text:
      "Documented by some publishers (for example Larian's --skip-launcher for Baldur's Gate 3). Useless or harmful on titles without a separate launcher step.",
    category: 'compatibility',
    advanced: false,
    community: false,
    applicable_methods: ['proton_run', 'steam_applaunch'],
    conflicts_with: ['nolauncher', 'launcher_skip'],
  },
  {
    id: 'nolauncher',
    tokens: ['--nolauncher'],
    label: 'Skip launcher (nolauncher)',
    description: 'Skip a publisher launcher on titles that honor the nolauncher switch.',
    help_text:
      'Weakly sourced publisher-specific switch. Pick this, --skip-launcher, or --launcher-skip based on what your game documents — not more than one.',
    category: 'compatibility',
    advanced: false,
    community: false,
    applicable_methods: ['proton_run', 'steam_applaunch'],
    conflicts_with: ['skip_launcher', 'launcher_skip'],
  },
  {
    id: 'launcher_skip',
    tokens: ['--launcher-skip'],
    label: 'Skip CD Projekt RED launcher (Witcher 3)',
    description: 'Skip the CD Projekt RED launcher on titles that honor the switch.',
    help_text:
      "Documented for The Witcher 3 next-gen (--launcher-skip). Do not confuse with Larian's --skip-launcher or other publisher switches.",
    category: 'compatibility',
    advanced: false,
    community: false,
    applicable_methods: ['proton_run', 'steam_applaunch'],
    conflicts_with: ['skip_launcher', 'nolauncher'],
  },
  {
    id: 'force_opengl',
    tokens: ['-force-glcore'],
    label: 'Force OpenGL renderer (Unity)',
    description: 'Pass an OpenGL-forcing switch when supported.',
    help_text:
      'Unity standalone flag (-force-glcore); ignored by non-Unity engines. Occasionally stabilizes older Unity builds on Linux/Proton.',
    category: 'graphics',
    advanced: true,
    community: false,
    applicable_methods: ['proton_run', 'steam_applaunch'],
    conflicts_with: ['force_vulkan', 'force_dx11', 'force_dx12'],
  },
];

const MOCK_COMMAND_ARGUMENT_CATALOG: CommandArgumentCatalogPayload = {
  catalog_version: 1,
  entries: MOCK_COMMAND_ARGUMENT_ENTRIES,
};

/** Shared mock catalog entries for launch preview / Steam options handlers. */
export const mockCommandArgumentCatalogEntries: readonly CommandArgumentEntry[] = MOCK_COMMAND_ARGUMENT_ENTRIES;

// --- offline ---

const MOCK_TRAINER_TYPE_CATALOG: TrainerTypeEntry[] = [
  {
    id: 'unknown',
    display_name: 'Unknown',
    offline_capability: 'unknown',
    requires_network: false,
    detection_hints: [],
    score_cap: null,
    info_modal: null,
  },
  {
    id: 'wemod',
    display_name: 'WeMod',
    offline_capability: 'online_only',
    requires_network: true,
    detection_hints: ['WeMod'],
    score_cap: 20,
    info_modal: 'WeMod requires an active internet connection and account.',
  },
  {
    id: 'fling',
    display_name: 'FLiNG Trainer',
    offline_capability: 'full',
    requires_network: false,
    detection_hints: ['FLiNG', 'Mr. Antifun'],
    score_cap: null,
    info_modal: null,
  },
];

// --- Handler registration ---

export function registerSystem(map: Map<string, Handler>): void {
  // --- metadata store status (?metadata=newer|disabled previews the degraded banner) ---

  map.set('metadata_store_status', async () => {
    const { metadataState } = getActiveToggles();
    if (metadataState === 'newer') return { state: 'newer_schema', found: 99, supported: 27 };
    if (metadataState === 'disabled') {
      return { state: 'disabled', reason: '[dev-mock] metadata database could not be opened' };
    }
    return { state: 'ok' };
  });

  // --- bench (YAN-782) ---

  map.set('bench_ready', async (): Promise<void> => {
    // No-op: benchmark hooks are inert unless the real backend sees CROSSHOOK_BENCH=1.
  });

  // --- discovery ---

  map.set('discovery_catalog', async (args): Promise<CatalogPage> => {
    const { query } = args as { query?: CatalogQuery };
    return buildMockCatalogPage(structuredClone(MOCK_CATALOG_PROFILE_ENTRIES), query);
  });

  map.set('discovery_search_external', async (_args): Promise<ExternalTrainerSearchResponse> => {
    return structuredClone(MOCK_EXTERNAL_SEARCH_RESPONSE);
  });

  map.set('discovery_check_version_compatibility', async (_args): Promise<VersionMatchResult> => {
    return {
      status: 'unknown',
      trainerGameVersion: undefined,
      installedGameVersion: undefined,
      detail: '[dev-mock] version compatibility always returns unknown in browser mode',
    };
  });

  map.set('discovery_list_external_sources', async (): Promise<ExternalTrainerSourceSubscription[]> => {
    return structuredClone(externalSources);
  });

  map.set('discovery_add_external_source', async (args): Promise<ExternalTrainerSourceSubscription[]> => {
    const { source } = args as { source: ExternalTrainerSourceSubscription };
    if (externalSources.some((s) => s.sourceId === source.sourceId)) {
      throw new Error(`[dev-mock] source with id "${source.sourceId}" already exists`);
    }
    externalSources = [...externalSources, source];
    return structuredClone(externalSources);
  });

  map.set('discovery_remove_external_source', async (args): Promise<ExternalTrainerSourceSubscription[]> => {
    const { source_id } = args as { source_id: string };
    const before = externalSources.length;
    externalSources = externalSources.filter((s) => s.sourceId !== source_id);
    if (externalSources.length === before) {
      throw new Error(`[dev-mock] no source with id "${source_id}" found`);
    }
    return structuredClone(externalSources);
  });

  // --- run_executable ---

  map.set('validate_run_executable_request', async (_args): Promise<void> => {
    console.warn('[dev-mock] validate_run_executable_request: suppressed in browser mode');
  });

  map.set('run_executable', async (_args): Promise<RunExecutableResult> => {
    console.warn('[dev-mock] run_executable: suppressed in browser mode — no process spawned');
    return {
      succeeded: true,
      message: '[dev-mock] run_executable: browser stub — no process spawned',
      helper_log_path: '/mock/logs/run-executable.log',
      resolved_prefix_path: '/home/devuser/.local/share/crosshook/_run-adhoc/mock-slug',
    };
  });

  map.set('cancel_run_executable', async (): Promise<void> => {
    console.warn('[dev-mock] cancel_run_executable: suppressed in browser mode');
  });

  map.set('stop_run_executable', async (): Promise<void> => {
    console.warn('[dev-mock] stop_run_executable: suppressed in browser mode');
  });

  // --- prefix_storage ---

  map.set('scan_prefix_storage', async (): Promise<PrefixStorageScanResult> => {
    return structuredClone(MOCK_PREFIX_SCAN);
  });

  map.set('cleanup_prefix_storage', async (args): Promise<PrefixCleanupResult> => {
    const { targets } = args as { targets: PrefixCleanupTarget[] };
    console.warn('[dev-mock] cleanup_prefix_storage: suppressed in browser mode');
    return {
      deleted: targets ?? [],
      skipped: [],
      reclaimed_bytes: 0,
    };
  });

  map.set('get_prefix_storage_history', async (): Promise<PrefixStorageHistoryResponse> => {
    return {
      available: true,
      snapshots: [],
      audit: [],
    };
  });

  // --- prefix_deps ---

  map.set('detect_protontricks_binary', async (): Promise<BinaryDetectionResult> => {
    return structuredClone(MOCK_BINARY_DETECTION);
  });

  map.set('check_prefix_dependencies', async (args): Promise<PrefixDependencyStatus[]> => {
    const { packages } = args as { packages: string[] };
    return (packages ?? []).map((pkg) => ({
      package_name: pkg,
      state: 'installed' as const,
      checked_at: new Date().toISOString(),
      installed_at: new Date().toISOString(),
      last_error: null,
    }));
  });

  map.set('install_prefix_dependency', async (_args): Promise<void> => {
    console.warn('[dev-mock] install_prefix_dependency: suppressed in browser mode — no install performed');
  });

  map.set('get_dependency_status', async (_args): Promise<PrefixDependencyStatus[]> => {
    return [];
  });

  map.set('get_prefix_version_repair_status', async (_args): Promise<PrefixVersionRepairStatus> => {
    return structuredClone(MOCK_PREFIX_VERSION_REPAIR_STATUS);
  });

  map.set('repair_prefix_windows_version', async (_args): Promise<PrefixVersionRepairStatus> => {
    console.warn('[dev-mock] repair_prefix_windows_version: no repair required in browser mode');
    return structuredClone(MOCK_PREFIX_VERSION_REPAIR_STATUS);
  });

  // --- diagnostics ---

  map.set('export_diagnostics', async (_args): Promise<DiagnosticBundleResult> => {
    console.warn('[dev-mock] export_diagnostics: suppressed in browser mode — no bundle written');
    return {
      archive_path: '/mock/diagnostics/crosshook-diagnostics-mock.zip',
      summary: {
        crosshook_version: '0.0.0-mock',
        profile_count: 8,
        log_file_count: 0,
        proton_install_count: 0,
        generated_at: new Date().toISOString(),
      },
    };
  });

  // --- catalog ---

  map.set('get_optimization_catalog', async (): Promise<OptimizationCatalogPayload> => {
    return structuredClone(MOCK_CATALOG);
  });

  map.set('get_command_argument_catalog', async (): Promise<CommandArgumentCatalogPayload> => {
    const fixture = getActiveFixture();
    if (fixture === 'error') throw forcedError('get_command_argument_catalog');
    if (fixture === 'loading') return neverResolving<CommandArgumentCatalogPayload>();
    if (fixture === 'empty') {
      return { catalog_version: MOCK_COMMAND_ARGUMENT_CATALOG.catalog_version, entries: [] };
    }
    return structuredClone(MOCK_COMMAND_ARGUMENT_CATALOG);
  });

  map.set('get_mangohud_presets', async () => {
    // MangoHud presets type is not exported from a shared TS type; return empty array.
    // The frontend falls back gracefully when no presets are available.
    return [];
  });

  // --- offline ---

  map.set('check_offline_readiness', async (args): Promise<OfflineReadinessReport> => {
    const { name } = args as { name: string };
    return {
      profile_name: name,
      score: 100,
      readiness_state: 'ready',
      trainer_type: 'unknown',
      checks: [],
      blocking_reasons: [],
      checked_at: new Date().toISOString(),
    };
  });

  map.set('batch_offline_readiness', async (): Promise<OfflineReadinessReport[]> => {
    return [];
  });

  map.set('verify_trainer_hash', async (args): Promise<HashVerifyResult> => {
    const { name } = args as { name: string };
    console.warn(`[dev-mock] verify_trainer_hash: returning synthetic hash for profile "${name}"`);
    return {
      hash: 'aabbccdd00112233aabbccdd00112233aabbccdd00112233aabbccdd00112233',
      from_cache: true,
      file_size: 1_048_576,
    };
  });

  map.set('check_network_status', async (): Promise<boolean> => {
    return false;
  });

  map.set('get_trainer_type_catalog', async (): Promise<TrainerTypeEntry[]> => {
    return structuredClone(MOCK_TRAINER_TYPE_CATALOG);
  });

  // --- background portal ---

  map.set('get_background_protection_state', async (): Promise<string> => {
    // Return 'NotApplicable' for non-Flatpak dev environment
    return 'NotApplicable';
  });

  // --- path normalization ---

  map.set('normalize_host_path', async (args): Promise<string> => {
    const { path } = args as { path: string };
    // In dev mode, just return the path as-is (no Flatpak normalization needed)
    return path;
  });
}

export function resetSystemMockState(): void {
  externalSources = structuredClone(DEFAULT_EXTERNAL_SOURCES);
}
