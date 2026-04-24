import * as Tabs from '@radix-ui/react-tabs';
import { type CSSProperties, useEffect, useMemo, useState } from 'react';
import { useGameCoverArt } from '../hooks/useGameCoverArt';
import { useImageDominantColor } from '../hooks/useImageDominantColor';
import type { GameProfile, GamescopeConfig, LaunchMethod } from '../types';
import { DEFAULT_GAMESCOPE_CONFIG } from '../types/profile';
import type { ProtonInstallOption } from '../types/proton';
import { resolveArtAppId } from '../utils/art';
import { GamescopeConfigPanel } from './GamescopeConfigPanel';
import LauncherExport from './LauncherExport';
import { GameMetadataBar } from './profile-sections/GameMetadataBar';
import { GameSection } from './profile-sections/GameSection';
import { MediaSection } from './profile-sections/MediaSection';
import { ProfileIdentitySection } from './profile-sections/ProfileIdentitySection';
import { RunnerMethodSection } from './profile-sections/RunnerMethodSection';
import { RuntimeSection } from './profile-sections/RuntimeSection';
import { TrainerSection } from './profile-sections/TrainerSection';

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

// Must mirror LaunchRequest::resolved_trainer_gamescope / LaunchSection::resolved_trainer_gamescope in crosshook-core — update both sites together.
function resolveTrainerGamescopeForDisplay(profile: GameProfile): {
  config: GamescopeConfig;
  isGeneratedFromGame: boolean;
} {
  const trainerGamescope = profile.launch.trainer_gamescope;

  if (trainerGamescope?.enabled) {
    return {
      config: trainerGamescope,
      isGeneratedFromGame: false,
    };
  }

  const gameGamescope = profile.launch.gamescope;
  if (gameGamescope?.enabled) {
    return {
      config: {
        ...DEFAULT_GAMESCOPE_CONFIG,
        ...gameGamescope,
        enabled: true,
        fullscreen: false,
        borderless: false,
        extra_args: gameGamescope.extra_args ?? [],
      },
      isGeneratedFromGame: true,
    };
  }

  return {
    config: DEFAULT_GAMESCOPE_CONFIG,
    isGeneratedFromGame: false,
  };
}

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
  const { coverArtUrl, loading: coverArtLoading } = useGameCoverArt(
    steamAppId || undefined,
    profile.game.custom_cover_art_path
  );
  const dominantColor = useImageDominantColor(coverArtUrl);

  const supportsLauncherExport = launchMethod === 'steam_applaunch' || launchMethod === 'proton_run';
  const trainerGamescopeDisplay = useMemo(() => resolveTrainerGamescopeForDisplay(profile), [profile]);

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
  }, [activeTab, tabs[0]]);

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
        <h2 className="crosshook-visually-hidden">Profile sections</h2>
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
          <div className="crosshook-subtab-content__inner crosshook-dashboard-route-section-stack">
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
          <div className="crosshook-subtab-content__inner crosshook-dashboard-route-section-stack">
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
            <div className="crosshook-subtab-content__inner crosshook-dashboard-route-section-stack">
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
                config={trainerGamescopeDisplay.config}
                onChange={(trainerGamescope) =>
                  onUpdateProfile((current) => ({
                    ...current,
                    launch: { ...current.launch, trainer_gamescope: trainerGamescope },
                  }))
                }
                isInsideGamescopeSession={false}
                enableHint="Required when the game also launches under gamescope. The trainer runs in its own compositor window so it can display alongside the game."
                derivedConfigNotice={
                  trainerGamescopeDisplay.isGeneratedFromGame
                    ? 'Trainer gamescope is auto-generated from the game config. Edit any value here and save the profile to create a trainer-specific override.'
                    : undefined
                }
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
