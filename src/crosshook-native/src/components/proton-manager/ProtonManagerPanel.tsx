import { useCallback, useMemo, useState } from 'react';
import { useProtonManager } from '../../hooks/useProtonManager';
import { classifyInstallProvider, normalizeInstallToTag } from '../../lib/protonup/classifyInstall';
import type { ProtonUpAvailableVersion, ProtonUpInstallRequest } from '../../types/protonup';
import { InstallProgressBar } from './InstallProgressBar';
import { InstallRootBadge } from './InstallRootBadge';
import { ProviderPicker } from './ProviderPicker';
import { VersionRow } from './VersionRow';

interface ProtonManagerPanelProps {
  steamClientInstallPath?: string;
}

export function ProtonManagerPanel({ steamClientInstallPath }: ProtonManagerPanelProps) {
  const manager = useProtonManager({ steamClientInstallPath });

  const [selectedRootPath, setSelectedRootPath] = useState<string | null>(null);
  const [uninstallWarning, setUninstallWarning] = useState<string | null>(null);
  const [uninstallError, setUninstallError] = useState<string | null>(null);
  const [installError, setInstallError] = useState<string | null>(null);
  const [cancelError, setCancelError] = useState<string | null>(null);

  const effectiveRoot = useMemo(() => {
    if (selectedRootPath !== null) {
      return manager.roots.find((r) => r.path === selectedRootPath) ?? manager.defaultRoot;
    }
    return manager.defaultRoot;
  }, [selectedRootPath, manager.roots, manager.defaultRoot]);

  const hasWritableRoot = manager.roots.some((r) => r.writable);

  const providersById = useMemo(() => new Map(manager.providers.map((p) => [p.id, p])), [manager.providers]);

  // Track which versions are actively installing (keyed by `${provider}:${version}`
  // so the same tag on two providers is tracked independently in All mode).
  const [installingKeys, setInstallingKeys] = useState<Set<string>>(new Set());

  const handleInstall = useCallback(
    async (versionDto: ProtonUpAvailableVersion) => {
      setInstallError(null);
      if (!effectiveRoot?.writable) {
        setInstallError('No writable install root available.');
        return;
      }

      const request: ProtonUpInstallRequest = {
        provider: versionDto.provider,
        version: versionDto.version,
        target_root: effectiveRoot.path,
      };

      const key = `${versionDto.provider}:${versionDto.version}`;
      setInstallingKeys((prev) => new Set(prev).add(key));
      try {
        await manager.install(request, versionDto);
      } catch (err) {
        setInstallError(err instanceof Error ? err.message : String(err));
      } finally {
        setInstallingKeys((prev) => {
          const next = new Set(prev);
          next.delete(key);
          return next;
        });
      }
    },
    [effectiveRoot, manager]
  );

  const handleCancel = useCallback(
    async (opId: string) => {
      setCancelError(null);
      try {
        const cancelled = await manager.cancel(opId);
        if (!cancelled) {
          setCancelError('Cancel request was not accepted. The install may have already completed.');
        }
      } catch (err) {
        setCancelError(err instanceof Error ? err.message : String(err));
      }
    },
    [manager]
  );

  const handleDismissOp = useCallback(
    (opId: string) => {
      manager.dismissOp(opId);
      // A terminal op may have produced a new install; rescan so the row
      // migrates from Available → Installed without requiring a page reload.
      manager.installs.reload();
    },
    [manager]
  );

  const handleUninstall = useCallback(
    async (toolPath: string) => {
      setUninstallWarning(null);
      setUninstallError(null);
      try {
        const plan = await manager.planUninstall(toolPath);
        if (!plan.success) {
          setUninstallError(plan.error_message ?? 'Uninstall plan failed.');
          return;
        }

        if (plan.conflicting_app_ids.length > 0) {
          const confirmed = window.confirm(
            `This Proton version is referenced by these Steam app IDs: ${plan.conflicting_app_ids.join(', ')}.\n\nProceed with uninstall?`
          );
          if (!confirmed) return;
        }

        const result = await manager.uninstall(toolPath);
        if (!result.success) {
          setUninstallError(result.error_message ?? 'Uninstall failed.');
        } else if (plan.conflicting_app_ids.length > 0) {
          setUninstallWarning(
            `Uninstalled. The following apps referenced this version: ${plan.conflicting_app_ids.join(', ')}`
          );
        }
      } catch (err) {
        setUninstallError(err instanceof Error ? err.message : String(err));
      }
    },
    [manager]
  );

  // Filter installed list by selected provider. In All mode, show everything.
  // `null` classification (tag doesn't match any known provider) is surfaced
  // only in All mode so a user switching to a specific provider doesn't see
  // unclassified installs erroneously attributed to that provider.
  const filteredInstalls = useMemo(() => {
    if (manager.selectedProviderId === null) {
      return manager.installs.installs;
    }
    return manager.installs.installs.filter((i) => classifyInstallProvider(i.name) === manager.selectedProviderId);
  }, [manager.installs.installs, manager.selectedProviderId]);

  // Set of keys identifying installed tools, normalized so Proton-CachyOS's
  // directory name (e.g. `proton-cachyos-<tag>-x86_64`) matches its catalog
  // tag (e.g. `cachyos-<tag>`). Key shape: `${providerId}:${normalizedTag}`.
  const installedKeySet = useMemo(() => {
    const keys = new Set<string>();
    for (const install of manager.installs.installs) {
      const pid = classifyInstallProvider(install.name);
      if (pid === null) continue;
      keys.add(`${pid}:${normalizeInstallToTag(install.name, pid)}`);
    }
    return keys;
  }, [manager.installs.installs]);

  const availableVersions = useMemo(
    () => manager.catalog.versions.filter((v) => !installedKeySet.has(`${v.provider}:${v.version}`)),
    [manager.catalog.versions, installedKeySet]
  );

  // Lookup table for enriching installed rows with catalog metadata
  // (release date, archive size). Keyed by `${provider}:${version}` so same
  // tag on two providers doesn't collide.
  const catalogByKey = useMemo(() => {
    const map = new Map<string, (typeof manager.catalog.versions)[number]>();
    for (const v of manager.catalog.versions) {
      map.set(`${v.provider}:${v.version}`, v);
    }
    return map;
  }, [manager.catalog.versions]);

  if (manager.loading) {
    return (
      <div className="crosshook-proton-manager" aria-busy="true">
        <p className="crosshook-muted">Loading Proton manager…</p>
      </div>
    );
  }

  const cacheMeta = manager.catalog.cacheMeta;
  const showStaleBanner = cacheMeta != null && (cacheMeta.stale || cacheMeta.offline);

  return (
    <div className="crosshook-proton-manager">
      <div className="crosshook-proton-manager__header">
        <ProviderPicker
          providers={manager.providers}
          selectedProviderId={manager.selectedProviderId}
          onSelect={manager.setSelectedProviderId}
        />

        {manager.roots.length > 0 ? (
          <fieldset className="crosshook-proton-manager__roots">
            <legend className="crosshook-provider-picker__legend">Install root</legend>
            {manager.roots.map((root) => (
              <InstallRootBadge
                key={root.path}
                root={root}
                isDefault={manager.defaultRoot?.path === root.path}
                isSelected={(effectiveRoot?.path ?? '') === root.path}
                onSelect={setSelectedRootPath}
              />
            ))}
          </fieldset>
        ) : null}
      </div>

      {showStaleBanner ? (
        <div className="crosshook-proton-manager__stale-banner" role="status">
          {cacheMeta?.offline
            ? 'Offline — showing cached catalog. Connect to the internet to refresh.'
            : 'Catalog data may be stale.'}
          {!cacheMeta?.offline ? (
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost crosshook-button--ghost--small"
              style={{ marginLeft: 8 }}
              onClick={() => manager.catalog.refreshCatalog()}
            >
              Refresh
            </button>
          ) : null}
        </div>
      ) : null}

      {!hasWritableRoot && manager.roots.length > 0 ? (
        <div className="crosshook-proton-manager__readonly-banner" role="alert">
          No writable compatibilitytools.d found. In Flatpak, this typically means the Steam data path is read-only.
          Install actions are disabled.
        </div>
      ) : null}

      {effectiveRoot && !effectiveRoot.writable ? (
        <div className="crosshook-proton-manager__readonly-banner" role="alert">
          The selected install root is read-only. Choose a writable root to install versions.
        </div>
      ) : null}

      {manager.activeOpIds.length > 0 ? (
        <div className="crosshook-proton-manager__progress-list">
          {manager.activeOpIds.map((opId) => (
            <InstallProgressBar key={opId} opId={opId} onCancel={handleCancel} onDismiss={handleDismissOp} />
          ))}
        </div>
      ) : null}

      {uninstallWarning ? (
        <div className="crosshook-proton-manager__stale-banner" role="status">
          {uninstallWarning}
        </div>
      ) : null}

      {uninstallError ? (
        <div className="crosshook-proton-manager__error-banner" role="alert">
          {uninstallError}
        </div>
      ) : null}

      {installError ? (
        <div className="crosshook-proton-manager__error-banner" role="alert">
          Install failed: {installError}
        </div>
      ) : null}

      {cancelError ? (
        <div className="crosshook-proton-manager__error-banner" role="alert">
          Cancel failed: {cancelError}
        </div>
      ) : null}

      {manager.error ? (
        <div className="crosshook-proton-manager__error-banner" role="alert">
          {manager.error}
        </div>
      ) : null}

      <ul className="crosshook-proton-manager__list" aria-label="Proton versions">
        {filteredInstalls.length > 0 ? (
          <>
            <li className="crosshook-proton-manager__section-label" aria-hidden="true">
              Installed
            </li>
            {filteredInstalls.map((install) => {
              const classified = classifyInstallProvider(install.name);
              const rowProvider = classified ?? 'unknown';
              const catalogMatch = classified
                ? (catalogByKey.get(`${classified}:${normalizeInstallToTag(install.name, classified)}`) ?? null)
                : null;
              return (
                <li key={install.name}>
                  <VersionRow
                    version={install.name}
                    provider={rowProvider}
                    installed={true}
                    installing={false}
                    canInstall={false}
                    onInstall={() => undefined}
                    onUninstall={() => void handleUninstall(install.path)}
                    publishedAt={catalogMatch?.published_at ?? null}
                    assetSize={catalogMatch?.asset_size ?? null}
                  />
                </li>
              );
            })}
          </>
        ) : null}

        {manager.catalog.catalogLoading ? (
          <li>
            <p className="crosshook-proton-manager__empty crosshook-muted">Loading catalog…</p>
          </li>
        ) : availableVersions.length > 0 ? (
          <>
            <li className="crosshook-proton-manager__section-label" aria-hidden="true">
              Available
            </li>
            {availableVersions.map((v) => {
              const providerDescriptor = providersById.get(v.provider);
              const providerSupportsInstall = providerDescriptor?.supports_install ?? false;
              const rowCanInstall = Boolean(effectiveRoot?.writable) && providerSupportsInstall;
              const key = `${v.provider}:${v.version}`;
              return (
                <li key={key}>
                  <VersionRow
                    version={v.version}
                    provider={v.provider}
                    installed={false}
                    installing={installingKeys.has(key)}
                    canInstall={rowCanInstall}
                    onInstall={() => void handleInstall(v)}
                    onUninstall={() => undefined}
                    publishedAt={v.published_at ?? null}
                    assetSize={v.asset_size ?? null}
                  />
                </li>
              );
            })}
          </>
        ) : !manager.catalog.catalogLoading && availableVersions.length === 0 && filteredInstalls.length === 0 ? (
          <li>
            <p className="crosshook-proton-manager__empty crosshook-muted">
              {manager.selectedProviderId === null
                ? 'No Proton versions found.'
                : 'No versions found for this provider.'}
            </p>
          </li>
        ) : null}
      </ul>
    </div>
  );
}
