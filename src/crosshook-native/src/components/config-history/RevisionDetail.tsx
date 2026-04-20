import type { ConfigDiffResult, ConfigRevisionSummary } from '../../types/profile-history';
import { DiffView } from './DiffView';
import { formatExactDate, SOURCE_LABELS } from './helpers';

interface RevisionDetailProps {
  revision: ConfigRevisionSummary;
  diff: ConfigDiffResult | null;
  diffLoading: boolean;
  diffError: string | null;
  markingKnownGood: boolean;
  markKnownGoodError: string | null;
  onRestore: () => void;
  onMarkKnownGood: () => void;
}

/**
 * Detail panel showing the selected revision's metadata, diff, and action buttons.
 * Displays the diff view when loaded, or loading/error states.
 */
export function RevisionDetail({
  revision,
  diff,
  diffLoading,
  diffError,
  markingKnownGood,
  markKnownGoodError,
  onRestore,
  onMarkKnownGood,
}: RevisionDetailProps) {
  return (
    <>
      <div className="crosshook-history-detail-header">
        <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', alignItems: 'center' }}>
          <span className="crosshook-history-badge">{SOURCE_LABELS[revision.source] ?? revision.source}</span>
          {revision.is_last_known_working && (
            <span className="crosshook-history-badge crosshook-history-badge--known-good">Known good</span>
          )}
        </div>
        <div className="crosshook-help-text" style={{ marginTop: 6 }}>
          {formatExactDate(revision.created_at)}
          {revision.source_revision_id !== null && (
            <span className="crosshook-muted"> — restored from #{revision.source_revision_id}</span>
          )}
        </div>
      </div>

      <div className="crosshook-history-diff-area">
        {diffLoading ? (
          <span className="crosshook-muted">Loading diff…</span>
        ) : diffError ? (
          <p className="crosshook-danger" role="alert">
            {diffError}
          </p>
        ) : diff ? (
          <DiffView diff={diff} />
        ) : null}
      </div>

      <div className="crosshook-history-detail-actions">
        <button type="button" className="crosshook-button" onClick={onRestore}>
          Restore snapshot
        </button>
        {!revision.is_last_known_working && (
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            disabled={markingKnownGood}
            onClick={onMarkKnownGood}
          >
            {markingKnownGood ? 'Marking…' : 'Mark as known good'}
          </button>
        )}
        {markKnownGoodError ? (
          <p className="crosshook-danger" role="alert" style={{ margin: '8px 0 0', width: '100%' }}>
            {markKnownGoodError}
          </p>
        ) : null}
      </div>
    </>
  );
}
