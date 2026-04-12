import { useCallback, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type { OnboardingWizardStage, ReadinessCheckResult } from '../types/onboarding';
import type { VersionCheckResult } from '../types/version';

const STAGE_SEQUENCE: OnboardingWizardStage[] = ['identity_game', 'runtime', 'trainer', 'media', 'review', 'completed'];

export interface UseOnboardingResult {
  stage: OnboardingWizardStage;
  readinessResult: ReadinessCheckResult | null;
  checkError: string | null;
  versionResult: VersionCheckResult | null;
  statusText: string;
  hintText: string;
  actionLabel: string;
  isIdentityGame: boolean;
  isRuntime: boolean;
  isTrainer: boolean;
  isMedia: boolean;
  isReview: boolean;
  isCompleted: boolean;
  runChecks: () => Promise<void>;
  advanceOrSkip: (launchMethod: string) => void;
  goBack: (launchMethod?: string) => void;
  dismiss: () => Promise<void>;
  reset: () => void;
  setCompletedProfileName: (name: string) => void;
}

function deriveStatusText(stage: OnboardingWizardStage): string {
  switch (stage) {
    case 'identity_game':
      return 'Set up your profile identity, game, and runner method.';
    case 'runtime':
      return 'Configure the runtime environment for the selected runner.';
    case 'trainer':
      return 'Configure your trainer for this game.';
    case 'media':
      return 'Pick cover, portrait, and background art for this profile.';
    case 'review':
      return 'Review required fields and save the profile.';
    case 'completed':
      return 'Profile saved successfully.';
  }
}

function deriveHintText(stage: OnboardingWizardStage): string {
  switch (stage) {
    case 'identity_game':
      return 'Enter the profile name, game name and path, and choose how to launch it.';
    case 'runtime':
      return 'Set the paths and settings for your chosen launch method.';
    case 'trainer':
      return 'Browse to your trainer executable and choose the loading mode.';
    case 'media':
      return 'Custom art overrides auto-downloaded images. Leave blank to use defaults.';
    case 'review':
      return 'Confirm the required fields, optionally apply a launch preset, then save.';
    case 'completed':
      return 'Your profile is ready — head to the Launch page to start your game.';
  }
}

function deriveActionLabel(stage: OnboardingWizardStage): string {
  switch (stage) {
    case 'identity_game':
    case 'runtime':
    case 'trainer':
    case 'media':
      return 'Next';
    case 'review':
      return 'Save Profile';
    case 'completed':
      return 'Done';
  }
}

function createInitialOnboardingState(): {
  stage: OnboardingWizardStage;
  readinessResult: ReadinessCheckResult | null;
} {
  return {
    stage: 'identity_game',
    readinessResult: null,
  };
}

export function useOnboarding(): UseOnboardingResult {
  const [stage, setStage] = useState<OnboardingWizardStage>(() => createInitialOnboardingState().stage);
  const [readinessResult, setReadinessResult] = useState<ReadinessCheckResult | null>(
    () => createInitialOnboardingState().readinessResult
  );
  const [checkError, setCheckError] = useState<string | null>(null);
  const [lastCreatedProfileName, setLastCreatedProfileName] = useState<string | null>(null);
  const [versionResult, setVersionResult] = useState<VersionCheckResult | null>(null);

  // BR-9 invariant: No profile is persisted until the user explicitly confirms in the review step.
  // - dismiss() and skip paths only set onboarding_completed=true via dismiss_onboarding;
  //   they do NOT write any profile data to TOML.
  // - advanceOrSkip() transitions wizard stages forward with no persistent storage side effects,
  //   skipping the trainer stage when launchMethod is 'native'.
  // - goBack() moves backward by 1 stage, mirroring the forward skip for native launch method.
  // - reset() returns all hook state to initial values (stage='identity_game', readinessResult=null).
  // - The wizard unmounts when showOnboarding=false in App.tsx, so re-opening always starts fresh.

  const runChecks = useCallback(async () => {
    try {
      const result = await callCommand<ReadinessCheckResult>('check_readiness');
      setReadinessResult(result);
      setCheckError(null);
    } catch (error) {
      setCheckError(error instanceof Error ? error.message : 'Failed to run readiness checks.');
    }
  }, []);

  const advanceOrSkip = useCallback((launchMethod: string) => {
    setStage((current) => {
      const currentIndex = STAGE_SEQUENCE.indexOf(current);
      let nextIndex = currentIndex + 1;
      if (nextIndex < STAGE_SEQUENCE.length && STAGE_SEQUENCE[nextIndex] === 'trainer' && launchMethod === 'native') {
        nextIndex += 1;
      }
      return nextIndex < STAGE_SEQUENCE.length ? STAGE_SEQUENCE[nextIndex] : current;
    });
  }, []);

  const goBack = useCallback((launchMethod?: string) => {
    setStage((current) => {
      const currentIndex = STAGE_SEQUENCE.indexOf(current);
      let prevIndex = currentIndex - 1;
      // Mirror the forward skip: skip the trainer stage in reverse for native launch method
      if (prevIndex >= 0 && STAGE_SEQUENCE[prevIndex] === 'trainer' && launchMethod === 'native') {
        prevIndex -= 1;
      }
      return prevIndex >= 0 ? STAGE_SEQUENCE[prevIndex] : current;
    });
  }, []);

  const dismiss = useCallback(async () => {
    await callCommand<void>('dismiss_onboarding');
    setStage('completed');
    // Best-effort: trigger a health check so the new profile gets a health_snapshots
    // row in the Health Dashboard from day one. Ignored if MetadataStore is disabled.
    callCommand('batch_validate_profiles').catch(() => {});
    // Version snapshot is recorded on first launch, not here
    if (lastCreatedProfileName) {
      callCommand<VersionCheckResult>('check_version_status', { name: lastCreatedProfileName })
        .then(setVersionResult)
        .catch(() => {});
    }
  }, [lastCreatedProfileName]);

  const reset = useCallback(() => {
    const initial = createInitialOnboardingState();
    setStage(initial.stage);
    setReadinessResult(initial.readinessResult);
    setCheckError(null);
    setLastCreatedProfileName(null);
    setVersionResult(null);
  }, []);

  const statusText = deriveStatusText(stage);
  const hintText = deriveHintText(stage);
  const actionLabel = deriveActionLabel(stage);

  return {
    stage,
    readinessResult,
    checkError,
    versionResult,
    statusText,
    hintText,
    actionLabel,
    isIdentityGame: stage === 'identity_game',
    isRuntime: stage === 'runtime',
    isTrainer: stage === 'trainer',
    isMedia: stage === 'media',
    isReview: stage === 'review',
    isCompleted: stage === 'completed',
    runChecks,
    advanceOrSkip,
    goBack,
    dismiss,
    reset,
    setCompletedProfileName: setLastCreatedProfileName,
  };
}
