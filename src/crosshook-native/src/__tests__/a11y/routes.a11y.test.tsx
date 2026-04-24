import { TooltipProvider } from '@radix-ui/react-tooltip';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CommunityPage } from '@/components/pages/CommunityPage';
import { CompatibilityPage } from '@/components/pages/CompatibilityPage';
import { DiscoverPage } from '@/components/pages/DiscoverPage';
import { HealthDashboardPage } from '@/components/pages/HealthDashboardPage';
import { HostToolsPage } from '@/components/pages/HostToolsPage';
import { InstallPage } from '@/components/pages/InstallPage';
import { LaunchPage } from '@/components/pages/LaunchPage';
import { LibraryPage } from '@/components/pages/LibraryPage';
import { ProfilesPage } from '@/components/pages/ProfilesPage';
import { ProtonManagerPage } from '@/components/pages/ProtonManagerPage';
import { SettingsPage } from '@/components/pages/SettingsPage';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { InspectorSelectionProvider } from '@/context/InspectorSelectionContext';
import { LaunchStateProvider } from '@/context/LaunchStateContext';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { renderWithMocks } from '@/test/render';
import { axe } from '@/test/setup';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

const noop = () => {};

/**
 * Empty profile list to avoid the happy-dom datalist CSS-selector crash with
 * Radix-generated IDs (e.g. `datalist#:r8:-suggestions` is invalid CSS).
 * Mirrors the `baseOverrides` pattern used in ProfilesRoute.test.tsx.
 */
const EMPTY_PROFILE_OVERRIDES = {
  profile_list: async () => [],
  profile_list_summaries: async () => [],
  profile_list_favorites: async () => [],
  collection_list: async () => [],
  collection_list_profiles: async () => [],
  batch_validate_profiles: async () => ({
    profiles: [],
    healthy_count: 0,
    stale_count: 0,
    broken_count: 0,
    total_count: 0,
    validated_at: '2026-04-23T12:00:00.000Z',
  }),
  get_cached_health_snapshots: async () => [],
  get_cached_offline_readiness_snapshots: async () => [],
  get_optimization_catalog: async () => null,
  get_trainer_type_catalog: async () => [],
  batch_offline_readiness: async () => [],
  settings_load: async () => ({
    auto_load_last_profile: false,
    last_used_profile: null,
    auto_install_prefix_deps: false,
    umu_preference: 'auto',
    steamgriddb_api_key: null,
    community_taps: [],
    steam_client_install_path: null,
  }),
  settings_save: async () => null,
  recent_files_load: async () => ({ game_paths: [], trainer_paths: [], dll_paths: [] }),
  recent_files_save: async () => null,
};

function AllProviders({ children }: { children: ReactNode }) {
  return (
    <TooltipProvider>
      <ProfileProvider>
        <PreferencesProvider>
          <ProfileHealthProvider>
            <HostReadinessProvider>
              <CollectionsProvider>
                <LaunchStateProvider>
                  <InspectorSelectionProvider>{children}</InspectorSelectionProvider>
                </LaunchStateProvider>
              </CollectionsProvider>
            </HostReadinessProvider>
          </ProfileHealthProvider>
        </PreferencesProvider>
      </ProfileProvider>
    </TooltipProvider>
  );
}

// Each entry returns a JSX element so we can pass required props.
const ROUTE_PAGES = [
  ['LibraryPage', () => <LibraryPage onNavigate={noop} onOpenCommandPalette={noop} />],
  ['ProfilesPage', () => <ProfilesPage />],
  ['LaunchPage', () => <LaunchPage />],
  ['HealthDashboardPage', () => <HealthDashboardPage />],
  ['HostToolsPage', () => <HostToolsPage />],
  ['ProtonManagerPage', () => <ProtonManagerPage />],
  ['CommunityPage', () => <CommunityPage />],
  ['DiscoverPage', () => <DiscoverPage />],
  ['CompatibilityPage', () => <CompatibilityPage />],
  ['SettingsPage', () => <SettingsPage />],
  ['InstallPage', () => <InstallPage onNavigate={noop} />],
] as const;

for (const [name, renderPage] of ROUTE_PAGES) {
  describe(`${name} accessibility`, () => {
    beforeEach(() => {
      // LibraryPage reads localStorage for view-mode persistence. Stub it so
      // the test environment doesn't crash on `localStorage.getItem`.
      const memory = new Map<string, string>();
      vi.stubGlobal('localStorage', {
        get length() {
          return memory.size;
        },
        clear: (): void => {
          memory.clear();
        },
        getItem: (key: string): string | null => memory.get(key) ?? null,
        key: (index: number): string | null => Array.from(memory.keys())[index] ?? null,
        removeItem: (key: string): void => {
          memory.delete(key);
        },
        setItem: (key: string, value: string): void => {
          memory.set(key, value);
        },
      } as Storage);
    });

    afterEach(() => {
      vi.unstubAllGlobals();
    });

    it('has no axe violations', async () => {
      const { container } = renderWithMocks(<AllProviders>{renderPage()}</AllProviders>, {
        handlerOverrides: EMPTY_PROFILE_OVERRIDES,
      });
      const results = await axe(container);
      expect(results).toHaveNoViolations();
    });
  });
}
