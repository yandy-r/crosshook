import * as Tabs from '@radix-ui/react-tabs';
import type { LaunchMethod } from '../../types';
import type { MangoHudConfig } from '../../types/profile';
import MangoHudConfigPanel from '../MangoHudConfigPanel';
import type { LaunchSubTabId } from './types';

interface MangoHudTabContentProps {
  activeTab: LaunchSubTabId;
  mangoHudConfig: MangoHudConfig;
  onMangoHudChange: (config: MangoHudConfig) => void;
  showMangoHudOverlayEnabled: boolean;
  launchMethod: LaunchMethod;
}

export function MangoHudTabContent({
  activeTab,
  mangoHudConfig,
  onMangoHudChange,
  showMangoHudOverlayEnabled,
  launchMethod,
}: MangoHudTabContentProps) {
  return (
    <Tabs.Content
      value="mangohud"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeTab === 'mangohud' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner">
        <MangoHudConfigPanel
          config={mangoHudConfig}
          onChange={onMangoHudChange}
          showMangoHudOverlayEnabled={showMangoHudOverlayEnabled}
          launchMethod={launchMethod}
        />
      </div>
    </Tabs.Content>
  );
}
