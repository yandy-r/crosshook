import * as Tabs from '@radix-ui/react-tabs';
import type { ReactNode } from 'react';
import type { LaunchMethod } from '../../types';
import type { CommandArgumentCatalogPayload, LaunchCommandArguments } from '../../types/launch-command-arguments';
import { DEFAULT_LAUNCH_COMMAND_ARGUMENTS } from '../../types/launch-command-arguments';
import CommandArgumentsPanel from '../CommandArgumentsPanel';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import type { LaunchSubTabId } from './types';

interface CommandArgumentsTabContentProps {
  activeTab: LaunchSubTabId;
  launchMethod: LaunchMethod;
  commandArguments?: LaunchCommandArguments;
  onToggleCommandArgument?: (argumentId: string, nextEnabled: boolean) => void;
  onUpdateCommandArgumentsCustomArgs?: (customArgs: readonly string[]) => void;
  commandArgumentCatalog?: CommandArgumentCatalogPayload | null;
  /** Autosave chip — rendered in panel header actions when this tab is active. */
  chipSlot?: ReactNode;
}

export function CommandArgumentsTabContent({
  activeTab,
  launchMethod,
  commandArguments = DEFAULT_LAUNCH_COMMAND_ARGUMENTS,
  onToggleCommandArgument,
  onUpdateCommandArgumentsCustomArgs,
  commandArgumentCatalog = null,
  chipSlot,
}: CommandArgumentsTabContentProps) {
  const showCommandArguments =
    onToggleCommandArgument !== undefined && onUpdateCommandArgumentsCustomArgs !== undefined;

  if (!showCommandArguments) {
    return null;
  }

  return (
    <Tabs.Content
      value="command-arguments"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeTab === 'command-arguments' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner">
        <DashboardPanelSection
          eyebrow="Command Arguments"
          title="Launch Command Arguments"
          titleAs="h3"
          actions={chipSlot}
        >
          <CommandArgumentsPanel
            method={launchMethod}
            commandArguments={commandArguments}
            catalog={commandArgumentCatalog}
            onToggleArgument={onToggleCommandArgument}
            onUpdateCustomArgs={onUpdateCommandArgumentsCustomArgs}
          />
        </DashboardPanelSection>
      </div>
    </Tabs.Content>
  );
}
