import type { ComponentProps, RefObject } from 'react';

import ConfigHistoryPanel from '../../ConfigHistoryPanel';
import { OnboardingWizard } from '../../OnboardingWizard';
import ProfilePreviewModal from '../../ProfilePreviewModal';

interface PendingDeleteInfo {
  name: string;
  launcherInfo?: {
    desktop_entry_path: string;
    script_path: string;
  } | null;
}

interface RenameToast {
  newName: string;
  oldName: string;
}

interface ProfilesOverlaysProps {
  canConfirmRename: boolean;
  pendingDelete: PendingDeleteInfo | null;
  pendingRename: string | null;
  previewContent: string;
  profileName: string;
  renameError: string | null;
  renameInputRef: RefObject<HTMLInputElement>;
  renameNameTrimmed: string;
  renameToast: RenameToast | null;
  renameToastDismissed: boolean;
  renameValue: string;
  renaming: boolean;
  selectedProfile: string;
  showHistoryPanel: boolean;
  showProfilePreview: boolean;
  showWizard: boolean;
  wizardMode: 'create' | 'edit';
  onAfterRollback: (name: string) => void;
  onCancelDelete: () => void;
  onCloseHistory: () => void;
  onClosePreview: () => void;
  onConfirmRename: (oldName: string, newName: string) => void;
  onDismissRenameToast: () => void;
  onExecuteDelete: () => void | Promise<void>;
  onSetPendingRename: (value: string | null) => void;
  onSetRenameValue: (value: string) => void;
  onToggleWizard: (open: boolean) => void;
  onUndoRename: () => void;
  rollbackConfig: ComponentProps<typeof ConfigHistoryPanel>['rollbackConfig'];
  fetchConfigDiff: ComponentProps<typeof ConfigHistoryPanel>['fetchConfigDiff'];
  fetchConfigHistory: ComponentProps<typeof ConfigHistoryPanel>['fetchConfigHistory'];
  markKnownGood: ComponentProps<typeof ConfigHistoryPanel>['markKnownGood'];
}

export function ProfilesOverlays({
  canConfirmRename,
  pendingDelete,
  pendingRename,
  previewContent,
  profileName,
  renameError,
  renameInputRef,
  renameNameTrimmed,
  renameToast,
  renameToastDismissed,
  renameValue,
  renaming,
  selectedProfile,
  showHistoryPanel,
  showProfilePreview,
  showWizard,
  wizardMode,
  onAfterRollback,
  onCancelDelete,
  onCloseHistory,
  onClosePreview,
  onConfirmRename,
  onExecuteDelete,
  onSetPendingRename,
  onSetRenameValue,
  onToggleWizard,
  onUndoRename,
  fetchConfigDiff,
  fetchConfigHistory,
  markKnownGood,
  rollbackConfig,
  onDismissRenameToast,
}: ProfilesOverlaysProps) {
  return (
    <>
      {pendingDelete ? (
        <div className="crosshook-profile-editor-delete-overlay" data-crosshook-focus-root="modal">
          <div className="crosshook-profile-editor-delete-dialog">
            <h3 style={{ margin: '0 0 12px' }}>Delete Profile</h3>
            <p>
              Delete profile <strong>{pendingDelete.name}</strong>?
            </p>
            {pendingDelete.launcherInfo ? (
              <div className="crosshook-profile-editor-delete-warning">
                <p style={{ margin: '0 0 6px', fontWeight: 600 }}>Launcher files will also be removed:</p>
                <p style={{ margin: '2px 0', color: '#d1d5db', wordBreak: 'break-all' }}>
                  {pendingDelete.launcherInfo.script_path}
                </p>
                <p style={{ margin: '2px 0', color: '#d1d5db', wordBreak: 'break-all' }}>
                  {pendingDelete.launcherInfo.desktop_entry_path}
                </p>
              </div>
            ) : null}
            <div className="crosshook-profile-editor-delete-actions">
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                onClick={onCancelDelete}
                data-crosshook-modal-close
              >
                Cancel
              </button>
              <button
                type="button"
                className="crosshook-profile-editor-delete-confirm"
                onClick={() => void onExecuteDelete()}
              >
                {pendingDelete.launcherInfo ? 'Delete Profile and Launcher' : 'Delete Profile'}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {pendingRename !== null ? (
        <div className="crosshook-profile-editor-delete-overlay" data-crosshook-focus-root="modal">
          <div
            className="crosshook-profile-editor-delete-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="rename-dialog-heading"
            style={{ marginBottom: 'auto', marginTop: '12vh' }}
          >
            <h3 id="rename-dialog-heading" style={{ margin: '0 0 12px' }}>
              Rename Profile
            </h3>
            <div className="crosshook-field">
              <label className="crosshook-label" htmlFor="rename-profile-input">
                New Name
              </label>
              <input
                id="rename-profile-input"
                ref={renameInputRef}
                className="crosshook-input"
                value={renameValue}
                onChange={(event) => onSetRenameValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' && canConfirmRename) {
                    onConfirmRename(pendingRename, renameNameTrimmed);
                  }

                  if (event.key === 'Escape') {
                    onSetPendingRename(null);
                  }
                }}
              />
              {renameError ? (
                <p className="crosshook-danger" role="alert">
                  {renameError}
                </p>
              ) : null}
            </div>
            <div className="crosshook-profile-editor-delete-actions">
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                onClick={() => onSetPendingRename(null)}
                data-crosshook-modal-close
              >
                Cancel
              </button>
              <button
                type="button"
                className="crosshook-button"
                disabled={!canConfirmRename}
                onClick={() => onConfirmRename(pendingRename, renameNameTrimmed)}
              >
                {renaming ? 'Renaming...' : 'Rename'}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {renameToast && !renameToastDismissed ? (
        <div className="crosshook-rename-toast" role="status" aria-live="polite">
          <span>Renamed to &lsquo;{renameToast.newName}&rsquo;</span>
          <button type="button" className="crosshook-button crosshook-button--ghost" onClick={onUndoRename}>
            Undo
          </button>
          <button
            type="button"
            className="crosshook-rename-toast-dismiss"
            onClick={onDismissRenameToast}
            aria-label="Dismiss"
          >
            &times;
          </button>
        </div>
      ) : null}

      {showProfilePreview ? (
        <ProfilePreviewModal tomlContent={previewContent} profileName={profileName} onClose={onClosePreview} />
      ) : null}

      {showHistoryPanel && selectedProfile ? (
        <ConfigHistoryPanel
          profileName={selectedProfile}
          onClose={onCloseHistory}
          fetchConfigHistory={fetchConfigHistory}
          fetchConfigDiff={fetchConfigDiff}
          rollbackConfig={rollbackConfig}
          markKnownGood={markKnownGood}
          onAfterRollback={onAfterRollback}
        />
      ) : null}

      {showWizard ? (
        <OnboardingWizard
          open={showWizard}
          mode={wizardMode}
          onComplete={() => onToggleWizard(false)}
          onDismiss={() => onToggleWizard(false)}
        />
      ) : null}
    </>
  );
}
