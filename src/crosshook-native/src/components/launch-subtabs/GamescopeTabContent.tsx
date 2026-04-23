import * as Tabs from '@radix-ui/react-tabs';
import type { GamescopeConfig } from '../../types/profile';
import GamescopeConfigPanel from '../GamescopeConfigPanel';
import type { LaunchSubTabId } from './types';

interface GamescopeTabContentProps {
  activeTab: LaunchSubTabId;
  gamescopeConfig: GamescopeConfig;
  onGamescopeChange: (config: GamescopeConfig) => void;
  isInsideGamescopeSession: boolean;
}

export function GamescopeTabContent({
  activeTab,
  gamescopeConfig,
  onGamescopeChange,
  isInsideGamescopeSession,
}: GamescopeTabContentProps) {
  return (
    <Tabs.Content
      value="gamescope"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeTab === 'gamescope' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner">
        <GamescopeConfigPanel
          config={gamescopeConfig}
          onChange={onGamescopeChange}
          isInsideGamescopeSession={isInsideGamescopeSession}
        />
      </div>
    </Tabs.Content>
  );
}
