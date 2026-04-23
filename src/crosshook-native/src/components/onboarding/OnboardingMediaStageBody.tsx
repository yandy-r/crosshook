import type { GameProfile, ResolvedLaunchMethod } from '../../types';
import { MediaSection } from '../profile-sections/MediaSection';

export interface OnboardingMediaStageBodyProps {
  profile: GameProfile;
  launchMethod: ResolvedLaunchMethod;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}

export function OnboardingMediaStageBody({ profile, launchMethod, onUpdateProfile }: OnboardingMediaStageBodyProps) {
  return (
    <section aria-label="Media" className="crosshook-onboarding-wizard__step-grid">
      <MediaSection profile={profile} onUpdateProfile={onUpdateProfile} launchMethod={launchMethod} />
    </section>
  );
}
