/**
 * Ordered, prop-driven section list for the Hero Detail profile editor.
 *
 * Canonical section order mirrors ProfileSubTabs.tsx (lines ~184-306):
 *
 *   1. Identity
 *   2. RunnerMethod
 *   3. Runtime
 *   4. Game
 *   5. GameMetadataBar
 *   6. Media
 *   7. Trainer (hidden for native launch)
 *   8. Trainer-Gamescope (hidden for native launch; supports derivation notice)
 *   9. PrefixDeps (shown when required_protontricks non-empty)
 *  10. Runtime suggestion banner (community-recommended Proton version)
 *  11. Health section (badges, stale note, issues list)
 *  12. LauncherExport slot (Task 3.2)
 *
 * Trainer-gamescope persistence: GamescopeConfigPanel.onChange is wired to
 * onUpdateProfile so all edits flow through the 350ms draft autosave owned
 * by useHeroProfilesAutosave — no separate granular save is used here. The
 * derived notice mirrors ProfileSubTabs.tsx via the shared
 * resolveTrainerGamescopeForDisplay utility.
 */
import type { ReactNode, RefObject } from 'react';
import type { TrendDirection } from '@/hooks/useProfileHealth';
import type { CachedHealthSnapshot, EnrichedProfileHealthReport } from '@/types/health';
import type { GameProfile, LaunchMethod } from '@/types/profile';
import type { ProtonInstallOption } from '@/types/proton';
import type { ProtonUpSuggestion } from '@/types/protonup';
import type { VersionCorrelationStatus } from '@/types/version';
import { resolveTrainerGamescopeForDisplay } from '@/utils/trainerGamescope';
import { GamescopeConfigPanel } from '../../GamescopeConfigPanel';
import { DashboardPanelSection } from '../../layout/DashboardPanelSection';
import { PrefixDepsPanel } from '../../PrefixDepsPanel';
import { GameMetadataBar } from '../../profile-sections/GameMetadataBar';
import { GameSection } from '../../profile-sections/GameSection';
import { MediaSection } from '../../profile-sections/MediaSection';
import { ProfileIdentitySection } from '../../profile-sections/ProfileIdentitySection';
import { RunnerMethodSection } from '../../profile-sections/RunnerMethodSection';
import { RuntimeSection } from '../../profile-sections/RuntimeSection';
import { TrainerSection } from '../../profile-sections/TrainerSection';
import { CollapsibleSection } from '../../ui/CollapsibleSection';
import { HeroProfileEditorHealthSection, HeroProfileEditorSuggestionBanner } from './HeroProfileEditorExtras';

// ── Props ─────────────────────────────────────────────────────────────────────

export interface HeroProfileEditorSectionsProps {
  // Core props required by the always-active sections
  profile: GameProfile;
  profileName: string;
  profileExists: boolean;
  profiles: string[];
  launchMethod: LaunchMethod;
  protonInstalls: ProtonInstallOption[];
  protonInstallsError: string | null;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  onProfileNameChange: (value: string) => void;

  // GameMetadataBar
  steamAppId?: string;

  // TrainerSection
  trainerVersion?: string | null;
  onVersionSet?: () => void;

  // Health affordances (mirrors ProfilesPage health block)
  selectedReport?: EnrichedProfileHealthReport;
  selectedCachedSnapshot?: CachedHealthSnapshot;
  selectedTrend?: TrendDirection | null;
  staleInfo?: { isStale: boolean; daysAgo: number };
  trainerTypeDisplayName?: string;
  showNetworkIsolationBadge?: boolean;
  versionStatus?: VersionCorrelationStatus | null;
  healthIssuesRef?: RefObject<HTMLDivElement>;

  // Runtime suggestion banner (mirrors ProfilesPage proton suggestion)
  suggestion?: ProtonUpSuggestion | null | undefined;
  suggestionDismissed?: boolean;
  suggestionInstallError?: string | null;
  protonUpInstalling?: boolean;
  effectiveSteamClientInstallPath?: string;
  onInstallSuggestedVersion?: () => void;
  onDismissSuggestion?: () => void;

  /**
   * Slot for LauncherExport panel (Task 3.2 — HeroProfileActionsBar).
   * Pass a non-null ReactNode to activate; omit/undefined/null to hide.
   */
  launcherExportSlot?: ReactNode;
}

// ── Component ─────────────────────────────────────────────────────────────────

export function HeroProfileEditorSections({
  profile,
  profileName,
  profileExists,
  profiles,
  launchMethod,
  protonInstalls,
  protonInstallsError,
  onUpdateProfile,
  onProfileNameChange,
  steamAppId,
  trainerVersion,
  onVersionSet,
  selectedReport,
  selectedCachedSnapshot,
  selectedTrend,
  staleInfo,
  trainerTypeDisplayName,
  showNetworkIsolationBadge = false,
  versionStatus,
  healthIssuesRef,
  suggestion,
  suggestionDismissed = false,
  suggestionInstallError = null,
  protonUpInstalling = false,
  effectiveSteamClientInstallPath = '',
  onInstallSuggestedVersion,
  onDismissSuggestion,
  launcherExportSlot,
}: HeroProfileEditorSectionsProps) {
  const supportsTrainerLaunch = launchMethod !== 'native';

  // Resolve trainer-gamescope display config (derivation logic mirrors ProfileSubTabs)
  const trainerGamescopeDisplay = resolveTrainerGamescopeForDisplay(profile);

  // Trainer path for health-badge trainer-type chip visibility
  const hasTrainerPath = profile.trainer.path.trim().length > 0;

  // Prefix deps: guard sparse profiles
  const requiredProtontricks = profile.trainer?.required_protontricks ?? [];
  const prefixPath = profile.runtime?.prefix_path ?? profile.steam?.compatdata_path ?? '';

  return (
    <>
      {/* 1. Identity */}
      <ProfileIdentitySection
        profileName={profileName}
        profile={profile}
        onProfileNameChange={onProfileNameChange}
        onUpdateProfile={onUpdateProfile}
        profileExists={profileExists}
        profiles={profiles}
      />

      {/* 2. RunnerMethod */}
      <RunnerMethodSection profile={profile} onUpdateProfile={onUpdateProfile} />

      {/* 3. Runtime */}
      <RuntimeSection
        profile={profile}
        onUpdateProfile={onUpdateProfile}
        launchMethod={launchMethod}
        protonInstalls={protonInstalls}
        protonInstallsError={protonInstallsError}
      />

      {/* 4. Game */}
      <GameSection profile={profile} onUpdateProfile={onUpdateProfile} launchMethod={launchMethod} />

      {/* 5. GameMetadataBar */}
      <GameMetadataBar steamAppId={steamAppId} />

      {/* 6. Media */}
      <MediaSection profile={profile} onUpdateProfile={onUpdateProfile} launchMethod={launchMethod} />

      {/* 7. Trainer — hidden for native launch (TrainerSection self-guards) */}
      {supportsTrainerLaunch ? (
        <TrainerSection
          profile={profile}
          onUpdateProfile={onUpdateProfile}
          launchMethod={launchMethod}
          profileName={profileName}
          profileExists={profileExists}
          trainerVersion={trainerVersion}
          onVersionSet={onVersionSet}
        />
      ) : null}

      {/* 8. Trainer-Gamescope — mirrors ProfileSubTabs.tsx:257-284.
           Changes flow through onUpdateProfile into the 350ms draft autosave
           (useHeroProfilesAutosave), not a separate granular save. */}
      {supportsTrainerLaunch ? (
        <DashboardPanelSection titleAs="h3" eyebrow="Trainer" title="Gamescope">
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
        </DashboardPanelSection>
      ) : null}

      {/* 9. PrefixDeps — only shown when required_protontricks is non-empty.
           Mirrors ProfilesPage.tsx:163-171. */}
      {requiredProtontricks.length > 0 ? (
        <CollapsibleSection title="Prefix Dependencies" className="crosshook-panel">
          <PrefixDepsPanel profileName={profileName} prefixPath={prefixPath} requiredPackages={requiredProtontricks} />
        </CollapsibleSection>
      ) : null}

      {/* 10. Runtime suggestion banner — mirrors ProfilesPage.tsx:173-217 */}
      <HeroProfileEditorSuggestionBanner
        suggestion={suggestion ?? null}
        suggestionDismissed={suggestionDismissed}
        suggestionInstallError={suggestionInstallError}
        protonUpInstalling={protonUpInstalling}
        hasEffectiveSteamClientInstallPath={effectiveSteamClientInstallPath.trim().length > 0}
        onInstallSuggestedVersion={onInstallSuggestedVersion ?? (() => {})}
        onDismissSuggestion={onDismissSuggestion ?? (() => {})}
      />

      {/* 11. Health section — badges, stale note, issues list */}
      <HeroProfileEditorHealthSection
        selectedReport={selectedReport}
        selectedCachedSnapshot={selectedCachedSnapshot}
        selectedTrend={selectedTrend}
        staleInfo={staleInfo}
        trainerTypeDisplayName={trainerTypeDisplayName}
        hasTrainerPath={hasTrainerPath}
        isNonNativeLaunch={supportsTrainerLaunch}
        showNetworkIsolationBadge={showNetworkIsolationBadge}
        versionStatus={versionStatus}
        healthIssuesRef={healthIssuesRef}
      />

      {/* 12. LauncherExport slot — Task 3.2 (HeroProfileActionsBar) */}
      {launcherExportSlot ?? null}
    </>
  );
}

export default HeroProfileEditorSections;
