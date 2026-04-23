import type { ProtonInstallOption } from '../../types/proton';
import { VersionRow } from './VersionRow';

export interface InstalledVersionEntry {
  install: ProtonInstallOption;
  rowProvider: string;
  publishedAt: string | null;
  assetSize: number | null;
}

export interface InstalledVersionsSectionProps {
  installs: InstalledVersionEntry[];
  onUninstall: (toolPath: string, versionLabel: string) => void;
}

export function InstalledVersionsSection({ installs, onUninstall }: InstalledVersionsSectionProps) {
  if (installs.length === 0) {
    return null;
  }

  return (
    <section aria-label="Installed Proton versions">
      <h2 className="crosshook-proton-manager__section-label">Installed</h2>
      <ul className="crosshook-proton-manager__list">
        {installs.map(({ install, rowProvider, publishedAt, assetSize }) => (
          <li key={install.path}>
            <VersionRow
              version={install.name}
              provider={rowProvider}
              installed={true}
              installing={false}
              canInstall={false}
              onInstall={() => undefined}
              onUninstall={() => onUninstall(install.path, install.name)}
              publishedAt={publishedAt}
              assetSize={assetSize}
            />
          </li>
        ))}
      </ul>
    </section>
  );
}

export default InstalledVersionsSection;
