import * as Tabs from '@radix-ui/react-tabs';
import type { ReactNode } from 'react';
import type { GamescopeConfig } from '../../types/profile';
import GamescopeConfigPanel from '../GamescopeConfigPanel';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import type { LaunchSubTabId } from './types';

interface GamescopeTabContentProps {
  activeTab: LaunchSubTabId;
  gamescopeConfig: GamescopeConfig;
  onGamescopeChange: (config: GamescopeConfig) => void;
  isInsideGamescopeSession: boolean;
  /** Autosave chip — rendered in panel header actions when this tab is active. */
  chipSlot?: ReactNode;
}

export function GamescopeTabContent({
  activeTab,
  gamescopeConfig,
  onGamescopeChange,
  isInsideGamescopeSession,
  chipSlot,
}: GamescopeTabContentProps) {
  return (
    <Tabs.Content
      value="gamescope"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeTab === 'gamescope' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner">
        <DashboardPanelSection eyebrow="Gamescope" title="Gamescope Configuration" titleAs="h3" actions={chipSlot}>
          <GamescopeConfigPanel
            config={gamescopeConfig}
            onChange={onGamescopeChange}
            isInsideGamescopeSession={isInsideGamescopeSession}
          />
        </DashboardPanelSection>
      </div>
    </Tabs.Content>
  );
}
