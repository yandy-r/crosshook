import { useMemo, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  deriveCommunityImportProfileName,
  type CommunityImportPreview,
  type CommunityCompatibilityRating,
  type CommunityProfileIndexEntry,
  type CommunityTapSubscription,
  type CommunityTapSyncResult,
  type UseCommunityProfilesResult,
  useCommunityProfiles,
} from '../hooks/useCommunityProfiles';
import { CollapsibleSection } from './ui/CollapsibleSection';
import { ThemedSelect } from './ui/ThemedSelect';
import CommunityImportWizardModal from './CommunityImportWizardModal';

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

/** Stable React key / row id for a tap subscription (same repo URL can appear on multiple branches or pins). */
function tapSubscriptionStableKey(sub: CommunityTapSubscription): string {
  return `${sub.url}::${sub.branch ?? ''}::${sub.pinned_commit ?? ''}`;
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
  onPin,
  onUnpin,
  headCommit,
  busy,
}: {
  tap: CommunityTapSubscription;
  onRemove: (tap: CommunityTapSubscription) => void;
  onPin: (tap: CommunityTapSubscription) => void;
  onUnpin: (tap: CommunityTapSubscription) => void;
  headCommit?: string;
  busy: boolean;
}) {
  const shortPinnedCommit = tap.pinned_commit ? tap.pinned_commit.slice(0, 12) : null;
  const shortHeadCommit = headCommit ? headCommit.slice(0, 12) : null;

  return (
    <div className="crosshook-community-tap">
      <div className="crosshook-community-tap__meta">
        <strong className="crosshook-community-tap__url">{tap.url}</strong>
        <span className="crosshook-community-tap__branch">
          {tap.branch ? `Branch: ${tap.branch}` : 'Default branch'}
        </span>
        <span className="crosshook-community-tap__branch">
          {shortPinnedCommit ? `Pinned: ${shortPinnedCommit}` : `Tracking: ${shortHeadCommit ?? 'unsynced'}`}
        </span>
      </div>
      <div className="crosshook-community-browser__button-row">
        {tap.pinned_commit ? (
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => onUnpin(tap)}
            disabled={busy}
          >
            Unpin
          </button>
        ) : (
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => onPin(tap)}
            disabled={busy || !headCommit}
            title={headCommit ? 'Pin this tap to the currently synced commit' : 'Sync taps first to capture a commit'}
          >
            Pin to Current Version
          </button>
        )}
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={() => onRemove(tap)}
          disabled={busy}
        >
          Remove
        </button>
      </div>
    </div>
  );
}

function CompatibilityBadge({ rating }: { rating: CommunityCompatibilityRating }) {
  return (
    <span className={`crosshook-community-rating-badge crosshook-community-rating-badge--${rating}`}>
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
  const [importDraft, setImportDraft] = useState<CommunityImportPreview | null>(null);
  const [importDraftSource, setImportDraftSource] = useState<string | null>(null);
  const internalState = useCommunityProfiles({
    profilesDirectoryPath,
  });
  const {
    taps,
    index,
    importedProfileNames,
    loading,
    syncing,
    importing,
    error,
    refreshProfiles,
    syncTaps,
    addTap,
    removeTap,
    pinTapToCurrentVersion,
    unpinTap,
    getTapHeadCommit,
    lastTapSyncResults,
    prepareCommunityImport,
    saveImportedProfile,
    setError,
  } = state ?? internalState;

  const cachedTapNotices = useMemo(() => {
    return lastTapSyncResults
      .filter((r: CommunityTapSyncResult) => r.from_cache)
      .map((r: CommunityTapSyncResult) => {
        const sub = r.workspace.subscription;
        const tapKey = tapSubscriptionStableKey(sub);
        const branchPart = sub.branch ? ` — ${sub.branch}` : ' — default branch';
        const pinPart = sub.pinned_commit ? ` — pinned ${sub.pinned_commit.slice(0, 12)}` : '';
        return {
          tapKey,
          labelPrefix: `${sub.url}${branchPart}${pinPart}`,
          lastSync:
            r.last_sync_at && !Number.isNaN(Date.parse(r.last_sync_at))
              ? new Date(r.last_sync_at).toLocaleString()
              : null,
        };
      });
  }, [lastTapSyncResults]);

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
      const draft = await prepareCommunityImport(path);
      setImportDraft(draft);
      setImportDraftSource('file');
    } catch (importError) {
      setError(importError instanceof Error ? importError.message : String(importError));
    }
  }

  async function handleImportEntry(entry: CommunityProfileIndexEntry) {
    setNotice(null);
    try {
      const draft = await prepareCommunityImport(entry.manifest_path);
      setImportDraft(draft);
      setImportDraftSource(entry.tap_url);
    } catch (importError) {
      setError(importError instanceof Error ? importError.message : String(importError));
    }
  }

  return (
    <section className="crosshook-card crosshook-community-browser" aria-label="Community profile browser">
      <header className="crosshook-community-browser__header">
        <div className="crosshook-heading-eyebrow">Community</div>
        <h2 className="crosshook-heading-title">Browse shared profiles</h2>
        <p className="crosshook-heading-copy">
          Search profiles from your configured taps, inspect compatibility metadata, and import a profile into your
          local CrossHook library.
        </p>
      </header>

      {cachedTapNotices.length > 0 ? (
        <div className="crosshook-community-browser__cache-banner" role="status" aria-live="polite">
          <span className="crosshook-status-chip crosshook-community-browser__cache-chip">Cached data</span>
          <div>
            <p className="crosshook-community-browser__helper" style={{ margin: 0 }}>
              Showing cached tap profiles (git fetch failed; local clone in use). Last successful sync:
            </p>
            <ul className="crosshook-community-browser__cache-banner-list">
              {cachedTapNotices.map((row) => (
                <li key={row.tapKey}>
                  <strong>{row.labelPrefix}</strong>
                  {row.lastSync ? ` — last synced: ${row.lastSync}` : ' — last synced: unknown'}
                </li>
              ))}
            </ul>
          </div>
        </div>
      ) : null}

      <CollapsibleSection title="Tap Management" className="crosshook-panel crosshook-community-browser__panel">
        <div className="crosshook-community-browser__footer">
          <div className="crosshook-community-browser__section-copy">
            <p className="crosshook-muted crosshook-community-browser__helper">
              Taps are persisted in CrossHook settings and synced through the backend community commands.
            </p>
          </div>
          <div className="crosshook-community-browser__button-row">
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

        <div className="crosshook-community-browser__toolbar">
          <div className="crosshook-community-browser__field">
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
          <div className="crosshook-community-browser__field">
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
          <div className="crosshook-community-browser__tap-list">
            {taps.map((tap) => (
              <TapChip
                key={tapSubscriptionStableKey(tap)}
                tap={tap}
                headCommit={getTapHeadCommit(tap)}
                busy={loading || syncing}
                onRemove={(tapToRemove) => {
                  void removeTap(tapToRemove).catch((removeError) => {
                    setError(removeError instanceof Error ? removeError.message : String(removeError));
                  });
                }}
                onPin={(tapToPin) => {
                  setNotice(null);
                  void pinTapToCurrentVersion(tapToPin)
                    .then(() =>
                      setNotice(
                        `Pinned ${tapToPin.url} to ${getTapHeadCommit(tapToPin)?.slice(0, 12) ?? 'current commit'}.`
                      )
                    )
                    .catch((pinError) => {
                      setError(pinError instanceof Error ? pinError.message : String(pinError));
                    });
                }}
                onUnpin={(tapToUnpin) => {
                  setNotice(null);
                  void unpinTap(tapToUnpin)
                    .then(() => setNotice(`Unpinned ${tapToUnpin.url}; next sync will track branch head.`))
                    .catch((unpinError) => {
                      setError(unpinError instanceof Error ? unpinError.message : String(unpinError));
                    });
                }}
              />
            ))}
          </div>
        ) : (
          <p className="crosshook-muted crosshook-community-browser__helper">
            Add a tap URL to populate the community browser.
          </p>
        )}
      </CollapsibleSection>

      <CollapsibleSection
        title="Community Profiles"
        className="crosshook-panel crosshook-community-browser__panel"
        meta={
          <span>
            {visibleEntries.length} of {index.entries.length} profiles
          </span>
        }
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
              onChange={(event) => setQuery(event.target.value)}
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
              onValueChange={(val) => setRatingFilter(val as 'all' | CommunityCompatibilityRating)}
              options={[
                { value: 'all', label: 'All ratings' },
                ...ratingOrder.map((rating) => ({ value: rating, label: ratingLabel[rating] })),
              ]}
            />
          </div>
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => void handleImportFromFile()}
          >
            Import JSON
          </button>
        </div>

        {notice ? <p className="crosshook-success crosshook-community-browser__helper">{notice}</p> : null}
        {error ? <p className="crosshook-community-browser__error">{error}</p> : null}
        {index.diagnostics.length > 0 ? (
          <div className="crosshook-community-browser__diagnostics">
            {index.diagnostics.map((diagnostic) => (
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
                      <span className="crosshook-community-browser__imported-badge" aria-label="Already imported">
                        Imported
                      </span>
                    ) : (
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
                    )}
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </CollapsibleSection>
      <CommunityImportWizardModal
        open={importDraft !== null}
        draft={importDraft}
        saving={importing}
        onClose={() => {
          setImportDraft(null);
          setImportDraftSource(null);
        }}
        onSave={async (profileName, profile, summary) => {
          await saveImportedProfile(profileName, profile);
          const sourceLabel =
            importDraftSource === 'file' || importDraftSource === null ? profilesDirectoryPath : importDraftSource;
          setNotice(
            `Imported ${profileName} (${summary.autoResolvedCount} auto-resolved, ${summary.unresolvedCount} unresolved) from ${sourceLabel}.`
          );
          setImportDraft(null);
          setImportDraftSource(null);
        }}
      />
    </section>
  );
}

export default CommunityBrowser;
