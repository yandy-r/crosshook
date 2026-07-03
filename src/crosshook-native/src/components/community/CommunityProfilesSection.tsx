import type { FacetKey } from '../../hooks/useCommunityCatalog';
import { deriveCatalogEntryProfileName } from '../../hooks/useCommunityProfiles';
import type { CatalogEntry, CatalogFacets, CatalogQuery } from '../../types/discovery';
import { DiscoveryDegradedBanner } from '../discovery/DiscoveryDegradedBanner';
import { DiscoveryFacetBar } from '../discovery/DiscoveryFacetBar';
import { catalogEntryBand } from '../discovery/DiscoveryResultCard';
import { DiscoveryShowMoreButton } from '../discovery/DiscoveryShowMoreButton';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import { CompatibilityBadge } from './CompatibilityBadge';

export interface CommunityProfilesSectionProps {
  entries: CatalogEntry[];
  totalCount: number;
  facets: CatalogFacets;
  catalogQuery: CatalogQuery;
  searchText: string;
  diagnostics: string[];
  catalogLoading: boolean;
  catalogError: string | null;
  degraded: boolean;
  importing: boolean;
  notice: string | null;
  error: string | null;
  importedProfileNames: Set<string>;
  onSearchTextChange: (value: string) => void;
  onSetFacet: (key: FacetKey, value: string | undefined) => void;
  onClearFilters: () => void;
  onLoadMore: () => void;
  onImportFromFile: () => void;
  onImportFromLutris: () => void;
  lutrisBusy?: boolean;
  onImportEntry: (entry: CatalogEntry) => void;
}

export function CommunityProfilesSection({
  entries,
  totalCount,
  facets,
  catalogQuery,
  searchText,
  diagnostics,
  catalogLoading,
  catalogError,
  degraded,
  importing,
  notice,
  error,
  importedProfileNames,
  onSearchTextChange,
  onSetFacet,
  onClearFilters,
  onLoadMore,
  onImportFromFile,
  onImportFromLutris,
  lutrisBusy = false,
  onImportEntry,
}: CommunityProfilesSectionProps) {
  return (
    <DashboardPanelSection
      eyebrow="Profile Index"
      title="Community Profiles"
      summary={`${entries.length} of ${totalCount} profiles`}
      titleAs="h2"
      className="crosshook-community-browser__panel"
    >
      <div className="crosshook-community-browser__toolbar">
        <div className="crosshook-community-browser__field">
          <label className="crosshook-label" htmlFor="community-search">
            Search profiles
          </label>
          <input
            id="community-search"
            className="crosshook-input"
            value={searchText}
            onChange={(event) => onSearchTextChange(event.target.value)}
            placeholder="Search game, trainer, author, tag..."
          />
        </div>
        <button type="button" className="crosshook-button crosshook-button--secondary" onClick={onImportFromFile}>
          Import JSON
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={onImportFromLutris}
          disabled={lutrisBusy || importing}
        >
          {lutrisBusy ? 'Scanning Lutris…' : 'Import from Lutris'}
        </button>
      </div>

      <DiscoveryFacetBar facets={facets} query={catalogQuery} onSetFacet={onSetFacet} onClearFilters={onClearFilters} />

      {degraded ? <DiscoveryDegradedBanner /> : null}

      {notice ? (
        <p
          className="crosshook-success crosshook-community-browser__helper"
          role="status"
          aria-live="polite"
          aria-atomic="true"
        >
          {notice}
        </p>
      ) : null}
      {error ? (
        <div className="crosshook-error-banner crosshook-error-banner--section" role="alert">
          {error}
        </div>
      ) : null}
      {catalogError ? (
        <div className="crosshook-error-banner crosshook-error-banner--section" role="alert">
          {catalogError}
        </div>
      ) : null}
      {diagnostics.length > 0 ? (
        <div className="crosshook-community-browser__diagnostics">
          {diagnostics.map((diagnostic) => (
            <p key={diagnostic} className="crosshook-community-browser__diagnostic">
              {diagnostic}
            </p>
          ))}
        </div>
      ) : null}

      {catalogLoading && entries.length === 0 ? (
        <p className="crosshook-muted crosshook-community-browser__helper">Loading community profiles...</p>
      ) : entries.length === 0 ? (
        <p className="crosshook-community-browser__empty">
          No community profiles matched the current search. Sync a tap or widen the filter.
        </p>
      ) : (
        <>
          <div className="crosshook-community-browser__profile-grid">
            {entries.map((entry) => {
              const importedProfileName = deriveCatalogEntryProfileName(entry);
              const isImported = importedProfileNames.has(importedProfileName);
              const platformTags = entry.platformTags?.split(' ').filter(Boolean) ?? [];
              return (
                <article
                  key={`${entry.tapUrl}::${entry.relativePath}`}
                  className="crosshook-community-browser__profile-card"
                >
                  <div className="crosshook-community-browser__profile-header">
                    <div className="crosshook-community-browser__profile-title">
                      <h3 className="crosshook-community-browser__profile-name">
                        {entry.gameName ?? 'Untitled profile'}
                      </h3>
                      <div className="crosshook-muted crosshook-community-browser__profile-author">
                        {entry.author ?? 'Unknown author'}
                      </div>
                    </div>
                    <CompatibilityBadge rating={catalogEntryBand(entry)} />
                  </div>

                  <div className="crosshook-community-browser__meta-grid">
                    <div className="crosshook-muted crosshook-community-browser__meta-line">
                      Trainer: {entry.trainerName ?? 'Unknown'}{' '}
                      {entry.trainerVersion ? `(${entry.trainerVersion})` : ''}
                    </div>
                    <div className="crosshook-muted crosshook-community-browser__meta-line">
                      Proton: {entry.protonVersion ?? 'Unknown'}
                    </div>
                    <div className="crosshook-muted crosshook-community-browser__meta-line">
                      Game version: {entry.gameVersion ?? 'Unknown'}
                    </div>
                    <p className="crosshook-heading-copy crosshook-community-browser__description">
                      {entry.description ?? 'No description provided.'}
                    </p>
                  </div>

                  <div className="crosshook-community-browser__chip-row">
                    {platformTags.length > 0 ? (
                      platformTags.map((tag) => (
                        <span key={tag} className="crosshook-community-browser__platform-tag">
                          {tag}
                        </span>
                      ))
                    ) : (
                      <span className="crosshook-muted crosshook-community-browser__platform-tag crosshook-community-browser__platform-tag--empty">
                        No platform tags
                      </span>
                    )}
                  </div>

                  <div className="crosshook-muted crosshook-community-browser__source">Source: {entry.tapUrl}</div>

                  <div className="crosshook-community-browser__button-row">
                    {isImported ? (
                      <span className="crosshook-community-browser__imported-badge">Imported</span>
                    ) : entry.manifestPath.length > 0 ? (
                      <button
                        type="button"
                        className="crosshook-button"
                        onClick={() => {
                          onImportEntry(entry);
                        }}
                        disabled={importing}
                      >
                        {importing ? 'Importing...' : 'Import'}
                      </button>
                    ) : null}
                  </div>
                </article>
              );
            })}
          </div>
          <DiscoveryShowMoreButton shownCount={entries.length} totalCount={totalCount} onLoadMore={onLoadMore} />
        </>
      )}
    </DashboardPanelSection>
  );
}

export default CommunityProfilesSection;
