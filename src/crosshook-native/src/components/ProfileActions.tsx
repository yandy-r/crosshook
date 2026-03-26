export interface ProfileActionsProps {
  dirty: boolean;
  loading: boolean;
  saving: boolean;
  deleting: boolean;
  duplicating: boolean;
  error: string | null;
  canSave: boolean;
  canDelete: boolean;
  canDuplicate: boolean;
  onSave: () => void | Promise<void>;
  onDelete: () => void | Promise<void>;
  onDuplicate: () => void | Promise<void>;
}

export function ProfileActions({
  dirty,
  loading,
  saving,
  deleting,
  duplicating,
  error,
  canSave,
  canDelete,
  canDuplicate,
  onSave,
  onDelete,
  onDuplicate,
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
