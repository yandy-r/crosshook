import type { ConfigRevisionSummary } from '../../types/profile-history';
import { formatRelativeTime } from '../../utils/format';
import { formatExactDate, SOURCE_LABELS } from './helpers';

interface RevisionTimelineProps {
  revisions: ConfigRevisionSummary[];
  selectedRevision: ConfigRevisionSummary | null;
  profileName: string;
  loading: boolean;
  error: string | null;
  onSelectRevision: (revision: ConfigRevisionSummary) => void;
}

/**
 * Timeline list of config revision snapshots.
 * Shows loading state, error state, empty state, or the list of revision items.
 */
export function RevisionTimeline({
  revisions,
  selectedRevision,
  profileName,
  loading,
  error,
  onSelectRevision,
}: RevisionTimelineProps) {
  if (loading) {
    return (
      <div
        className="crosshook-history-timeline"
        role="listbox"
        aria-label="Revision history"
        aria-orientation="vertical"
      >
        <div className="crosshook-history-empty">
          <span className="crosshook-muted">Loading history…</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div
        className="crosshook-history-timeline"
        role="listbox"
        aria-label="Revision history"
        aria-orientation="vertical"
      >
        <div className="crosshook-history-empty">
          <p className="crosshook-danger" style={{ margin: 0 }}>
            Couldn't load configuration history.
          </p>
          <p className="crosshook-help-text" style={{ marginTop: 6 }}>
            {error}
          </p>
        </div>
      </div>
    );
  }

  if (revisions.length === 0) {
    return (
      <div
        className="crosshook-history-timeline"
        role="listbox"
        aria-label="Revision history"
        aria-orientation="vertical"
      >
        <div className="crosshook-history-empty">
          <p style={{ margin: 0, fontWeight: 600 }}>No snapshots yet</p>
          <p className="crosshook-help-text" style={{ marginTop: 6 }}>
            Snapshots are created when you save or when changes are auto-captured.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div
      className="crosshook-history-timeline"
      role="listbox"
      aria-label="Revision history"
      aria-orientation="vertical"
    >
      {revisions.map((rev) => (
        <button
          key={rev.id}
          type="button"
          role="option"
          aria-selected={selectedRevision?.id === rev.id}
          className={
            'crosshook-history-timeline-item' +
            (selectedRevision?.id === rev.id ? ' crosshook-history-timeline-item--selected' : '')
          }
          onClick={() => onSelectRevision(rev)}
        >
          <div className="crosshook-history-timeline-item__header">
            <span className="crosshook-history-badge">{SOURCE_LABELS[rev.source] ?? rev.source}</span>
            {rev.is_last_known_working && (
              <span className="crosshook-history-badge crosshook-history-badge--known-good">Known good</span>
            )}
          </div>
          <div className="crosshook-history-timeline-item__time" title={formatExactDate(rev.created_at)}>
            {formatRelativeTime(rev.created_at)}
          </div>
          {rev.profile_name_at_write !== profileName && (
            <div className="crosshook-history-timeline-item__oldname crosshook-muted">
              was: {rev.profile_name_at_write}
            </div>
          )}
        </button>
      ))}
    </div>
  );
}
