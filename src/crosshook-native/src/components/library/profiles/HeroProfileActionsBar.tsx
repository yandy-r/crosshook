/**
 * HeroProfileActionsBar — action bar for the Hero Detail Profiles tab.
 *
 * Renders the Duplicate / Rename / Delete / TOML Preview / Community Export /
 * Config History / Mark as Verified buttons for the in-shell profile editor,
 * plus the accompanying overlay surfaces (delete confirm, rename modal, rename
 * toast, TOML preview modal, config history panel).
 *
 * All profile writes route through `useProfileActions` / context mutators —
 * no raw `invoke('profile_save*')` calls are made here.
 *
 * Rename-pause contract: the autosave in `useHeroProfilesAutosave` pauses
 * whenever `profileName !== selectedProfile`. `pendingRename` is populated by
 * `useProfilesPageNotifications` (via `useProfileActions`) before the modal
 * opens, which diverges the names and satisfies the autosave guard automatically.
 *
 * F2 shortcut: registered via `useProfilesPageNotifications` (called inside
 * `useProfileActions`). The listener attaches to `document` and is cleaned up
 * on component unmount — same lifecycle as the legacy ProfilesPage approach.
 */
import type { ComponentProps } from 'react';
import { useProfileContext } from '@/context/ProfileContext';
import { useProfileHealthContext } from '@/context/ProfileHealthContext';
import type { UseProfileActionsResult } from '@/hooks/profile/useProfileActions';
import {
  presentAcknowledgeVersionChangeOutcome,
  useAcknowledgeVersionChange,
} from '@/hooks/useAcknowledgeVersionChange';
import type { VersionCorrelationStatus } from '@/types/version';
import type { ConfigHistoryPanel } from '../../ConfigHistoryPanel';
import { ProfilesOverlays } from './ProfilesOverlays';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type HistoryHandlers = Pick<
  ComponentProps<typeof ConfigHistoryPanel>,
  'fetchConfigHistory' | 'fetchConfigDiff' | 'rollbackConfig' | 'markKnownGood'
>;

export interface HeroProfileActionsBarProps {
  /** All action state / handlers from useProfileActions. */
  actions: UseProfileActionsResult;
  /** Called after a successful rollback so health data can be refreshed. */
  onAfterRollback: (name: string) => void;
  /** Current version-correlation status, used to decide Mark as Verified visibility. */
  versionStatus: VersionCorrelationStatus | null | undefined;
  /** Config-history IPC handlers surfaced by ProfileContext. */
  historyHandlers: HistoryHandlers;
}

const VERSION_MISMATCH_STATUSES = new Set<VersionCorrelationStatus>([
  'game_updated',
  'trainer_changed',
  'both_changed',
]);

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function HeroProfileActionsBar({
  actions,
  onAfterRollback,
  versionStatus,
  historyHandlers,
}: HeroProfileActionsBarProps) {
  const {
    selectedProfile,
    profileName,
    pendingDelete,
    deleting,
    duplicateProfile,
    duplicating,
    renaming,
    confirmDelete,
    executeDelete,
    cancelDelete,
  } = useProfileContext();

  const { revalidateSingle } = useProfileHealthContext();
  const { acknowledgeVersionChange, busy: markingVerified } = useAcknowledgeVersionChange();

  const {
    // can-* guards
    canDelete,
    canDuplicate,
    canRename,
    canPreview,
    canExportCommunity,
    canViewHistory,

    // busy flags
    previewing,
    exportingCommunity,

    // errors / success
    previewError,
    communityExportError,
    communityExportSuccess,

    // TOML preview state
    showProfilePreview,
    profilePreviewContent,
    profilePreviewHooksStripped,
    handlePreviewProfile,
    handleIncludeHooksInPreview,
    handleCloseProfilePreview,
    handleExportCommunityProfile,

    // History panel
    showHistoryPanel,
    setShowHistoryPanel,

    // Rename modal / toast / F2 (from useProfilesPageNotifications via spread)
    canConfirmRename,
    pendingRename,
    renameError,
    renameInputRef,
    renameNameTrimmed,
    renameToast,
    renameToastDismissed,
    renameValue,
    setPendingRename,
    setRenameValue,
    dismissRenameToast,
    handleRenameConfirm,
    undoRename,
  } = actions;

  const showMarkVerified = versionStatus != null && VERSION_MISMATCH_STATUSES.has(versionStatus);

  const handleMarkVerified = async () => {
    if (!selectedProfile.trim()) return;
    const outcome = await acknowledgeVersionChange(selectedProfile, revalidateSingle);
    presentAcknowledgeVersionChangeOutcome(outcome);
  };

  const handleDuplicate = async () => {
    if (!selectedProfile.trim() || !canDuplicate || duplicating) return;
    await duplicateProfile(selectedProfile);
  };

  const handleDelete = async () => {
    if (!selectedProfile.trim() || !canDelete || deleting) return;
    await confirmDelete(selectedProfile);
  };

  const secondaryActionClass = 'crosshook-button crosshook-button--secondary crosshook-button--small';
  const dangerActionClass = 'crosshook-button crosshook-button--danger crosshook-button--small';

  return (
    <>
      <div className="crosshook-hero-detail__profile-actions" role="toolbar" aria-label="Profile actions">
        <div className="crosshook-hero-detail__profile-actions-group" role="group" aria-label="Edit profile">
          <button
            type="button"
            className={secondaryActionClass}
            onClick={() => void handleDuplicate()}
            disabled={!canDuplicate || duplicating}
          >
            {duplicating ? 'Duplicating…' : 'Duplicate'}
          </button>

          <button
            type="button"
            className={secondaryActionClass}
            onClick={() => {
              if (!canRename || !selectedProfile) return;
              setPendingRename(selectedProfile);
              setRenameValue(selectedProfile);
            }}
            disabled={!canRename || renaming}
          >
            {renaming ? 'Renaming…' : 'Rename'}
          </button>
        </div>

        <div className="crosshook-hero-detail__profile-actions-divider" role="presentation" aria-hidden="true" />

        <div className="crosshook-hero-detail__profile-actions-group" role="group" aria-label="Preview and export">
          <button
            type="button"
            className={secondaryActionClass}
            onClick={() => void handlePreviewProfile()}
            disabled={!canPreview || previewing}
            aria-label={previewing ? 'Loading preview' : 'Preview profile'}
          >
            {previewing ? 'Loading…' : 'Preview'}
          </button>

          <button
            type="button"
            className={secondaryActionClass}
            onClick={() => void handleExportCommunityProfile()}
            disabled={!canExportCommunity || exportingCommunity}
            aria-label={exportingCommunity ? 'Exporting community profile' : 'Export as community profile'}
            title="Export as Community Profile"
          >
            {exportingCommunity ? 'Exporting…' : 'Community Export'}
          </button>
        </div>

        {showMarkVerified ? (
          <>
            <div className="crosshook-hero-detail__profile-actions-divider" role="presentation" aria-hidden="true" />

            <div
              className="crosshook-hero-detail__profile-actions-group"
              role="group"
              aria-label="Version verification"
            >
              <button
                type="button"
                className={secondaryActionClass}
                onClick={() => void handleMarkVerified()}
                disabled={markingVerified}
              >
                {markingVerified ? 'Verifying…' : 'Mark as Verified'}
              </button>
            </div>
          </>
        ) : null}

        <div className="crosshook-hero-detail__profile-actions-divider" role="presentation" aria-hidden="true" />

        <div className="crosshook-hero-detail__profile-actions-group" role="group" aria-label="Configuration history">
          <button
            type="button"
            className={secondaryActionClass}
            onClick={() => setShowHistoryPanel(true)}
            disabled={!canViewHistory}
          >
            History
          </button>
        </div>

        <div
          className="crosshook-hero-detail__profile-actions-group crosshook-hero-detail__profile-actions-group--trailing"
          role="group"
          aria-label="Delete profile"
        >
          <button
            type="button"
            className={dangerActionClass}
            onClick={() => void handleDelete()}
            disabled={!canDelete || deleting}
          >
            {deleting ? 'Deleting…' : 'Delete'}
          </button>
        </div>
      </div>

      {/* Inline error / status surfaces */}
      {previewError ? (
        <p className="crosshook-danger" role="alert" style={{ marginTop: 8 }}>
          Preview failed: {previewError}
        </p>
      ) : null}

      {communityExportError ? (
        <p className="crosshook-danger" role="alert" style={{ marginTop: 8 }}>
          Community export failed: {communityExportError}
        </p>
      ) : null}

      {communityExportSuccess ? (
        <p className="crosshook-help-text" role="status" style={{ marginTop: 8 }}>
          {communityExportSuccess}
        </p>
      ) : null}

      <ProfilesOverlays
        canConfirmRename={canConfirmRename}
        pendingDelete={pendingDelete}
        pendingRename={pendingRename}
        previewContent={profilePreviewContent}
        profileName={profileName}
        previewHooksStripped={profilePreviewHooksStripped}
        previewIncludeHooksPending={previewing}
        onIncludeHooksInPreview={() => void handleIncludeHooksInPreview()}
        renameError={renameError}
        renameInputRef={renameInputRef}
        renameNameTrimmed={renameNameTrimmed}
        renameToast={renameToast}
        renameToastDismissed={renameToastDismissed}
        renameValue={renameValue}
        renaming={renaming}
        selectedProfile={selectedProfile}
        showHistoryPanel={showHistoryPanel}
        showProfilePreview={showProfilePreview}
        showWizard={false}
        wizardMode="edit"
        onAfterRollback={onAfterRollback}
        onCancelDelete={cancelDelete}
        onCloseHistory={() => setShowHistoryPanel(false)}
        onClosePreview={handleCloseProfilePreview}
        onConfirmRename={handleRenameConfirm}
        onDismissRenameToast={dismissRenameToast}
        onExecuteDelete={executeDelete}
        onSetPendingRename={setPendingRename}
        onSetRenameValue={setRenameValue}
        onToggleWizard={() => undefined}
        onUndoRename={undoRename}
        fetchConfigDiff={historyHandlers.fetchConfigDiff}
        fetchConfigHistory={historyHandlers.fetchConfigHistory}
        markKnownGood={historyHandlers.markKnownGood}
        rollbackConfig={historyHandlers.rollbackConfig}
      />
    </>
  );
}

export default HeroProfileActionsBar;
