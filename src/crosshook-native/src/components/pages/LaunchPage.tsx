import { useLaunchSubTabsProps } from '../../hooks/launch/useLaunchSubTabsProps';
import type { AppNavigateOptions, GameDetailOrigin } from '../../types/navigation';
import LaunchPanel from '../LaunchPanel';
import { LaunchSubTabs } from '../LaunchSubTabs';
import { buildGameDetailTrail } from '../layout/game-detail-trail';
import { RouteBanner } from '../layout/RouteBanner';
import type { AppRoute } from '../layout/Sidebar';
import { LaunchDepGateModal } from './launch/LaunchDepGateModal';
import { LaunchProfileSelector } from './launch/LaunchProfileSelector';
import { useLaunchDepGate } from './launch/useLaunchDepGate';
import { useLaunchPageState } from './launch/useLaunchPageState';

// NOTE(hero-detail-consolidation): delete with Phase 10 route removal.
export interface LaunchPageProps {
  origin?: GameDetailOrigin | null;
  onNavigate?: (route: AppRoute, options?: AppNavigateOptions) => void;
}

export function LaunchPage({ origin, onNavigate }: LaunchPageProps = {}) {
  const {
    activeCollection,
    activeCollectionId,
    effectiveSteamClientInstallPath,
    filteredProfiles,
    hasSavedSelectedProfile,
    handleTogglePin,
    launchRequest,
    pinnedSet,
    profile,
    profileId,
    profileState,
    resolvedSteamAppId,
    selectedName,
    setActiveCollectionId,
    settings,
    showNetworkIsolationBadge,
  } = useLaunchPageState();

  const depGate = useLaunchDepGate({
    profile,
    selectedName,
    autoInstallPrefixDeps: settings.auto_install_prefix_deps,
  });

  const launchSubTabsProps = useLaunchSubTabsProps({
    isGamescopeRunning: depGate.isGamescopeRunning,
    resolvedSteamAppId,
    hasSavedSelectedProfile,
  });

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--launch">
      <div className="crosshook-route-stack crosshook-launch-page__grid">
        <RouteBanner route="launch" trail={buildGameDetailTrail(origin, onNavigate, 'Launch')} />
        <LaunchPanel
          profileId={profileId}
          method={profileState.launchMethod}
          request={launchRequest}
          profile={profile}
          infoSlot={
            <dl className="crosshook-dashboard-kv-list">
              <div className="crosshook-dashboard-kv-row">
                <dt className="crosshook-dashboard-kv-row__label">Selected profile</dt>
                <dd className="crosshook-dashboard-kv-row__value">
                  {selectedName.trim() !== '' ? selectedName : <span className="crosshook-muted">None selected</span>}
                </dd>
              </div>
              {effectiveSteamClientInstallPath ? (
                <div className="crosshook-dashboard-kv-row">
                  <dt className="crosshook-dashboard-kv-row__label">Steam path</dt>
                  <dd
                    className="crosshook-dashboard-kv-row__value"
                    style={{ fontFamily: 'var(--crosshook-font-mono)', fontSize: '0.85rem' }}
                  >
                    {effectiveSteamClientInstallPath}
                  </dd>
                </div>
              ) : null}
              <div className="crosshook-dashboard-kv-row">
                <dt className="crosshook-dashboard-kv-row__label">umu preference</dt>
                <dd className="crosshook-dashboard-kv-row__value">
                  <span className="crosshook-editor-field-readonly">
                    {profile.runtime?.umu_preference ?? settings.umu_preference}
                  </span>
                </dd>
              </div>
            </dl>
          }
          profileSelectSlot={
            <LaunchProfileSelector
              activeCollection={activeCollection}
              activeCollectionId={activeCollectionId}
              filteredProfiles={filteredProfiles}
              pinnedSet={pinnedSet}
              selectedProfile={profileState.selectedProfile}
              showNetworkIsolationBadge={showNetworkIsolationBadge}
              onClearCollectionFilter={() => setActiveCollectionId(null)}
              onSelectProfile={(name) =>
                void profileState.selectProfile(name, {
                  collectionId: activeCollectionId ?? undefined,
                })
              }
              onTogglePin={handleTogglePin}
            />
          }
          tabsSlot={<LaunchSubTabs {...launchSubTabsProps} />}
          onBeforeLaunch={depGate.handleBeforeLaunch}
        />
      </div>

      <LaunchDepGateModal depGate={depGate} profile={profile} selectedName={selectedName} />
    </div>
  );
}

export default LaunchPage;
