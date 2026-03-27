/**
 * Props for the profile action bar (Save, Duplicate, Delete buttons and status indicator).
 *
 * The `canDuplicate` and `duplicating` props were added for the profile duplication
 * feature (#56). `canDuplicate` should be true when a saved profile is selected;
 * `duplicating` is true while the backend IPC call is in-flight.
 */
export interface ProfileActionsProps {
  dirty: boolean;
  loading: boolean;
  saving: boolean;
  deleting: boolean;
  /** True while the `profile_duplicate` IPC call is in-flight. Disables the Duplicate button. */
  duplicating: boolean;
  /** True while the `profile_rename` IPC call is in-flight. Disables the Rename button. */
  renaming: boolean;
  error: string | null;
  canSave: boolean;
  canDelete: boolean;
  /** True when a saved profile is selected and eligible for duplication. */
  canDuplicate: boolean;
  /** True when a saved profile is selected and eligible for renaming. */
  canRename: boolean;
  onSave: () => void | Promise<void>;
  onDelete: () => void | Promise<void>;
  /** Initiates profile duplication via the backend. */
  onDuplicate: () => void | Promise<void>;
  /** Opens the rename modal for the current profile. */
  onRename: () => void | Promise<void>;
}

export function ProfileActions({
  dirty,
  loading,
  saving,
  deleting,
  duplicating,
  renaming,
  error,
  canSave,
  canDelete,
  canDuplicate,
  canRename,
  onSave,
  onDelete,
  onDuplicate,
  onRename,
}: ProfileActionsProps) {
  return (
    <div>
      <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap', marginTop: 18 }}>
        <button type="button" className="crosshook-button" onClick={() => void onSave()} disabled={!canSave}>
          {saving ? 'Saving...' : 'Save'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={() => void onDuplicate()}
          disabled={!canDuplicate || duplicating}
        >
          {duplicating ? 'Duplicating...' : 'Duplicate'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={() => void onRename()}
          disabled={!canRename || renaming}
        >
          {renaming ? 'Renaming...' : 'Rename'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={() => void onDelete()}
          disabled={!canDelete}
        >
          {deleting ? 'Deleting...' : 'Delete'}
        </button>
        <div style={{ display: 'flex', alignItems: 'center', color: dirty ? '#ffd166' : '#9bb1c8' }}>
          {loading ? 'Loading...' : dirty ? 'Unsaved changes' : 'No unsaved changes'}
        </div>
      </div>

      {error ? <div className="crosshook-error-banner crosshook-error-banner--section">{error}</div> : null}
    </div>
  );
}

export default ProfileActions;
