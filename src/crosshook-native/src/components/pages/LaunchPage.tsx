import { useLaunchSubTabsProps } from '../../hooks/launch/useLaunchSubTabsProps';
import type { AppNavigateOptions, GameDetailOrigin } from '../../types/navigation';
import LaunchPanel from '../LaunchPanel';
import { LaunchSubTabs } from '../LaunchSubTabs';
import { Breadcrumb } from '../layout/Breadcrumb';
import { buildGameDetailTrail } from '../layout/game-detail-trail';
import { LaunchArt } from '../layout/PageBanner';
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
  const trail = buildGameDetailTrail(origin, onNavigate, 'Launch');

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--launch">
      <div className="crosshook-route-stack crosshook-launch-page__grid">
        <section className="crosshook-route-banner crosshook-panel" aria-labelledby="crosshook-legacy-launch-title">
          <div className="crosshook-route-banner__inner">
            <div className="crosshook-route-banner__body">
              {trail && trail.length > 0 ? (
                <Breadcrumb segments={trail} className="crosshook-route-banner__eyebrow" />
              ) : (
                <p className="crosshook-route-banner__eyebrow crosshook-heading-eyebrow">Game</p>
              )}
              <h1 id="crosshook-legacy-launch-title" className="crosshook-route-banner__title">
                Launch
              </h1>
              <p className="crosshook-route-banner__summary crosshook-heading-copy">
                Run the game or trainer with the active profile's launch configuration.
              </p>
            </div>
            <div className="crosshook-route-banner__icon" aria-hidden="true">
              <LaunchArt />
            </div>
          </div>
        </section>
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
