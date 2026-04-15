import type { HealthIssue } from './health';

/** Actionable installation guidance for umu-launcher, mirroring the Rust `UmuInstallGuidance` struct.
 * Present only when running inside a Flatpak sandbox and `umu-run` cannot be resolved on the host.
 */
export interface UmuInstallGuidance {
  /** Host shell command the user can run to install umu-launcher. */
  install_command: string;
  /** URL pointing to official umu-launcher install documentation. */
  docs_url: string;
  /** Human-readable description for the guidance row. */
  description: string;
}

export interface ReadinessCheckResult {
  checks: HealthIssue[];
  all_passed: boolean;
  critical_failures: number;
  warnings: number;
  /** Actionable umu install guidance; non-null only for Flatpak + missing umu-run. */
  umu_install_guidance: UmuInstallGuidance | null;
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
