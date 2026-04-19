import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import { OnboardingWizard } from '../OnboardingWizard';

const useOnboardingMock = vi.fn();
const usePreferencesContextMock = vi.fn();
const useProfileContextMock = vi.fn();
const useProtonInstallsMock = vi.fn();
const evaluateWizardRequiredFieldsMock = vi.fn();

vi.mock('@/hooks/useOnboarding', () => ({
  useOnboarding: () => useOnboardingMock(),
}));

vi.mock('@/context/PreferencesContext', () => ({
  usePreferencesContext: () => usePreferencesContextMock(),
}));

vi.mock('@/context/ProfileContext', () => ({
  useProfileContext: () => useProfileContextMock(),
}));

vi.mock('@/hooks/useProtonInstalls', () => ({
  useProtonInstalls: () => useProtonInstallsMock(),
}));

vi.mock('@/components/profile-sections/ProfileIdentitySection', () => ({
  ProfileIdentitySection: () => <div>Identity Section</div>,
}));
vi.mock('@/components/profile-sections/GameSection', () => ({
  GameSection: () => <div>Game Section</div>,
}));
vi.mock('@/components/profile-sections/RunnerMethodSection', () => ({
  RunnerMethodSection: () => <div>Runner Section</div>,
}));
vi.mock('@/components/profile-sections/RuntimeSection', () => ({
  RuntimeSection: () => <div>Runtime Section</div>,
}));
vi.mock('@/components/profile-sections/TrainerSection', () => ({
  TrainerSection: () => <div>Trainer Section</div>,
}));
vi.mock('@/components/profile-sections/MediaSection', () => ({
  MediaSection: () => <div>Media Section</div>,
}));
vi.mock('@/components/host-readiness/HostToolDashboardHandoff', () => ({
  HostToolDashboardHandoff: () => <div>Host Tools Handoff</div>,
}));
vi.mock('@/components/layout/ControllerPrompts', () => ({
  ControllerPrompts: () => <div>Controller Prompts</div>,
}));
vi.mock('@/components/CustomEnvironmentVariablesSection', () => ({
  CustomEnvironmentVariablesSection: () => <div>Custom Env Section</div>,
}));
vi.mock('@/components/wizard/WizardPresetPicker', () => ({
  WizardPresetPicker: () => <div>Preset Picker</div>,
}));
vi.mock('@/components/wizard/WizardReviewSummary', () => ({
  WizardReviewSummary: () => <div>Review Summary</div>,
}));
vi.mock('@/components/wizard/wizardValidation', () => ({
  evaluateWizardRequiredFields: (...args: unknown[]) => evaluateWizardRequiredFieldsMock(...args),
}));

function buildOnboardingState(overrides: Record<string, unknown> = {}) {
  return {
    stage: 'identity_game',
    readinessResult: null,
    checkError: null,
    isRunningChecks: false,
    lastCheckedAt: null,
    umuInstallGuidance: null,
    steamDeckCaveats: null,
    isIdentityGame: true,
    isRuntime: false,
    isTrainer: false,
    isMedia: false,
    isReview: false,
    isCompleted: false,
    runChecks: vi.fn(),
    advanceOrSkip: vi.fn(),
    goBack: vi.fn(),
    dismiss: vi.fn().mockResolvedValue(undefined),
    dismissUmuInstallNag: vi.fn().mockResolvedValue(undefined),
    dismissSteamDeckCaveats: vi.fn().mockResolvedValue(undefined),
    dismissReadinessNag: vi.fn().mockResolvedValue(undefined),
    setCompletedProfileName: vi.fn(),
    ...overrides,
  };
}

describe('OnboardingWizard', () => {
  beforeEach(() => {
    useOnboardingMock.mockReturnValue(buildOnboardingState());
    usePreferencesContextMock.mockReturnValue({
      defaultSteamClientInstallPath: '/home/devuser/.steam/steam',
    });
    useProfileContextMock.mockReturnValue({
      profileName: 'Synthetic Quest',
      profile: makeProfileDraft(),
      saving: false,
      error: null,
      setProfileName: vi.fn(),
      updateProfile: vi.fn(),
      persistProfileDraft: vi.fn().mockResolvedValue({ ok: true }),
      selectProfile: vi.fn().mockResolvedValue(undefined),
      steamClientInstallPath: '/home/devuser/.steam/steam',
      bundledOptimizationPresets: [],
      applyBundledOptimizationPreset: vi.fn().mockResolvedValue(undefined),
      switchLaunchOptimizationPreset: vi.fn().mockResolvedValue(undefined),
      optimizationPresetActionBusy: false,
    });
    useProtonInstallsMock.mockReturnValue({
      installs: [],
      error: null,
    });
    evaluateWizardRequiredFieldsMock.mockReturnValue({
      isReady: true,
      firstMissingId: null,
    });
  });

  it('mounts in a portal and focuses the heading when opened', async () => {
    render(<OnboardingWizard open onComplete={vi.fn()} onDismiss={vi.fn()} />);

    const heading = await screen.findByRole('heading', { name: 'Identity & Game' });

    await waitFor(() => {
      expect(heading).toHaveFocus();
    });
    expect(screen.getByRole('button', { name: 'Skip Setup' })).toBeInTheDocument();
  });

  it('shows native review progress and disables save when validation fails', () => {
    useOnboardingMock.mockReturnValue(
      buildOnboardingState({
        stage: 'review',
        isIdentityGame: false,
        isReview: true,
      })
    );
    useProfileContextMock.mockReturnValue({
      ...useProfileContextMock(),
      profile: makeProfileDraft({
        launch: {
          ...makeProfileDraft().launch,
          method: 'native',
        },
      }),
    });
    evaluateWizardRequiredFieldsMock.mockReturnValue({
      isReady: false,
      firstMissingId: 'profile_name',
    });

    render(<OnboardingWizard open onComplete={vi.fn()} onDismiss={vi.fn()} />);

    expect(screen.getByText('Step 4 of 4')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save Profile' })).toBeDisabled();
  });

  it('dismisses onboarding before invoking onDismiss from Skip Setup', async () => {
    const dismiss = vi.fn().mockResolvedValue(undefined);
    const onDismiss = vi.fn();
    const user = userEvent.setup();

    useOnboardingMock.mockReturnValue(
      buildOnboardingState({
        dismiss,
      })
    );

    render(<OnboardingWizard open onComplete={vi.fn()} onDismiss={onDismiss} />);

    await user.click(screen.getByRole('button', { name: 'Skip Setup' }));

    expect(dismiss).toHaveBeenCalledTimes(1);
    await waitFor(() => {
      expect(onDismiss).toHaveBeenCalledTimes(1);
    });
  });
});
