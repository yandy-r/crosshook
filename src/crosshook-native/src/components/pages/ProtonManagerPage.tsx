import { useMemo } from 'react';
import { usePreferencesContext } from '../../context/PreferencesContext';
import { useProfileContext } from '../../context/ProfileContext';
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
    () => steamPathProp ?? defaultSteamClientInstallPath ?? profileSteamPath ?? '',
    [steamPathProp, defaultSteamClientInstallPath, profileSteamPath]
  );

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--proton-manager">
      <div className="crosshook-route-stack" data-crosshook-focus-zone="content">
        <div className="crosshook-route-stack__body--scroll">
          <RouteBanner route="proton-manager" />
          <div className="crosshook-panel">
            <ProtonManagerPanel
              steamClientInstallPath={effectiveSteamPath.length > 0 ? effectiveSteamPath : undefined}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

export default ProtonManagerPage;
