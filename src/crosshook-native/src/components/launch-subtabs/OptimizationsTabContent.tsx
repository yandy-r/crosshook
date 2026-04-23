import * as Tabs from '@radix-ui/react-tabs';
import type { BundledOptimizationPreset, LaunchMethod } from '../../types';
import type { LaunchOptimizationId } from '../../types/launch-optimizations';
import type { OptimizationCatalogPayload } from '../../utils/optimization-catalog';
import LaunchOptimizationsPanel from '../LaunchOptimizationsPanel';
import type { LaunchOptimizationsPanelStatus } from '../LaunchOptimizationsPanel';
import type { LaunchSubTabId } from './types';

interface OptimizationsTabContentProps {
  activeTab: LaunchSubTabId;
  launchMethod: LaunchMethod;
  enabledOptionIds: readonly LaunchOptimizationId[];
  onToggleOption: (optionId: LaunchOptimizationId, nextEnabled: boolean) => void;
  launchOptimizationsStatus?: LaunchOptimizationsPanelStatus;
  optimizationPresetNames?: readonly string[];
  activeOptimizationPreset?: string;
  onSelectOptimizationPreset?: (presetName: string) => void;
  bundledOptimizationPresets?: readonly BundledOptimizationPreset[];
  onApplyBundledPreset?: (presetId: string) => void;
  optimizationPresetActionBusy?: boolean;
  onSaveManualPreset?: (presetName: string) => Promise<void>;
  catalog: OptimizationCatalogPayload | null;
}

export function OptimizationsTabContent({
  activeTab,
  launchMethod,
  enabledOptionIds,
  onToggleOption,
  launchOptimizationsStatus,
  optimizationPresetNames,
  activeOptimizationPreset,
  onSelectOptimizationPreset,
  bundledOptimizationPresets,
  onApplyBundledPreset,
  optimizationPresetActionBusy,
  onSaveManualPreset,
  catalog,
}: OptimizationsTabContentProps) {
  return (
    <Tabs.Content
      value="optimizations"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeTab === 'optimizations' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner">
        <LaunchOptimizationsPanel
          method={launchMethod}
          enabledOptionIds={enabledOptionIds}
          onToggleOption={onToggleOption}
          optimizationPresetNames={optimizationPresetNames}
          activeOptimizationPreset={activeOptimizationPreset}
          onSelectOptimizationPreset={onSelectOptimizationPreset}
          bundledOptimizationPresets={bundledOptimizationPresets}
          onApplyBundledPreset={onApplyBundledPreset}
          optimizationPresetActionBusy={optimizationPresetActionBusy}
          onSaveManualPreset={onSaveManualPreset}
          catalog={catalog}
        />
      </div>
    </Tabs.Content>
  );
}
