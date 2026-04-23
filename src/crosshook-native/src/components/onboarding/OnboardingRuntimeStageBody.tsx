import type { GameProfile, ProtonInstallOption, ResolvedLaunchMethod } from '../../types';
import { HostToolDashboardHandoff } from '../host-readiness/HostToolDashboardHandoff';
import { RuntimeSection } from '../profile-sections/RuntimeSection';

export interface OnboardingRuntimeStageBodyProps {
  profile: GameProfile;
  launchMethod: ResolvedLaunchMethod;
  protonInstalls: ProtonInstallOption[];
  protonInstallsError: string | null;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  onOpenHostToolDashboard?: () => void;
}

export function OnboardingRuntimeStageBody({
  profile,
  launchMethod,
  protonInstalls,
  protonInstallsError,
  onUpdateProfile,
  onOpenHostToolDashboard,
}: OnboardingRuntimeStageBodyProps) {
  return (
    <section aria-label="Runtime" className="crosshook-onboarding-wizard__step-grid">
      <RuntimeSection
        profile={profile}
        onUpdateProfile={onUpdateProfile}
        launchMethod={launchMethod}
        protonInstalls={protonInstalls}
        protonInstallsError={protonInstallsError}
      />
      {onOpenHostToolDashboard ? (
        <HostToolDashboardHandoff
          onOpen={onOpenHostToolDashboard}
          description="Need the full host tool details while setting up runtime paths? Open the Host Tools page without finishing onboarding."
        />
      ) : null}
    </section>
  );
}
