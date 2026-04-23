import { useMemo, useState } from 'react';
import { open } from '@/lib/plugin-stubs/dialog';
import {
  type CommunityCompatibilityRating,
  type CommunityImportPreview,
  type CommunityProfileIndexEntry,
  type CommunityTapSubscription,
  type CommunityTapSyncResult,
  type UseCommunityProfilesResult,
  useCommunityProfiles,
} from '../hooks/useCommunityProfiles';
import CommunityImportWizardModal from './CommunityImportWizardModal';
import { CommunityProfilesSection } from './community/CommunityProfilesSection';
import { CommunityTapManagementSection } from './community/CommunityTapManagementSection';
import { ratingOrder } from './community/CompatibilityBadge';

export interface CommunityBrowserProps {
  profilesDirectoryPath?: string;
  state?: UseCommunityProfilesResult;
}

const DEFAULT_PROFILES_DIRECTORY = '~/.config/crosshook/profiles';

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

      <CommunityTapManagementSection
        taps={taps}
        tapUrl={tapUrl}
        tapBranch={tapBranch}
        loading={loading}
        syncing={syncing}
        onTapUrlChange={setTapUrl}
        onTapBranchChange={setTapBranch}
        onAddTap={() => {
          void handleAddTap();
        }}
        onRefresh={() => {
          void refreshProfiles().catch((refreshError) => {
            setError(refreshError instanceof Error ? refreshError.message : String(refreshError));
          });
        }}
        onSync={() => {
          void syncTaps().catch((syncError) => {
            setError(syncError instanceof Error ? syncError.message : String(syncError));
          });
        }}
        onRemoveTap={(tapToRemove) => {
          void removeTap(tapToRemove).catch((removeError) => {
            setError(removeError instanceof Error ? removeError.message : String(removeError));
          });
        }}
        onPinTap={(tapToPin) => {
          setNotice(null);
          void pinTapToCurrentVersion(tapToPin)
            .then(() =>
              setNotice(`Pinned ${tapToPin.url} to ${getTapHeadCommit(tapToPin)?.slice(0, 12) ?? 'current commit'}.`)
            )
            .catch((pinError) => {
              setError(pinError instanceof Error ? pinError.message : String(pinError));
            });
        }}
        onUnpinTap={(tapToUnpin) => {
          setNotice(null);
          void unpinTap(tapToUnpin)
            .then(() => setNotice(`Unpinned ${tapToUnpin.url}; next sync will track branch head.`))
            .catch((unpinError) => {
              setError(unpinError instanceof Error ? unpinError.message : String(unpinError));
            });
        }}
        getTapHeadCommit={getTapHeadCommit}
      />

      <CommunityProfilesSection
        visibleEntries={visibleEntries}
        totalEntries={index.entries.length}
        diagnostics={index.diagnostics}
        query={query}
        ratingFilter={ratingFilter}
        loading={loading}
        importing={importing}
        notice={notice}
        error={error}
        importedProfileNames={importedProfileNames}
        onQueryChange={setQuery}
        onRatingFilterChange={setRatingFilter}
        onImportFromFile={() => void handleImportFromFile()}
        onImportEntry={(entry) => void handleImportEntry(entry)}
      />

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
