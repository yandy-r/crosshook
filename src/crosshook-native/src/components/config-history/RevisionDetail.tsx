import { useId, useState } from 'react';
import type { ConfigDiffMode, ConfigDiffResult, ConfigRevisionSummary } from '../../types/profile-history';
import { DiffView } from './DiffView';
import { formatExactDate, SOURCE_LABELS } from './helpers';

interface RevisionDetailProps {
  revision: ConfigRevisionSummary;
  diff: ConfigDiffResult | null;
  diffLoading: boolean;
  diffError: string | null;
  diffMode: ConfigDiffMode;
  onDiffModeChange: (mode: ConfigDiffMode) => void;
  markingKnownGood: boolean;
  markKnownGoodError: string | null;
  onRestore: () => void;
  onMarkKnownGood: () => void;
}

/**
 * Detail panel showing the selected revision's metadata, diff, and action buttons.
 */
export function RevisionDetail({
  revision,
  diff,
  diffLoading,
  diffError,
  diffMode,
  onDiffModeChange,
  markingKnownGood,
  markKnownGoodError,
  onRestore,
  onMarkKnownGood,
}: RevisionDetailProps) {
  const [showUnchangedSections, setShowUnchangedSections] = useState(false);
  const diffModeGroupId = useId();
  const collapseToggleId = useId();

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

      <div
        className="crosshook-history-diff-controls"
        style={{ display: 'flex', gap: 12, flexWrap: 'wrap', marginBottom: 8 }}
      >
        <fieldset
          className="crosshook-history-diff-mode"
          aria-labelledby={`${diffModeGroupId}-label`}
          style={{ border: 'none', padding: 0, margin: 0 }}
        >
          <legend id={`${diffModeGroupId}-label`} className="crosshook-label" style={{ marginBottom: 4 }}>
            Diff view
          </legend>
          <div style={{ display: 'flex', gap: 8 }}>
            <label className="crosshook-help-text">
              <input
                type="radio"
                name={`${diffModeGroupId}-mode`}
                checked={diffMode === 'unified'}
                onChange={() => onDiffModeChange('unified')}
              />{' '}
              Unified
            </label>
            <label className="crosshook-help-text">
              <input
                type="radio"
                name={`${diffModeGroupId}-mode`}
                checked={diffMode === 'semantic'}
                onChange={() => onDiffModeChange('semantic')}
              />{' '}
              Semantic
            </label>
          </div>
        </fieldset>

        {diffMode === 'unified' ? (
          <label className="crosshook-help-text" htmlFor={collapseToggleId}>
            <input
              id={collapseToggleId}
              type="checkbox"
              checked={showUnchangedSections}
              onChange={(event) => setShowUnchangedSections(event.target.checked)}
              aria-label="Show unchanged sections in unified diff"
            />{' '}
            Show unchanged sections
          </label>
        ) : null}
      </div>

      <div className="crosshook-history-diff-area">
        {diffLoading ? (
          <span className="crosshook-muted">Loading diff…</span>
        ) : diffError ? (
          <p className="crosshook-danger" role="alert">
            {diffError}
          </p>
        ) : diff ? (
          <DiffView diff={diff} mode={diffMode} showUnchangedSections={showUnchangedSections} />
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
