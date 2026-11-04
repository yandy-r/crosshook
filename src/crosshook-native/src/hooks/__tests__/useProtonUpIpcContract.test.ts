import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { resolveProtonUpProviderForVersion, useProtonUp } from '../useProtonUp';

// Tauri v2 maps the Rust `force_refresh: Option<bool>` param to the top-level
// invoke key `forceRefresh`. A snake_case key is dropped, so the param is
// always None and a forced catalog refresh never happens.
const callCommandMock = vi.fn();
vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

const EMPTY_CATALOG = { versions: [], cache: { stale: false, offline: false } };

function listVersionsCalls(): Record<string, unknown>[] {
  return callCommandMock.mock.calls
    .filter(([name]) => name === 'protonup_list_available_versions')
    .map(([, args]) => args as Record<string, unknown>);
}

describe('useProtonUp IPC contract', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    callCommandMock.mockResolvedValue(EMPTY_CATALOG);
  });

  it('resolveProtonUpProviderForVersion sends forceRefresh=false as a camelCase key', async () => {
    await resolveProtonUpProviderForVersion('GE-Proton9-1');

    const calls = listVersionsCalls();
    expect(calls.length).toBeGreaterThan(0);
    for (const args of calls) {
      expect(args).toHaveProperty('forceRefresh', false);
      expect(args).not.toHaveProperty('force_refresh');
    }
  });

  it('sends forceRefresh=false on the initial fetch and forceRefresh=true after refreshCatalog', async () => {
    const { result } = renderHook(() => useProtonUp({ autoFetchCatalog: true, catalogProvider: 'ge-proton' }));

    await waitFor(() => expect(listVersionsCalls()).toHaveLength(1));
    expect(listVersionsCalls()[0]).toEqual({ provider: 'ge-proton', forceRefresh: false });

    act(() => {
      result.current.refreshCatalog();
    });

    await waitFor(() => expect(listVersionsCalls()).toHaveLength(2));
    const refreshArgs = listVersionsCalls()[1];
    expect(refreshArgs).toHaveProperty('forceRefresh', true);
    expect(refreshArgs).not.toHaveProperty('force_refresh');
  });

  it('forces only the refresh fetch: a later provider change reads the cache again', async () => {
    const { result, rerender } = renderHook(
      ({ provider }: { provider: 'ge-proton' | 'proton-cachyos' }) =>
        useProtonUp({ autoFetchCatalog: true, catalogProvider: provider }),
      { initialProps: { provider: 'ge-proton' } }
    );

    await waitFor(() => expect(listVersionsCalls()).toHaveLength(1));

    act(() => {
      result.current.refreshCatalog();
    });

    await waitFor(() => expect(listVersionsCalls()).toHaveLength(2));
    expect(listVersionsCalls()[1]).toEqual({ provider: 'ge-proton', forceRefresh: true });
    await waitFor(() => expect(result.current.catalogLoading).toBe(false));

    rerender({ provider: 'proton-cachyos' });

    await waitFor(() => expect(listVersionsCalls()).toHaveLength(3));
    expect(listVersionsCalls()[2]).toEqual({ provider: 'proton-cachyos', forceRefresh: false });
  });

  it('keeps the refresh intent when the forced fetch is superseded before it resolves', async () => {
    const { result, rerender } = renderHook(
      ({ provider }: { provider: 'ge-proton' | 'proton-cachyos' }) =>
        useProtonUp({ autoFetchCatalog: true, catalogProvider: provider }),
      { initialProps: { provider: 'ge-proton' } }
    );

    await waitFor(() => expect(listVersionsCalls()).toHaveLength(1));

    const refreshGate = Promise.withResolvers<typeof EMPTY_CATALOG>();
    callCommandMock.mockReturnValueOnce(refreshGate.promise);
    act(() => {
      result.current.refreshCatalog();
    });
    await waitFor(() => expect(listVersionsCalls()).toHaveLength(2));

    rerender({ provider: 'proton-cachyos' });

    await waitFor(() => expect(listVersionsCalls()).toHaveLength(3));
    expect(listVersionsCalls()[2]).toEqual({ provider: 'proton-cachyos', forceRefresh: true });
    await act(async () => {
      refreshGate.resolve(EMPTY_CATALOG);
      await refreshGate.promise;
    });
  });
});
