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

function buildProfileContextValue(overrides: Record<string, unknown> = {}) {
  return {
    profiles: [],
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
    ...overrides,
  };
}

describe('OnboardingWizard', () => {
  beforeEach(() => {
    useOnboardingMock.mockReturnValue(buildOnboardingState());
    usePreferencesContextMock.mockReturnValue({
      defaultSteamClientInstallPath: '/home/devuser/.steam/steam',
    });
    useProfileContextMock.mockReturnValue(buildProfileContextValue());
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
    useProfileContextMock.mockReturnValue(
      buildProfileContextValue({
        profile: makeProfileDraft({
          launch: {
            ...makeProfileDraft().launch,
            method: 'native',
          },
        }),
      })
    );
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

  // ---------------------------------------------------------------------------
  // Seed application
  // ---------------------------------------------------------------------------

  describe('createSeed application', () => {
    it('applies suggestedName and seed fields after create-mode reset', async () => {
      const setProfileName = vi.fn();
      const updateProfile = vi.fn();
      const selectProfile = vi.fn().mockResolvedValue(undefined);

      useProfileContextMock.mockReturnValue(buildProfileContextValue({ setProfileName, updateProfile, selectProfile }));

      const seed = {
        suggestedName: 'My Game',
        gameName: 'My Game Title',
        executablePath: '/games/mygame.exe',
      };

      render(<OnboardingWizard open mode="create" createSeed={seed} onComplete={vi.fn()} onDismiss={vi.fn()} />);

      await waitFor(() => {
        expect(selectProfile).toHaveBeenCalledWith('');
      });

      await waitFor(() => {
        expect(setProfileName).toHaveBeenCalledWith('My Game');
        expect(updateProfile).toHaveBeenCalled();
      });

      // Verify the updater merges the seed into the profile
      const updater = updateProfile.mock.calls[0][0] as (p: ReturnType<typeof makeProfileDraft>) => unknown;
      const base = makeProfileDraft();
      const result = updater(base) as ReturnType<typeof makeProfileDraft>;
      expect(result.game.name).toBe('My Game Title');
      expect(result.game.executable_path).toBe('/games/mygame.exe');
    });

    it('does not call setProfileName when suggestedName is absent', async () => {
      const setProfileName = vi.fn();
      const selectProfile = vi.fn().mockResolvedValue(undefined);

      useProfileContextMock.mockReturnValue(buildProfileContextValue({ setProfileName, selectProfile }));

      render(
        <OnboardingWizard
          open
          mode="create"
          createSeed={{ gameName: 'Only Game Name' }}
          onComplete={vi.fn()}
          onDismiss={vi.fn()}
        />
      );

      await waitFor(() => {
        expect(selectProfile).toHaveBeenCalledWith('');
      });

      await waitFor(() => {
        expect(setProfileName).not.toHaveBeenCalled();
      });
    });

    it('seed identity change while open does NOT re-trigger the reset', async () => {
      const selectProfile = vi.fn().mockResolvedValue(undefined);

      useProfileContextMock.mockReturnValue(buildProfileContextValue({ selectProfile }));

      const seed1 = { suggestedName: 'First' };
      const seed2 = { suggestedName: 'Second' };

      const { rerender } = render(
        <OnboardingWizard open mode="create" createSeed={seed1} onComplete={vi.fn()} onDismiss={vi.fn()} />
      );

      await waitFor(() => {
        expect(selectProfile).toHaveBeenCalledTimes(1);
      });

      // Change the seed object while the wizard stays open
      rerender(<OnboardingWizard open mode="create" createSeed={seed2} onComplete={vi.fn()} onDismiss={vi.fn()} />);

      // selectProfile must still be called only once — not again due to seed change
      await new Promise((r) => setTimeout(r, 20));
      expect(selectProfile).toHaveBeenCalledTimes(1);
    });

    it('legacy no-seed mount does not call setProfileName or updateProfile seed paths', async () => {
      const setProfileName = vi.fn();
      const updateProfile = vi.fn();
      const selectProfile = vi.fn().mockResolvedValue(undefined);

      useProfileContextMock.mockReturnValue(buildProfileContextValue({ setProfileName, updateProfile, selectProfile }));

      render(<OnboardingWizard open mode="create" onComplete={vi.fn()} onDismiss={vi.fn()} />);

      await waitFor(() => {
        expect(selectProfile).toHaveBeenCalledWith('');
      });

      // After selectProfile resolves with no seed, neither setter should be called
      await waitFor(() => {
        expect(setProfileName).not.toHaveBeenCalled();
        expect(updateProfile).not.toHaveBeenCalled();
      });
    });
  });

  // ---------------------------------------------------------------------------
  // Duplicate-name guard
  // ---------------------------------------------------------------------------

  describe('create-mode duplicate name guard', () => {
    it('shows collision banner and does not persist when name already exists', async () => {
      const persistProfileDraft = vi.fn().mockResolvedValue({ ok: true });
      const user = userEvent.setup();

      useProfileContextMock.mockReturnValue(
        buildProfileContextValue({
          profiles: ['Existing Game'],
          profileName: 'Existing Game',
          persistProfileDraft,
        })
      );
      useOnboardingMock.mockReturnValue(
        buildOnboardingState({
          isIdentityGame: false,
          isReview: true,
        })
      );
      evaluateWizardRequiredFieldsMock.mockReturnValue({ isReady: true, firstMissingId: null });

      render(<OnboardingWizard open mode="create" onComplete={vi.fn()} onDismiss={vi.fn()} />);

      await user.click(screen.getByRole('button', { name: 'Save Profile' }));

      await waitFor(() => {
        expect(screen.getByText(/A profile named "Existing Game" already exists/)).toBeInTheDocument();
      });
      expect(persistProfileDraft).not.toHaveBeenCalled();
    });

    it('clears the collision error when profileName changes', async () => {
      const persistProfileDraft = vi.fn().mockResolvedValue({ ok: true });
      const user = userEvent.setup();
      let currentProfileName = 'Existing Game';
      const setProfileNameMock = vi.fn((name: string) => {
        currentProfileName = name;
      });

      useProfileContextMock.mockReturnValue(
        buildProfileContextValue({
          profiles: ['Existing Game'],
          profileName: 'Existing Game',
          persistProfileDraft,
          setProfileName: setProfileNameMock,
        })
      );
      useOnboardingMock.mockReturnValue(buildOnboardingState({ isIdentityGame: false, isReview: true }));
      evaluateWizardRequiredFieldsMock.mockReturnValue({ isReady: true, firstMissingId: null });

      const { rerender } = render(<OnboardingWizard open mode="create" onComplete={vi.fn()} onDismiss={vi.fn()} />);

      await user.click(screen.getByRole('button', { name: 'Save Profile' }));

      await waitFor(() => {
        expect(screen.getByRole('alert')).toBeInTheDocument();
      });

      // Simulate profileName changing
      currentProfileName = 'New Unique Name';
      useProfileContextMock.mockReturnValue(
        buildProfileContextValue({
          profiles: ['Existing Game'],
          profileName: currentProfileName,
          persistProfileDraft,
          setProfileName: setProfileNameMock,
        })
      );
      rerender(<OnboardingWizard open mode="create" onComplete={vi.fn()} onDismiss={vi.fn()} />);

      await waitFor(() => {
        expect(screen.queryByRole('alert')).not.toBeInTheDocument();
      });
    });

    it('persists successfully after renaming to a unique name', async () => {
      const persistProfileDraft = vi.fn().mockResolvedValue({ ok: true });
      const onComplete = vi.fn();
      const user = userEvent.setup();
      const dismiss = vi.fn().mockResolvedValue(undefined);

      useProfileContextMock.mockReturnValue(
        buildProfileContextValue({
          profiles: [],
          profileName: 'New Unique Name',
          persistProfileDraft,
        })
      );
      useOnboardingMock.mockReturnValue(buildOnboardingState({ isIdentityGame: false, isReview: true, dismiss }));
      evaluateWizardRequiredFieldsMock.mockReturnValue({ isReady: true, firstMissingId: null });

      render(<OnboardingWizard open mode="create" onComplete={onComplete} onDismiss={vi.fn()} />);

      await user.click(screen.getByRole('button', { name: 'Save Profile' }));

      await waitFor(() => {
        expect(persistProfileDraft).toHaveBeenCalledWith('New Unique Name', expect.anything());
        expect(onComplete).toHaveBeenCalledWith('New Unique Name');
      });
    });

    it('does not apply duplicate guard in edit mode', async () => {
      const persistProfileDraft = vi.fn().mockResolvedValue({ ok: true });
      const onComplete = vi.fn();
      const user = userEvent.setup();
      const dismiss = vi.fn().mockResolvedValue(undefined);

      useProfileContextMock.mockReturnValue(
        buildProfileContextValue({
          profiles: ['Same Name'],
          profileName: 'Same Name',
          persistProfileDraft,
        })
      );
      useOnboardingMock.mockReturnValue(buildOnboardingState({ isIdentityGame: false, isReview: true, dismiss }));
      evaluateWizardRequiredFieldsMock.mockReturnValue({ isReady: true, firstMissingId: null });

      render(<OnboardingWizard open mode="edit" onComplete={onComplete} onDismiss={vi.fn()} />);

      await user.click(screen.getByRole('button', { name: 'Save Profile' }));

      await waitFor(() => {
        expect(persistProfileDraft).toHaveBeenCalled();
      });
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    });
  });

  // ---------------------------------------------------------------------------
  // Name-carrying completion
  // ---------------------------------------------------------------------------

  describe('onComplete receives the trimmed name', () => {
    it('calls onComplete with the trimmed profile name after successful persist', async () => {
      const persistProfileDraft = vi.fn().mockResolvedValue({ ok: true });
      const onComplete = vi.fn();
      const dismiss = vi.fn().mockResolvedValue(undefined);
      const user = userEvent.setup();

      useProfileContextMock.mockReturnValue(
        buildProfileContextValue({
          profiles: [],
          profileName: '  Trimmed Name  ',
          persistProfileDraft,
        })
      );
      useOnboardingMock.mockReturnValue(buildOnboardingState({ isIdentityGame: false, isReview: true, dismiss }));
      evaluateWizardRequiredFieldsMock.mockReturnValue({ isReady: true, firstMissingId: null });

      render(<OnboardingWizard open mode="create" onComplete={onComplete} onDismiss={vi.fn()} />);

      await user.click(screen.getByRole('button', { name: 'Save Profile' }));

      await waitFor(() => {
        expect(onComplete).toHaveBeenCalledWith('Trimmed Name');
      });
    });

    it('does not call onComplete when persistProfileDraft returns ok:false', async () => {
      const persistProfileDraft = vi.fn().mockResolvedValue({ ok: false, error: 'Save failed' });
      const onComplete = vi.fn();
      const user = userEvent.setup();

      useProfileContextMock.mockReturnValue(
        buildProfileContextValue({
          profiles: [],
          profileName: 'Good Name',
          persistProfileDraft,
        })
      );
      useOnboardingMock.mockReturnValue(buildOnboardingState({ isIdentityGame: false, isReview: true }));
      evaluateWizardRequiredFieldsMock.mockReturnValue({ isReady: true, firstMissingId: null });

      render(<OnboardingWizard open mode="create" onComplete={onComplete} onDismiss={vi.fn()} />);

      await user.click(screen.getByRole('button', { name: 'Save Profile' }));

      await waitFor(() => {
        expect(persistProfileDraft).toHaveBeenCalled();
      });
      expect(onComplete).not.toHaveBeenCalled();
    });
  });
});
