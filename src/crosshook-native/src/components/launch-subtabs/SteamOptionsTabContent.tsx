import * as Tabs from '@radix-ui/react-tabs';
import type { ReactNode } from 'react';
import type { LaunchCommandArguments } from '../../types/launch-command-arguments';
import { DEFAULT_LAUNCH_COMMAND_ARGUMENTS } from '../../types/launch-command-arguments';
import type { LaunchOptimizationId } from '../../types/launch-optimizations';
import type { GamescopeConfig } from '../../types/profile';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import SteamLaunchOptionsPanel from '../SteamLaunchOptionsPanel';
import type { LaunchSubTabId } from './types';

interface SteamOptionsTabContentProps {
  activeTab: LaunchSubTabId;
  enabledOptionIds: readonly LaunchOptimizationId[];
  customEnvVars?: Readonly<Record<string, string>>;
  commandArguments?: LaunchCommandArguments;
  gamescopeConfig: GamescopeConfig;
  /** Autosave chip — rendered in panel header actions when this tab is active. */
  chipSlot?: ReactNode;
}

export function SteamOptionsTabContent({
  activeTab,
  enabledOptionIds,
  customEnvVars,
  commandArguments = DEFAULT_LAUNCH_COMMAND_ARGUMENTS,
  gamescopeConfig,
  chipSlot,
}: SteamOptionsTabContentProps) {
  return (
    <Tabs.Content
      value="steam-options"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeTab === 'steam-options' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner">
        <DashboardPanelSection eyebrow="Steam Options" title="Steam Launch Options" titleAs="h3" actions={chipSlot}>
          <SteamLaunchOptionsPanel
            enabledOptionIds={enabledOptionIds}
            customEnvVars={customEnvVars}
            commandArguments={commandArguments}
            gamescopeConfig={gamescopeConfig}
          />
        </DashboardPanelSection>
      </div>
    </Tabs.Content>
  );
}
