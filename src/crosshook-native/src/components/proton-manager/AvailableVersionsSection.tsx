import type { ProtonUpAvailableVersion, ProtonUpProviderDescriptor } from '../../types/protonup';
import { VersionRow } from './VersionRow';

export interface AvailableVersionsSectionProps {
  versions: ProtonUpAvailableVersion[];
  providersById: ReadonlyMap<string, ProtonUpProviderDescriptor>;
  installingKeys: ReadonlySet<string>;
  selectedProviderId: string | null;
  catalogLoading: boolean;
  canInstallToSelectedRoot: boolean;
  hasInstalledVersions: boolean;
  onInstall: (version: ProtonUpAvailableVersion) => void;
}

export function AvailableVersionsSection({
  versions,
  providersById,
  installingKeys,
  selectedProviderId,
  catalogLoading,
  canInstallToSelectedRoot,
  hasInstalledVersions,
  onInstall,
}: AvailableVersionsSectionProps) {
  const emptyMessage =
    selectedProviderId === null
      ? hasInstalledVersions
        ? 'All available Proton versions are already installed.'
        : 'No Proton versions found.'
      : hasInstalledVersions
        ? 'No additional versions found for this provider.'
        : 'No versions found for this provider.';

  return (
    <section aria-label="Available Proton versions">
      <h3 className="crosshook-proton-manager__section-label">Available</h3>
      <ul className="crosshook-proton-manager__list">
        {catalogLoading ? (
          <li>
            <p className="crosshook-proton-manager__empty crosshook-muted">Loading catalog…</p>
          </li>
        ) : versions.length > 0 ? (
          versions.map((version) => {
            const providerDescriptor = providersById.get(version.provider);
            const providerSupportsInstall = providerDescriptor?.supports_install ?? false;
            const key = `${version.provider}:${version.version}`;
            return (
              <li key={key}>
                <VersionRow
                  version={version.version}
                  provider={version.provider}
                  installed={false}
                  installing={installingKeys.has(key)}
                  canInstall={canInstallToSelectedRoot && providerSupportsInstall}
                  onInstall={() => onInstall(version)}
                  onUninstall={() => undefined}
                  publishedAt={version.published_at ?? null}
                  assetSize={version.asset_size ?? null}
                />
              </li>
            );
          })
        ) : (
          <li>
            <p className="crosshook-proton-manager__empty crosshook-muted">{emptyMessage}</p>
          </li>
        )}
      </ul>
    </section>
  );
}

export default AvailableVersionsSection;
