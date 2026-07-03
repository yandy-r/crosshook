import type { UseCommunityCatalogReturn } from '../../hooks/useCommunityCatalog';
import { useRovingTabindex } from '../../hooks/useRovingTabindex';
import type { CatalogEntry, CatalogQuery } from '../../types/discovery';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import { DiscoveryDegradedBanner } from './DiscoveryDegradedBanner';
import { DiscoveryEmptyState } from './DiscoveryEmptyState';
import { DiscoveryFacetBar } from './DiscoveryFacetBar';
import { DiscoveryResultCard } from './DiscoveryResultCard';
import { DiscoveryShowMoreButton } from './DiscoveryShowMoreButton';

export function catalogEntryKey(entry: CatalogEntry): string {
  return `${entry.tapUrl}::${entry.relativePath}`;
}

function hasActiveFilters(query: CatalogQuery, searchText: string): boolean {
  return (
    searchText.trim().length > 0 ||
    (query.gameTitles?.length ?? 0) > 0 ||
    (query.loadingModes?.length ?? 0) > 0 ||
    (query.compatibilityBands?.length ?? 0) > 0 ||
    (query.tapUrls?.length ?? 0) > 0
  );
}

export interface DiscoveryCatalogSectionProps {
  catalog: UseCommunityCatalogReturn;
  /** `${tapUrl}::${relativePath}` of the entry currently importing, or null. */
  importingKey: string | null;
  onImport: (entry: CatalogEntry) => void;
  onNavigateToBrowse?: () => void;
}

export function DiscoveryCatalogSection({
  catalog,
  importingKey,
  onImport,
  onNavigateToBrowse,
}: DiscoveryCatalogSectionProps) {
  const listRef = useRovingTabindex({ itemSelector: '[data-roving-item]' });
  const { data, loading, error } = catalog;

  const entries = data?.entries ?? [];
  const totalCount = data?.totalCount ?? 0;
  const tapCount = data?.tapCount ?? 0;
  const noTaps = data !== null && tapCount === 0;
  const filtered = hasActiveFilters(catalog.query, catalog.searchText);

  return (
    <DashboardPanelSection
      eyebrow="Community"
      title="Trainer Catalog"
      summary="Browse trainer profiles across your synced community taps. CrossHook guides you to sources — it never hosts or distributes trainers."
      titleAs="h2"
    >
      <div className="crosshook-discovery-panel__search-row">
        <div className="crosshook-discovery-search crosshook-discovery-panel__search-field">
          <label className="crosshook-label" htmlFor="discovery-catalog-search">
            Search trainer catalog
          </label>
          <input
            id="discovery-catalog-search"
            className="crosshook-input"
            type="search"
            value={catalog.searchText}
            onChange={(event) => catalog.setSearchText(event.target.value)}
            placeholder="Search games or trainers..."
            aria-label="Search trainer catalog"
          />
          {catalog.searchText.length > 0 && (
            <button
              type="button"
              className="crosshook-discovery-search__clear"
              aria-label="Clear search"
              onClick={() => catalog.setSearchText('')}
            >
              ×
            </button>
          )}
        </div>
      </div>

      {data && !noTaps ? (
        <DiscoveryFacetBar
          facets={data.facets}
          query={catalog.query}
          onSetFacet={catalog.setFacet}
          onClearFilters={catalog.clearFilters}
        />
      ) : null}

      {error ? (
        <div className="crosshook-error-banner crosshook-error-banner--section" role="alert">
          {error}
        </div>
      ) : null}

      {data?.degraded ? <DiscoveryDegradedBanner /> : null}

      <div className="crosshook-discovery-catalog__count" role="status" aria-live="polite" aria-atomic="true">
        {loading ? (
          <span className="crosshook-muted">Loading trainer catalog…</span>
        ) : totalCount > 0 ? (
          <span className="crosshook-muted">
            {`${totalCount} trainer profile${totalCount !== 1 ? 's' : ''}${
              filtered ? '' : ` across ${tapCount} tap${tapCount !== 1 ? 's' : ''}`
            }`}
          </span>
        ) : null}
      </div>

      {noTaps ? (
        <DiscoveryEmptyState variant="no-taps" onNavigateToBrowse={onNavigateToBrowse} />
      ) : data && totalCount === 0 && !loading ? (
        <DiscoveryEmptyState variant="no-matches" />
      ) : (
        entries.length > 0 && (
          <>
            <ul className="crosshook-discovery-results crosshook-list-reset" ref={listRef}>
              {entries.map((entry) => (
                <li key={catalogEntryKey(entry)}>
                  <DiscoveryResultCard
                    entry={entry}
                    importing={importingKey === catalogEntryKey(entry)}
                    onImport={onImport}
                  />
                </li>
              ))}
            </ul>
            <DiscoveryShowMoreButton
              shownCount={entries.length}
              totalCount={totalCount}
              onLoadMore={() => {
                void catalog.loadMore();
              }}
            />
          </>
        )
      )}
    </DashboardPanelSection>
  );
}

export default DiscoveryCatalogSection;
