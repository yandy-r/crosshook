import type { BundledOptimizationPreset, GameProfile } from '../../types';
import type { ReadinessCheckResult, SteamDeckCaveats, UmuInstallGuidance } from '../../types/onboarding';
import { CustomEnvironmentVariablesSection } from '../CustomEnvironmentVariablesSection';
import { HostToolDashboardHandoff } from '../host-readiness/HostToolDashboardHandoff';
import { WizardPresetPicker } from '../wizard/WizardPresetPicker';
import { WizardReviewSummary } from '../wizard/WizardReviewSummary';
import type { WizardValidationResult } from '../wizard/wizardValidation';

export interface OnboardingReviewStageBodyProps {
  profile: GameProfile;
  profileName: string;
  mode: 'create' | 'edit';
  validation: WizardValidationResult;
  bundledOptimizationPresets: BundledOptimizationPreset[];
  optimizationPresetActionBusy: boolean;
  readinessResult: ReadinessCheckResult | null;
  checkError: string | null;
  umuInstallGuidance: UmuInstallGuidance | null;
  steamDeckCaveats: SteamDeckCaveats | null;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  onApplyBundledPreset: (presetId: string) => Promise<void>;
  onSelectSavedPreset: (presetName: string) => Promise<void>;
  onDismissUmuInstallNag: () => void;
  onDismissSteamDeckCaveats: () => void;
  onDismissReadinessNag: (toolId: string) => void;
  onOpenHostToolDashboard?: () => void;
}

export function OnboardingReviewStageBody({
  profile,
  profileName,
  mode,
  validation,
  bundledOptimizationPresets,
  optimizationPresetActionBusy,
  readinessResult,
  checkError,
  umuInstallGuidance,
  steamDeckCaveats,
  onUpdateProfile,
  onApplyBundledPreset,
  onSelectSavedPreset,
  onDismissUmuInstallNag,
  onDismissSteamDeckCaveats,
  onDismissReadinessNag,
  onOpenHostToolDashboard,
}: OnboardingReviewStageBodyProps) {
  return (
    <section aria-label="Review & Save" className="crosshook-onboarding-wizard__step-grid">
      <WizardPresetPicker
        bundledPresets={bundledOptimizationPresets}
        savedPresetNames={Object.keys(profile.launch.presets ?? {})}
        activePresetKey={profile.launch.active_preset ?? ''}
        busy={mode === 'edit' ? optimizationPresetActionBusy : false}
        onApplyBundled={onApplyBundledPreset}
        onSelectSaved={onSelectSavedPreset}
      />
      <CustomEnvironmentVariablesSection
        profileName={profileName}
        customEnvVars={profile.launch.custom_env_vars}
        onUpdateProfile={onUpdateProfile}
        idPrefix="onboarding-wizard"
      />
      <WizardReviewSummary
        validation={validation}
        readinessResult={readinessResult}
        checkError={checkError}
        umuInstallGuidance={umuInstallGuidance}
        onDismissUmuInstallNag={onDismissUmuInstallNag}
        steamDeckCaveats={steamDeckCaveats}
        onDismissSteamDeckCaveats={onDismissSteamDeckCaveats}
        onDismissReadinessNag={onDismissReadinessNag}
      />
      {onOpenHostToolDashboard ? (
        <HostToolDashboardHandoff
          onOpen={onOpenHostToolDashboard}
          description="Want the full host readiness dashboard before saving? Open the Host Tools page and return to onboarding later."
        />
      ) : null}
    </section>
  );
}
