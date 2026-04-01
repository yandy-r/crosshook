import { useDeferredValue, useMemo, useState, type ChangeEvent } from 'react';
import { CollapsibleSection } from './ui/CollapsibleSection';

export type CompatibilityRating = 'unknown' | 'broken' | 'partial' | 'working' | 'platinum';

export interface CompatibilityProfile {
  game_name: string;
  trainer_name: string;
  compatibility_rating: CompatibilityRating;
  platform_tags: string[];
  game_version?: string;
  trainer_version?: string;
  proton_version?: string;
  author?: string;
  description?: string;
}

export interface CompatibilityDatabaseEntry {
  id: string;
  tap_url: string;
  tap_branch?: string | null;
  manifest_path: string;
  relative_path?: string;
  metadata: CompatibilityProfile;
}

export interface CompatibilityViewerProps {
  entries: CompatibilityDatabaseEntry[];
  title?: string;
  description?: string;
  onSelectEntry?: (entry: CompatibilityDatabaseEntry) => void;
  onImportEntry?: (entry: CompatibilityDatabaseEntry) => void;
  selectedEntryId?: string | null;
  loading?: boolean;
  error?: string | null;
  emptyMessage?: string;
  recentGames?: string[];
  recentTrainers?: string[];
  recentPlatforms?: string[];
}

function normalizeText(value: string): string {
  return value.trim().toLowerCase();
}

function toUniqueSorted(values: string[]): string[] {
  return Array.from(new Set(values.map((value) => value.trim()).filter(Boolean))).sort((left, right) =>
    left.localeCompare(right)
  );
}

function matchesQuery(value: string, query: string): boolean {
  if (!query) {
    return true;
  }

  return normalizeText(value).includes(query);
}

function getCompatibilityRatingLabel(rating: CompatibilityRating): string {
  switch (rating) {
    case 'broken':
      return 'Broken';
    case 'partial':
      return 'Partial';
    case 'working':
      return 'Working';
    case 'platinum':
      return 'Platinum';
    case 'unknown':
    default:
      return 'Unknown';
  }
}

function CompatibilityBadge({ rating }: { rating: CompatibilityRating }) {
  return (
    <span className={`crosshook-status-chip crosshook-compatibility-badge crosshook-compatibility-badge--${rating}`}>
      {getCompatibilityRatingLabel(rating)}
    </span>
  );
}

function selectPreview(values: string[]): string {
  if (values.length === 0) {
    return 'Any';
  }

  if (values.length === 1) {
    return values[0];
  }

  return `${values[0]} +${values.length - 1}`;
}

export function CompatibilityViewer({
  entries,
  title = 'Trainer Compatibility',
  description = 'Browse known trainer-game compatibility data with filters for game, trainer, and platform.',
  onSelectEntry,
  onImportEntry,
  selectedEntryId = null,
  loading = false,
  error = null,
  emptyMessage = 'No compatibility entries are available yet.',
  recentGames = [],
  recentTrainers = [],
  recentPlatforms = [],
}: CompatibilityViewerProps) {
  const [gameFilter, setGameFilter] = useState('');
  const [trainerFilter, setTrainerFilter] = useState('');
  const [platformFilter, setPlatformFilter] = useState('');
  const deferredGameFilter = useDeferredValue(gameFilter);
  const deferredTrainerFilter = useDeferredValue(trainerFilter);
  const deferredPlatformFilter = useDeferredValue(platformFilter);

  const filterOptions = useMemo(
    () => ({
      games: toUniqueSorted([...recentGames, ...entries.map((entry) => entry.metadata.game_name)]),
      trainers: toUniqueSorted([...recentTrainers, ...entries.map((entry) => entry.metadata.trainer_name)]),
      platforms: toUniqueSorted([...recentPlatforms, ...entries.flatMap((entry) => entry.metadata.platform_tags)]),
    }),
    [entries, recentGames, recentPlatforms, recentTrainers]
  );

  const filteredEntries = useMemo(() => {
    const gameQuery = normalizeText(deferredGameFilter);
    const trainerQuery = normalizeText(deferredTrainerFilter);
    const platformQuery = normalizeText(deferredPlatformFilter);

    return entries.filter((entry) => {
      const { metadata } = entry;

      return (
        matchesQuery(metadata.game_name, gameQuery) &&
        matchesQuery(metadata.trainer_name, trainerQuery) &&
        (platformQuery.length === 0 || metadata.platform_tags.some((tag) => normalizeText(tag).includes(platformQuery)))
      );
    });
  }, [deferredGameFilter, deferredPlatformFilter, deferredTrainerFilter, entries]);

  return (
    <section className="crosshook-card crosshook-compatibility-viewer" aria-label="Trainer compatibility viewer">
      <header className="crosshook-compatibility-viewer__header">
        <div className="crosshook-heading-eyebrow">Compatibility</div>
        <h2 className="crosshook-heading-title">{title}</h2>
        <p className="crosshook-heading-copy">{description}</p>
        <div className="crosshook-status-chip crosshook-compatibility-viewer__count">
          {filteredEntries.length} of {entries.length} entries
        </div>
      </header>

      <CollapsibleSection title="Filters" className="crosshook-panel">
        <div className="crosshook-compatibility-viewer__filters">
          <label className="crosshook-field crosshook-compatibility-viewer__field">
            <span className="crosshook-label">Game</span>
            <input
              className="crosshook-input"
              list="crosshook-compat-games"
              value={gameFilter}
              onChange={(event: ChangeEvent<HTMLInputElement>) => setGameFilter(event.target.value)}
              placeholder="Filter by game"
            />
            <datalist id="crosshook-compat-games">
              {filterOptions.games.map((game) => (
                <option key={game} value={game} />
              ))}
            </datalist>
          </label>

          <label className="crosshook-field crosshook-compatibility-viewer__field">
            <span className="crosshook-label">Trainer</span>
            <input
              className="crosshook-input"
              list="crosshook-compat-trainers"
              value={trainerFilter}
              onChange={(event: ChangeEvent<HTMLInputElement>) => setTrainerFilter(event.target.value)}
              placeholder="Filter by trainer"
            />
            <datalist id="crosshook-compat-trainers">
              {filterOptions.trainers.map((trainer) => (
                <option key={trainer} value={trainer} />
              ))}
            </datalist>
          </label>

          <label className="crosshook-field crosshook-compatibility-viewer__field">
            <span className="crosshook-label">Platform</span>
            <input
              className="crosshook-input"
              list="crosshook-compat-platforms"
              value={platformFilter}
              onChange={(event: ChangeEvent<HTMLInputElement>) => setPlatformFilter(event.target.value)}
              placeholder="Filter by platform"
            />
            <datalist id="crosshook-compat-platforms">
              {filterOptions.platforms.map((platform) => (
                <option key={platform} value={platform} />
              ))}
            </datalist>
          </label>
        </div>
      </CollapsibleSection>

      <CollapsibleSection
        title="Results"
        className="crosshook-panel"
        meta={
          <span>
            {filteredEntries.length} of {entries.length} entries
          </span>
        }
      >
        {loading ? (
          <div className="crosshook-panel crosshook-compatibility-viewer__message">Loading compatibility data...</div>
        ) : error ? (
          <div className="crosshook-panel crosshook-compatibility-viewer__message">
            <div className="crosshook-danger">{error}</div>
          </div>
        ) : filteredEntries.length === 0 ? (
          <div className="crosshook-panel crosshook-compatibility-viewer__message">{emptyMessage}</div>
        ) : (
          <div className="crosshook-compatibility-viewer__result-grid">
            {filteredEntries.map((entry) => {
              const isSelected = selectedEntryId === entry.id;
              const { metadata } = entry;

              return (
                <article
                  key={entry.id}
                  className={[
                    'crosshook-compatibility-viewer__result-card',
                    isSelected ? 'crosshook-compatibility-viewer__result-card--selected' : '',
                  ]
                    .filter(Boolean)
                    .join(' ')}
                >
                  <div className="crosshook-compatibility-viewer__result-header">
                    <div className="crosshook-compatibility-viewer__result-title">
                      <div className="crosshook-heading-title crosshook-compatibility-viewer__result-game">
                        {metadata.game_name}
                      </div>
                      <div className="crosshook-muted crosshook-compatibility-viewer__result-trainer">
                        {metadata.trainer_name}
                        {metadata.trainer_version ? ` • ${metadata.trainer_version}` : ''}
                      </div>
                    </div>

                    <div className="crosshook-compatibility-viewer__result-badges">
                      <CompatibilityBadge rating={metadata.compatibility_rating} />
                      {metadata.platform_tags.map((tag) => (
                        <span key={tag} className="crosshook-status-chip crosshook-compatibility-chip">
                          {tag}
                        </span>
                      ))}
                    </div>
                  </div>

                  <div className="crosshook-compatibility-viewer__result-meta">
                    {metadata.game_version ? (
                      <span className="crosshook-status-chip crosshook-compatibility-chip">
                        Game {metadata.game_version}
                      </span>
                    ) : null}
                    {metadata.proton_version ? (
                      <span className="crosshook-status-chip crosshook-compatibility-chip">
                        Proton {metadata.proton_version}
                      </span>
                    ) : null}
                    {metadata.author ? (
                      <span className="crosshook-status-chip crosshook-compatibility-chip">By {metadata.author}</span>
                    ) : null}
                  </div>

                  {metadata.description ? <p className="crosshook-heading-copy">{metadata.description}</p> : null}

                  <div className="crosshook-compatibility-viewer__result-actions">
                    {onSelectEntry ? (
                      <button
                        type="button"
                        className="crosshook-button crosshook-button--secondary"
                        onClick={() => onSelectEntry(entry)}
                      >
                        Select
                      </button>
                    ) : null}
                    {onImportEntry ? (
                      <button type="button" className="crosshook-button" onClick={() => onImportEntry(entry)}>
                        Import
                      </button>
                    ) : null}
                    <span className="crosshook-muted crosshook-compatibility-viewer__result-source">
                      {selectPreview([entry.tap_url, entry.relative_path ?? entry.manifest_path])}
                    </span>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </CollapsibleSection>
    </section>
  );
}

export default CompatibilityViewer;
