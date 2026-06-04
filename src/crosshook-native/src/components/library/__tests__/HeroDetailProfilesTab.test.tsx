import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { launchOptimizationsAutosaveDelayMs } from '@/hooks/profile/constants';
import { makeLibraryCardData, makeProfileDraft } from '@/test/fixtures';
import type { LibraryCardData, ProfileSummary } from '@/types/library';
import type { GameProfile, LaunchMethod } from '@/types/profile';
import { HeroDetailProfilesTab } from '../HeroDetailProfilesTab';

const profileContextMock = vi.fn();
const preferencesContextMock = vi.fn();
const callCommandMock = vi.fn();

const selectProfileSpy = vi.fn();
const persistProfileDraftSpy = vi.fn();
const updateProfileSpy = vi.fn();
const setProfileNameSpy = vi.fn();

vi.mock('@/context/ProfileContext', () => ({
  useProfileContext: () => profileContextMock(),
}));

vi.mock('@/context/PreferencesContext', () => ({
  usePreferencesContext: () => preferencesContextMock(),
}));

vi.mock('@/lib/ipc', () => ({
  callCommand: (...args: unknown[]) => callCommandMock(...args),
}));

vi.mock('@/components/OnboardingWizard', () => ({
  OnboardingWizard: ({ open }: { open: boolean }) => (open ? <div role="dialog">Onboarding Wizard</div> : null),
}));

vi.mock('@/components/profile-sections/ProfileIdentitySection', () => ({
  ProfileIdentitySection: () => <div>Identity Section</div>,
}));

vi.mock('@/components/profile-sections/RuntimeSection', () => ({
  RuntimeSection: () => <div>Runtime Section</div>,
}));

vi.mock('@/components/profile-sections/GameSection', () => ({
  GameSection: () => <div>Game Section</div>,
}));

vi.mock('@/components/profile-sections/MediaSection', () => ({
  MediaSection: () => <div>Media Section</div>,
}));

vi.mock('@/components/profile-sections/RunnerMethodSection', () => ({
  RunnerMethodSection: () => <div>Runner Method Section</div>,
}));

vi.mock('@/components/profile-sections/TrainerSection', () => ({
  TrainerSection: () => <div>Trainer Section</div>,
}));

vi.mock('@/components/profile-sections/GameMetadataBar', () => ({
  GameMetadataBar: () => null,
}));

vi.mock('@/components/GamescopeConfigPanel', () => ({
  GamescopeConfigPanel: () => <div>Gamescope Panel</div>,
}));

vi.mock('@/components/PrefixDepsPanel', () => ({
  PrefixDepsPanel: () => <div>Prefix Deps Panel</div>,
}));

vi.mock('@/context/ProfileHealthContext', () => ({
  useProfileHealthContext: () => ({
    healthByName: {},
    staleInfoByName: {},
    cachedSnapshots: {},
    trendByName: {},
    summary: null,
    loading: false,
    error: null,
    batchValidate: vi.fn(),
    revalidateSingle: vi.fn(),
  }),
}));

vi.mock('@/hooks/profile/useProfileActions', () => ({
  useProfileActions: () => ({
    canSave: false,
    canDelete: false,
    canDuplicate: false,
    canRename: false,
    canPreview: false,
    canExportCommunity: false,
    canViewHistory: false,
    previewing: false,
    previewError: null,
    showProfilePreview: false,
    profilePreviewContent: '',
    handlePreviewProfile: vi.fn(),
    handleCloseProfilePreview: vi.fn(),
    exportingCommunity: false,
    communityExportError: null,
    communityExportSuccess: null,
    handleExportCommunityProfile: vi.fn(),
    handleSave: vi.fn(),
    handleRefreshStatus: vi.fn(),
    handleAfterRollback: vi.fn(),
    showHistoryPanel: false,
    setShowHistoryPanel: vi.fn(),
    showWizard: false,
    wizardMode: 'create' as const,
    openWizard: vi.fn(),
    setShowWizard: vi.fn(),
    canConfirmRename: false,
    dismissHealthBanner: vi.fn(),
    dismissRenameToast: vi.fn(),
    handleRenameConfirm: vi.fn(),
    healthBannerDismissed: false,
    pendingRename: null,
    renameError: null,
    renameInputRef: { current: null },
    renameNameTrimmed: '',
    renameToast: null,
    renameToastDismissed: false,
    renameValue: '',
    setPendingRename: vi.fn(),
    setRenameValue: vi.fn(),
    undoRename: vi.fn(),
  }),
}));

vi.mock('@/components/library/profiles/HeroProfileActionsBar', () => ({
  HeroProfileActionsBar: () => null,
}));

vi.mock('@/components/library/profiles/HeroProfileEditorSections', () => ({
  HeroProfileEditorSections: (props: { launcherExportSlot?: import('react').ReactNode }) => (
    <div data-testid="hero-profile-editor-sections">{props.launcherExportSlot ?? null}</div>
  ),
}));

vi.mock('@/components/LauncherExport', () => ({
  LauncherExport: () => <div data-testid="launcher-export-panel">LauncherExport</div>,
}));

vi.mock('@/hooks/useTrainerTypeCatalog', () => ({
  useTrainerTypeCatalog: () => ({
    catalog: [],
    labels: {},
    error: null,
    selectOptions: [],
  }),
}));

vi.mock('@/components/pages/profiles/useProfilesPageProton', () => ({
  useProfilesPageProton: () => ({
    suggestion: null,
    suggestionDismissed: false,
    suggestionInstallError: null,
    protonUp: { installing: false },
    handleInstallSuggestedVersion: vi.fn(),
    setSuggestionDismissed: vi.fn(),
    protonInstalls: [],
    protonInstallsError: null,
  }),
}));

type ProfileContextState = {
  profile: GameProfile;
  profileName: string;
  selectedProfile: string;
  profiles: string[];
  dirty: boolean;
  saving: boolean;
  error: string | null;
  selectProfile: typeof selectProfileSpy;
  updateProfile: typeof updateProfileSpy;
  setProfileName: typeof setProfileNameSpy;
  persistProfileDraft: typeof persistProfileDraftSpy;
  steamClientInstallPath: string;
  // Additional fields consumed by HeroDetailProfilesTab (Task 3.2)
  targetHomePath: string;
  pendingDelete: null;
  deleting: boolean;
  duplicating: boolean;
  renaming: boolean;
  duplicateProfile: ReturnType<typeof vi.fn>;
  confirmDelete: ReturnType<typeof vi.fn>;
  executeDelete: ReturnType<typeof vi.fn>;
  cancelDelete: ReturnType<typeof vi.fn>;
  fetchConfigHistory: ReturnType<typeof vi.fn>;
  fetchConfigDiff: ReturnType<typeof vi.fn>;
  rollbackConfig: ReturnType<typeof vi.fn>;
  markKnownGood: ReturnType<typeof vi.fn>;
};

const card1: ProfileSummary = {
  name: 'card1',
  gameName: 'Synthetic Quest',
  steamAppId: '9999001',
  networkIsolation: false,
};

const card2: ProfileSummary = {
  name: 'card2',
  gameName: 'Synthetic Quest',
  steamAppId: '9999001',
  networkIsolation: false,
};

const summary: LibraryCardData = makeLibraryCardData({
  name: card1.name,
  gameName: card1.gameName,
  steamAppId: card1.steamAppId,
  networkIsolation: card1.networkIsolation,
});

let contextState: ProfileContextState;
let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

function buildContextState(overrides: Partial<ProfileContextState> = {}): ProfileContextState {
  const selectedProfile = overrides.selectedProfile ?? card1.name;
  const profileName = overrides.profileName ?? selectedProfile;

  return {
    profile: makeProfileDraft({
      game: {
        name: profileName,
        executable_path: '',
      },
    }),
    profileName,
    selectedProfile,
    profiles: [card1.name, card2.name],
    dirty: false,
    saving: false,
    error: null,
    selectProfile: selectProfileSpy,
    updateProfile: updateProfileSpy,
    setProfileName: setProfileNameSpy,
    persistProfileDraft: persistProfileDraftSpy,
    steamClientInstallPath: '/home/devuser/.steam/steam',
    targetHomePath: '/home/devuser',
    pendingDelete: null,
    deleting: false,
    duplicating: false,
    renaming: false,
    duplicateProfile: vi.fn().mockResolvedValue(undefined),
    confirmDelete: vi.fn().mockResolvedValue(undefined),
    executeDelete: vi.fn().mockResolvedValue(undefined),
    cancelDelete: vi.fn(),
    fetchConfigHistory: vi.fn().mockResolvedValue([]),
    fetchConfigDiff: vi.fn().mockResolvedValue({}),
    rollbackConfig: vi.fn().mockResolvedValue({}),
    markKnownGood: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function renderProfilesTab(props: Partial<React.ComponentProps<typeof HeroDetailProfilesTab>> = {}) {
  return render(
    <HeroDetailProfilesTab
      summary={summary}
      profileList={[card1, card2]}
      loadState="ready"
      profileError={null}
      healthByName={{}}
      {...props}
    />
  );
}

describe('HeroDetailProfilesTab', () => {
  beforeEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();

    contextState = buildContextState();
    profileContextMock.mockImplementation(() => contextState);
    preferencesContextMock.mockReturnValue({
      defaultSteamClientInstallPath: '/home/devuser/.steam/steam',
    });
    persistProfileDraftSpy.mockResolvedValue({ ok: true });
    selectProfileSpy.mockResolvedValue(undefined);
    callCommandMock.mockImplementation(async (name: string) => {
      if (name === 'profile_load') {
        return makeProfileDraft();
      }
      if (name === 'list_launch_history_for_profile') {
        return [
          {
            operation_id: 'launch-1',
            launch_method: 'steam',
            status: 'success',
            started_at: '2025-01-01T00:00:00.000Z',
            finished_at: '2025-01-01T00:01:00.000Z',
            exit_code: 0,
            signal: null,
            severity: null,
            failure_mode: null,
          },
        ];
      }
      if (name === 'list_proton_installs') {
        return [];
      }
      throw new Error(`Unhandled command: ${name}`);
    });
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    expect(consoleErrorSpy).not.toHaveBeenCalled();
    consoleErrorSpy.mockRestore();
  });

  it('autosaves dirty profile drafts after the debounce without firing immediately', async () => {
    contextState = buildContextState({ dirty: true });

    renderProfilesTab();

    expect(persistProfileDraftSpy).not.toHaveBeenCalled();

    await waitFor(
      () => {
        expect(persistProfileDraftSpy).toHaveBeenCalledWith(card1.name, expect.any(Object));
      },
      { timeout: 1000 }
    );
  });

  it('selects another profile card and reflects the selected card after context rerender', async () => {
    const user = userEvent.setup();
    const view = renderProfilesTab();

    await user.click(screen.getByRole('button', { name: /card2/i }));

    expect(selectProfileSpy).toHaveBeenCalledWith(card2.name);

    contextState = buildContextState({ selectedProfile: card2.name, profileName: card2.name });
    view.rerender(
      <HeroDetailProfilesTab
        summary={summary}
        profileList={[card1, card2]}
        loadState="ready"
        profileError={null}
        healthByName={{}}
      />
    );

    expect(screen.getByRole('heading', { name: card2.name })).toBeInTheDocument();
  });

  it('aligns singleton profile selection to the current game on mount', async () => {
    contextState = buildContextState({
      selectedProfile: 'unrelated-profile',
      profileName: 'unrelated-profile',
      profiles: ['unrelated-profile', card1.name],
    });

    renderProfilesTab({ profileList: [card1] });

    await waitFor(() => {
      expect(selectProfileSpy).toHaveBeenCalledWith(summary.name);
    });
  });

  it('selects a native button card from keyboard activation', async () => {
    const user = userEvent.setup();
    renderProfilesTab();

    const cardButton = screen.getByRole('button', { name: /card2/i });
    cardButton.focus();
    await user.keyboard('{Enter}');

    expect(selectProfileSpy).toHaveBeenCalledWith(card2.name);
  });

  it('pauses autosave during rename when draft name no longer matches selected profile', async () => {
    contextState = buildContextState({
      dirty: true,
      selectedProfile: card1.name,
      profileName: 'renamed-card',
    });

    renderProfilesTab();

    await new Promise((resolve) => window.setTimeout(resolve, launchOptimizationsAutosaveDelayMs + 100));

    expect(persistProfileDraftSpy).not.toHaveBeenCalled();
  });

  it('flushes a dirty saved profile before switching cards', async () => {
    const user = userEvent.setup();
    contextState = buildContextState({ dirty: true });

    renderProfilesTab();

    await user.click(screen.getByRole('button', { name: /card2/i }));

    await waitFor(() => {
      expect(persistProfileDraftSpy).toHaveBeenCalledWith(card1.name, expect.any(Object));
      expect(selectProfileSpy).toHaveBeenCalledWith(card2.name);
    });

    const persistCallOrder = persistProfileDraftSpy.mock.invocationCallOrder[0];
    const selectCard2CallOrder =
      selectProfileSpy.mock.invocationCallOrder[selectProfileSpy.mock.calls.findIndex(([name]) => name === card2.name)];

    expect(persistCallOrder).toBeLessThan(selectCard2CallOrder);
  });

  it('triggers autosave after runner-method change mutates the profile draft', async () => {
    // Start with a dirty profile to simulate an in-flight edit caused by a
    // runner-method change. The autosave fires after launchOptimizationsAutosaveDelayMs.
    contextState = buildContextState({
      dirty: true,
      profile: makeProfileDraft({
        launch: {
          method: 'proton_run' as LaunchMethod,
          optimizations: { enabled_option_ids: [] },
          custom_env_vars: {},
        },
      }),
    });

    renderProfilesTab();

    expect(persistProfileDraftSpy).not.toHaveBeenCalled();

    await waitFor(
      () => {
        expect(persistProfileDraftSpy).toHaveBeenCalledWith(card1.name, expect.any(Object));
      },
      { timeout: 1000 }
    );
  });

  it('shows the LauncherExport panel slot when launch method supports it and profile exists', () => {
    contextState = buildContextState({
      profile: makeProfileDraft({
        launch: {
          method: 'proton_run' as LaunchMethod,
          optimizations: { enabled_option_ids: [] },
          custom_env_vars: {},
        },
      }),
    });

    renderProfilesTab();

    expect(screen.getByTestId('launcher-export-panel')).toBeInTheDocument();
  });

  it('does not render the LauncherExport panel when launch method is native', () => {
    contextState = buildContextState({
      profile: makeProfileDraft({
        launch: {
          method: 'native' as LaunchMethod,
          optimizations: { enabled_option_ids: [] },
          custom_env_vars: {},
        },
      }),
    });

    renderProfilesTab();

    expect(screen.queryByTestId('launcher-export-panel')).not.toBeInTheDocument();
  });

  it('shows the loading state while profile load is in progress', () => {
    renderProfilesTab({ loadState: 'loading' });

    expect(screen.getByText(/loading profile details/i)).toBeInTheDocument();
  });

  it('shows the error state when profile load fails', () => {
    renderProfilesTab({ loadState: 'error', profileError: 'Profile not found' });

    expect(screen.getByText(/profile not found/i)).toBeInTheDocument();
  });

  it('shows the select-a-profile prompt when no singleton owns the game', () => {
    contextState = buildContextState({
      selectedProfile: 'unrelated',
      profileName: 'unrelated',
    });

    renderProfilesTab({ profileList: [card1] });

    expect(screen.getByRole('status')).toHaveTextContent(/select a profile card/i);
  });
});
