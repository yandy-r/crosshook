import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useProtonManager } from '../useProtonManager';

// Tauri v2 maps Rust `force_refresh` / `steam_client_install_path` params to the
// top-level invoke keys `forceRefresh` / `steamClientInstallPath`. Snake_case
// keys are dropped, so the Option params are always None.
const callCommandMock = vi.fn();
vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

const STEAM_PATH = '/home/user/.local/share/Steam';

function callsFor(command: string): Record<string, unknown>[] {
  return callCommandMock.mock.calls
    .filter(([name]) => name === command)
    .map(([, args]) => args as Record<string, unknown>);
}

describe('useProtonManager IPC contract', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    callCommandMock.mockImplementation((name: string) => {
      switch (name) {
        case 'protonup_list_providers':
          return Promise.resolve([
            { id: 'ge-proton', display_name: 'GE-Proton', supports_install: true, checksum_kind: 'sha512-sidecar' },
          ]);
        case 'protonup_list_available_versions':
          return Promise.resolve({ versions: [], cache: { stale: false, offline: false } });
        default:
          return Promise.resolve([]);
      }
    });
  });

  it('resolves install roots with a camelCase steamClientInstallPath key', async () => {
    renderHook(() => useProtonManager({ steamClientInstallPath: STEAM_PATH }));

    await waitFor(() => expect(callsFor('protonup_resolve_install_roots')).toHaveLength(1));
    const args = callsFor('protonup_resolve_install_roots')[0];
    expect(args).toHaveProperty('steamClientInstallPath', STEAM_PATH);
    expect(args).not.toHaveProperty('steam_client_install_path');
  });

  it('fans out catalog fetches with a camelCase forceRefresh key', async () => {
    const { result } = renderHook(() => useProtonManager({ steamClientInstallPath: STEAM_PATH }));

    await waitFor(() => expect(callsFor('protonup_list_available_versions')).toHaveLength(1));
    expect(callsFor('protonup_list_available_versions')[0]).toEqual({ provider: 'ge-proton', forceRefresh: false });

    act(() => {
      result.current.catalog.refreshCatalog();
    });

    await waitFor(() => expect(callsFor('protonup_list_available_versions')).toHaveLength(2));
    const refreshArgs = callsFor('protonup_list_available_versions')[1];
    expect(refreshArgs).toHaveProperty('forceRefresh', true);
    expect(refreshArgs).not.toHaveProperty('force_refresh');
  });
});
