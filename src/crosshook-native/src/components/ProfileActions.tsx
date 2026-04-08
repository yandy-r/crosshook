import { useState } from 'react';
import { callCommand } from '@/lib/ipc';

import { useProfileContext } from '../context/ProfileContext';
import { useProfileHealthContext } from '../context/ProfileHealthContext';
import type { VersionCorrelationStatus } from '../types/version';

/**
 * Props for the profile action bar (Save, Duplicate, Delete buttons and status indicator).
 *
 * The `canDuplicate` and `duplicating` props were added for the profile duplication
 * feature (#56). `canDuplicate` should be true when a saved profile is selected;
 * `duplicating` is true while the backend IPC call is in-flight.
 *
 * Community export (#55): `canExportCommunity` when a saved profile exists; `exportingCommunity`
 * while the export IPC call is in-flight.
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
  /** True when a saved profile is selected and eligible for preview. */
  canPreview: boolean;
  /** True while the profile TOML export is in-flight. Disables the Preview button. */
  previewing: boolean;
  /** True when a saved profile is selected and community JSON export is allowed. */
  canExportCommunity: boolean;
  /** True while the `community_export_profile` IPC call is in-flight. */
  exportingCommunity: boolean;
  /** True when a saved profile is selected and config history can be viewed. */
  canViewHistory: boolean;
  onSave: () => void | Promise<void>;
  onDelete: () => void | Promise<void>;
  /** Initiates profile duplication via the backend. */
  onDuplicate: () => void | Promise<void>;
  /** Opens the rename modal for the current profile. */
  onRename: () => void | Promise<void>;
  /** Exports the profile as TOML and shows a preview modal. */
  onPreview: () => void | Promise<void>;
  /** Exports the profile as community-shareable JSON (save dialog + backend). */
  onExportCommunity: () => void | Promise<void>;
  /** Opens the configuration history panel for the current profile. */
  onViewHistory: () => void;
  /** Profiles page footer: tighter top spacing for the pinned action bar. */
  layoutVariant?: 'default' | 'footer';
}

const VERSION_MISMATCH_STATUSES = new Set<VersionCorrelationStatus>([
  'game_updated',
  'trainer_changed',
  'both_changed',
]);

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
  canPreview,
  previewing,
  canExportCommunity,
  exportingCommunity,
  canViewHistory,
  onSave,
  onDelete,
  onDuplicate,
  onRename,
  onPreview,
  onExportCommunity,
  onViewHistory,
  layoutVariant = 'default',
}: ProfileActionsProps) {
  const { selectedProfile } = useProfileContext();
  const { healthByName, revalidateSingle } = useProfileHealthContext();
  const [markingVerified, setMarkingVerified] = useState(false);

  const versionStatus = healthByName[selectedProfile]?.metadata?.version_status;
  const showMarkVerified = versionStatus != null && VERSION_MISMATCH_STATUSES.has(versionStatus);

  const handleMarkVerified = async () => {
    setMarkingVerified(true);
    try {
      await callCommand('acknowledge_version_change', { name: selectedProfile });
      try {
        await revalidateSingle(selectedProfile);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        console.error('Failed to refresh profile health after acknowledge_version_change', error);
        window.alert(`Version change was acknowledged, but health data refresh failed: ${message}`);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error('Failed to acknowledge version change', error);
      window.alert(`Could not mark profile as verified: ${message}`);
    } finally {
      setMarkingVerified(false);
    }
  };

  return (
    <div className={layoutVariant === 'footer' ? 'crosshook-profile-actions crosshook-profile-actions--footer' : 'crosshook-profile-actions'}>
      <div className="crosshook-profile-actions__toolbar">
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
          onClick={() => void onPreview()}
          disabled={!canPreview || previewing}
        >
          {previewing ? 'Loading Preview...' : 'Preview Profile'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={() => void onExportCommunity()}
          disabled={!canExportCommunity || exportingCommunity}
        >
          {exportingCommunity ? 'Exporting...' : 'Export as Community Profile'}
        </button>
        {showMarkVerified && (
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => void handleMarkVerified()}
            disabled={markingVerified}
          >
            {markingVerified ? 'Verifying...' : 'Mark as Verified'}
          </button>
        )}
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={onViewHistory}
          disabled={!canViewHistory}
        >
          History
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
