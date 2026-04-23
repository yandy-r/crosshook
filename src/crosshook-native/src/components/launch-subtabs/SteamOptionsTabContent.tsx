import * as Tabs from '@radix-ui/react-tabs';
import type { LaunchOptimizationId } from '../../types/launch-optimizations';
import type { GamescopeConfig } from '../../types/profile';
import SteamLaunchOptionsPanel from '../SteamLaunchOptionsPanel';
import type { LaunchSubTabId } from './types';

interface SteamOptionsTabContentProps {
  activeTab: LaunchSubTabId;
  enabledOptionIds: readonly LaunchOptimizationId[];
  customEnvVars?: Readonly<Record<string, string>>;
  gamescopeConfig: GamescopeConfig;
}

export function SteamOptionsTabContent({
  activeTab,
  enabledOptionIds,
  customEnvVars,
  gamescopeConfig,
}: SteamOptionsTabContentProps) {
  return (
    <Tabs.Content
      value="steam-options"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeTab === 'steam-options' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner">
        <SteamLaunchOptionsPanel
          enabledOptionIds={enabledOptionIds}
          customEnvVars={customEnvVars}
          gamescopeConfig={gamescopeConfig}
        />
      </div>
    </Tabs.Content>
  );
}
