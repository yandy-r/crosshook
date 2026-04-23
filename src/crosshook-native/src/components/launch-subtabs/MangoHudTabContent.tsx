import * as Tabs from '@radix-ui/react-tabs';
import type { ReactNode } from 'react';
import type { LaunchMethod } from '../../types';
import type { MangoHudConfig } from '../../types/profile';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import MangoHudConfigPanel from '../MangoHudConfigPanel';
import type { LaunchSubTabId } from './types';

interface MangoHudTabContentProps {
  activeTab: LaunchSubTabId;
  mangoHudConfig: MangoHudConfig;
  onMangoHudChange: (config: MangoHudConfig) => void;
  showMangoHudOverlayEnabled: boolean;
  launchMethod: LaunchMethod;
  /** Autosave chip — rendered in panel header actions when this tab is active. */
  chipSlot?: ReactNode;
}

export function MangoHudTabContent({
  activeTab,
  mangoHudConfig,
  onMangoHudChange,
  showMangoHudOverlayEnabled,
  launchMethod,
  chipSlot,
}: MangoHudTabContentProps) {
  return (
    <Tabs.Content
      value="mangohud"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeTab === 'mangohud' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner">
        <DashboardPanelSection eyebrow="MangoHud" title="MangoHud Configuration" titleAs="h3" actions={chipSlot}>
          <MangoHudConfigPanel
            config={mangoHudConfig}
            onChange={onMangoHudChange}
            showMangoHudOverlayEnabled={showMangoHudOverlayEnabled}
            launchMethod={launchMethod}
          />
        </DashboardPanelSection>
      </div>
    </Tabs.Content>
  );
}
