import type { VersionCorrelationStatus } from '../../types';

interface LaunchPanelVersionStatusProps {
  versionStatus: VersionCorrelationStatus | null;
  onAcknowledge: () => void;
  busy: boolean;
}

function versionMismatchMessage(versionStatus: VersionCorrelationStatus | null): string {
  if (versionStatus === 'both_changed') {
    return 'Game and trainer have both changed since last successful launch';
  }
  if (versionStatus === 'trainer_changed') {
    return 'Trainer has changed since last successful launch';
  }
  return 'Game version has changed since last successful launch';
}

export function LaunchPanelVersionStatus({ versionStatus, onAcknowledge, busy }: LaunchPanelVersionStatusProps) {
  const hasVersionMismatch =
    versionStatus === 'game_updated' || versionStatus === 'trainer_changed' || versionStatus === 'both_changed';

  const isUpdateInProgress = versionStatus === 'update_in_progress';

  if (hasVersionMismatch) {
    return (
      <div
        className="crosshook-launch-panel__feedback"
        data-kind="version"
        data-severity="warning"
        role="alert"
        aria-live="polite"
      >
        <div className="crosshook-launch-panel__feedback-header">
          <span className="crosshook-launch-panel__feedback-badge">Warning</span>
          <p className="crosshook-launch-panel__feedback-title">{versionMismatchMessage(versionStatus)}</p>
        </div>
        <div className="crosshook-launch-panel__feedback-actions">
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary crosshook-launch-panel__feedback-action"
            onClick={() => void onAcknowledge()}
            disabled={busy}
          >
            {busy ? 'Verifying\u2026' : 'Mark as Verified'}
          </button>
        </div>
      </div>
    );
  }

  if (isUpdateInProgress) {
    return (
      <div className="crosshook-launch-panel__feedback" data-kind="version" data-severity="info" role="status">
        <p className="crosshook-launch-panel__feedback-title">Steam update in progress \u2014 version check skipped</p>
      </div>
    );
  }

  return null;
}
