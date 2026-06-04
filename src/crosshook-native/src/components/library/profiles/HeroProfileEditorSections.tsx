import type { ReactNode } from 'react';
import type { GameProfile, LaunchMethod } from '@/types/profile';
import type { ProtonInstallOption } from '@/types/proton';
import { GameSection } from '../../profile-sections/GameSection';
import { MediaSection } from '../../profile-sections/MediaSection';
import { ProfileIdentitySection } from '../../profile-sections/ProfileIdentitySection';
import { RuntimeSection } from '../../profile-sections/RuntimeSection';

/**
 * Canonical section order for the Hero Detail profile editor, mirroring
 * ProfileSubTabs.tsx (lines ~184-306):
 *
 *   1. Identity          — active
 *   2. RunnerMethod      — slot prop (not yet wired; Task 2.2)
 *   3. Runtime           — active
 *   4. Game              — active
 *   5. GameMetadataBar   — slot prop (not yet wired; Task 2.2)
 *   6. Media             — active
 *   7. Trainer           — slot prop (not yet wired; Task 2.2)
 *   8. Trainer-Gamescope — slot prop (not yet wired; Task 2.2)
 *   9. LauncherExport    — slot prop (not yet wired; Task 2.2)
 */

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

  // Optional slot props for sections not yet shipped in Hero Detail.
  // Pass a non-null ReactNode to activate the slot; omit or pass undefined/null to hide it.
  /** Slot for RunnerMethodSection (Task 2.2). */
  runnerMethodSlot?: ReactNode;
  /** Slot for GameMetadataBar (Task 2.2). */
  gameMetadataBarSlot?: ReactNode;
  /** Slot for TrainerSection (Task 2.2). */
  trainerSlot?: ReactNode;
  /** Slot for trainer GamescopeConfigPanel (Task 2.2). */
  trainerGamescopeSlot?: ReactNode;
  /** Slot for LauncherExport (Task 2.2). */
  launcherExportSlot?: ReactNode;
}

/**
 * Ordered, prop-driven section list for the Hero Detail profile editor.
 * Only the 4 sections Hero Detail currently renders are active:
 * Identity, Runtime, Game, Media.
 *
 * Non-shipped sections are behind optional slot props so Task 2.2 can
 * wire them incrementally without changing this component's interface.
 */
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
  runnerMethodSlot,
  gameMetadataBarSlot,
  trainerSlot,
  trainerGamescopeSlot,
  launcherExportSlot,
}: HeroProfileEditorSectionsProps) {
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

      {/* 2. RunnerMethod — not yet shipped in Hero Detail */}
      {runnerMethodSlot ?? null}

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

      {/* 5. GameMetadataBar — not yet shipped in Hero Detail */}
      {gameMetadataBarSlot ?? null}

      {/* 6. Media */}
      <MediaSection profile={profile} onUpdateProfile={onUpdateProfile} launchMethod={launchMethod} />

      {/* 7. Trainer — not yet shipped in Hero Detail */}
      {trainerSlot ?? null}

      {/* 8. Trainer-Gamescope — not yet shipped in Hero Detail */}
      {trainerGamescopeSlot ?? null}

      {/* 9. LauncherExport — not yet shipped in Hero Detail */}
      {launcherExportSlot ?? null}
    </>
  );
}

export default HeroProfileEditorSections;
