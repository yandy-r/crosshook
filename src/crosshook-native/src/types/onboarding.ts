export type {
  Capability,
  CapabilityState,
  HostToolCheckResult,
  HostToolDetails,
  HostToolInstallCommand,
  ReadinessCheckResult,
  SteamDeckCaveats,
  TrainerGuidanceContent,
  TrainerGuidanceEntry,
  UmuInstallGuidance,
} from './generated/onboarding';

export type OnboardingWizardStage = 'identity_game' | 'runtime' | 'trainer' | 'media' | 'review' | 'completed';
