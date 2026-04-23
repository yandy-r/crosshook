import * as Tabs from '@radix-ui/react-tabs';
import { useState } from 'react';
import { useCommunityProfiles } from '../../hooks/useCommunityProfiles';
import CompatibilityViewer, { type CompatibilityDatabaseEntry } from '../CompatibilityViewer';
import { DEFAULT_COMPAT_TOOLS_DIR, ProtonVersionsPanel } from '../compatibility/ProtonVersionsPanel';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import { RouteBanner } from '../layout/RouteBanner';

const DEFAULT_PROFILES_DIRECTORY = '~/.config/crosshook/profiles';

type CompatTabId = 'trainer' | 'proton';

const TAB_LABELS: Record<CompatTabId, string> = {
  trainer: 'Trainer',
  proton: 'Proton',
};

function toCompatibilityEntries(
  entries: ReturnType<typeof useCommunityProfiles>['index']['entries']
): CompatibilityDatabaseEntry[] {
  return entries.map((entry) => ({
    id: `${entry.tap_url}::${entry.relative_path}`,
    tap_url: entry.tap_url,
    tap_branch: entry.tap_branch,
    manifest_path: entry.manifest_path,
    relative_path: entry.relative_path,
    metadata: entry.manifest.metadata,
  }));
}

export function CompatibilityPage() {
  const [activeTab, setActiveTab] = useState<CompatTabId>('trainer');

  const communityState = useCommunityProfiles({
    profilesDirectoryPath: DEFAULT_PROFILES_DIRECTORY,
  });
  const compatibilityEntries = toCompatibilityEntries(communityState.index.entries);

  const trainerStatus =
    communityState.loading || communityState.syncing
      ? 'Syncing trainer reports'
      : communityState.error
        ? 'Trainer data has inline errors'
        : compatibilityEntries.length > 0
          ? `${compatibilityEntries.length} trainer report${compatibilityEntries.length === 1 ? '' : 's'} indexed`
          : 'No trainer reports indexed yet';

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--compatibility">
      <div className="crosshook-route-stack crosshook-compatibility-page">
        <div className="crosshook-route-stack__body--fill crosshook-compatibility-page__body">
          <RouteBanner route="compatibility" />
          <div className="crosshook-route-card-host">
            <div className="crosshook-route-card-scroll">
              <div className="crosshook-dashboard-route-body">
                <DashboardPanelSection
                  eyebrow="Compatibility hub"
                  title="Keep trainer reports and Proton runtimes in the same workflow"
                  description="Browse community compatibility data and manage installable Proton releases from one dashboard surface without resetting tab-local state."
                  actions={<div className="crosshook-status-chip">Active: {TAB_LABELS[activeTab]}</div>}
                >
                  <div className="crosshook-dashboard-pill-row">
                    <span className="crosshook-dashboard-pill">{trainerStatus}</span>
                    <span className="crosshook-dashboard-pill">Profiles: {DEFAULT_PROFILES_DIRECTORY}</span>
                    <span className="crosshook-dashboard-pill">Compat tools: {DEFAULT_COMPAT_TOOLS_DIR}</span>
                  </div>
                </DashboardPanelSection>

                <Tabs.Root
                  className="crosshook-subtabs-root"
                  value={activeTab}
                  onValueChange={(value) => setActiveTab(value as CompatTabId)}
                >
                  <DashboardPanelSection
                    className="crosshook-subtabs-shell crosshook-compatibility-subtabs"
                    eyebrow="Workspace"
                    title="Trainer reports and Proton releases"
                    description="Switch between compatibility intelligence and runtime catalog management while keeping both panels mounted."
                    actions={
                      <Tabs.List className="crosshook-subtab-row" aria-label="Compatibility sections">
                        {(Object.keys(TAB_LABELS) as CompatTabId[]).map((tab) => (
                          <Tabs.Trigger
                            key={tab}
                            value={tab}
                            className={`crosshook-subtab${activeTab === tab ? ' crosshook-subtab--active' : ''}`}
                          >
                            {TAB_LABELS[tab]}
                          </Tabs.Trigger>
                        ))}
                      </Tabs.List>
                    }
                    bodyClassName="crosshook-dashboard-route-section-stack"
                  >
                    <Tabs.Content
                      value="trainer"
                      forceMount
                      className="crosshook-subtab-content"
                      style={{ display: activeTab === 'trainer' ? undefined : 'none' }}
                    >
                      <div className="crosshook-subtab-content__inner">
                        <CompatibilityViewer
                          entries={compatibilityEntries}
                          title="Trainer compatibility database"
                          description="Filter community trainer reports by game, trainer, and platform while keeping loading, error, and empty states inline."
                          loading={communityState.loading || communityState.syncing}
                          error={communityState.error}
                          emptyMessage="No indexed community compatibility entries are available yet."
                        />
                      </div>
                    </Tabs.Content>

                    <Tabs.Content
                      value="proton"
                      forceMount
                      className="crosshook-subtab-content"
                      style={{ display: activeTab === 'proton' ? undefined : 'none' }}
                    >
                      <div className="crosshook-subtab-content__inner">
                        <ProtonVersionsPanel />
                      </div>
                    </Tabs.Content>
                  </DashboardPanelSection>
                </Tabs.Root>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default CompatibilityPage;
