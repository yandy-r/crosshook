import type { ReactNode } from 'react';
import { LAUNCH_PANEL_ACTION_BUTTON_STYLE } from '../../../utils/launchPanelActionButtonStyle';
import { RouteBanner } from '../../layout/RouteBanner';
import { ThemedSelect } from '../../ui/ThemedSelect';

interface ProfilesHeroProps {
  activeCollectionName: string | null;
  filteredProfiles: string[];
  hasSelectedProfile: boolean;
  healthBannerDismissed: boolean;
  healthLoading: boolean;
  selectedProfile: string;
  statusBadges: ReactNode;
  summary: { broken_count: number; total_count: number; stale_count: number } | null;
  onClearCollectionFilter: () => void;
  onDismissHealthBanner: () => void;
  onOpenEditWizard: () => void;
  onOpenNewWizard: () => void;
  onRefreshStatus: () => void | Promise<void>;
  onSelectProfile: (value: string) => void;
  optionBadgeForProfile: (name: string) => { badge?: string; badgeTitle?: string };
}

export function ProfilesHero({
  activeCollectionName,
  filteredProfiles,
  hasSelectedProfile,
  healthBannerDismissed,
  healthLoading,
  selectedProfile,
  statusBadges,
  summary,
  onClearCollectionFilter,
  onDismissHealthBanner,
  onOpenEditWizard,
  onOpenNewWizard,
  onRefreshStatus,
  onSelectProfile,
  optionBadgeForProfile,
}: ProfilesHeroProps) {
  const needsAttention = summary !== null && summary.stale_count + summary.broken_count > 0;

  return (
    <>
      {summary !== null && !healthLoading && summary.broken_count > 0 && !healthBannerDismissed ? (
        <div className="crosshook-rename-toast" role="status" aria-live="polite">
          <span>
            {summary.broken_count} profile{summary.broken_count !== 1 ? 's' : ''} have issues that may prevent launching
          </span>
          <button
            type="button"
            className="crosshook-rename-toast-dismiss"
            onClick={onDismissHealthBanner}
            aria-label="Dismiss"
          >
            &times;
          </button>
        </div>
      ) : null}

      <RouteBanner route="profiles" />
      <div className="crosshook-panel crosshook-profiles-hero-outer">
        <section className="crosshook-launch-panel crosshook-route-hero-launch-panel">
          <div className="crosshook-launch-panel__profile-row">
            <label
              className="crosshook-label"
              htmlFor="profile-selector-top"
              style={{ margin: 0, whiteSpace: 'nowrap' }}
            >
              Active Profile
            </label>
            <div className="crosshook-launch-panel__profile-row-select">
              {activeCollectionName !== null && (
                <div className="crosshook-launch-collection-filter">
                  Filtering by: <strong>{activeCollectionName}</strong>
                  <button
                    type="button"
                    className="crosshook-button crosshook-button--ghost crosshook-button--small"
                    onClick={onClearCollectionFilter}
                    aria-label="Clear collection filter"
                  >
                    ×
                  </button>
                </div>
              )}
              <ThemedSelect
                id="profile-selector-top"
                value={selectedProfile}
                onValueChange={onSelectProfile}
                placeholder="Create New"
                options={[
                  { value: '', label: 'Create New' },
                  ...filteredProfiles.map((name) => ({
                    value: name,
                    label: name,
                    ...optionBadgeForProfile(name),
                  })),
                ]}
              />
            </div>
            <div className="crosshook-launch-panel__profile-row-actions">
              <button
                type="button"
                className="crosshook-button crosshook-launch-panel__action"
                style={LAUNCH_PANEL_ACTION_BUTTON_STYLE}
                onClick={onOpenNewWizard}
              >
                New Profile
              </button>
              {hasSelectedProfile ? (
                <button
                  type="button"
                  className="crosshook-button crosshook-button--secondary crosshook-launch-panel__action crosshook-launch-panel__action--secondary"
                  style={LAUNCH_PANEL_ACTION_BUTTON_STYLE}
                  onClick={onOpenEditWizard}
                >
                  Edit in Wizard
                </button>
              ) : null}
            </div>
          </div>

          <div className="crosshook-profiles-hero-status">
            {statusBadges}
            <div className="crosshook-profiles-hero-status__action">
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary crosshook-launch-panel__action crosshook-launch-panel__action--secondary"
                style={LAUNCH_PANEL_ACTION_BUTTON_STYLE}
                onClick={() => void onRefreshStatus()}
              >
                {needsAttention ? (healthLoading ? 'Checking...' : 'Re-check') : 'Refresh'}
              </button>
            </div>
          </div>
        </section>
      </div>
    </>
  );
}
