import type { ResolvedLaunchMethod } from '../../types';
import type { OnboardingWizardStage } from '../../types/onboarding';

/** Human-readable title for each wizard stage, shown in the modal heading. */
export const STAGE_TITLES: Record<OnboardingWizardStage, string> = {
  identity_game: 'Identity & Game',
  runtime: 'Runtime',
  trainer: 'Trainer',
  media: 'Media',
  review: 'Review & Save',
  completed: 'Setup Complete',
};

/** Returns the 1-based step number shown in the eyebrow (native skips trainer, shifting later stages). */
export function getVisibleStepNumber(stage: OnboardingWizardStage, launchMethod: ResolvedLaunchMethod): number {
  const skipsTrainer = launchMethod === 'native';
  switch (stage) {
    case 'identity_game':
      return 1;
    case 'runtime':
      return 2;
    case 'trainer':
      return 3;
    case 'media':
      return skipsTrainer ? 3 : 4;
    case 'review':
      return skipsTrainer ? 4 : 5;
    case 'completed':
      return skipsTrainer ? 4 : 5;
  }
}

export function getTotalVisibleSteps(launchMethod: ResolvedLaunchMethod): number {
  return launchMethod === 'native' ? 4 : 5;
}
