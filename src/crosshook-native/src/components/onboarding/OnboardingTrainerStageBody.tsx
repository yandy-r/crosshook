import type { GameProfile, ResolvedLaunchMethod } from '../../types';
import { TrainerSection } from '../profile-sections/TrainerSection';

export interface OnboardingTrainerStageBodyProps {
  profile: GameProfile;
  profileName: string;
  launchMethod: ResolvedLaunchMethod;
  profileExists: boolean;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}

export function OnboardingTrainerStageBody({
  profile,
  profileName,
  launchMethod,
  profileExists,
  onUpdateProfile,
}: OnboardingTrainerStageBodyProps) {
  return (
    <section aria-label="Trainer" className="crosshook-onboarding-wizard__step-grid">
      <TrainerSection
        profile={profile}
        onUpdateProfile={onUpdateProfile}
        launchMethod={launchMethod}
        profileName={profileName}
        profileExists={profileExists}
      />
    </section>
  );
}
