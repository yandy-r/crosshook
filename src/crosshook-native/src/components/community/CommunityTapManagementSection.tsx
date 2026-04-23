import type { CommunityTapSubscription } from '../../hooks/useCommunityProfiles';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { TapChip } from './TapChip';

export interface CommunityTapManagementSectionProps {
  taps: CommunityTapSubscription[];
  tapUrl: string;
  tapBranch: string;
  loading: boolean;
  syncing: boolean;
  onTapUrlChange: (value: string) => void;
  onTapBranchChange: (value: string) => void;
  onAddTap: () => void;
  onRefresh: () => void;
  onSync: () => void;
  onRemoveTap: (tap: CommunityTapSubscription) => void;
  onPinTap: (tap: CommunityTapSubscription) => void;
  onUnpinTap: (tap: CommunityTapSubscription) => void;
  getTapHeadCommit: (tap: CommunityTapSubscription) => string | undefined;
}

/** Stable React key / row id for a tap subscription (same repo URL can appear on multiple branches or pins). */
function tapSubscriptionStableKey(sub: CommunityTapSubscription): string {
  return `${sub.url}::${sub.branch ?? ''}::${sub.pinned_commit ?? ''}`;
}

export function CommunityTapManagementSection({
  taps,
  tapUrl,
  tapBranch,
  loading,
  syncing,
  onTapUrlChange,
  onTapBranchChange,
  onAddTap,
  onRefresh,
  onSync,
  onRemoveTap,
  onPinTap,
  onUnpinTap,
  getTapHeadCommit,
}: CommunityTapManagementSectionProps) {
  return (
    <CollapsibleSection title="Tap Management" className="crosshook-panel crosshook-community-browser__panel">
      <div className="crosshook-community-browser__footer">
        <div className="crosshook-community-browser__section-copy">
          <p className="crosshook-muted crosshook-community-browser__helper">
            Taps are persisted in CrossHook settings and synced through the backend community commands.
          </p>
        </div>
        <div className="crosshook-community-browser__button-row">
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={onRefresh}
            disabled={loading || syncing}
          >
            Refresh Index
          </button>
          <button
            type="button"
            className="crosshook-button"
            onClick={onSync}
            disabled={loading || syncing || taps.length === 0}
          >
            {syncing ? 'Syncing...' : 'Sync Taps'}
          </button>
        </div>
      </div>

      <div className="crosshook-community-browser__toolbar">
        <div className="crosshook-community-browser__field">
          <label className="crosshook-label" htmlFor="tap-url">
            Tap URL
          </label>
          <input
            id="tap-url"
            className="crosshook-input"
            value={tapUrl}
            onChange={(event) => onTapUrlChange(event.target.value)}
            placeholder="https://github.com/example/community-profiles.git"
          />
        </div>
        <div className="crosshook-community-browser__field">
          <label className="crosshook-label" htmlFor="tap-branch">
            Branch
          </label>
          <input
            id="tap-branch"
            className="crosshook-input"
            value={tapBranch}
            onChange={(event) => onTapBranchChange(event.target.value)}
            placeholder="main"
          />
        </div>
        <button
          type="button"
          className="crosshook-button"
          onClick={onAddTap}
          disabled={loading || syncing || tapUrl.trim().length === 0}
        >
          Add Tap
        </button>
      </div>

      {taps.length > 0 ? (
        <div className="crosshook-community-browser__tap-list">
          {taps.map((tap) => (
            <TapChip
              key={tapSubscriptionStableKey(tap)}
              tap={tap}
              headCommit={getTapHeadCommit(tap)}
              busy={loading || syncing}
              onRemove={onRemoveTap}
              onPin={onPinTap}
              onUnpin={onUnpinTap}
            />
          ))}
        </div>
      ) : (
        <p className="crosshook-muted crosshook-community-browser__helper">
          Add a tap URL to populate the community browser.
        </p>
      )}
    </CollapsibleSection>
  );
}

export default CommunityTapManagementSection;
