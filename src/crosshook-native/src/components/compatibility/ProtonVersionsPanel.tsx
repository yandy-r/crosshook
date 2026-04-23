import { useCallback, useMemo, useState } from 'react';
import { useProtonInstalls } from '../../hooks/useProtonInstalls';
import { useProtonUp } from '../../hooks/useProtonUp';
import type { ProtonUpAvailableVersion, ProtonUpInstallResult, ProtonUpProvider } from '../../types/protonup';
import { CollapsibleSection } from '../ui/CollapsibleSection';

export const DEFAULT_COMPAT_TOOLS_DIR = '~/.local/share/Steam/compatibilitytools.d';

function formatAssetSize(bytes: number): string {
  return `${(bytes / 1_048_576).toFixed(0)} MB`;
}

function normalizeForComparison(value: string): string {
  return value.replace(/[^a-z0-9]/gi, '').toLowerCase();
}

export function ProtonVersionsPanel() {
  const [catalogProvider, setCatalogProvider] = useState<ProtonUpProvider>('ge-proton');
  const protonUp = useProtonUp({ autoFetchCatalog: true, catalogProvider });
  const { installs, reload: reloadInstalls } = useProtonInstalls();
  const [installResult, setInstallResult] = useState<ProtonUpInstallResult | null>(null);
  const [installingVersion, setInstallingVersion] = useState<string | null>(null);

  const installedNames = useMemo(() => new Set(installs.map((item) => normalizeForComparison(item.name))), [installs]);

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
    [protonUp, reloadInstalls]
  );

  const dismissResult = useCallback(() => setInstallResult(null), []);

  return (
    <section className="crosshook-dashboard-route-section-stack" aria-label="Proton runtime catalog">
      <div className="crosshook-dashboard-panel-section__header">
        <div className="crosshook-dashboard-panel-section__heading-group">
          <p className="crosshook-dashboard-panel-section__eyebrow crosshook-heading-eyebrow">Proton runtime catalog</p>
          <div className="crosshook-dashboard-panel-section__title-group">
            <h3 className="crosshook-heading-title crosshook-heading-title--card crosshook-dashboard-panel-section__title">
              Install compatibility tools without leaving Compatibility
            </h3>
            <p className="crosshook-dashboard-panel-section__summary crosshook-heading-copy">
              Browse provider catalogs, keep tab-local install state mounted, and install releases into Steam&apos;s
              default compatibility tools directory.
            </p>
          </div>
        </div>
        <div className="crosshook-dashboard-panel-section__actions">
          <div className="crosshook-status-chip">{installs.length} installed</div>
        </div>
      </div>

      <div className="crosshook-dashboard-pill-row">
        <span className="crosshook-dashboard-pill">Target: {DEFAULT_COMPAT_TOOLS_DIR}</span>
        <span className="crosshook-dashboard-pill">
          Source: {catalogProvider === 'ge-proton' ? 'GE-Proton' : 'Proton-CachyOS'}
        </span>
      </div>

      <CollapsibleSection
        title="Catalog source"
        className="crosshook-panel"
        meta={<span>{protonUp.catalogLoading ? 'Refreshing…' : 'Mounted across tab switches'}</span>}
      >
        <div className="crosshook-protonup-catalog">
          {/* biome-ignore lint/a11y/useSemanticElements: legend does not associate with plain buttons; fieldset is non-idiomatic here */}
          <div
            role="group"
            aria-labelledby="proton-catalog-source-heading"
            className="crosshook-field crosshook-fieldset-reset--mb-12"
          >
            <div id="proton-catalog-source-heading" className="crosshook-label">
              Catalog source
            </div>
            <div
              className="crosshook-protonup-catalog__provider-toggle"
              style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginTop: 6 }}
            >
              <button
                type="button"
                className={`crosshook-button crosshook-button--small${
                  catalogProvider === 'ge-proton' ? ' crosshook-button--primary' : ' crosshook-button--ghost'
                }`}
                onClick={() => setCatalogProvider('ge-proton')}
                aria-pressed={catalogProvider === 'ge-proton'}
              >
                GE-Proton
              </button>
              <button
                type="button"
                className={`crosshook-button crosshook-button--small${
                  catalogProvider === 'proton-cachyos' ? ' crosshook-button--primary' : ' crosshook-button--ghost'
                }`}
                onClick={() => setCatalogProvider('proton-cachyos')}
                aria-pressed={catalogProvider === 'proton-cachyos'}
              >
                Proton-CachyOS
              </button>
            </div>
          </div>

          {protonUp.cacheMeta ? (
            <p className="crosshook-help-text">
              {protonUp.cacheMeta.offline
                ? 'Offline — showing cached versions.'
                : protonUp.cacheMeta.stale
                  ? 'Showing stale cached versions. Refresh to update.'
                  : protonUp.cacheMeta.fetched_at
                    ? `Last updated: ${new Date(protonUp.cacheMeta.fetched_at).toLocaleString()}`
                    : null}
            </p>
          ) : null}

          {protonUp.catalogError ? (
            <p className="crosshook-danger" role="alert">
              Failed to load versions: {protonUp.catalogError}
            </p>
          ) : null}

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

          <div className="crosshook-protonup-catalog__actions">
            <button
              type="button"
              className="crosshook-button crosshook-button--small crosshook-button--ghost"
              onClick={protonUp.refreshCatalog}
              disabled={protonUp.catalogLoading}
            >
              {protonUp.catalogLoading ? 'Refreshing…' : 'Refresh catalog'}
            </button>
          </div>
        </div>
      </CollapsibleSection>

      <CollapsibleSection
        title="Available versions"
        className="crosshook-panel"
        meta={
          <span>
            {protonUp.catalogLoading
              ? 'Loading…'
              : `${protonUp.versions.length} version${protonUp.versions.length !== 1 ? 's' : ''}`}
          </span>
        }
      >
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
                      {isInstallingThis ? 'Installing…' : 'Install'}
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
      </CollapsibleSection>
    </section>
  );
}

export default ProtonVersionsPanel;
