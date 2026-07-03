import { useMemo, useState } from 'react';
import { open } from '@/lib/plugin-stubs/dialog';
import { useCommunityCatalog } from '../hooks/useCommunityCatalog';
import {
  type CommunityImportPreview,
  type CommunityTapSyncResult,
  type UseCommunityProfilesResult,
  useCommunityProfiles,
} from '../hooks/useCommunityProfiles';
import { type LutrisImportEntry, type LutrisImportPreview, useLutrisImport } from '../hooks/useLutrisImport';
import type { CatalogEntry, CatalogFacets } from '../types/discovery';
import { chooseDirectory } from '../utils/dialog';
import CommunityImportWizardModal from './CommunityImportWizardModal';
import { CommunityProfilesSection } from './community/CommunityProfilesSection';
import { CommunityTapManagementSection } from './community/CommunityTapManagementSection';
import { tapSubscriptionStableKey } from './community/tapSubscriptionKey';
import LutrisImportModal from './LutrisImportModal';

export interface CommunityBrowserProps {
  profilesDirectoryPath?: string;
  state?: UseCommunityProfilesResult;
}

const DEFAULT_PROFILES_DIRECTORY = '~/.config/crosshook/profiles';

const EMPTY_CATALOG_FACETS: CatalogFacets = {
  gameTitles: [],
  loadingModes: [],
  compatibilityBands: [],
  taps: [],
};

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
  const [notice, setNotice] = useState<string | null>(null);
  const [importDraft, setImportDraft] = useState<CommunityImportPreview | null>(null);
  const [importDraftSource, setImportDraftSource] = useState<string | null>(null);
  const [lutrisPreview, setLutrisPreview] = useState<LutrisImportPreview | null>(null);
  const {
    isPreparing: isLutrisPreparing,
    isImporting: isLutrisImporting,
    importResult: lutrisImportResult,
    importError: lutrisImportError,
    clearImportState: clearLutrisImportState,
    prepare: prepareLutrisImport,
    importProfiles: importLutrisProfiles,
  } = useLutrisImport();
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
  const catalog = useCommunityCatalog();

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
      void catalog.refresh();
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

  async function handleImportFromLutris() {
    setNotice(null);
    setError(null);
    clearLutrisImportState();

    try {
      let preview = await prepareLutrisImport();

      if (!preview.lutris_root) {
        const directory = await chooseDirectory('Select Lutris config directory');
        if (!directory) {
          return;
        }

        preview = await prepareLutrisImport(directory);
      }

      if (!preview.lutris_root) {
        setError(preview.diagnostics.join(' ') || 'Lutris library directory was not found.');
        return;
      }

      setLutrisPreview(preview);
    } catch (importError) {
      setError(importError instanceof Error ? importError.message : String(importError));
    }
  }

  async function handleLutrisImport(entries: LutrisImportEntry[]) {
    const result = await importLutrisProfiles(entries);
    if (result && result.imported_count > 0) {
      await refreshProfiles().catch(() => undefined);
    }
  }

  function handleCloseLutrisImportModal() {
    if (lutrisImportResult) {
      setNotice(
        `Imported ${lutrisImportResult.imported_count} Lutris profile${lutrisImportResult.imported_count !== 1 ? 's' : ''}${
          lutrisImportResult.skipped_count > 0 ? ` (${lutrisImportResult.skipped_count} skipped)` : ''
        }${lutrisImportResult.failed_count > 0 ? ` (${lutrisImportResult.failed_count} failed)` : ''}.`
      );
    } else if (lutrisImportError) {
      setError(lutrisImportError);
    }

    setLutrisPreview(null);
    clearLutrisImportState();
  }

  async function handleImportEntry(entry: CatalogEntry) {
    setNotice(null);
    try {
      const draft = await prepareCommunityImport(entry.manifestPath);
      setImportDraft(draft);
      setImportDraftSource(entry.tapUrl);
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
          void syncTaps()
            .then(() => catalog.refresh())
            .catch((syncError) => {
              setError(syncError instanceof Error ? syncError.message : String(syncError));
            });
        }}
        onRemoveTap={(tapToRemove) => {
          void removeTap(tapToRemove)
            .then(() => catalog.refresh())
            .catch((removeError) => {
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
        entries={catalog.data?.entries ?? []}
        totalCount={catalog.data?.totalCount ?? 0}
        facets={catalog.data?.facets ?? EMPTY_CATALOG_FACETS}
        catalogQuery={catalog.query}
        searchText={catalog.searchText}
        diagnostics={index.diagnostics}
        catalogLoading={loading || catalog.loading}
        catalogError={catalog.error}
        degraded={catalog.data?.degraded ?? false}
        importing={importing}
        notice={notice}
        error={error}
        importedProfileNames={importedProfileNames}
        onSearchTextChange={catalog.setSearchText}
        onSetFacet={catalog.setFacet}
        onClearFilters={catalog.clearFilters}
        onLoadMore={() => void catalog.loadMore()}
        onImportFromFile={() => void handleImportFromFile()}
        onImportFromLutris={() => void handleImportFromLutris()}
        lutrisBusy={isLutrisPreparing || isLutrisImporting}
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

      {lutrisPreview ? (
        <LutrisImportModal
          preview={lutrisPreview}
          onClose={handleCloseLutrisImportModal}
          onImport={(entries) => {
            void handleLutrisImport(entries);
          }}
          isImporting={isLutrisImporting}
          importResult={lutrisImportResult}
          importError={lutrisImportError}
        />
      ) : null}
    </section>
  );
}

export default CommunityBrowser;
