/**
 * Focused tests for HeroProfileActionsBar.
 *
 * Tests cover each lifecycle action (duplicate, rename + undo toast, delete confirm
 * overlay, TOML preview modal, community export, config history, mark-as-verified),
 * their busy labels, and role="alert" error states.
 *
 * Strategy A: hand-rolled vi.mock of contexts/IPC per the HeroDetailProfilesTab
 * pattern; heavy sub-components stubbed to <div>; real HeroProfileActionsBar
 * rendered with context mocks only.
 */
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { UseProfileActionsResult } from '@/hooks/profile/useProfileActions';
import { makeProfileDraft } from '@/test/fixtures';
import type { VersionCorrelationStatus } from '@/types/version';
import { HeroProfileActionsBar } from '../profiles/HeroProfileActionsBar';

// ---------------------------------------------------------------------------
// Context / IPC mocks
// ---------------------------------------------------------------------------

const profileContextMock = vi.fn();
const profileHealthContextMock = vi.fn();
const acknowledgeVersionChangeMock = vi.fn();
const callCommandMock = vi.fn();

vi.mock('@/context/ProfileContext', () => ({
  useProfileContext: () => profileContextMock(),
}));

vi.mock('@/context/ProfileHealthContext', () => ({
  useProfileHealthContext: () => profileHealthContextMock(),
}));

vi.mock('@/hooks/useAcknowledgeVersionChange', () => ({
  useAcknowledgeVersionChange: () => ({
    acknowledgeVersionChange: acknowledgeVersionChangeMock,
    busy: false,
  }),
  presentAcknowledgeVersionChangeOutcome: vi.fn(),
}));

vi.mock('@/lib/ipc', () => ({
  callCommand: (...args: unknown[]) => callCommandMock(...args),
}));

// Stub heavy sub-panels so tests stay fast and focused
vi.mock('../../ConfigHistoryPanel', () => ({
  ConfigHistoryPanel: ({ profileName }: { profileName: string }) => (
    <div data-testid="config-history-panel">{profileName}</div>
  ),
  default: ({ profileName }: { profileName: string }) => <div data-testid="config-history-panel">{profileName}</div>,
}));

vi.mock('../../ProfilePreviewModal', () => ({
  ProfilePreviewModal: ({ profileName }: { profileName: string }) => (
    <div data-testid="profile-preview-modal" role="dialog">
      Preview: {profileName}
    </div>
  ),
  default: ({ profileName }: { profileName: string }) => (
    <div data-testid="profile-preview-modal" role="dialog">
      Preview: {profileName}
    </div>
  ),
}));

// ---------------------------------------------------------------------------
// Fixtures / builders
// ---------------------------------------------------------------------------

const defaultSelectedProfile = 'my-profile';

type PendingDelete = {
  name: string;
  launcherInfo: null | { script_path: string; desktop_entry_path: string };
};

function buildProfileContext(
  overrides: Partial<{
    selectedProfile: string;
    profileName: string;
    pendingDelete: PendingDelete | null;
    deleting: boolean;
    duplicating: boolean;
    renaming: boolean;
    duplicateProfile: ReturnType<typeof vi.fn>;
    confirmDelete: ReturnType<typeof vi.fn>;
    executeDelete: ReturnType<typeof vi.fn>;
    cancelDelete: ReturnType<typeof vi.fn>;
  }> = {}
) {
  return {
    selectedProfile: defaultSelectedProfile,
    profileName: defaultSelectedProfile,
    profile: makeProfileDraft(),
    profiles: [defaultSelectedProfile],
    profileExists: true,
    dirty: false,
    saving: false,
    loading: false,
    error: null,
    pendingDelete: null,
    deleting: false,
    duplicating: false,
    renaming: false,
    duplicateProfile: vi.fn().mockResolvedValue(undefined),
    confirmDelete: vi.fn().mockResolvedValue(undefined),
    executeDelete: vi.fn().mockResolvedValue(undefined),
    cancelDelete: vi.fn(),
    ...overrides,
  };
}

function buildActions(overrides: Partial<UseProfileActionsResult> = {}): UseProfileActionsResult {
  return {
    canSave: true,
    canDelete: true,
    canDuplicate: true,
    canRename: true,
    canPreview: true,
    canExportCommunity: true,
    canViewHistory: true,
    previewing: false,
    previewError: null,
    showProfilePreview: false,
    profilePreviewContent: '',
    profilePreviewHooksStripped: false,
    profileHasConfiguredHooks: false,
    handlePreviewProfile: vi.fn().mockResolvedValue(undefined),
    handleIncludeHooksInPreview: vi.fn().mockResolvedValue(undefined),
    handleCloseProfilePreview: vi.fn(),
    exportingCommunity: false,
    communityExportError: null,
    communityExportSuccess: null,
    handleExportCommunityProfile: vi.fn().mockResolvedValue(undefined),
    handleSave: vi.fn().mockResolvedValue(undefined),
    handleRefreshStatus: vi.fn().mockResolvedValue(undefined),
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
    ...overrides,
  };
}

const historyHandlers = {
  fetchConfigHistory: vi.fn().mockResolvedValue([]),
  fetchConfigDiff: vi.fn().mockResolvedValue({}),
  rollbackConfig: vi.fn().mockResolvedValue({}),
  markKnownGood: vi.fn().mockResolvedValue(undefined),
};

function renderActionsBar(
  actionsOverrides: Partial<UseProfileActionsResult> = {},
  versionStatus: VersionCorrelationStatus | null | undefined = null
) {
  const actions = buildActions(actionsOverrides);
  return render(
    <HeroProfileActionsBar
      actions={actions}
      onAfterRollback={actions.handleAfterRollback}
      versionStatus={versionStatus}
      historyHandlers={historyHandlers}
    />
  );
}

let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  vi.clearAllMocks();

  profileContextMock.mockImplementation(() => buildProfileContext());
  profileHealthContextMock.mockImplementation(() => ({
    healthByName: {},
    staleInfoByName: {},
    cachedSnapshots: {},
    trendByName: {},
    summary: null,
    loading: false,
    error: null,
    batchValidate: vi.fn(),
    revalidateSingle: vi.fn().mockResolvedValue(undefined),
  }));
  acknowledgeVersionChangeMock.mockResolvedValue({ ok: true });
  callCommandMock.mockResolvedValue(undefined);

  consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
});

afterEach(() => {
  expect(consoleErrorSpy).not.toHaveBeenCalled();
  consoleErrorSpy.mockRestore();
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('HeroProfileActionsBar', () => {
  describe('Duplicate', () => {
    it('calls duplicateProfile with selected profile name on click', async () => {
      const user = userEvent.setup();
      const duplicateProfile = vi.fn().mockResolvedValue(undefined);
      profileContextMock.mockImplementation(() => buildProfileContext({ duplicateProfile }));

      renderActionsBar();

      await user.click(screen.getByRole('button', { name: /duplicate/i }));

      expect(duplicateProfile).toHaveBeenCalledWith(defaultSelectedProfile);
    });

    it('shows busy label while duplicating', () => {
      profileContextMock.mockImplementation(() => buildProfileContext({ duplicating: true }));
      renderActionsBar({ canDuplicate: false });

      expect(screen.getByRole('button', { name: /duplicating/i })).toBeDisabled();
    });

    it('disables duplicate button when canDuplicate is false', () => {
      renderActionsBar({ canDuplicate: false });

      expect(screen.getByRole('button', { name: /duplicate/i })).toBeDisabled();
    });
  });

  describe('Rename', () => {
    it('opens rename modal when Rename is clicked', async () => {
      const user = userEvent.setup();
      const setPendingRename = vi.fn();
      const setRenameValue = vi.fn();

      renderActionsBar({ setPendingRename, setRenameValue });

      await user.click(screen.getByRole('button', { name: /^rename$/i }));

      expect(setPendingRename).toHaveBeenCalledWith(defaultSelectedProfile);
      expect(setRenameValue).toHaveBeenCalledWith(defaultSelectedProfile);
    });

    it('renders rename modal with dialog role when pendingRename is set', () => {
      renderActionsBar({ pendingRename: defaultSelectedProfile, renameValue: defaultSelectedProfile });

      expect(screen.getByRole('dialog', { name: /rename profile/i })).toBeInTheDocument();
    });

    it('shows rename busy label while renaming', () => {
      profileContextMock.mockImplementation(() => buildProfileContext({ renaming: true }));
      // Do NOT open the modal (no pendingRename) so only the main bar button shows
      renderActionsBar({ canRename: false });

      // The main actions bar Rename button shows "Renaming…" when renaming is true
      expect(screen.getByRole('button', { name: /renaming/i })).toBeDisabled();
    });

    it('shows rename error with role="alert" inside the modal', () => {
      renderActionsBar({
        pendingRename: defaultSelectedProfile,
        renameValue: defaultSelectedProfile,
        renameError: 'A profile with that name already exists.',
      });

      const alert = screen.getByRole('alert');
      expect(alert).toHaveTextContent(/a profile with that name already exists/i);
    });

    it('renders rename success toast with undo button', () => {
      renderActionsBar({
        renameToast: { oldName: defaultSelectedProfile, newName: 'new-profile' },
        renameToastDismissed: false,
      });

      expect(screen.getByText(/renamed to/i)).toBeInTheDocument();
      expect(screen.getByRole('button', { name: /undo/i })).toBeInTheDocument();
    });

    it('calls undoRename when Undo is clicked in toast', async () => {
      const user = userEvent.setup();
      const undoRename = vi.fn();

      renderActionsBar({
        renameToast: { oldName: defaultSelectedProfile, newName: 'new-profile' },
        renameToastDismissed: false,
        undoRename,
      });

      await user.click(screen.getByRole('button', { name: /undo/i }));

      expect(undoRename).toHaveBeenCalled();
    });

    it('hides rename toast when renameToastDismissed is true', () => {
      renderActionsBar({
        renameToast: { oldName: defaultSelectedProfile, newName: 'new-profile' },
        renameToastDismissed: true,
      });

      expect(screen.queryByText(/renamed to/i)).not.toBeInTheDocument();
    });
  });

  describe('Delete', () => {
    it('calls confirmDelete with selected profile name on click', async () => {
      const user = userEvent.setup();
      const confirmDelete = vi.fn().mockResolvedValue(undefined);
      profileContextMock.mockImplementation(() => buildProfileContext({ confirmDelete }));

      renderActionsBar();

      await user.click(screen.getByRole('button', { name: /^delete$/i }));

      expect(confirmDelete).toHaveBeenCalledWith(defaultSelectedProfile);
    });

    it('disables delete and does not call confirmDelete when canDelete is false', async () => {
      const user = userEvent.setup();
      const confirmDelete = vi.fn().mockResolvedValue(undefined);
      profileContextMock.mockImplementation(() => buildProfileContext({ confirmDelete }));

      renderActionsBar({ canDelete: false });

      const deleteButton = screen.getByRole('button', { name: /^delete$/i });
      expect(deleteButton).toBeDisabled();
      await user.click(deleteButton);

      expect(confirmDelete).not.toHaveBeenCalled();
    });

    it('renders delete confirm overlay when pendingDelete is set', () => {
      profileContextMock.mockImplementation(() =>
        buildProfileContext({
          pendingDelete: { name: defaultSelectedProfile, launcherInfo: null },
        })
      );

      renderActionsBar();

      // The overlay heading "Delete Profile" should appear
      expect(screen.getByRole('heading', { name: /delete profile/i })).toBeInTheDocument();
      // The confirm button text is "Delete Profile"
      expect(screen.getAllByText(/delete profile/i).length).toBeGreaterThanOrEqual(1);
    });

    it('shows launcher warning when pendingDelete includes launcherInfo', () => {
      profileContextMock.mockImplementation(() =>
        buildProfileContext({
          pendingDelete: {
            name: defaultSelectedProfile,
            launcherInfo: {
              script_path: '/home/user/.local/share/scripts/my-profile.sh',
              desktop_entry_path: '/home/user/.local/share/applications/my-profile.desktop',
            },
          },
        })
      );

      renderActionsBar();

      expect(screen.getByText(/launcher files will also be removed/i)).toBeInTheDocument();
    });

    it('calls cancelDelete when Cancel is clicked in the confirm overlay', async () => {
      const user = userEvent.setup();
      const cancelDelete = vi.fn();
      profileContextMock.mockImplementation(() =>
        buildProfileContext({
          pendingDelete: { name: defaultSelectedProfile, launcherInfo: null },
          cancelDelete,
        })
      );

      renderActionsBar();

      await user.click(screen.getByRole('button', { name: /cancel/i }));

      expect(cancelDelete).toHaveBeenCalled();
    });

    it('calls executeDelete when confirm button is clicked', async () => {
      const user = userEvent.setup();
      const executeDelete = vi.fn().mockResolvedValue(undefined);
      profileContextMock.mockImplementation(() =>
        buildProfileContext({
          pendingDelete: { name: defaultSelectedProfile, launcherInfo: null },
          executeDelete,
        })
      );

      renderActionsBar();

      await user.click(screen.getByRole('button', { name: /delete profile$/i }));

      expect(executeDelete).toHaveBeenCalled();
    });

    it('shows busy label while deleting', () => {
      profileContextMock.mockImplementation(() => buildProfileContext({ deleting: true }));

      renderActionsBar();

      expect(screen.getByRole('button', { name: /deleting/i })).toBeDisabled();
    });
  });

  describe('TOML Preview', () => {
    it('calls handlePreviewProfile when Preview Profile button is clicked', async () => {
      const user = userEvent.setup();
      const handlePreviewProfile = vi.fn().mockResolvedValue(undefined);

      renderActionsBar({ handlePreviewProfile });

      await user.click(screen.getByRole('button', { name: /preview profile/i }));

      expect(handlePreviewProfile).toHaveBeenCalled();
    });

    it('renders preview modal when showProfilePreview is true', () => {
      renderActionsBar({
        showProfilePreview: true,
        profilePreviewContent: '[profile]\nname = "my-profile"',
      });

      expect(screen.getByTestId('profile-preview-modal')).toBeInTheDocument();
    });

    it('shows busy label while previewing', () => {
      renderActionsBar({ previewing: true, canPreview: false });

      expect(screen.getByRole('button', { name: /loading preview/i })).toBeDisabled();
    });

    it('shows preview error with role="alert"', () => {
      renderActionsBar({ previewError: 'Failed to serialize profile to TOML.' });

      const alert = screen.getByRole('alert');
      expect(alert).toHaveTextContent(/preview failed/i);
      expect(alert).toHaveTextContent(/failed to serialize profile to toml/i);
    });
  });

  describe('Community Export', () => {
    it('calls handleExportCommunityProfile when Export button is clicked', async () => {
      const user = userEvent.setup();
      const handleExportCommunityProfile = vi.fn().mockResolvedValue(undefined);

      renderActionsBar({ handleExportCommunityProfile });

      await user.click(screen.getByRole('button', { name: /export as community profile/i }));

      expect(handleExportCommunityProfile).toHaveBeenCalled();
    });

    it('shows busy label while exporting', () => {
      renderActionsBar({ exportingCommunity: true, canExportCommunity: false });

      expect(screen.getByRole('button', { name: /exporting/i })).toBeDisabled();
    });

    it('shows community export error with role="alert"', () => {
      renderActionsBar({ communityExportError: 'Export failed: permission denied.' });

      const alert = screen.getByRole('alert');
      expect(alert).toHaveTextContent(/community export failed/i);
      expect(alert).toHaveTextContent(/permission denied/i);
    });

    it('shows community export success with role="status"', () => {
      renderActionsBar({ communityExportSuccess: 'Community profile saved to /tmp/my-profile.json' });

      expect(screen.getByRole('status')).toHaveTextContent(/community profile saved/i);
    });
  });

  describe('Config History', () => {
    it('opens history panel when History button is clicked', async () => {
      const user = userEvent.setup();
      const setShowHistoryPanel = vi.fn();

      renderActionsBar({ setShowHistoryPanel });

      await user.click(screen.getByRole('button', { name: /history/i }));

      expect(setShowHistoryPanel).toHaveBeenCalledWith(true);
    });

    it('renders ConfigHistoryPanel when showHistoryPanel is true', () => {
      renderActionsBar({ showHistoryPanel: true });

      expect(screen.getByTestId('config-history-panel')).toBeInTheDocument();
      expect(screen.getByTestId('config-history-panel')).toHaveTextContent(defaultSelectedProfile);
    });

    it('disables History button when canViewHistory is false', () => {
      renderActionsBar({ canViewHistory: false });

      expect(screen.getByRole('button', { name: /history/i })).toBeDisabled();
    });
  });

  describe('Mark as Verified', () => {
    it('renders Mark as Verified button when versionStatus is a mismatch', () => {
      renderActionsBar({}, 'game_updated');

      expect(screen.getByRole('button', { name: /mark as verified/i })).toBeInTheDocument();
    });

    it('does not render Mark as Verified when versionStatus is null', () => {
      renderActionsBar({}, null);

      expect(screen.queryByRole('button', { name: /mark as verified/i })).not.toBeInTheDocument();
    });

    it('does not render Mark as Verified when versionStatus is matched', () => {
      renderActionsBar({}, 'matched');

      expect(screen.queryByRole('button', { name: /mark as verified/i })).not.toBeInTheDocument();
    });

    it('calls acknowledgeVersionChange when Mark as Verified is clicked', async () => {
      const user = userEvent.setup();

      renderActionsBar({}, 'trainer_changed');

      await user.click(screen.getByRole('button', { name: /mark as verified/i }));

      await waitFor(() => {
        expect(acknowledgeVersionChangeMock).toHaveBeenCalledWith(defaultSelectedProfile, expect.any(Function));
      });
    });

    it('renders Mark as Verified for both_changed status', () => {
      renderActionsBar({}, 'both_changed');

      expect(screen.getByRole('button', { name: /mark as verified/i })).toBeInTheDocument();
    });
  });
});
