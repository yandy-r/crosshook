import type { Handler } from './types';

import type {
  TrainerSearchResponse,
  ExternalTrainerSearchResponse,
  ExternalTrainerSourceSubscription,
  VersionMatchResult,
} from '../../../types/discovery';
import type {
  PrefixStorageScanResult,
  PrefixCleanupResult,
  PrefixStorageHistoryResponse,
  PrefixCleanupTarget,
} from '../../../types/prefix-storage';
import type { RunExecutableResult } from '../../../types/run-executable';
import type { BinaryDetectionResult, PrefixDependencyStatus } from '../../../types/prefix-deps';
import type { DiagnosticBundleResult } from '../../../types/diagnostics';
import type { OptimizationCatalogPayload, OptimizationEntry } from '../../../utils/optimization-catalog';
import type {
  OfflineReadinessReport,
  HashVerifyResult,
  TrainerTypeEntry,
} from '../../../types/offline';

// --- Module-scope state ---

let externalSources: ExternalTrainerSourceSubscription[] = [
  {
    sourceId: 'mock-source-1',
    displayName: 'Mock Trainer Index',
    baseUrl: 'https://mock.example.invalid/trainers',
    sourceType: 'rss',
    enabled: true,
  },
];

// --- discovery ---

const MOCK_TRAINER_RESULTS: TrainerSearchResponse = {
  results: [
    {
      id: 1,
      gameName: 'Synthetic Quest',
      steamAppId: 9999001,
      sourceName: 'Mock Trainer Index',
      sourceUrl: 'https://mock.example.invalid/trainers/synthetic-quest',
      trainerVersion: '1.0.0',
      gameVersion: '2.0.1',
      notes: 'Synthetic data — not a real trainer.',
      sha256: 'aabbccdd00112233aabbccdd00112233aabbccdd00112233aabbccdd00112233',
      relativePath: 'trainers/synthetic-quest/trainer.exe',
      tapUrl: 'https://mock.example.invalid/tap/synthetic-quest',
      tapLocalPath: '/mock/tap/synthetic-quest',
      relevanceScore: 0.95,
    },
    {
      id: 2,
      gameName: 'Dev Test Game',
      steamAppId: 9999002,
      sourceName: 'Mock Trainer Index',
      sourceUrl: 'https://mock.example.invalid/trainers/dev-test-game',
      trainerVersion: '0.9.0',
      gameVersion: '1.5.0',
      notes: undefined,
      sha256: undefined,
      relativePath: 'trainers/dev-test-game/trainer.exe',
      tapUrl: 'https://mock.example.invalid/tap/dev-test-game',
      tapLocalPath: '/mock/tap/dev-test-game',
      relevanceScore: 0.72,
    },
  ],
  totalCount: 2,
};

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
  // --- discovery ---

  map.set('discovery_search_trainers', async (_args): Promise<TrainerSearchResponse> => {
    return structuredClone(MOCK_TRAINER_RESULTS);
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

  // --- diagnostics ---

  map.set('export_diagnostics', async (_args): Promise<DiagnosticBundleResult> => {
    console.warn('[dev-mock] export_diagnostics: suppressed in browser mode — no bundle written');
    return {
      archive_path: '/mock/diagnostics/crosshook-diagnostics-mock.zip',
      summary: {
        crosshook_version: '0.0.0-mock',
        profile_count: 2,
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
}
