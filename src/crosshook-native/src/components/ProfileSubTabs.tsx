import { type CSSProperties, useEffect, useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';

import { CustomEnvironmentVariablesSection } from './CustomEnvironmentVariablesSection';
import LauncherExport from './LauncherExport';
import ProtonDbLookupCard from './ProtonDbLookupCard';
import { GameMetadataBar } from './profile-sections/GameMetadataBar';
import { GameSection } from './profile-sections/GameSection';
import { MediaSection } from './profile-sections/MediaSection';
import { ProfileIdentitySection } from './profile-sections/ProfileIdentitySection';
import { RunnerMethodSection } from './profile-sections/RunnerMethodSection';
import { RuntimeSection } from './profile-sections/RuntimeSection';
import { TrainerSection } from './profile-sections/TrainerSection';
import { useGameCoverArt } from '../hooks/useGameCoverArt';
import { useImageDominantColor } from '../hooks/useImageDominantColor';
import type { PendingProtonDbOverwrite } from './ProfileFormSections';
import type { ProtonDbRecommendationGroup } from '../types/protondb';
import type { GameProfile, LaunchMethod } from '../types';
import type { ProtonInstallOption } from '../types/proton';

type SubTabId = 'setup' | 'runtime' | 'environment' | 'trainer' | 'export';

export interface ProfileSubTabsProps {
  profile: GameProfile;
  profileName: string;
  profileExists: boolean;
  profiles?: string[];
  launchMethod: LaunchMethod;
  protonInstalls: ProtonInstallOption[];
  protonInstallsError: string | null;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  onProfileNameChange: (value: string) => void;
  // Trainer props
  trainerVersion?: string | null;
  onVersionSet?: () => void;
  // ProtonDB props
  showProtonDbLookup: boolean;
  onApplyProtonDbEnvVars: (group: ProtonDbRecommendationGroup) => void;
  applyingProtonDbGroupId: string | null;
  protonDbStatusMessage: string | null;
  pendingProtonDbOverwrite: PendingProtonDbOverwrite | null;
  onConfirmProtonDbOverwrite: (overwriteKeys: readonly string[]) => void;
  onCancelProtonDbOverwrite: () => void;
  onUpdateProtonDbResolution: (key: string, resolution: 'keep_current' | 'use_suggestion') => void;
  // Launcher export props
  steamClientInstallPath: string;
  targetHomePath: string;
  pendingReExport?: boolean;
  onReExportHandled?: () => void;
}

const TAB_LABELS: Record<SubTabId, string> = {
  setup: 'Setup',
  runtime: 'Runtime',
  environment: 'Environment',
  trainer: 'Trainer',
  export: 'Export',
};

export function ProfileSubTabs({
  profile,
  profileName,
  profileExists,
  profiles,
  launchMethod,
  protonInstalls,
  protonInstallsError,
  onUpdateProfile,
  onProfileNameChange,
  trainerVersion,
  onVersionSet,
  showProtonDbLookup,
  onApplyProtonDbEnvVars,
  applyingProtonDbGroupId,
  protonDbStatusMessage,
  pendingProtonDbOverwrite,
  onConfirmProtonDbOverwrite,
  onCancelProtonDbOverwrite,
  onUpdateProtonDbResolution,
  steamClientInstallPath,
  targetHomePath,
  pendingReExport,
  onReExportHandled,
}: ProfileSubTabsProps) {
  const [activeTab, setActiveTab] = useState<SubTabId>('setup');
  const supportsTrainerLaunch = launchMethod !== 'native';

  const steamAppId = profile.steam.app_id;
  const { coverArtUrl, loading: coverArtLoading } = useGameCoverArt(
    steamAppId,
    profile.game.custom_cover_art_path,
  );
  const dominantColor = useImageDominantColor(coverArtUrl);

  const supportsLauncherExport = launchMethod === 'steam_applaunch' || launchMethod === 'proton_run';

  const tabs: SubTabId[] = [
    'setup',
    'runtime',
    'environment',
    ...(supportsTrainerLaunch ? ['trainer' as const] : []),
    ...(supportsLauncherExport ? ['export' as const] : []),
  ];

  useEffect(() => {
    if (tabs.length > 0 && !tabs.includes(activeTab)) {
      setActiveTab(tabs[0]);
    }
  }, [tabs.join(','), activeTab]);

  // Apply game color as CSS custom properties for the themed tab bar
  const gameColorStyle: CSSProperties | undefined = dominantColor
    ? {
        '--crosshook-game-color-r': String(dominantColor[0]),
        '--crosshook-game-color-g': String(dominantColor[1]),
        '--crosshook-game-color-b': String(dominantColor[2]),
      } as CSSProperties
    : undefined;

  const showCoverArt = Boolean(coverArtUrl) || coverArtLoading;

  return (
    <Tabs.Root
      className="crosshook-subtabs-root"
      value={activeTab}
      onValueChange={(val) => setActiveTab(val as SubTabId)}
      style={gameColorStyle}
    >
      <div
        className={[
          'crosshook-subtabs-backdrop',
          !showCoverArt ? 'crosshook-subtabs-backdrop--empty' : '',
        ]
          .filter(Boolean)
          .join(' ')}
        aria-hidden="true"
      >
        {coverArtUrl ? (
          <img
            src={coverArtUrl}
            className="crosshook-subtabs-backdrop__art"
            alt=""
            aria-hidden="true"
          />
        ) : null}
        {coverArtLoading && !coverArtUrl ? (
          <div className="crosshook-subtabs-backdrop__skeleton crosshook-skeleton" />
        ) : null}
        <div className="crosshook-subtabs-backdrop__veil" />
      </div>

      <div className="crosshook-subtabs-foreground">
        <Tabs.List
          className={`crosshook-subtab-row${dominantColor ? ' crosshook-subtab-row--themed' : ''}`}
          aria-label="Profile sections"
        >
          {tabs.map((tab) => (
            <Tabs.Trigger
              key={tab}
              value={tab}
              className={`crosshook-subtab${activeTab === tab ? ' crosshook-subtab--active' : ''}`}
            >
              {TAB_LABELS[tab]}
            </Tabs.Trigger>
          ))}
        </Tabs.List>

        <div className="crosshook-subtabs-metadata">
          <GameMetadataBar steamAppId={steamAppId} />
        </div>

        {/* Setup tab — profile identity, game path, runner method */}
        <Tabs.Content
          value="setup"
          forceMount
          className="crosshook-subtab-content"
          style={{ display: activeTab === 'setup' ? undefined : 'none' }}
        >
          <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
            <ProfileIdentitySection
              profileName={profileName}
              profile={profile}
              onProfileNameChange={onProfileNameChange}
              onUpdateProfile={onUpdateProfile}
              profileExists={profileExists}
              profiles={profiles}
            />
            <GameSection profile={profile} onUpdateProfile={onUpdateProfile} launchMethod={launchMethod} />
            <RunnerMethodSection profile={profile} onUpdateProfile={onUpdateProfile} />
          </div>
        </Tabs.Content>

        {/* Runtime tab — runner-conditional fields */}
        <Tabs.Content
          value="runtime"
          forceMount
          className="crosshook-subtab-content"
          style={{ display: activeTab === 'runtime' ? undefined : 'none' }}
        >
          <div className="crosshook-subtab-content__inner">
            <RuntimeSection
              profile={profile}
              onUpdateProfile={onUpdateProfile}
              launchMethod={launchMethod}
              protonInstalls={protonInstalls}
              protonInstallsError={protonInstallsError}
            />
            <MediaSection
              profile={profile}
              onUpdateProfile={onUpdateProfile}
              launchMethod={launchMethod}
            />
          </div>
        </Tabs.Content>

        {/* Environment tab — env vars + ProtonDB lookup */}
        <Tabs.Content
          value="environment"
          forceMount
          className="crosshook-subtab-content"
          style={{ display: activeTab === 'environment' ? undefined : 'none' }}
        >
          <div className="crosshook-subtab-content__inner">
            <CustomEnvironmentVariablesSection
              profileName={profileName}
              customEnvVars={profile.launch.custom_env_vars}
              onUpdateProfile={onUpdateProfile}
              idPrefix="profile-subtabs"
            />

            {showProtonDbLookup ? (
              <div className="crosshook-protondb-panel">
                <ProtonDbLookupCard
                  appId={profile.steam.app_id}
                  trainerVersion={trainerVersion ?? null}
                  versionContext={null}
                  onApplyEnvVars={onApplyProtonDbEnvVars}
                  applyingGroupId={applyingProtonDbGroupId}
                />

                {protonDbStatusMessage ? (
                  <p className="crosshook-help-text" role="status">
                    {protonDbStatusMessage}
                  </p>
                ) : null}

                {pendingProtonDbOverwrite ? (
                  <div
                    className="crosshook-protondb-card__recommendation-group"
                    role="group"
                    aria-label="ProtonDB overwrite confirmation"
                  >
                    <div className="crosshook-protondb-card__meta">
                      <h3 className="crosshook-protondb-card__recommendation-group-title">
                        Confirm conflicting environment-variable updates
                      </h3>
                      <p className="crosshook-protondb-card__recommendation-group-copy">
                        Choose per key whether CrossHook should keep the current profile value or use the ProtonDB
                        suggestion.
                      </p>
                    </div>

                    <div className="crosshook-protondb-card__recommendation-list">
                      {pendingProtonDbOverwrite.conflicts.map((conflict) => {
                        const resolution = pendingProtonDbOverwrite.resolutions[conflict.key] ?? 'keep_current';
                        return (
                          <div key={conflict.key} className="crosshook-protondb-card__recommendation-item">
                            <p className="crosshook-protondb-card__recommendation-label">
                              <code>{conflict.key}</code>
                            </p>
                            <p className="crosshook-protondb-card__recommendation-note">
                              Current: <code>{conflict.currentValue}</code>
                            </p>
                            <p className="crosshook-protondb-card__recommendation-note">
                              Suggested: <code>{conflict.suggestedValue}</code>
                            </p>
                            <div className="crosshook-protondb-card__actions">
                              <button
                                type="button"
                                className="crosshook-button crosshook-button--secondary"
                                onClick={() => onUpdateProtonDbResolution(conflict.key, 'keep_current')}
                              >
                                {resolution === 'keep_current' ? 'Keeping current value' : 'Keep current'}
                              </button>
                              <button
                                type="button"
                                className="crosshook-button"
                                onClick={() => onUpdateProtonDbResolution(conflict.key, 'use_suggestion')}
                              >
                                {resolution === 'use_suggestion' ? 'Using suggestion' : 'Use suggestion'}
                              </button>
                            </div>
                          </div>
                        );
                      })}
                    </div>

                    <div className="crosshook-protondb-card__actions">
                      <button
                        type="button"
                        className="crosshook-button crosshook-button--secondary"
                        onClick={onCancelProtonDbOverwrite}
                      >
                        Cancel
                      </button>
                      <button
                        type="button"
                        className="crosshook-button"
                        onClick={() =>
                          onConfirmProtonDbOverwrite(
                            Object.entries(pendingProtonDbOverwrite.resolutions)
                              .filter(([, resolution]) => resolution === 'use_suggestion')
                              .map(([key]) => key)
                          )
                        }
                      >
                        Apply selected changes
                      </button>
                    </div>
                  </div>
                ) : null}
              </div>
            ) : null}
          </div>
        </Tabs.Content>

        {/* Trainer tab — hidden when native launch method */}
        {supportsTrainerLaunch ? (
          <Tabs.Content
            value="trainer"
            forceMount
            className="crosshook-subtab-content"
            style={{ display: activeTab === 'trainer' ? undefined : 'none' }}
          >
            <div className="crosshook-subtab-content__inner">
              <TrainerSection
                profile={profile}
                onUpdateProfile={onUpdateProfile}
                launchMethod={launchMethod}
                profileName={profileName}
                profileExists={profileExists}
                trainerVersion={trainerVersion}
                onVersionSet={onVersionSet}
              />
            </div>
          </Tabs.Content>
        ) : null}

        {/* Export tab — launcher export for Steam/Proton methods */}
        {supportsLauncherExport ? (
          <Tabs.Content
            value="export"
            forceMount
            className="crosshook-subtab-content"
            style={{ display: activeTab === 'export' ? undefined : 'none' }}
          >
            <div className="crosshook-subtab-content__inner">
              <LauncherExport
                profile={profile}
                profileName={profileName}
                method={launchMethod as Exclude<LaunchMethod, '' | 'native'>}
                steamClientInstallPath={steamClientInstallPath}
                targetHomePath={targetHomePath}
                pendingReExport={pendingReExport}
                onReExportHandled={onReExportHandled}
              />
            </div>
          </Tabs.Content>
        ) : null}
      </div>
    </Tabs.Root>
  );
}

export default ProfileSubTabs;
