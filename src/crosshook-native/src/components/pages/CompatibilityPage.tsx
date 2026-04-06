import { useCallback, useMemo, useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';

import CompatibilityViewer, { type CompatibilityDatabaseEntry } from '../CompatibilityViewer';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { useCommunityProfiles } from '../../hooks/useCommunityProfiles';
import { useProtonInstalls } from '../../hooks/useProtonInstalls';
import { useProtonUp } from '../../hooks/useProtonUp';
import type { ProtonUpAvailableVersion, ProtonUpInstallResult } from '../../types/protonup';

const DEFAULT_PROFILES_DIRECTORY = '~/.config/crosshook/profiles';
const DEFAULT_COMPAT_TOOLS_DIR = '~/.local/share/Steam/compatibilitytools.d';

type CompatTabId = 'trainer' | 'proton';

const TAB_LABELS: Record<CompatTabId, string> = {
  trainer: 'Trainer',
  proton: 'Proton',
};

function toCompatibilityEntries(
  entries: ReturnType<typeof useCommunityProfiles>['index']['entries']
): CompatibilityDatabaseEntry[] {
  return entries.map((entry) => ({
    id: `${entry.tap_url}::${entry.relative_path}`,
    tap_url: entry.tap_url,
    tap_branch: entry.tap_branch,
    manifest_path: entry.manifest_path,
    relative_path: entry.relative_path,
    metadata: entry.manifest.metadata,
  }));
}

function formatAssetSize(bytes: number): string {
  return `${(bytes / 1_048_576).toFixed(0)} MB`;
}

function normalizeForComparison(value: string): string {
  return value.replace(/[^a-z0-9]/gi, '').toLowerCase();
}

function ProtonVersionsPanel() {
  const protonUp = useProtonUp({ autoFetchCatalog: true });
  const { installs, reload: reloadInstalls } = useProtonInstalls();
  const [installResult, setInstallResult] = useState<ProtonUpInstallResult | null>(null);
  const [installingVersion, setInstallingVersion] = useState<string | null>(null);

  const installedNames = useMemo(
    () => new Set(installs.map((i) => normalizeForComparison(i.name))),
    [installs],
  );

  const handleInstall = useCallback(
    async (version: ProtonUpAvailableVersion) => {
      setInstallResult(null);
      setInstallingVersion(version.version);
      const result = await protonUp.installVersion({
        provider: version.provider,
        version: version.version,
        target_root: DEFAULT_COMPAT_TOOLS_DIR,
      });
      setInstallResult(result);
      setInstallingVersion(null);
      if (result.success) {
        protonUp.refreshCatalog();
        reloadInstalls();
      }
    },
    [protonUp, reloadInstalls],
  );

  const dismissResult = useCallback(() => setInstallResult(null), []);

  return (
    <div className="crosshook-subtab-content__inner">
      <CollapsibleSection
        title="Proton Runtimes"
        className="crosshook-panel"
        meta={
          <span>
            {protonUp.catalogLoading
              ? 'Loading...'
              : `${protonUp.versions.length} version${protonUp.versions.length !== 1 ? 's' : ''}`}
          </span>
        }
      >
        <div className="crosshook-protonup-catalog">
          {/* Cache status */}
          {protonUp.cacheMeta ? (
            <p className="crosshook-help-text">
              {protonUp.cacheMeta.offline
                ? 'Offline \u2014 showing cached versions.'
                : protonUp.cacheMeta.stale
                  ? 'Showing stale cached versions. Refresh to update.'
                  : protonUp.cacheMeta.fetched_at
                    ? `Last updated: ${new Date(protonUp.cacheMeta.fetched_at).toLocaleString()}`
                    : null}
            </p>
          ) : null}

          {/* Error state */}
          {protonUp.catalogError ? (
            <p className="crosshook-danger" role="alert">
              Failed to load versions: {protonUp.catalogError}
            </p>
          ) : null}

          {/* Install result feedback */}
          {installResult ? (
            <div
              className={`crosshook-protonup-catalog__result ${installResult.success ? '' : 'crosshook-protonup-catalog__result--error'}`}
              role="status"
            >
              <p className={installResult.success ? 'crosshook-help-text' : 'crosshook-danger'}>
                {installResult.success
                  ? `Installed ${installResult.installed_path ?? 'successfully'}.`
                  : `Install failed: ${installResult.error_message ?? installResult.error_kind ?? 'unknown error'}`}
              </p>
              <button
                type="button"
                className="crosshook-button crosshook-button--small crosshook-button--ghost"
                onClick={dismissResult}
              >
                Dismiss
              </button>
            </div>
          ) : null}

          {/* Version list */}
          {protonUp.versions.length > 0 ? (
            <ul className="crosshook-protonup-catalog__list">
              {protonUp.versions.map((version) => {
                const isInstallingThis = installingVersion === version.version;
                const isInstalled = installedNames.has(normalizeForComparison(version.version));

                return (
                  <li key={`${version.provider}:${version.version}`} className="crosshook-protonup-catalog__item">
                    <div className="crosshook-protonup-catalog__item-info">
                      <span className="crosshook-protonup-catalog__item-name">{version.version}</span>
                      {version.asset_size ? (
                        <span className="crosshook-muted crosshook-protonup-catalog__item-size">
                          {formatAssetSize(version.asset_size)}
                        </span>
                      ) : null}
                    </div>
                    {isInstalled ? (
                      <span className="crosshook-muted crosshook-protonup-catalog__installed-label">Installed</span>
                    ) : (
                      <button
                        type="button"
                        className="crosshook-button crosshook-button--small crosshook-button--primary"
                        onClick={() => void handleInstall(version)}
                        disabled={protonUp.installing}
                      >
                        {isInstallingThis ? 'Installing\u2026' : 'Install'}
                      </button>
                    )}
                  </li>
                );
              })}
            </ul>
          ) : !protonUp.catalogLoading && !protonUp.catalogError ? (
            protonUp.cacheMeta?.offline ? (
              <p className="crosshook-help-text">
                Version catalog is unavailable offline. Connect to the internet and refresh.
              </p>
            ) : (
              <p className="crosshook-help-text">No versions available.</p>
            )
          ) : null}

          {/* Refresh action */}
          <div className="crosshook-protonup-catalog__actions">
            <button
              type="button"
              className="crosshook-button crosshook-button--small crosshook-button--ghost"
              onClick={protonUp.refreshCatalog}
              disabled={protonUp.catalogLoading}
            >
              {protonUp.catalogLoading ? 'Refreshing\u2026' : 'Refresh catalog'}
            </button>
          </div>
        </div>
      </CollapsibleSection>
    </div>
  );
}

export function CompatibilityPage() {
  const [activeTab, setActiveTab] = useState<CompatTabId>('trainer');

  const communityState = useCommunityProfiles({
    profilesDirectoryPath: DEFAULT_PROFILES_DIRECTORY,
  });
  const compatibilityEntries = toCompatibilityEntries(communityState.index.entries);

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--compatibility">
      <div className="crosshook-route-stack crosshook-compatibility-page">
        <div className="crosshook-route-stack__body--fill crosshook-compatibility-page__body">
          <div className="crosshook-route-card-host">
            <div className="crosshook-route-card-scroll">
              <div className="crosshook-panel crosshook-subtabs-shell crosshook-compatibility-subtabs">
                <Tabs.Root
                  className="crosshook-subtabs-root"
                  value={activeTab}
                  onValueChange={(val) => setActiveTab(val as CompatTabId)}
                >
                  <Tabs.List className="crosshook-subtab-row" aria-label="Compatibility sections">
                    {(Object.keys(TAB_LABELS) as CompatTabId[]).map((tab) => (
                      <Tabs.Trigger
                        key={tab}
                        value={tab}
                        className={`crosshook-subtab${activeTab === tab ? ' crosshook-subtab--active' : ''}`}
                      >
                        {TAB_LABELS[tab]}
                      </Tabs.Trigger>
                    ))}
                  </Tabs.List>

                  {/* Trainer tab */}
                  <Tabs.Content
                    value="trainer"
                    forceMount
                    className="crosshook-subtab-content"
                    style={{ display: activeTab === 'trainer' ? undefined : 'none' }}
                  >
                    <div className="crosshook-subtab-content__inner">
                      <CompatibilityViewer
                        entries={compatibilityEntries}
                        loading={communityState.loading || communityState.syncing}
                        error={communityState.error}
                        emptyMessage="No indexed community compatibility entries are available yet."
                      />
                    </div>
                  </Tabs.Content>

                  {/* Proton tab */}
                  <Tabs.Content
                    value="proton"
                    forceMount
                    className="crosshook-subtab-content"
                    style={{ display: activeTab === 'proton' ? undefined : 'none' }}
                  >
                    <ProtonVersionsPanel />
                  </Tabs.Content>
                </Tabs.Root>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default CompatibilityPage;
