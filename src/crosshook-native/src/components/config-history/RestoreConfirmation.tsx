import type { ConfigRevisionSummary } from '../../types/profile-history';
import { formatExactDate, SOURCE_LABELS } from './helpers';

interface RestoreConfirmationProps {
  revision: ConfigRevisionSummary;
  restoring: boolean;
  error: string | null;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Confirmation dialog shown when the user is about to restore a config snapshot.
 * Explains that the current config will be saved before the restore.
 */
export function RestoreConfirmation({ revision, restoring, error, onConfirm, onCancel }: RestoreConfirmationProps) {
  return (
    <section className="crosshook-history-confirm" aria-label="Restore confirmation">
      <h3 style={{ margin: '0 0 12px' }}>Restore this configuration snapshot?</h3>
      <p className="crosshook-help-text" style={{ marginBottom: 16 }}>
        You're restoring the snapshot from <strong>{formatExactDate(revision.created_at)}</strong> (
        {SOURCE_LABELS[revision.source] ?? revision.source}). Your current config will be saved as a new snapshot first.
      </p>
      {error ? (
        <p className="crosshook-danger" role="alert" style={{ marginBottom: 12 }}>
          {error}
        </p>
      ) : null}
      <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
        <button type="button" className="crosshook-button" disabled={restoring} onClick={onConfirm}>
          {restoring ? 'Restoring…' : 'Restore snapshot'}
        </button>
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          disabled={restoring}
          onClick={onCancel}
        >
          Keep current config
        </button>
      </div>
    </section>
  );
}
