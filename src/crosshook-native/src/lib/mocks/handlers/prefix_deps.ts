import type {
  BinaryDetectionResult,
  PrefixDepCompletePayload,
  PrefixDependencyStatus,
  PrefixVersionRepairStatus,
} from '../../../types/prefix-deps';
import { emitMockEvent } from '../eventBus';
import type { Handler } from './types';

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
  in_progress: false,
  last_recovery: null,
};

const PREFIX_DEP_LINE_INTERVAL_MS = 300;
const pendingPrefixDepTimers = new Set<number>();

function schedulePrefixDepTimeout(callback: () => void, delayMs: number): void {
  const id = window.setTimeout(() => {
    pendingPrefixDepTimers.delete(id);
    callback();
  }, delayMs);
  pendingPrefixDepTimers.add(id);
}

export function registerPrefixDeps(map: Map<string, Handler>): void {
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

  map.set('install_prefix_dependency', async (args): Promise<void> => {
    const { profileName, prefixPath, packages } = args as {
      profileName: string;
      prefixPath: string;
      packages: string[];
    };
    const lines = (packages ?? []).flatMap((pkg) => [
      `[dev-mock] Executing winetricks ${pkg}`,
      `[dev-mock] ${pkg} installed`,
    ]);
    lines.forEach((line, index) => {
      schedulePrefixDepTimeout(
        () => {
          emitMockEvent('prefix-dep-log', { profile_name: profileName, prefix_path: prefixPath, line });
        },
        PREFIX_DEP_LINE_INTERVAL_MS * (index + 1)
      );
    });
    schedulePrefixDepTimeout(
      () => {
        const payload: PrefixDepCompletePayload = {
          profile_name: profileName,
          prefix_path: prefixPath,
          succeeded: true,
          exit_code: 0,
          install_succeeded: true,
          install_exit_code: 0,
          restore_state: 'not_required',
          restore_error: null,
        };
        emitMockEvent('prefix-dep-complete', payload);
      },
      PREFIX_DEP_LINE_INTERVAL_MS * (lines.length + 1)
    );
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
}

export function resetPrefixDepsMockState(): void {
  for (const timerId of pendingPrefixDepTimers) {
    window.clearTimeout(timerId);
  }
  pendingPrefixDepTimers.clear();
}
