import { type CSSProperties, useEffect, useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';

import { GamescopeConfigPanel } from './GamescopeConfigPanel';
import LauncherExport from './LauncherExport';
import { GameMetadataBar } from './profile-sections/GameMetadataBar';
import { GameSection } from './profile-sections/GameSection';
import { MediaSection } from './profile-sections/MediaSection';
import { ProfileIdentitySection } from './profile-sections/ProfileIdentitySection';
import { RunnerMethodSection } from './profile-sections/RunnerMethodSection';
import { RuntimeSection } from './profile-sections/RuntimeSection';
import { TrainerSection } from './profile-sections/TrainerSection';
import { useGameCoverArt } from '../hooks/useGameCoverArt';
import { useImageDominantColor } from '../hooks/useImageDominantColor';
import { resolveArtAppId } from '../utils/art';
import type { GameProfile, LaunchMethod } from '../types';
import type { ProtonInstallOption } from '../types/proton';

type SubTabId = 'setup' | 'runtime' | 'game_art' | 'trainer' | 'gamescope' | 'export';

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
  // Launcher export props
  steamClientInstallPath: string;
  targetHomePath: string;
  pendingReExport?: boolean;
  onReExportHandled?: () => void;
}

const TAB_LABELS: Record<SubTabId, string> = {
  setup: 'Setup',
  runtime: 'Runtime',
  game_art: 'Game Art',
  trainer: 'Trainer',
  gamescope: 'Gamescope',
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
  steamClientInstallPath,
  targetHomePath,
  pendingReExport,
  onReExportHandled,
}: ProfileSubTabsProps) {
  const [activeTab, setActiveTab] = useState<SubTabId>('setup');
  const supportsTrainerLaunch = launchMethod !== 'native';

  const steamAppId = resolveArtAppId(profile);
  const { coverArtUrl, loading: coverArtLoading } = useGameCoverArt(steamAppId || undefined, profile.game.custom_cover_art_path);
  const dominantColor = useImageDominantColor(coverArtUrl);

  const supportsLauncherExport = launchMethod === 'steam_applaunch' || launchMethod === 'proton_run';

  const tabs: SubTabId[] = [
    'setup',
    'runtime',
    'game_art',
    ...(supportsTrainerLaunch ? ['trainer' as const] : []),
    ...(supportsLauncherExport ? ['gamescope' as const] : []),
    ...(supportsLauncherExport ? ['export' as const] : []),
  ];

  useEffect(() => {
    if (tabs.length > 0 && !tabs.includes(activeTab)) {
      setActiveTab(tabs[0]);
    }
  }, [tabs.join(','), activeTab]);

  // Apply game color as CSS custom properties for the themed tab bar
  const gameColorStyle: CSSProperties | undefined = dominantColor
    ? ({
        '--crosshook-game-color-r': String(dominantColor[0]),
        '--crosshook-game-color-g': String(dominantColor[1]),
        '--crosshook-game-color-b': String(dominantColor[2]),
      } as CSSProperties)
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
        className={['crosshook-subtabs-backdrop', !showCoverArt ? 'crosshook-subtabs-backdrop--empty' : '']
          .filter(Boolean)
          .join(' ')}
        aria-hidden="true"
      >
        {coverArtUrl ? (
          <img src={coverArtUrl} className="crosshook-subtabs-backdrop__art" alt="" aria-hidden="true" />
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
          </div>
        </Tabs.Content>

        {/* Game Art tab — cover, portrait, background art + launcher icon */}
        <Tabs.Content
          value="game_art"
          forceMount
          className="crosshook-subtab-content"
          style={{ display: activeTab === 'game_art' ? undefined : 'none' }}
        >
          <div className="crosshook-subtab-content__inner">
            <MediaSection profile={profile} onUpdateProfile={onUpdateProfile} launchMethod={launchMethod} />
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

        {/* Gamescope tab — trainer gamescope config for Steam/Proton methods */}
        {supportsLauncherExport ? (
          <Tabs.Content
            value="gamescope"
            forceMount
            className="crosshook-subtab-content"
            style={{ display: activeTab === 'gamescope' ? undefined : 'none' }}
          >
            <div className="crosshook-subtab-content__inner">
              <GamescopeConfigPanel
                config={
                  profile.launch.trainer_gamescope ?? {
                    enabled: false,
                    fullscreen: false,
                    borderless: false,
                    grab_cursor: false,
                    force_grab_cursor: false,
                    hdr_enabled: false,
                    allow_nested: false,
                    extra_args: [],
                  }
                }
                onChange={(trainerGamescope) =>
                  onUpdateProfile((current) => ({
                    ...current,
                    launch: { ...current.launch, trainer_gamescope: trainerGamescope },
                  }))
                }
                isInsideGamescopeSession={false}
                enableHint="Required when the game also launches under gamescope. The trainer runs in its own compositor window so it can display alongside the game."
                lockedFullscreen
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
