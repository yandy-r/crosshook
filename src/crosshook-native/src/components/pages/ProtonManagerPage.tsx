import { useMemo } from 'react';
import { usePreferencesContext } from '../../context/PreferencesContext';
import { useProfileContext } from '../../context/ProfileContext';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import { RouteBanner } from '../layout/RouteBanner';
import { ProtonManagerPanel } from '../proton-manager/ProtonManagerPanel';

import '../../styles/proton-manager.css';

interface ProtonManagerPageProps {
  steamClientInstallPath?: string;
}

export function ProtonManagerPage({ steamClientInstallPath: steamPathProp }: ProtonManagerPageProps) {
  const { defaultSteamClientInstallPath } = usePreferencesContext();
  const { steamClientInstallPath: profileSteamPath } = useProfileContext();

  const effectiveSteamPath = useMemo(
    () => steamPathProp || defaultSteamClientInstallPath || profileSteamPath || '',
    [steamPathProp, defaultSteamClientInstallPath, profileSteamPath]
  );
  const effectiveSteamPathSource = useMemo(() => {
    if (steamPathProp) {
      return 'Route override';
    }
    if (defaultSteamClientInstallPath) {
      return 'Preferences';
    }
    if (profileSteamPath) {
      return 'Profile context';
    }
    return 'Runtime discovery';
  }, [steamPathProp, defaultSteamClientInstallPath, profileSteamPath]);

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--proton-manager">
      <div className="crosshook-route-stack" data-crosshook-focus-zone="content">
        <div className="crosshook-route-stack__body--scroll">
          <RouteBanner route="proton-manager" />
          <div className="crosshook-dashboard-route-body crosshook-proton-manager-page">
            <DashboardPanelSection
              eyebrow="Steam compatibility tools"
              title="Manage installed Proton builds"
              description="Catalogs, install roots, progress, and uninstall safeguards stay inline."
              className="crosshook-proton-manager-page__hero-panel"
              actions={
                <dl className="crosshook-dashboard-kv-list crosshook-proton-manager-page__hero-kv">
                  <div className="crosshook-dashboard-kv-row crosshook-proton-manager-page__hero-kv-row--source">
                    <dt className="crosshook-dashboard-kv-row__label">Effective Steam path source</dt>
                    <dd className="crosshook-dashboard-kv-row__value">
                      <span className="crosshook-dashboard-pill crosshook-proton-manager-page__hero-kv-pill">
                        {effectiveSteamPathSource}
                      </span>
                    </dd>
                  </div>
                  <div className="crosshook-dashboard-kv-row">
                    <dt className="crosshook-dashboard-kv-row__label">Resolved Steam path</dt>
                    <dd className="crosshook-dashboard-kv-row__value crosshook-proton-manager-page__path-value">
                      {effectiveSteamPath.length > 0
                        ? effectiveSteamPath
                        : 'No explicit path provided; runtime detection applies.'}
                    </dd>
                  </div>
                </dl>
              }
            />

            <DashboardPanelSection
              eyebrow="Catalog and installs"
              title="Installed versions and available releases"
              description="Provider switching, root selection, catalog refresh state, active operations, and uninstall warnings remain visible alongside the installed and available version sections."
              className="crosshook-proton-manager-page__manager-panel"
              bodyClassName="crosshook-proton-manager-page__manager-body"
            >
              <ProtonManagerPanel
                steamClientInstallPath={effectiveSteamPath.length > 0 ? effectiveSteamPath : undefined}
              />
            </DashboardPanelSection>
          </div>
        </div>
      </div>
    </div>
  );
}

export default ProtonManagerPage;
