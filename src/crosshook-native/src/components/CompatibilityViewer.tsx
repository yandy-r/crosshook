import { useDeferredValue, useMemo, useState, type ChangeEvent, type CSSProperties } from 'react';

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

const cardStyle: CSSProperties = {
  display: 'grid',
  gap: 16,
};

const filterRowStyle: CSSProperties = {
  display: 'grid',
  gap: 12,
  gridTemplateColumns: 'repeat(3, minmax(0, 1fr))',
};

const fieldStyle: CSSProperties = {
  display: 'grid',
  gap: 8,
};

const inputStyle: CSSProperties = {
  width: '100%',
  minHeight: 48,
  borderRadius: 12,
  border: '1px solid var(--crosshook-color-border-strong)',
  background: 'rgba(8, 14, 26, 0.96)',
  color: 'var(--crosshook-color-text)',
  padding: '0 14px',
};

const resultGridStyle: CSSProperties = {
  display: 'grid',
  gap: 12,
};

const resultCardStyle: CSSProperties = {
  display: 'grid',
  gap: 10,
  padding: 16,
  borderRadius: 16,
  background: 'rgba(8, 14, 26, 0.78)',
  border: '1px solid var(--crosshook-color-border)',
};

const metaRowStyle: CSSProperties = {
  display: 'flex',
  flexWrap: 'wrap',
  gap: 8,
  alignItems: 'center',
};

const chipStyle: CSSProperties = {
  minHeight: 30,
  padding: '0 10px',
  borderRadius: 999,
  display: 'inline-flex',
  alignItems: 'center',
  gap: 6,
  fontSize: 12,
  fontWeight: 700,
};

const ratingStyles: Record<CompatibilityRating, CSSProperties> = {
  unknown: {
    background: 'rgba(148, 163, 184, 0.18)',
    border: '1px solid rgba(148, 163, 184, 0.32)',
    color: '#cbd5e1',
  },
  broken: {
    background: 'rgba(248, 113, 113, 0.16)',
    border: '1px solid rgba(248, 113, 113, 0.32)',
    color: '#fecaca',
  },
  partial: {
    background: 'rgba(245, 158, 11, 0.16)',
    border: '1px solid rgba(245, 158, 11, 0.32)',
    color: '#fde68a',
  },
  working: {
    background: 'rgba(34, 197, 94, 0.16)',
    border: '1px solid rgba(34, 197, 94, 0.32)',
    color: '#bbf7d0',
  },
  platinum: {
    background: 'rgba(0, 120, 212, 0.18)',
    border: '1px solid rgba(0, 120, 212, 0.34)',
    color: '#bfdbfe',
  },
};

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
    <span className="crosshook-status-chip" style={{ ...chipStyle, ...ratingStyles[rating] }}>
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
    <section className="crosshook-card" aria-label="Trainer compatibility viewer" style={cardStyle}>
      <header style={{ display: 'grid', gap: 8 }}>
        <div className="crosshook-heading-eyebrow">Compatibility</div>
        <h2 className="crosshook-heading-title">{title}</h2>
        <p className="crosshook-heading-copy">{description}</p>
        <div className="crosshook-status-chip" style={{ width: 'fit-content' }}>
          {filteredEntries.length} of {entries.length} entries
        </div>
      </header>

      <div style={filterRowStyle}>
        <label className="crosshook-field">
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

        <label className="crosshook-field">
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

        <label className="crosshook-field">
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

      {loading ? (
        <div className="crosshook-panel">Loading compatibility data...</div>
      ) : error ? (
        <div className="crosshook-panel">
          <div className="crosshook-danger">{error}</div>
        </div>
      ) : filteredEntries.length === 0 ? (
        <div className="crosshook-panel">{emptyMessage}</div>
      ) : (
        <div style={resultGridStyle}>
          {filteredEntries.map((entry) => {
            const isSelected = selectedEntryId === entry.id;
            const { metadata } = entry;

            return (
              <article
                key={entry.id}
                style={{
                  ...resultCardStyle,
                  borderColor: isSelected ? 'var(--crosshook-color-accent-strong)' : 'var(--crosshook-color-border)',
                  boxShadow: isSelected
                    ? '0 0 0 1px rgba(0, 120, 212, 0.45), var(--crosshook-shadow-strong)'
                    : undefined,
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
                  <div style={{ display: 'grid', gap: 6 }}>
                    <div className="crosshook-heading-title" style={{ fontSize: '1.15rem' }}>
                      {metadata.game_name}
                    </div>
                    <div className="crosshook-muted">
                      {metadata.trainer_name}
                      {metadata.trainer_version ? ` • ${metadata.trainer_version}` : ''}
                    </div>
                  </div>

                  <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
                    <CompatibilityBadge rating={metadata.compatibility_rating} />
                    {metadata.platform_tags.map((tag) => (
                      <span key={tag} className="crosshook-status-chip" style={chipStyle}>
                        {tag}
                      </span>
                    ))}
                  </div>
                </div>

                <div style={metaRowStyle}>
                  {metadata.game_version ? (
                    <span className="crosshook-status-chip" style={chipStyle}>
                      Game {metadata.game_version}
                    </span>
                  ) : null}
                  {metadata.proton_version ? (
                    <span className="crosshook-status-chip" style={chipStyle}>
                      Proton {metadata.proton_version}
                    </span>
                  ) : null}
                  {metadata.author ? (
                    <span className="crosshook-status-chip" style={chipStyle}>
                      By {metadata.author}
                    </span>
                  ) : null}
                </div>

                {metadata.description ? <p className="crosshook-heading-copy">{metadata.description}</p> : null}

                <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
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
                  <span className="crosshook-muted" style={{ alignSelf: 'center', wordBreak: 'break-word' }}>
                    {selectPreview([entry.tap_url, entry.relative_path ?? entry.manifest_path])}
                  </span>
                </div>
              </article>
            );
          })}
        </div>
      )}
    </section>
  );
}

export default CompatibilityViewer;
