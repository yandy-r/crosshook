import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { UseCommunityCatalogReturn } from '@/hooks/useCommunityCatalog';
import type { CatalogEntry, CatalogPage } from '@/types/discovery';
import { DiscoveryCatalogSection } from '../DiscoveryCatalogSection';
import { DEGRADED_CATALOG_MESSAGE } from '../DiscoveryDegradedBanner';

vi.mock('@/lib/plugin-stubs/shell', () => ({
  open: vi.fn().mockResolvedValue(undefined),
}));

function buildEntry(overrides: Partial<CatalogEntry> = {}): CatalogEntry {
  return {
    id: 1,
    tapUrl: 'https://example.com/tap.git',
    tapLocalPath: '/tmp/tap',
    relativePath: 'elden/community-profile.json',
    manifestPath: '/tmp/tap/elden/community-profile.json',
    gameName: 'Elden Ring',
    compatibilityRating: 'working',
    trainerLoadingMode: 'source_directory',
    schemaVersion: 1,
    sources: [],
    ...overrides,
  };
}

function buildPage(overrides: Partial<CatalogPage> = {}): CatalogPage {
  return {
    entries: [],
    facets: {
      gameTitles: [{ value: 'Elden Ring', count: 1 }],
      loadingModes: [{ value: 'source_directory', count: 1 }],
      compatibilityBands: [{ value: 'working', count: 1 }],
      taps: [{ value: 'https://example.com/tap.git', count: 1 }],
    },
    totalCount: 0,
    tapCount: 0,
    degraded: false,
    ...overrides,
  };
}

function buildCatalog(overrides: Partial<UseCommunityCatalogReturn> = {}): UseCommunityCatalogReturn {
  return {
    data: null,
    loading: false,
    error: null,
    query: {},
    searchText: '',
    setSearchText: vi.fn(),
    setFacet: vi.fn(),
    clearFilters: vi.fn(),
    loadMore: vi.fn().mockResolvedValue(undefined),
    refresh: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function renderSection(catalog: UseCommunityCatalogReturn, importingKey: string | null = null) {
  const onImport = vi.fn();
  render(<DiscoveryCatalogSection catalog={catalog} importingKey={importingKey} onImport={onImport} />);
  return { onImport };
}

describe('DiscoveryCatalogSection', () => {
  it('renders the catalog list without any query', () => {
    const entries = [buildEntry(), buildEntry({ relativePath: 'sekiro/community-profile.json', gameName: 'Sekiro' })];
    renderSection(buildCatalog({ data: buildPage({ entries, totalCount: 2, tapCount: 1 }) }));

    const list = screen.getByRole('list');
    expect(within(list).getAllByRole('listitem')).toHaveLength(2);
    expect(screen.getByRole('heading', { name: 'Elden Ring' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Sekiro' })).toBeInTheDocument();
  });

  it('typing forwards to setSearchText', async () => {
    const catalog = buildCatalog({ data: buildPage({ tapCount: 1 }) });
    renderSection(catalog);

    await userEvent.type(screen.getByRole('searchbox', { name: 'Search trainer catalog' }), 'e');

    expect(catalog.setSearchText).toHaveBeenCalledWith('e');
  });

  it('aria-live count line shows tap coverage only for the unfiltered catalog', () => {
    const entries = [buildEntry()];
    renderSection(buildCatalog({ data: buildPage({ entries, totalCount: 3, tapCount: 2 }) }));

    const status = screen.getByText('3 trainer profiles across 2 taps');
    expect(status.closest('[role="status"]')).toHaveAttribute('aria-live', 'polite');
  });

  it('count line drops the tap coverage while a search or facet filter is active', () => {
    const entries = [buildEntry()];
    renderSection(buildCatalog({ data: buildPage({ entries, totalCount: 3, tapCount: 2 }), searchText: 'elden' }));
    expect(screen.getByText('3 trainer profiles')).toBeInTheDocument();
    expect(screen.queryByText(/across \d+ taps?/)).not.toBeInTheDocument();

    document.body.innerHTML = '';
    renderSection(
      buildCatalog({
        data: buildPage({ entries, totalCount: 3, tapCount: 2 }),
        query: { compatibilityBands: ['working'] },
      })
    );
    expect(screen.getByText('3 trainer profiles')).toBeInTheDocument();
    expect(screen.queryByText(/across \d+ taps?/)).not.toBeInTheDocument();
  });

  it('announces loading through the live region and leaves no-match copy to the empty state', () => {
    renderSection(buildCatalog({ loading: true }));
    expect(screen.getByText('Loading trainer catalog…')).toBeInTheDocument();

    document.body.innerHTML = '';
    renderSection(buildCatalog({ data: buildPage({ totalCount: 0, tapCount: 1 }) }));
    expect(screen.getAllByText('No trainers match the current filters.')).toHaveLength(1);
  });

  it('ArrowDown roves focus across data-roving-item cards', () => {
    const entries = [buildEntry(), buildEntry({ relativePath: 'sekiro/community-profile.json', gameName: 'Sekiro' })];
    renderSection(buildCatalog({ data: buildPage({ entries, totalCount: 2, tapCount: 1 }) }));

    const cards = Array.from(document.querySelectorAll<HTMLElement>('[data-roving-item]'));
    expect(cards).toHaveLength(2);
    expect(cards[0].tabIndex).toBe(0);
    expect(cards[1].tabIndex).toBe(-1);

    act(() => {
      cards[0].focus();
      cards[0].dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
    });

    expect(document.activeElement).toBe(cards[1]);
    expect(cards[1].tabIndex).toBe(0);
    expect(cards[0].tabIndex).toBe(-1);
  });

  it('tapCount zero shows guidance empty state and hides the facet bar', () => {
    renderSection(buildCatalog({ data: buildPage({ tapCount: 0 }) }));

    expect(screen.getByText('No community taps are synced yet.')).toBeInTheDocument();
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
    expect(screen.queryByRole('list')).not.toBeInTheDocument();
  });

  it('shows the no-matches empty state when taps exist but nothing matches', () => {
    renderSection(buildCatalog({ data: buildPage({ totalCount: 0, tapCount: 1 }) }));

    expect(screen.getByText('No trainers match the current filters.')).toBeInTheDocument();
    // Facet bar stays visible so filters can be removed.
    expect(screen.getByRole('combobox', { name: 'Game title' })).toBeInTheDocument();
  });

  it('degraded banner has role status and the list still renders', () => {
    const entries = [buildEntry()];
    renderSection(buildCatalog({ data: buildPage({ entries, totalCount: 1, tapCount: 1, degraded: true }) }));

    const banner = screen.getByText(DEGRADED_CATALOG_MESSAGE);
    expect(banner).toHaveAttribute('role', 'status');
    expect(screen.getAllByRole('listitem')).toHaveLength(1);
  });

  it('shows an alert banner when the catalog errors', () => {
    renderSection(buildCatalog({ error: 'metadata store unavailable' }));

    expect(screen.getByRole('alert')).toHaveTextContent('metadata store unavailable');
  });

  it('entries render in backend order verbatim', () => {
    // Deliberately shuffled ranks: the section must not re-sort.
    const entries = [
      buildEntry({ relativePath: 'b.json', gameName: 'Broken Game', compatibilityRating: 'broken' }),
      buildEntry({ relativePath: 'p.json', gameName: 'Platinum Game', compatibilityRating: 'platinum' }),
      buildEntry({ relativePath: 'a.json', gameName: 'Alpha Game', compatibilityRating: 'working' }),
    ];
    renderSection(buildCatalog({ data: buildPage({ entries, totalCount: 3, tapCount: 1 }) }));

    const headings = screen.getAllByRole('heading', { level: 3 }).map((heading) => heading.textContent);
    expect(headings).toEqual(['Broken Game', 'Platinum Game', 'Alpha Game']);
  });

  it('show more calls loadMore when entries fewer than totalCount', async () => {
    const entries = [buildEntry()];
    const catalog = buildCatalog({ data: buildPage({ entries, totalCount: 5, tapCount: 1 }) });
    renderSection(catalog);

    await userEvent.click(screen.getByRole('button', { name: 'Show more (1 of 5)' }));

    expect(catalog.loadMore).toHaveBeenCalledTimes(1);
  });

  it('hides show more when all entries are visible', () => {
    const entries = [buildEntry()];
    renderSection(buildCatalog({ data: buildPage({ entries, totalCount: 1, tapCount: 1 }) }));

    expect(screen.queryByRole('button', { name: /Show more/ })).not.toBeInTheDocument();
  });

  it('marks the importing entry busy', () => {
    const entry = buildEntry();
    renderSection(
      buildCatalog({ data: buildPage({ entries: [entry], totalCount: 1, tapCount: 1 }) }),
      `${entry.tapUrl}::${entry.relativePath}`
    );

    expect(screen.getByRole('button', { name: 'Importing…' })).toBeDisabled();
  });
});
