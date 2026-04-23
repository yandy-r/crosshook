import type { GameProfile, ResolvedLaunchMethod } from '../../types';
import { GameSection } from '../profile-sections/GameSection';
import { ProfileIdentitySection } from '../profile-sections/ProfileIdentitySection';
import { RunnerMethodSection } from '../profile-sections/RunnerMethodSection';

export interface OnboardingIdentityStageBodyProps {
  profile: GameProfile;
  profileName: string;
  launchMethod: ResolvedLaunchMethod;
  profileExists: boolean;
  onProfileNameChange: (value: string) => void;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}

export function OnboardingIdentityStageBody({
  profile,
  profileName,
  launchMethod,
  profileExists,
  onProfileNameChange,
  onUpdateProfile,
}: OnboardingIdentityStageBodyProps) {
  return (
    <section aria-label="Identity & Game" className="crosshook-onboarding-wizard__step-grid">
      <ProfileIdentitySection
        profileName={profileName}
        profile={profile}
        onProfileNameChange={onProfileNameChange}
        onUpdateProfile={onUpdateProfile}
        profileExists={profileExists}
      />
      <GameSection profile={profile} onUpdateProfile={onUpdateProfile} launchMethod={launchMethod} />
      <RunnerMethodSection profile={profile} onUpdateProfile={onUpdateProfile} />
    </section>
  );
}
