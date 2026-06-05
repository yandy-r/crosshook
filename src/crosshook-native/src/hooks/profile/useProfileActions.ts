/**
 * useProfileActions — shared action-bar logic for profile surfaces.
 *
 * Extracts the save / duplicate / delete / TOML-preview / community-export /
 * config-history / mark-verified handlers, their busy flags, error state, and
 * `can*` guard derivations from `useProfilesPageState` so the same logic can be
 * reused by both the legacy ProfilesPage route and the Hero Detail Profiles tab.
 *
 * Design constraints:
 * - Reads from ProfileContext and ProfileHealthContext — no hard dependency on
 *   ProfilesPage-specific state.
 * - Accepts `setPendingLauncherReExport` as a parameter since that's caller-owned
 *   cross-cutting UI state (launcher re-export banner after rename).
 * - The rename modal / toast / undo / F2 wiring is delegated to
 *   `useProfileNotifications` and re-exported for callers.
 * - Single persistence path: context mutators / existing IPC hook surfaces only.
 *   No raw `invoke('profile_save*')` calls.
 */
import { useCallback, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import { useProfileContext } from '../../context/ProfileContext';
import { useProfileHealthContext } from '../../context/ProfileHealthContext';
import { chooseSaveFile } from '../../utils/dialog';
import type { CommunityExportResult } from '../useCommunityProfiles';
import { suggestedCommunityExportFilename } from './communityExport';
import { useProfileNotifications } from './useProfileNotifications';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface UseProfileActionsOptions {
  /**
   * Setter for the launcher re-export pending flag.  Owned by the caller (page
   * or parent hook) because it may also be set by external rename events.
   */
  setPendingLauncherReExport: (value: boolean) => void;
}

export interface UseProfileActionsResult {
  // --- can-* guard derivations ---
  canSave: boolean;
  canDelete: boolean;
  canDuplicate: boolean;
  canRename: boolean;
  canPreview: boolean;
  canExportCommunity: boolean;
  canViewHistory: boolean;

  // --- TOML preview ---
  previewing: boolean;
  previewError: string | null;
  showProfilePreview: boolean;
  profilePreviewContent: string;
  profilePreviewHooksStripped: boolean;
  profileHasConfiguredHooks: boolean;
  handlePreviewProfile: () => Promise<void>;
  handleIncludeHooksInPreview: () => Promise<void>;
  handleCloseProfilePreview: () => void;

  // --- Community export ---
  exportingCommunity: boolean;
  communityExportError: string | null;
  communityExportSuccess: string | null;
  handleExportCommunityProfile: () => Promise<void>;

  // --- Save / refresh ---
  handleSave: () => Promise<void>;
  handleRefreshStatus: () => Promise<void>;
  handleAfterRollback: (name: string) => void;

  // --- History panel ---
  showHistoryPanel: boolean;
  setShowHistoryPanel: (open: boolean) => void;

  // --- Wizard ---
  showWizard: boolean;
  wizardMode: 'create' | 'edit';
  openWizard: (mode: 'create' | 'edit') => void;
  setShowWizard: (open: boolean) => void;

  // --- Rename modal / toast / F2 (delegated to useProfileNotifications) ---
  canConfirmRename: boolean;
  dismissHealthBanner: () => void;
  dismissRenameToast: () => void;
  handleRenameConfirm: (oldName: string, newName: string) => void;
  healthBannerDismissed: boolean;
  pendingRename: string | null;
  renameError: string | null;
  renameInputRef: React.RefObject<HTMLInputElement>;
  renameNameTrimmed: string;
  renameToast: import('./profileNotificationConstants').RenameToast | null;
  renameToastDismissed: boolean;
  renameValue: string;
  setPendingRename: (name: string | null) => void;
  setRenameValue: (value: string) => void;
  undoRename: () => void;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useProfileActions({ setPendingLauncherReExport }: UseProfileActionsOptions): UseProfileActionsResult {
  const {
    deleting,
    dirty: _dirty,
    duplicateProfile: _duplicateProfile,
    duplicating,
    loading,
    pendingDelete,
    profile,
    profileExists,
    profileName,
    profiles,
    refreshProfiles,
    renameProfile,
    renaming,
    saveProfile,
    saving,
    selectedProfile,
  } = useProfileContext();

  const { batchValidate, revalidateSingle } = useProfileHealthContext();

  // --- can-* guard derivations ---
  const canSave =
    profileName.trim().length > 0 && profile.game.executable_path.trim().length > 0 && !saving && !deleting && !loading;
  const canDelete = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
  const canDuplicate = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
  const canRename = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
  const canPreview = profileName.trim().length > 0 && !loading;

  // --- TOML preview state ---
  const [previewing, setPreviewing] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [showProfilePreview, setShowProfilePreview] = useState(false);
  const [profilePreviewContent, setProfilePreviewContent] = useState('');
  const [profilePreviewHooksStripped, setProfilePreviewHooksStripped] = useState(false);

  const profileHasConfiguredHooks =
    (profile.pre_launch_hooks?.length ?? 0) > 0 || (profile.post_exit_hooks?.length ?? 0) > 0;

  // --- Community export state ---
  const [exportingCommunity, setExportingCommunity] = useState(false);
  const [communityExportError, setCommunityExportError] = useState<string | null>(null);
  const [communityExportSuccess, setCommunityExportSuccess] = useState<string | null>(null);

  // --- History panel state ---
  const [showHistoryPanel, setShowHistoryPanel] = useState(false);

  // --- Wizard state ---
  const [showWizard, setShowWizard] = useState(false);
  const [wizardMode, setWizardMode] = useState<'create' | 'edit'>('create');

  // Derived can-* that depend on local state
  const canExportCommunity =
    profileExists && !saving && !deleting && !loading && !duplicating && !renaming && !exportingCommunity;
  const canViewHistory =
    Boolean(selectedProfile.trim()) &&
    profiles.includes(selectedProfile.trim()) &&
    !saving &&
    !deleting &&
    !loading &&
    !duplicating &&
    !renaming &&
    !exportingCommunity;

  // --- Rename notifications (F2 / modal / toast / undo) ---
  const notifications = useProfileNotifications({
    canRename,
    hasPendingDelete: pendingDelete !== null,
    profiles,
    renaming,
    renameProfile,
    selectedProfile,
    setPendingLauncherReExport,
  });

  // --- Handlers ---

  const handleSave = useCallback(async () => {
    await saveProfile();
    if (profileName.trim()) {
      void revalidateSingle(profileName.trim());
    }
  }, [profileName, revalidateSingle, saveProfile]);

  const handleAfterRollback = useCallback(
    (name: string) => {
      void revalidateSingle(name);
    },
    [revalidateSingle]
  );

  const handleExportCommunityProfile = useCallback(async () => {
    const nameOnDisk = selectedProfile.trim();
    if (!nameOnDisk || !profiles.includes(nameOnDisk)) {
      setCommunityExportError('Save the profile before exporting as a community manifest.');
      setCommunityExportSuccess(null);
      return;
    }

    setCommunityExportError(null);
    setCommunityExportSuccess(null);

    const outputPath = await chooseSaveFile('Export community profile', {
      defaultPath: suggestedCommunityExportFilename(nameOnDisk),
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });

    if (outputPath === null) {
      return;
    }

    setExportingCommunity(true);
    try {
      const result = await callCommand<CommunityExportResult>('community_export_profile', {
        profile_name: nameOnDisk,
        output_path: outputPath,
      });
      setCommunityExportSuccess(`Community profile saved to ${result.output_path}`);
      setCommunityExportError(null);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error('Community profile export failed:', err);
      setCommunityExportError(message);
      setCommunityExportSuccess(null);
    } finally {
      setExportingCommunity(false);
    }
  }, [profiles, selectedProfile]);

  const handlePreviewProfile = useCallback(async () => {
    setPreviewing(true);
    setPreviewError(null);
    try {
      const toml = await callCommand<string>('profile_export_toml', {
        name: profileName,
        data: profile,
        include_hooks: false,
      });
      setProfilePreviewContent(toml);
      setProfilePreviewHooksStripped(profileHasConfiguredHooks);
      setPreviewError(null);
      setShowProfilePreview(true);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error('Profile preview failed:', err);
      setPreviewError(message);
    } finally {
      setPreviewing(false);
    }
  }, [profile, profileHasConfiguredHooks, profileName]);

  const handleIncludeHooksInPreview = useCallback(async () => {
    setPreviewing(true);
    setPreviewError(null);
    try {
      const toml = await callCommand<string>('profile_export_toml', {
        name: profileName,
        data: profile,
        include_hooks: true,
      });
      setProfilePreviewContent(toml);
      setProfilePreviewHooksStripped(false);
      setPreviewError(null);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error('Profile preview failed:', err);
      setPreviewError(message);
    } finally {
      setPreviewing(false);
    }
  }, [profile, profileName]);

  const handleCloseProfilePreview = useCallback(() => {
    setShowProfilePreview(false);
    setProfilePreviewHooksStripped(false);
    setPreviewError(null);
  }, []);

  const handleRefreshStatus = useCallback(async () => {
    await refreshProfiles();
    await batchValidate();
  }, [batchValidate, refreshProfiles]);

  const openWizard = useCallback((mode: 'create' | 'edit') => {
    setWizardMode(mode);
    setShowWizard(true);
  }, []);

  return {
    // can-* guards
    canSave,
    canDelete,
    canDuplicate,
    canRename,
    canPreview,
    canExportCommunity,
    canViewHistory,

    // TOML preview
    previewing,
    previewError,
    showProfilePreview,
    profilePreviewContent,
    profilePreviewHooksStripped,
    profileHasConfiguredHooks,
    handlePreviewProfile,
    handleIncludeHooksInPreview,
    handleCloseProfilePreview,

    // Community export
    exportingCommunity,
    communityExportError,
    communityExportSuccess,
    handleExportCommunityProfile,

    // Save / refresh
    handleSave,
    handleRefreshStatus,
    handleAfterRollback,

    // History panel
    showHistoryPanel,
    setShowHistoryPanel,

    // Wizard
    showWizard,
    wizardMode,
    openWizard,
    setShowWizard,

    // Rename (delegated)
    ...notifications,
  };
}
