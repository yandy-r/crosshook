import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CatalogEntry, CatalogPage, CatalogQuery } from '@/types/discovery';
import { useCommunityCatalog } from '../useCommunityCatalog';

const callCommandMock = vi.fn();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

function buildEntry(overrides: Partial<CatalogEntry> = {}): CatalogEntry {
  return {
    id: 1,
    tapUrl: 'https://example.com/tap.git',
    tapLocalPath: '/tmp/tap',
    relativePath: 'elden/community-profile.json',
    manifestPath: '/tmp/tap/elden/community-profile.json',
    gameName: 'Elden Ring',
    schemaVersion: 1,
    sources: [],
    ...overrides,
  };
}

function buildPage(overrides: Partial<CatalogPage> = {}): CatalogPage {
  return {
    entries: [],
    facets: { gameTitles: [], loadingModes: [], compatibilityBands: [], taps: [] },
    totalCount: 0,
    tapCount: 0,
    degraded: false,
    ...overrides,
  };
}

function lastQuery(): CatalogQuery {
  const call = callCommandMock.mock.calls.at(-1);
  if (!call) {
    throw new Error('callCommand was never invoked');
  }
  return (call[1] as { query: CatalogQuery }).query;
}

async function flush(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
  });
}

describe('useCommunityCatalog', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    callCommandMock.mockReset();
    callCommandMock.mockResolvedValue(buildPage());
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('fetches the full catalog on mount with an empty query', async () => {
    const page = buildPage({ entries: [buildEntry()], totalCount: 1, tapCount: 1 });
    callCommandMock.mockResolvedValue(page);

    const { result } = renderHook(() => useCommunityCatalog());
    await flush();

    expect(callCommandMock).toHaveBeenCalledTimes(1);
    expect(callCommandMock).toHaveBeenCalledWith('discovery_catalog', { query: {} });
    expect(result.current.data).toEqual(page);
    expect(result.current.loading).toBe(false);
  });

  it('debounces search text by 300ms and sends query.query', async () => {
    const { result } = renderHook(() => useCommunityCatalog());
    await flush();
    expect(callCommandMock).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.setSearchText('eld');
      result.current.setSearchText('elden');
    });
    expect(result.current.searchText).toBe('elden');

    act(() => {
      vi.advanceTimersByTime(299);
    });
    expect(callCommandMock).toHaveBeenCalledTimes(1);

    act(() => {
      vi.advanceTimersByTime(1);
    });
    await flush();

    expect(callCommandMock).toHaveBeenCalledTimes(2);
    expect(lastQuery().query).toBe('elden');
  });

  it('ignores stale out-of-order responses', async () => {
    const resolvers: Array<(page: CatalogPage) => void> = [];
    callCommandMock.mockImplementation(
      () =>
        new Promise<CatalogPage>((resolve) => {
          resolvers.push(resolve);
        })
    );

    const { result } = renderHook(() => useCommunityCatalog());
    await flush();

    act(() => {
      result.current.setFacet('compatibility', 'working');
    });
    await flush();
    expect(resolvers).toHaveLength(2);

    const stalePage = buildPage({ totalCount: 111 });
    const freshPage = buildPage({ totalCount: 222 });

    await act(async () => {
      resolvers[1](freshPage);
      await Promise.resolve();
    });
    await act(async () => {
      resolvers[0](stalePage);
      await Promise.resolve();
    });

    expect(result.current.data?.totalCount).toBe(222);
    expect(result.current.loading).toBe(false);
  });

  it('facet change refetches immediately and resets offset', async () => {
    const { result } = renderHook(() => useCommunityCatalog());
    await flush();

    act(() => {
      result.current.setFacet('loadingMode', 'copy_to_prefix');
    });
    await flush();

    expect(callCommandMock).toHaveBeenCalledTimes(2);
    const query = lastQuery();
    expect(query.loadingModes).toEqual(['copy_to_prefix']);
    expect(query.offset).toBeUndefined();
    expect(result.current.query.loadingModes).toEqual(['copy_to_prefix']);
  });

  it('enabled false performs zero IPC', async () => {
    const { result } = renderHook(() => useCommunityCatalog({ enabled: false }));
    await flush();

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    await flush();

    expect(callCommandMock).not.toHaveBeenCalled();
    expect(result.current.data).toBeNull();
  });

  it('loadMore appends entries preserving order and updates totalCount', async () => {
    const first = buildEntry({ relativePath: 'a.json' });
    const second = buildEntry({ relativePath: 'b.json', gameName: 'Sekiro' });
    callCommandMock.mockResolvedValueOnce(buildPage({ entries: [first], totalCount: 2, tapCount: 1 }));

    const { result } = renderHook(() => useCommunityCatalog());
    await flush();
    expect(result.current.data?.entries).toEqual([first]);

    callCommandMock.mockResolvedValueOnce(buildPage({ entries: [second], totalCount: 2, tapCount: 1 }));
    await act(async () => {
      await result.current.loadMore();
    });

    expect(lastQuery().offset).toBe(1);
    expect(result.current.data?.entries).toEqual([first, second]);
    expect(result.current.data?.totalCount).toBe(2);
  });

  it('loadMore no-ops while a filter change fetch is in flight, keeping the new page clean', async () => {
    const resolvers: Array<(page: CatalogPage) => void> = [];
    callCommandMock.mockImplementation(
      () =>
        new Promise<CatalogPage>((resolve) => {
          resolvers.push(resolve);
        })
    );

    const { result } = renderHook(() => useCommunityCatalog());
    await flush();
    expect(resolvers).toHaveLength(1);

    const initialEntry = buildEntry({ relativePath: 'a.json' });
    await act(async () => {
      resolvers[0](buildPage({ entries: [initialEntry], totalCount: 2, tapCount: 1 }));
      await Promise.resolve();
    });
    expect(result.current.data?.entries).toEqual([initialEntry]);

    // Change facet, then loadMore before the page-1 fetch for the new query resolves.
    act(() => {
      result.current.setFacet('compatibility', 'working');
    });
    await flush();
    expect(resolvers).toHaveLength(2);

    await act(async () => {
      await result.current.loadMore();
    });
    expect(callCommandMock).toHaveBeenCalledTimes(2);
    expect(lastQuery().offset).toBeUndefined();

    const freshEntry = buildEntry({ relativePath: 'working.json', gameName: 'Sekiro' });
    const freshPage = buildPage({ entries: [freshEntry], totalCount: 1, tapCount: 1 });
    await act(async () => {
      resolvers[1](freshPage);
      await Promise.resolve();
    });

    expect(result.current.data?.entries).toEqual([freshEntry]);
    expect(result.current.data?.totalCount).toBe(1);
  });

  it('clearFilters resets to the unfiltered catalog', async () => {
    const { result } = renderHook(() => useCommunityCatalog());
    await flush();

    act(() => {
      result.current.setFacet('gameTitle', 'Elden Ring');
    });
    await flush();
    act(() => {
      result.current.setFacet('tap', 'https://example.com/tap.git');
    });
    await flush();

    act(() => {
      result.current.clearFilters();
    });
    await flush();

    const query = lastQuery();
    expect(query.gameTitles).toEqual([]);
    expect(query.loadingModes).toEqual([]);
    expect(query.compatibilityBands).toEqual([]);
    expect(query.tapUrls).toEqual([]);
    expect(query.offset).toBeUndefined();
  });

  it('passes degraded flag through', async () => {
    callCommandMock.mockResolvedValue(buildPage({ degraded: true }));

    const { result } = renderHook(() => useCommunityCatalog());
    await flush();

    expect(result.current.data?.degraded).toBe(true);
  });

  it('sets error and keeps data null when the initial fetch rejects', async () => {
    callCommandMock.mockRejectedValue(new Error('metadata store unavailable'));

    const { result } = renderHook(() => useCommunityCatalog());
    await flush();

    expect(result.current.error).toBe('metadata store unavailable');
    expect(result.current.data).toBeNull();
    expect(result.current.loading).toBe(false);
  });
});
