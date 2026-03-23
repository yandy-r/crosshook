import { useMemo, useState, type CSSProperties } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  type CommunityCompatibilityRating,
  type CommunityProfileIndexEntry,
  type CommunityTapSubscription,
  type UseCommunityProfilesResult,
  useCommunityProfiles,
} from '../hooks/useCommunityProfiles';

export interface CommunityBrowserProps {
  profilesDirectoryPath?: string;
  state?: UseCommunityProfilesResult;
}

const DEFAULT_PROFILES_DIRECTORY = '~/.config/crosshook/profiles';

const ratingOrder: CommunityCompatibilityRating[] = ['platinum', 'working', 'partial', 'broken', 'unknown'];

const ratingLabel: Record<CommunityCompatibilityRating, string> = {
  unknown: 'Unknown',
  broken: 'Broken',
  partial: 'Partial',
  working: 'Working',
  platinum: 'Platinum',
};

const ratingStyles: Record<CommunityCompatibilityRating, { color: string; background: string; border: string }> = {
  unknown: {
    color: '#cbd5e1',
    background: 'rgba(100, 116, 139, 0.18)',
    border: '1px solid rgba(148, 163, 184, 0.25)',
  },
  broken: {
    color: '#fecaca',
    background: 'rgba(153, 27, 27, 0.22)',
    border: '1px solid rgba(239, 68, 68, 0.28)',
  },
  partial: {
    color: '#fde68a',
    background: 'rgba(180, 83, 9, 0.22)',
    border: '1px solid rgba(245, 158, 11, 0.28)',
  },
  working: {
    color: '#bbf7d0',
    background: 'rgba(22, 101, 52, 0.22)',
    border: '1px solid rgba(74, 222, 128, 0.28)',
  },
  platinum: {
    color: '#bfdbfe',
    background: 'rgba(37, 99, 235, 0.22)',
    border: '1px solid rgba(96, 165, 250, 0.32)',
  },
};

const panelStyles: Record<string, CSSProperties> = {
  root: {
    display: 'grid',
    gap: 20,
  },
  header: {
    display: 'grid',
    gap: 8,
  },
  toolbar: {
    display: 'grid',
    gap: 12,
    gridTemplateColumns: 'minmax(0, 1fr) auto auto',
    alignItems: 'end',
  },
  field: {
    display: 'grid',
    gap: 8,
  },
  tapPanel: {
    display: 'grid',
    gap: 12,
  },
  tapList: {
    display: 'grid',
    gap: 10,
  },
  profileGrid: {
    display: 'grid',
    gap: 12,
    gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
  },
  profileCard: {
    display: 'grid',
    gap: 12,
    padding: 18,
    borderRadius: 16,
    background: 'rgba(8, 14, 26, 0.78)',
    border: '1px solid var(--crosshook-color-border)',
  },
  profileHeader: {
    display: 'flex',
    justifyContent: 'space-between',
    gap: 12,
    alignItems: 'start',
  },
  metaGrid: {
    display: 'grid',
    gap: 8,
  },
  chipRow: {
    display: 'flex',
    flexWrap: 'wrap',
    gap: 8,
  },
  footer: {
    display: 'flex',
    justifyContent: 'space-between',
    gap: 12,
    alignItems: 'center',
    flexWrap: 'wrap',
  },
  buttonRow: {
    display: 'flex',
    gap: 10,
    flexWrap: 'wrap',
  },
  input: {
    width: '100%',
    minHeight: 44,
    borderRadius: 12,
    border: '1px solid var(--crosshook-color-border-strong)',
    background: 'rgba(8, 14, 26, 0.96)',
    color: 'var(--crosshook-color-text)',
    padding: '0 14px',
  },
  textarea: {
    width: '100%',
    minHeight: 120,
    borderRadius: 12,
    border: '1px solid var(--crosshook-color-border-strong)',
    background: 'rgba(8, 14, 26, 0.96)',
    color: 'var(--crosshook-color-text)',
    padding: 14,
    resize: 'vertical',
  },
  helper: {
    margin: 0,
    color: 'var(--crosshook-color-text-muted)',
    lineHeight: 1.6,
  },
  diagnostic: {
    margin: 0,
    padding: 12,
    borderRadius: 12,
    background: 'rgba(180, 83, 9, 0.16)',
    border: '1px solid rgba(245, 158, 11, 0.24)',
    color: '#fde68a',
    lineHeight: 1.55,
  },
  error: {
    margin: 0,
    padding: 12,
    borderRadius: 12,
    background: 'rgba(153, 27, 27, 0.2)',
    border: '1px solid rgba(239, 68, 68, 0.24)',
    color: '#fecaca',
    lineHeight: 1.55,
  },
  empty: {
    margin: 0,
    padding: 16,
    borderRadius: 12,
    background: 'rgba(8, 14, 26, 0.58)',
    border: '1px dashed rgba(148, 163, 184, 0.18)',
    color: 'var(--crosshook-color-text-muted)',
  },
};

function matchesQuery(entry: CommunityProfileIndexEntry, query: string): boolean {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return true;
  }

  const haystack = [
    entry.manifest.metadata.game_name,
    entry.manifest.metadata.game_version,
    entry.manifest.metadata.trainer_name,
    entry.manifest.metadata.trainer_version,
    entry.manifest.metadata.proton_version,
    entry.manifest.metadata.author,
    entry.manifest.metadata.description,
    entry.manifest.metadata.platform_tags.join(' '),
    entry.tap_url,
    entry.relative_path,
  ]
    .join(' ')
    .toLowerCase();

  return haystack.includes(normalized);
}

function sortProfiles(entries: CommunityProfileIndexEntry[]): CommunityProfileIndexEntry[] {
  return [...entries].sort((left, right) => {
    const rank = (value: CommunityCompatibilityRating) => ratingOrder.indexOf(value as CommunityCompatibilityRating);

    return (
      rank(left.manifest.metadata.compatibility_rating) - rank(right.manifest.metadata.compatibility_rating) ||
      left.manifest.metadata.game_name.localeCompare(right.manifest.metadata.game_name) ||
      left.manifest_path.localeCompare(right.manifest_path)
    );
  });
}

async function chooseCommunityProfileImport(): Promise<string | null> {
  const result = await open({
    directory: false,
    multiple: false,
    title: 'Select Community Profile JSON',
    filters: [{ name: 'JSON', extensions: ['json'] }],
  });

  if (Array.isArray(result)) {
    return result[0] ?? null;
  }

  return result ?? null;
}

function TapChip({
  tap,
  onRemove,
}: {
  tap: CommunityTapSubscription;
  onRemove: (tap: CommunityTapSubscription) => void;
}) {
  return (
    <div
      style={{
        display: 'flex',
        justifyContent: 'space-between',
        gap: 12,
        alignItems: 'center',
        padding: '10px 12px',
        borderRadius: 12,
        background: 'rgba(8, 14, 26, 0.78)',
        border: '1px solid var(--crosshook-color-border)',
      }}
    >
      <div style={{ display: 'grid', gap: 4 }}>
        <strong style={{ color: 'var(--crosshook-color-text)' }}>{tap.url}</strong>
        <span style={{ color: 'var(--crosshook-color-text-muted)', fontSize: 13 }}>
          {tap.branch ? `Branch: ${tap.branch}` : 'Default branch'}
        </span>
      </div>
      <button type="button" className="crosshook-button crosshook-button--secondary" onClick={() => onRemove(tap)}>
        Remove
      </button>
    </div>
  );
}

function CompatibilityBadge({ rating }: { rating: CommunityCompatibilityRating }) {
  const styles = ratingStyles[rating];

  return (
    <span
      style={{
        minHeight: 32,
        display: 'inline-flex',
        alignItems: 'center',
        gap: 8,
        padding: '0 12px',
        borderRadius: 999,
        color: styles.color,
        background: styles.background,
        border: styles.border,
        fontSize: 13,
        fontWeight: 700,
        letterSpacing: '0.01em',
      }}
    >
      {ratingLabel[rating]}
    </span>
  );
}

export function CommunityBrowser({ profilesDirectoryPath = DEFAULT_PROFILES_DIRECTORY, state }: CommunityBrowserProps) {
  const [tapUrl, setTapUrl] = useState('');
  const [tapBranch, setTapBranch] = useState('');
  const [query, setQuery] = useState('');
  const [ratingFilter, setRatingFilter] = useState<'all' | CommunityCompatibilityRating>('all');
  const [notice, setNotice] = useState<string | null>(null);
  const internalState = useCommunityProfiles({
    profilesDirectoryPath,
  });
  const {
    taps,
    index,
    loading,
    syncing,
    importing,
    error,
    refreshProfiles,
    syncTaps,
    addTap,
    removeTap,
    importCommunityProfile,
    setError,
  } = state ?? internalState;

  const visibleEntries = useMemo(() => {
    const filtered = index.entries.filter((entry) => {
      const matchesRating = ratingFilter === 'all' || entry.manifest.metadata.compatibility_rating === ratingFilter;
      return matchesRating && matchesQuery(entry, query);
    });

    return sortProfiles(filtered);
  }, [index.entries, query, ratingFilter]);

  async function handleAddTap() {
    setNotice(null);
    try {
      await addTap({
        url: tapUrl,
        branch: tapBranch,
      });
      setTapUrl('');
      setTapBranch('');
      setNotice('Tap saved.');
    } catch (tapError) {
      setError(tapError instanceof Error ? tapError.message : String(tapError));
    }
  }

  async function handleImportFromFile() {
    setNotice(null);
    const path = await chooseCommunityProfileImport();
    if (!path) {
      return;
    }

    try {
      const imported = await importCommunityProfile(path);
      setNotice(`Imported ${imported.profile_name} into ${profilesDirectoryPath}.`);
    } catch (importError) {
      setError(importError instanceof Error ? importError.message : String(importError));
    }
  }

  async function handleImportEntry(entry: CommunityProfileIndexEntry) {
    setNotice(null);
    try {
      const imported = await importCommunityProfile(entry.manifest_path);
      setNotice(`Imported ${imported.profile_name} from ${entry.tap_url}.`);
    } catch (importError) {
      setError(importError instanceof Error ? importError.message : String(importError));
    }
  }

  return (
    <section className="crosshook-card" aria-label="Community profile browser" style={panelStyles.root}>
      <header style={panelStyles.header}>
        <div className="crosshook-heading-eyebrow">Community</div>
        <h2 className="crosshook-heading-title">Browse shared profiles</h2>
        <p className="crosshook-heading-copy">
          Search profiles from your configured taps, inspect compatibility metadata, and import a profile into your
          local CrossHook library.
        </p>
      </header>

      <section className="crosshook-panel" style={panelStyles.tapPanel}>
        <div style={panelStyles.footer}>
          <div style={{ display: 'grid', gap: 6 }}>
            <div className="crosshook-heading-eyebrow">Tap Management</div>
            <p className="crosshook-muted" style={panelStyles.helper}>
              Taps are persisted in CrossHook settings and synced through the backend community commands.
            </p>
          </div>
          <div style={panelStyles.buttonRow}>
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              onClick={() => {
                void refreshProfiles().catch((refreshError) => {
                  setError(refreshError instanceof Error ? refreshError.message : String(refreshError));
                });
              }}
              disabled={loading || syncing}
            >
              Refresh Index
            </button>
            <button
              type="button"
              className="crosshook-button"
              onClick={() => {
                void syncTaps().catch((syncError) => {
                  setError(syncError instanceof Error ? syncError.message : String(syncError));
                });
              }}
              disabled={loading || syncing || taps.length === 0}
            >
              {syncing ? 'Syncing...' : 'Sync Taps'}
            </button>
          </div>
        </div>

        <div style={panelStyles.toolbar}>
          <div style={panelStyles.field}>
            <label className="crosshook-label" htmlFor="tap-url">
              Tap URL
            </label>
            <input
              id="tap-url"
              className="crosshook-input"
              value={tapUrl}
              onChange={(event) => setTapUrl(event.target.value)}
              placeholder="https://github.com/example/community-profiles.git"
            />
          </div>
          <div style={panelStyles.field}>
            <label className="crosshook-label" htmlFor="tap-branch">
              Branch
            </label>
            <input
              id="tap-branch"
              className="crosshook-input"
              value={tapBranch}
              onChange={(event) => setTapBranch(event.target.value)}
              placeholder="main"
            />
          </div>
          <button
            type="button"
            className="crosshook-button"
            onClick={() => {
              void handleAddTap();
            }}
            disabled={loading || syncing || tapUrl.trim().length === 0}
          >
            Add Tap
          </button>
        </div>

        {taps.length > 0 ? (
          <div style={panelStyles.tapList}>
            {taps.map((tap) => (
              <TapChip
                key={`${tap.url}::${tap.branch ?? ''}`}
                tap={tap}
                onRemove={(tapToRemove) => {
                  void removeTap(tapToRemove).catch((removeError) => {
                    setError(removeError instanceof Error ? removeError.message : String(removeError));
                  });
                }}
              />
            ))}
          </div>
        ) : (
          <p className="crosshook-muted" style={panelStyles.helper}>
            Add a tap URL to populate the community browser.
          </p>
        )}
      </section>

      <section className="crosshook-panel" style={{ display: 'grid', gap: 12 }}>
        <div style={panelStyles.toolbar}>
          <div style={panelStyles.field}>
            <label className="crosshook-label" htmlFor="community-search">
              Search profiles
            </label>
            <input
              id="community-search"
              className="crosshook-input"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search game, trainer, author, tag..."
            />
          </div>
          <div style={panelStyles.field}>
            <label className="crosshook-label" htmlFor="compatibility-filter">
              Compatibility
            </label>
            <select
              id="compatibility-filter"
              className="crosshook-select"
              value={ratingFilter}
              onChange={(event) => setRatingFilter(event.target.value as 'all' | CommunityCompatibilityRating)}
            >
              <option value="all">All ratings</option>
              {ratingOrder.map((rating) => (
                <option key={rating} value={rating}>
                  {ratingLabel[rating]}
                </option>
              ))}
            </select>
          </div>
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => void handleImportFromFile()}
          >
            Import JSON
          </button>
        </div>

        {notice ? (
          <p className="crosshook-success" style={panelStyles.helper}>
            {notice}
          </p>
        ) : null}
        {error ? <p style={panelStyles.error}>{error}</p> : null}
        {index.diagnostics.length > 0 ? (
          <div style={{ display: 'grid', gap: 8 }}>
            {index.diagnostics.map((diagnostic) => (
              <p key={diagnostic} style={panelStyles.diagnostic}>
                {diagnostic}
              </p>
            ))}
          </div>
        ) : null}

        {loading ? (
          <p className="crosshook-muted" style={panelStyles.helper}>
            Loading community profiles...
          </p>
        ) : visibleEntries.length === 0 ? (
          <p style={panelStyles.empty}>
            No community profiles matched the current search. Sync a tap or widen the filter.
          </p>
        ) : (
          <div style={panelStyles.profileGrid}>
            {visibleEntries.map((entry) => (
              <article key={`${entry.tap_url}::${entry.relative_path}`} style={panelStyles.profileCard}>
                <div style={panelStyles.profileHeader}>
                  <div style={{ display: 'grid', gap: 4 }}>
                    <h3 style={{ margin: 0, color: 'var(--crosshook-color-text)' }}>
                      {entry.manifest.metadata.game_name || 'Untitled profile'}
                    </h3>
                    <div className="crosshook-muted" style={{ fontSize: 13 }}>
                      {entry.manifest.metadata.author || 'Unknown author'}
                    </div>
                  </div>
                  <CompatibilityBadge rating={entry.manifest.metadata.compatibility_rating} />
                </div>

                <div style={panelStyles.metaGrid}>
                  <div className="crosshook-muted" style={{ fontSize: 13 }}>
                    Trainer: {entry.manifest.metadata.trainer_name || 'Unknown'}{' '}
                    {entry.manifest.metadata.trainer_version ? `(${entry.manifest.metadata.trainer_version})` : ''}
                  </div>
                  <div className="crosshook-muted" style={{ fontSize: 13 }}>
                    Proton: {entry.manifest.metadata.proton_version || 'Unknown'}
                  </div>
                  <div className="crosshook-muted" style={{ fontSize: 13 }}>
                    Game version: {entry.manifest.metadata.game_version || 'Unknown'}
                  </div>
                  <p className="crosshook-heading-copy" style={{ margin: 0 }}>
                    {entry.manifest.metadata.description || 'No description provided.'}
                  </p>
                </div>

                <div style={panelStyles.chipRow}>
                  {entry.manifest.metadata.platform_tags.length > 0 ? (
                    entry.manifest.metadata.platform_tags.map((tag) => (
                      <span
                        key={tag}
                        style={{
                          minHeight: 30,
                          display: 'inline-flex',
                          alignItems: 'center',
                          padding: '0 10px',
                          borderRadius: 999,
                          background: 'rgba(255, 255, 255, 0.05)',
                          border: '1px solid rgba(255, 255, 255, 0.08)',
                          color: 'var(--crosshook-color-text-muted)',
                          fontSize: 12,
                          fontWeight: 600,
                        }}
                      >
                        {tag}
                      </span>
                    ))
                  ) : (
                    <span className="crosshook-muted" style={{ fontSize: 12 }}>
                      No platform tags
                    </span>
                  )}
                </div>

                <div className="crosshook-muted" style={{ fontSize: 12, wordBreak: 'break-word' }}>
                  Source: {entry.tap_url}
                </div>

                <div style={panelStyles.buttonRow}>
                  <button
                    type="button"
                    className="crosshook-button"
                    onClick={() => {
                      void handleImportEntry(entry);
                    }}
                    disabled={importing}
                  >
                    {importing ? 'Importing...' : 'Import'}
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}
      </section>
    </section>
  );
}

export default CommunityBrowser;
