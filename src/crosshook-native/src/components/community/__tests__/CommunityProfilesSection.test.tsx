import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { CatalogEntry, CatalogFacets } from '@/types/discovery';
import { DEGRADED_CATALOG_MESSAGE } from '../../discovery/DiscoveryDegradedBanner';
import { CommunityProfilesSection, type CommunityProfilesSectionProps } from '../CommunityProfilesSection';

const EMPTY_FACETS: CatalogFacets = {
  gameTitles: [],
  loadingModes: [],
  compatibilityBands: [],
  taps: [],
};

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

function renderSection(overrides: Partial<CommunityProfilesSectionProps> = {}) {
  const props: CommunityProfilesSectionProps = {
    entries: [],
    totalCount: 0,
    facets: EMPTY_FACETS,
    catalogQuery: {},
    searchText: '',
    diagnostics: [],
    catalogLoading: false,
    catalogError: null,
    degraded: false,
    importing: false,
    notice: null,
    error: null,
    importedProfileNames: new Set<string>(),
    onSearchTextChange: vi.fn(),
    onSetFacet: vi.fn(),
    onClearFilters: vi.fn(),
    onLoadMore: vi.fn(),
    onImportFromFile: vi.fn(),
    onImportFromLutris: vi.fn(),
    onImportEntry: vi.fn(),
    ...overrides,
  };
  render(<CommunityProfilesSection {...props} />);
  return props;
}

describe('CommunityProfilesSection', () => {
  it('renders entries in backend order verbatim', () => {
    // Deliberately shuffled ranks and names: the section must not re-sort.
    const entries = [
      buildEntry({ relativePath: 'z.json', gameName: 'Zeta Game', compatibilityRating: 'broken' }),
      buildEntry({ relativePath: 'a.json', gameName: 'Alpha Game', compatibilityRating: 'platinum' }),
      buildEntry({ relativePath: 'm.json', gameName: 'Middle Game', compatibilityRating: 'working' }),
    ];
    renderSection({ entries, totalCount: 3 });

    const headings = screen.getAllByRole('heading', { level: 3 }).map((heading) => heading.textContent);
    expect(headings).toEqual(['Zeta Game', 'Alpha Game', 'Middle Game']);
  });

  it('facet bar replaces the rating select', () => {
    renderSection();

    for (const name of ['Game title', 'Loading mode', 'Compatibility', 'Source tap']) {
      expect(screen.getByRole('combobox', { name })).toBeInTheDocument();
    }
    // The old client-side rating filter select is gone.
    expect(screen.queryByRole('combobox', { name: 'Compatibility rating' })).not.toBeInTheDocument();
  });

  it('import uses manifestPath', async () => {
    const entry = buildEntry();
    const props = renderSection({ entries: [entry], totalCount: 1 });

    await userEvent.click(screen.getByRole('button', { name: 'Import' }));

    expect(props.onImportEntry).toHaveBeenCalledTimes(1);
    const imported = (props.onImportEntry as ReturnType<typeof vi.fn>).mock.calls[0][0] as CatalogEntry;
    expect(imported.manifestPath).toBe('/tmp/tap/elden/community-profile.json');
  });

  it('summary shows visible-of-total counts and show more forwards to onLoadMore', async () => {
    const props = renderSection({ entries: [buildEntry()], totalCount: 4 });

    expect(screen.getByText('1 of 4 profiles')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Show more (1 of 4)' }));
    expect(props.onLoadMore).toHaveBeenCalledTimes(1);
  });

  it('search input forwards to onSearchTextChange', async () => {
    const props = renderSection();

    await userEvent.type(screen.getByRole('textbox', { name: 'Search profiles' }), 'e');

    expect(props.onSearchTextChange).toHaveBeenCalledWith('e');
  });

  it('renders the degraded banner with role status', () => {
    renderSection({ degraded: true });

    expect(screen.getByText(DEGRADED_CATALOG_MESSAGE)).toHaveAttribute('role', 'status');
  });

  it('marks already-imported entries instead of an import button', () => {
    const entry = buildEntry();
    renderSection({
      entries: [entry],
      totalCount: 1,
      importedProfileNames: new Set(['elden-ring']),
    });

    expect(screen.getByText('Imported')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Import' })).not.toBeInTheDocument();
  });

  it('hides the import button for source-only entries without a manifest', () => {
    const entry = buildEntry({
      relativePath: 'trainer-sources/source-only-game',
      manifestPath: '',
      gameName: 'Source Only Game',
    });
    renderSection({ entries: [entry], totalCount: 1 });

    expect(screen.queryByRole('button', { name: 'Import' })).not.toBeInTheDocument();
    expect(screen.queryByText('Imported')).not.toBeInTheDocument();
  });
});
