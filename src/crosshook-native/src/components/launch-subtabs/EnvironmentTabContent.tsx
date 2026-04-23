import * as Tabs from '@radix-ui/react-tabs';
import type { GameProfile } from '../../types/profile';
import type { AcceptSuggestionRequest, ProtonDbRecommendationGroup, ProtonDbSuggestionSet } from '../../types/protondb';
import type { PendingProtonDbOverwrite } from '../../utils/protondb';
import { CustomEnvironmentVariablesSection } from '../CustomEnvironmentVariablesSection';
import ProtonDbLookupCard from '../ProtonDbLookupCard';
import ProtonDbOverwriteConfirmation from '../ProtonDbOverwriteConfirmation';
import type { LaunchSubTabId } from './types';

interface EnvironmentTabContentProps {
  activeTab: LaunchSubTabId;
  profileName: string;
  customEnvVars?: Readonly<Record<string, string>>;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  onEnvironmentBlurAutoSave?: (
    trigger: 'key' | 'value',
    row: Readonly<{ key: string; value: string }>,
    nextEnvVars: Readonly<Record<string, string>>
  ) => void;
  showProtonDbLookup: boolean;
  steamAppId: string | undefined;
  trainerVersion?: string | null;
  onApplyProtonDbEnvVars: (group: ProtonDbRecommendationGroup) => void;
  applyingProtonDbGroupId: string | null;
  protonDbStatusMessage: string | null;
  pendingProtonDbOverwrite: PendingProtonDbOverwrite | null;
  onConfirmProtonDbOverwrite: (overwriteKeys: readonly string[]) => void;
  onCancelProtonDbOverwrite: () => void;
  onUpdateProtonDbResolution: (key: string, resolution: 'keep_current' | 'use_suggestion') => void;
  suggestionSet?: ProtonDbSuggestionSet | null;
  onAcceptSuggestion?: (request: AcceptSuggestionRequest) => Promise<void>;
  onDismissSuggestion?: (suggestionKey: string) => void;
}

export function EnvironmentTabContent({
  activeTab,
  profileName,
  customEnvVars,
  onUpdateProfile,
  onEnvironmentBlurAutoSave,
  showProtonDbLookup,
  steamAppId,
  trainerVersion,
  onApplyProtonDbEnvVars,
  applyingProtonDbGroupId,
  protonDbStatusMessage,
  pendingProtonDbOverwrite,
  onConfirmProtonDbOverwrite,
  onCancelProtonDbOverwrite,
  onUpdateProtonDbResolution,
  suggestionSet,
  onAcceptSuggestion,
  onDismissSuggestion,
}: EnvironmentTabContentProps) {
  return (
    <Tabs.Content
      value="environment"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeTab === 'environment' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner">
        <CustomEnvironmentVariablesSection
          profileName={profileName}
          customEnvVars={customEnvVars ?? {}}
          onUpdateProfile={onUpdateProfile}
          idPrefix="launch-subtabs"
          onAutoSaveBlur={onEnvironmentBlurAutoSave}
        />

        {showProtonDbLookup && steamAppId ? (
          <div className="crosshook-protondb-panel">
            <ProtonDbLookupCard
              appId={steamAppId}
              trainerVersion={trainerVersion ?? null}
              versionContext={null}
              onApplyEnvVars={onApplyProtonDbEnvVars}
              applyingGroupId={applyingProtonDbGroupId}
              suggestionSet={suggestionSet}
              onAcceptSuggestion={onAcceptSuggestion}
              onDismissSuggestion={onDismissSuggestion}
            />

            {protonDbStatusMessage ? (
              <p className="crosshook-help-text" role="status">
                {protonDbStatusMessage}
              </p>
            ) : null}

            {pendingProtonDbOverwrite ? (
              <ProtonDbOverwriteConfirmation
                pendingProtonDbOverwrite={pendingProtonDbOverwrite}
                onUpdateProtonDbResolution={onUpdateProtonDbResolution}
                onCancelProtonDbOverwrite={onCancelProtonDbOverwrite}
                onConfirmProtonDbOverwrite={onConfirmProtonDbOverwrite}
              />
            ) : null}
          </div>
        ) : null}
      </div>
    </Tabs.Content>
  );
}
