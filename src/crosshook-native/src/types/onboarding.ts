import type { HealthIssue } from './health';

export interface ReadinessCheckResult {
  checks: HealthIssue[];
  all_passed: boolean;
  critical_failures: number;
  warnings: number;
}

export type OnboardingWizardStage = 'identity_game' | 'runtime' | 'trainer' | 'media' | 'review' | 'completed';

export interface TrainerGuidanceEntry {
  id: string;
  title: string;
  description: string;
  when_to_use: string;
  examples: string[];
}

export interface TrainerGuidanceContent {
  loading_modes: TrainerGuidanceEntry[];
  trainer_sources: TrainerGuidanceEntry[];
  verification_steps: string[];
}

/** Startup event payload for the `onboarding-check` Tauri event. */
export interface OnboardingCheckPayload {
  show: boolean;
  has_profiles: boolean;
}
