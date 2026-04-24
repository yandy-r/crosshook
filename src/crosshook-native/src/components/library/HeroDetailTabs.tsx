import * as Tabs from '@radix-ui/react-tabs';
import { HeroDetailPanels, type HeroDetailPanelsProps } from './HeroDetailPanels';
import { HERO_DETAIL_TABS, type HeroDetailTabId, heroDetailTabTestId } from './hero-detail-model';

export interface HeroDetailTabsProps {
  activeTab: HeroDetailTabId;
  onActiveTabChange: (tab: HeroDetailTabId) => void;
  panelProps: Omit<HeroDetailPanelsProps, 'mode'>;
}

export function HeroDetailTabs({ activeTab, onActiveTabChange, panelProps }: HeroDetailTabsProps) {
  return (
    <Tabs.Root
      className="crosshook-subtabs-root"
      value={activeTab}
      onValueChange={(value) => onActiveTabChange(value as HeroDetailTabId)}
    >
      <Tabs.List className="crosshook-subtab-row" aria-label="Game detail sections">
        {HERO_DETAIL_TABS.map((tab) => (
          <Tabs.Trigger
            key={tab.id}
            value={tab.id}
            className={`crosshook-subtab${activeTab === tab.id ? ' crosshook-subtab--active' : ''}`}
          >
            {tab.label}
          </Tabs.Trigger>
        ))}
      </Tabs.List>
      {HERO_DETAIL_TABS.map((tab) => {
        const testId = heroDetailTabTestId(tab.id);
        return (
          <Tabs.Content
            key={tab.id}
            value={tab.id}
            className="crosshook-subtab-content crosshook-hero-detail__tab-content"
            style={{ display: activeTab === tab.id ? undefined : 'none' }}
            {...(testId ? { 'data-testid': testId } : {})}
          >
            <div className="crosshook-subtab-content__inner crosshook-hero-detail__panel-inner">
              <HeroDetailPanels mode={tab.id} {...panelProps} />
            </div>
          </Tabs.Content>
        );
      })}
    </Tabs.Root>
  );
}
