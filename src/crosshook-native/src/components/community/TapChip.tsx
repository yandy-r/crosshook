import type { CommunityTapSubscription } from '../../hooks/useCommunityProfiles';

export interface TapChipProps {
  tap: CommunityTapSubscription;
  onRemove: (tap: CommunityTapSubscription) => void;
  onPin: (tap: CommunityTapSubscription) => void;
  onUnpin: (tap: CommunityTapSubscription) => void;
  headCommit?: string;
  busy: boolean;
}

export function TapChip({ tap, onRemove, onPin, onUnpin, headCommit, busy }: TapChipProps) {
  const shortPinnedCommit = tap.pinned_commit ? tap.pinned_commit.slice(0, 12) : null;
  const shortHeadCommit = headCommit ? headCommit.slice(0, 12) : null;

  return (
    <div className="crosshook-community-tap">
      <div className="crosshook-community-tap__meta">
        <strong className="crosshook-community-tap__url">{tap.url}</strong>
        <span className="crosshook-community-tap__branch">
          {tap.branch ? `Branch: ${tap.branch}` : 'Default branch'}
        </span>
        <span className="crosshook-community-tap__branch">
          {shortPinnedCommit ? `Pinned: ${shortPinnedCommit}` : `Tracking: ${shortHeadCommit ?? 'unsynced'}`}
        </span>
      </div>
      <div className="crosshook-community-browser__button-row">
        {tap.pinned_commit ? (
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => onUnpin(tap)}
            disabled={busy}
          >
            Unpin
          </button>
        ) : (
          <>
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              onClick={() => onPin(tap)}
              disabled={busy || !headCommit}
              title={headCommit ? 'Pin this tap to the currently synced commit' : undefined}
              aria-describedby={!headCommit ? `tap-pin-hint-${btoa(tap.url)}` : undefined}
            >
              Pin to Current Version
            </button>
            {!headCommit && (
              <span id={`tap-pin-hint-${btoa(tap.url)}`} className="crosshook-muted" style={{ fontSize: '0.85em' }}>
                Sync taps first to capture a commit
              </span>
            )}
          </>
        )}
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={() => onRemove(tap)}
          disabled={busy}
        >
          Remove
        </button>
      </div>
    </div>
  );
}

export default TapChip;
