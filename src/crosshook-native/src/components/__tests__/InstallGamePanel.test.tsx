import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import { InstallGamePanel } from '../InstallGamePanel';

// ---------------------------------------------------------------------------
// Module mocks — keep IPC and heavy child sections out of scope
// ---------------------------------------------------------------------------

const useInstallGameMock = vi.fn();
const useProfileContextMock = vi.fn();
const useProtonInstallsMock = vi.fn();

vi.mock('@/hooks/useInstallGame', () => ({
  useInstallGame: () => useInstallGameMock(),
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

vi.mock('@/components/install/InstallReviewSummary', () => ({
  InstallReviewSummary: () => <div>Install Review Summary</div>,
}));

vi.mock('@/components/wizard/WizardPresetPicker', () => ({
  WizardPresetPicker: () => <div>Preset Picker</div>,
}));

vi.mock('@/components/CustomEnvironmentVariablesSection', () => ({
  CustomEnvironmentVariablesSection: () => <div>Custom Env Section</div>,
}));

vi.mock('@/components/ui/InstallField', () => ({
  InstallField: () => <div>Install Field</div>,
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function buildInstallGameState(overrides: Record<string, unknown> = {}) {
  const draft = makeProfileDraft({
    launch: { ...makeProfileDraft().launch, method: 'proton_run' },
  });
  return {
    profileName: 'Synthetic Quest',
    setProfileName: vi.fn(),
    draftProfile: draft,
    updateDraftProfile: vi.fn(),
    installerInputs: { installer_path: '' },
    updateInstallerInputs: vi.fn(),
    validation: { fieldErrors: {}, generalError: null },
    stage: 'idle' as const,
    result: null,
    reviewProfile: null,
    error: null,
    defaultPrefixPath: '',
    defaultPrefixPathState: 'unknown' as const,
    defaultPrefixPathError: null,
    candidateOptions: [],
    actionLabel: 'Run Installer',
    statusText: '',
    hintText: '',
    isIdle: true,
    isPreparing: false,
    isRunningInstaller: false,
    isReviewRequired: false,
    isReadyToSave: false,
    hasFailed: false,
    isResolvingDefaultPrefixPath: false,
    setFieldError: vi.fn(),
    setGeneralError: vi.fn(),
    clearValidation: vi.fn(),
    setStage: vi.fn(),
    setResult: vi.fn(),
    setError: vi.fn(),
    setInstalledExecutablePath: vi.fn(),
    startInstall: vi.fn().mockResolvedValue(undefined),
    reset: vi.fn(),
    ...overrides,
  };
}

describe('InstallGamePanel', () => {
  beforeEach(() => {
    useInstallGameMock.mockReturnValue(buildInstallGameState());
    useProfileContextMock.mockReturnValue({
      bundledOptimizationPresets: [],
    });
    useProtonInstallsMock.mockReturnValue({
      installs: [],
      error: null,
    });
  });

  // (a) Shell chrome: root <section> renders with .crosshook-install-shell class
  it('renders the install shell root section', () => {
    const { container } = render(<InstallGamePanel onOpenProfileReview={vi.fn()} />);

    expect(container.querySelector('.crosshook-install-shell')).toBeInTheDocument();
    expect(container.querySelector('section.crosshook-install-shell')).toBeInTheDocument();
  });

  // (b) Error-banner regression: no alert role in the happy path
  it('does not show an error banner in the happy path', () => {
    render(<InstallGamePanel onOpenProfileReview={vi.fn()} />);

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  // (c) Tab triggers are all visible and forceMount keeps content in the DOM across tab switches
  it('renders all five install flow tab triggers', () => {
    render(<InstallGamePanel onOpenProfileReview={vi.fn()} />);

    expect(screen.getByRole('tab', { name: 'Identity & Game' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Runtime' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Trainer' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Media' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Installer & Review' })).toBeInTheDocument();

    expect(screen.getByRole('heading', { name: 'Identity & Game', hidden: true })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Runtime', hidden: true })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Trainer', hidden: true })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Media', hidden: true })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Installer & Review', hidden: true })).toBeInTheDocument();
  });

  it('keeps non-active tab content in DOM via forceMount after switching tabs', async () => {
    const user = userEvent.setup();
    render(<InstallGamePanel onOpenProfileReview={vi.fn()} />);

    // Identity Section is visible in the identity tab (active by default)
    expect(screen.getByText('Identity Section')).toBeInTheDocument();

    // Switch to Runtime tab
    await user.click(screen.getByRole('tab', { name: 'Runtime' }));

    // forceMount: identity section content stays in the DOM even when not active
    expect(screen.getByText('Identity Section')).toBeInTheDocument();
    expect(screen.getByText('Runtime Section')).toBeInTheDocument();
  });

  // Reset Form button renders
  it('renders the Reset Form action button', () => {
    render(<InstallGamePanel onOpenProfileReview={vi.fn()} />);

    expect(screen.getByRole('button', { name: 'Reset Form' })).toBeInTheDocument();
  });
});
