import * as Tabs from '@radix-ui/react-tabs';
import { useLaunchStateContext } from '../../context/LaunchStateContext';
import { OfflineReadinessPanel } from '../OfflineReadinessPanel';
import { OfflineStatusBadge } from '../OfflineStatusBadge';
import type { LaunchSubTabId } from './types';

interface OfflineTabContentProps {
  activeTab: LaunchSubTabId;
}

export function OfflineTabContent({ activeTab }: OfflineTabContentProps) {
  const {
    offlineReadiness,
    offlineReadinessError,
    offlineReadinessLoading,
    launchPathWarnings,
    trainerHashUpdateBusy,
    updateStoredTrainerHash,
    dismissTrainerHashCommunityWarning,
  } = useLaunchStateContext();

  return (
    <Tabs.Content
      value="offline"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeTab === 'offline' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner">
        <div
          className="crosshook-launch-panel__offline"
          style={{ display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap', marginBottom: 12 }}
        >
          <OfflineStatusBadge report={offlineReadiness} loading={offlineReadinessLoading && !offlineReadiness} />
          {!offlineReadinessLoading && offlineReadiness ? (
            <span className="crosshook-muted" style={{ fontSize: '0.85rem' }}>
              {offlineReadiness.readiness_state.replace(/_/g, ' ')}
            </span>
          ) : null}
        </div>
        <OfflineReadinessPanel
          report={offlineReadiness}
          error={offlineReadinessError}
          loading={offlineReadinessLoading}
        />
        {launchPathWarnings.length > 0 ? (
          <ul className="crosshook-launch-panel__feedback-list" aria-label="Launch path warnings">
            {launchPathWarnings.map((issue, index) => (
              <li
                // biome-ignore lint/suspicious/noArrayIndexKey: tiebreaker when severity+code/message may collide
                key={`${issue.severity}-${issue.code ?? issue.message}-${index}`}
                className="crosshook-launch-panel__feedback-item"
              >
                <div className="crosshook-launch-panel__feedback-header">
                  <span className="crosshook-launch-panel__feedback-badge" data-severity={issue.severity}>
                    {issue.severity}
                  </span>
                  <p className="crosshook-launch-panel__feedback-title">{issue.message}</p>
                </div>
                <p className="crosshook-launch-panel__feedback-help">{issue.help}</p>
                {issue.code === 'trainer_hash_mismatch' ? (
                  <div
                    className="crosshook-launch-panel__feedback-actions"
                    style={{ marginTop: 10, display: 'flex', gap: 8, flexWrap: 'wrap' }}
                  >
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary"
                      disabled={trainerHashUpdateBusy}
                      onClick={() => void updateStoredTrainerHash()}
                    >
                      {trainerHashUpdateBusy ? 'Updating…' : 'Update stored hash'}
                    </button>
                  </div>
                ) : null}
                {issue.code === 'trainer_hash_community_mismatch' ? (
                  <div
                    className="crosshook-launch-panel__feedback-actions"
                    style={{ marginTop: 10, display: 'flex', gap: 8, flexWrap: 'wrap' }}
                  >
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary"
                      onClick={dismissTrainerHashCommunityWarning}
                    >
                      Dismiss
                    </button>
                  </div>
                ) : null}
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </Tabs.Content>
  );
}
