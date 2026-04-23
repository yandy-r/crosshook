import type { CommunityCompatibilityRating, CommunityProfileIndexEntry } from '../../hooks/useCommunityProfiles';
import { deriveCommunityImportProfileName } from '../../hooks/useCommunityProfiles';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import { ThemedSelect } from '../ui/ThemedSelect';
import { CompatibilityBadge, ratingLabel, ratingOrder } from './CompatibilityBadge';

export interface CommunityProfilesSectionProps {
  visibleEntries: CommunityProfileIndexEntry[];
  totalEntries: number;
  diagnostics: string[];
  query: string;
  ratingFilter: 'all' | CommunityCompatibilityRating;
  loading: boolean;
  importing: boolean;
  notice: string | null;
  error: string | null;
  importedProfileNames: Set<string>;
  onQueryChange: (value: string) => void;
  onRatingFilterChange: (value: 'all' | CommunityCompatibilityRating) => void;
  onImportFromFile: () => void;
  onImportEntry: (entry: CommunityProfileIndexEntry) => void;
}

export function CommunityProfilesSection({
  visibleEntries,
  totalEntries,
  diagnostics,
  query,
  ratingFilter,
  loading,
  importing,
  notice,
  error,
  importedProfileNames,
  onQueryChange,
  onRatingFilterChange,
  onImportFromFile,
  onImportEntry,
}: CommunityProfilesSectionProps) {
  return (
    <DashboardPanelSection
      eyebrow="Profile Index"
      title="Community Profiles"
      summary={`${visibleEntries.length} of ${totalEntries} profiles`}
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
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder="Search game, trainer, author, tag..."
          />
        </div>
        <div className="crosshook-community-browser__field">
          <label className="crosshook-label" htmlFor="compatibility-filter">
            Compatibility
          </label>
          <ThemedSelect
            id="compatibility-filter"
            value={ratingFilter}
            onValueChange={(val) => onRatingFilterChange(val as 'all' | CommunityCompatibilityRating)}
            options={[
              { value: 'all', label: 'All ratings' },
              ...ratingOrder.map((rating) => ({ value: rating, label: ratingLabel[rating] })),
            ]}
          />
        </div>
        <button type="button" className="crosshook-button crosshook-button--secondary" onClick={onImportFromFile}>
          Import JSON
        </button>
      </div>

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
      {diagnostics.length > 0 ? (
        <div className="crosshook-community-browser__diagnostics">
          {diagnostics.map((diagnostic) => (
            <p key={diagnostic} className="crosshook-community-browser__diagnostic">
              {diagnostic}
            </p>
          ))}
        </div>
      ) : null}

      {loading ? (
        <p className="crosshook-muted crosshook-community-browser__helper">Loading community profiles...</p>
      ) : visibleEntries.length === 0 ? (
        <p className="crosshook-community-browser__empty">
          No community profiles matched the current search. Sync a tap or widen the filter.
        </p>
      ) : (
        <div className="crosshook-community-browser__profile-grid">
          {visibleEntries.map((entry) => {
            const importedProfileName = deriveCommunityImportProfileName(entry);
            const isImported = importedProfileNames.has(importedProfileName);
            return (
              <article
                key={`${entry.tap_url}::${entry.relative_path}`}
                className="crosshook-community-browser__profile-card"
              >
                <div className="crosshook-community-browser__profile-header">
                  <div className="crosshook-community-browser__profile-title">
                    <h3 className="crosshook-community-browser__profile-name">
                      {entry.manifest.metadata.game_name || 'Untitled profile'}
                    </h3>
                    <div className="crosshook-muted crosshook-community-browser__profile-author">
                      {entry.manifest.metadata.author || 'Unknown author'}
                    </div>
                  </div>
                  <CompatibilityBadge rating={entry.manifest.metadata.compatibility_rating} />
                </div>

                <div className="crosshook-community-browser__meta-grid">
                  <div className="crosshook-muted crosshook-community-browser__meta-line">
                    Trainer: {entry.manifest.metadata.trainer_name || 'Unknown'}{' '}
                    {entry.manifest.metadata.trainer_version ? `(${entry.manifest.metadata.trainer_version})` : ''}
                  </div>
                  <div className="crosshook-muted crosshook-community-browser__meta-line">
                    Proton: {entry.manifest.metadata.proton_version || 'Unknown'}
                  </div>
                  <div className="crosshook-muted crosshook-community-browser__meta-line">
                    Game version: {entry.manifest.metadata.game_version || 'Unknown'}
                  </div>
                  <p className="crosshook-heading-copy crosshook-community-browser__description">
                    {entry.manifest.metadata.description || 'No description provided.'}
                  </p>
                </div>

                <div className="crosshook-community-browser__chip-row">
                  {entry.manifest.metadata.platform_tags.length > 0 ? (
                    entry.manifest.metadata.platform_tags.map((tag) => (
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

                <div className="crosshook-muted crosshook-community-browser__source">Source: {entry.tap_url}</div>

                <div className="crosshook-community-browser__button-row">
                  {isImported ? (
                    <span className="crosshook-community-browser__imported-badge">Imported</span>
                  ) : (
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
                  )}
                </div>
              </article>
            );
          })}
        </div>
      )}
    </DashboardPanelSection>
  );
}

export default CommunityProfilesSection;
