import { useMemo } from 'react';
import { useProfileContext } from '@/context/ProfileContext';
import type { LaunchPreview, LaunchRequest } from '@/types/launch';
import type { LibraryCardData } from '@/types/library';
import type { GameProfile, HookStage, LaunchHook } from '@/types/profile';
import { resolveArtAppId } from '@/utils/art';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import { HookListPanel } from './HookListPanel';
import { HeroLaunchGate } from './launch/HeroLaunchGate';
import { useHeroLaunchHooksAutosave } from './launch/useHeroLaunchHooksAutosave';

export interface HeroDetailLaunchTabProps {
  summary: LibraryCardData;
  launchRequest: LaunchRequest | null;
  previewLoading: boolean;
  preview: LaunchPreview | null;
  previewError: string | null;
  onPreviewLaunch?: (request: LaunchRequest) => void | Promise<void>;
  onLaunch?: (name: string) => void | Promise<void>;
  launchingName?: string;
  displayProfileName?: string;
}

export function HeroDetailLaunchTab({
  summary,
  launchRequest,
  previewLoading,
  preview,
  previewError,
  onPreviewLaunch,
  launchingName,
  displayProfileName,
}: HeroDetailLaunchTabProps) {
  const { profile, profileName, selectedProfile, profiles, updateProfile, persistProfileDraft } = useProfileContext();

  const selectedTrimmed = selectedProfile.trim();
  const profileNameTrimmed = profileName.trim();
  const resolvedProfileName = displayProfileName?.trim() || selectedTrimmed || profileNameTrimmed || summary.name;

  // `hasSavedSelectedProfile` gates the environment autosave path inside the
  // bridge hook. Mirrors the same guard used by LaunchPage.
  const hasSavedSelectedProfile =
    selectedTrimmed.length > 0 && profiles.includes(selectedTrimmed) && profileNameTrimmed === selectedTrimmed;

  const isLaunching = launchingName === resolvedProfileName;

  // The LaunchSubTabs panels write through ProfileContext's selected profile.
  // If the displayed profile (displayProfileName) differs from the context's
  // selectedProfile, writes would target the wrong profile. We surface a
  // disabled hint rather than silently hiding the sub-tabs.
  //
  // `profileNameTrimmed === selectedTrimmed` is true when ProfileContext has
  // loaded the same profile that GameDetail is displaying (singletonOwnsGame
  // path). When GameDetail falls back to useGameDetailsProfile
  // (!singletonOwnsGame), profileName may not match the displayed profile name.
  const profileMismatch = useMemo(() => {
    if (selectedTrimmed.length === 0) {
      return false;
    }
    const displayedName = displayProfileName?.trim() ?? '';
    if (displayedName.length === 0) {
      return false;
    }
    return displayedName !== selectedTrimmed;
  }, [selectedTrimmed, displayProfileName]);

  // Steam App ID from the active ProfileContext profile — used by ProtonDB
  // and cover-art inside LaunchSubTabs. When the profile mismatches,
  // LaunchSubTabs still shows but is gated, so we still resolve from profile.
  const resolvedSteamAppId = useMemo(() => resolveArtAppId(profile), [profile]);
  const { scheduleHookAutosave } = useHeroLaunchHooksAutosave({
    hasSavedSelectedProfile,
    profile,
    profileName,
    persistProfileDraft,
  });

  const preLaunchHooks = profile.pre_launch_hooks ?? [];
  const postExitHooks = profile.post_exit_hooks ?? [];

  function applyHooks(current: GameProfile, stage: HookStage, hooks: LaunchHook[]): GameProfile {
    if (stage === 'pre-launch') {
      return { ...current, pre_launch_hooks: hooks.map((hook) => ({ ...hook, stage })) };
    }
    return { ...current, post_exit_hooks: hooks.map((hook) => ({ ...hook, stage })) };
  }

  function updateHooks(stage: HookStage, hooks: LaunchHook[]) {
    const nextProfile = applyHooks(profile, stage, hooks);
    updateProfile(() => nextProfile);
    scheduleHookAutosave(nextProfile);
  }

  return (
    <div className="crosshook-hero-detail__launch-tab">
      {/* HeroLaunchGate owns the command block, dep gate, in-place launch,
          pipeline visualization, and sub-tabs host in one unit. */}
      <HeroLaunchGate
        launchRequest={launchRequest}
        previewLoading={previewLoading}
        preview={preview}
        previewError={previewError}
        onPreviewLaunch={onPreviewLaunch}
        resolvedProfileName={resolvedProfileName}
        resolvedSteamAppId={resolvedSteamAppId}
        hasSavedSelectedProfile={hasSavedSelectedProfile}
        profileMismatch={profileMismatch}
        displayProfileName={displayProfileName ?? resolvedProfileName}
        isLaunching={isLaunching}
      />

      <DashboardPanelSection title="Pre/post hooks" titleAs="h3" className="crosshook-hero-detail__section">
        <div className="crosshook-hero-detail__hooks-stack">
          <div className="crosshook-hero-detail__hook-banner">
            <p>These hooks are saved to your profile. Runtime execution is coming in a future release.</p>
            <a href="https://github.com/yandy-r/crosshook/issues/482" target="_blank" rel="noreferrer">
              Track runtime
            </a>
          </div>

          {profileMismatch ? (
            <p className="crosshook-hero-detail__muted" role="status">
              Hook settings apply to the selected profile ({selectedProfile || 'none'}). Select this game's profile to
              edit its hook declarations here.
            </p>
          ) : (
            <>
              <HookListPanel
                hooks={preLaunchHooks}
                stage="pre-launch"
                onUpdate={(hooks) => updateHooks('pre-launch', hooks)}
              />
              <HookListPanel
                hooks={postExitHooks}
                stage="post-exit"
                onUpdate={(hooks) => updateHooks('post-exit', hooks)}
              />
            </>
          )}
        </div>
      </DashboardPanelSection>
    </div>
  );
}

export default HeroDetailLaunchTab;
