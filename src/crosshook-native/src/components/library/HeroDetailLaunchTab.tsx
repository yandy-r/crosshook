import { useMemo } from 'react';
import { useProfileContext } from '@/context/ProfileContext';
import type { LaunchPreview, LaunchRequest } from '@/types/launch';
import type { LibraryCardData } from '@/types/library';
import { resolveArtAppId } from '@/utils/art';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import { HeroLaunchGate } from './launch/HeroLaunchGate';

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
  const { profile, profileName, selectedProfile, profiles } = useProfileContext();

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

      {/* Pre/post hooks placeholder — owned by #471, do not remove */}
      <DashboardPanelSection title="Pre/post hooks" titleAs="h3" className="crosshook-hero-detail__section">
        <div className="crosshook-hero-detail__hook-placeholder">
          <p className="crosshook-hero-detail__muted">No pre/post hooks configured yet</p>
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            disabled
            aria-label="Add hook (not yet available)"
            title="Add hook (not yet available)"
          >
            Add hook
          </button>
        </div>
      </DashboardPanelSection>
    </div>
  );
}

export default HeroDetailLaunchTab;
