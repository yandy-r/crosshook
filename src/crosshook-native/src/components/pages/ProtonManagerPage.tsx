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
              title="Manage installed Proton builds with clearer route context"
              description="CrossHook keeps provider catalogs, compatibilitytools.d roots, install progress, and uninstall safeguards inline while this route adopts the shared dashboard shell."
              className="crosshook-proton-manager-page__hero-panel"
              bodyClassName="crosshook-proton-manager-page__hero-body"
            >
              <div className="crosshook-proton-manager-page__hero-grid">
                <div className="crosshook-proton-manager-page__hero-copy">
                  <p className="crosshook-proton-manager-page__hero-lede">
                    The effective Steam path still resolves in the existing order: route prop, saved preferences, then
                    profile context. This route rework only changes the presentation layer.
                  </p>
                  <div className="crosshook-dashboard-pill-row">
                    <span className="crosshook-dashboard-pill">
                      <strong className="crosshook-proton-manager-page__pill-label">Path source</strong>
                      {effectiveSteamPathSource}
                    </span>
                    <span className="crosshook-dashboard-pill">
                      <strong className="crosshook-proton-manager-page__pill-label">Action model</strong>
                      Inline install and uninstall flows
                    </span>
                  </div>
                </div>

                <dl className="crosshook-dashboard-kv-list crosshook-proton-manager-page__hero-kv">
                  <div className="crosshook-dashboard-kv-row">
                    <dt className="crosshook-dashboard-kv-row__label">Effective Steam path source</dt>
                    <dd className="crosshook-dashboard-kv-row__value">{effectiveSteamPathSource}</dd>
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
              </div>
            </DashboardPanelSection>

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
