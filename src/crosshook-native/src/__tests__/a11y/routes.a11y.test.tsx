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
import { makeLibraryCardData, makeProfileDraft } from '@/test/fixtures';
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

// ---------------------------------------------------------------------------
// Populated-fixture axe tests (F005)
// ---------------------------------------------------------------------------
// These run axe against major page variants with representative data so that
// interactive controls (library cards, profile rows, launch panel) are actually
// rendered. `color-contrast` is still disabled globally in `test/setup.ts`
// because happy-dom does not compute real CSS; use `@axe-core/playwright`
// against `?fixture=populated` in CI for a true color-contrast audit.
//
// Note: ProfilesPage is substituted by HealthDashboardPage here. A populated
// ProfilesPage renders a `<datalist id=":rN:-suggestions">` whose Radix-
// generated ID is an invalid CSS selector that crashes happy-dom's
// querySelectorAll when axe traverses the DOM.
// ---------------------------------------------------------------------------

const POPULATED_LAUNCH_OVERRIDES = {
  profile_list: async () => ['Test Game Alpha', 'Dev Game Beta'],
  profile_list_summaries: async () => [
    makeLibraryCardData({ name: 'Test Game Alpha', gameName: 'Test Game Alpha', steamAppId: '9999001' }),
    makeLibraryCardData({ name: 'Dev Game Beta', gameName: 'Dev Game Beta', steamAppId: '9999002' }),
  ],
  profile_list_favorites: async () => [],
  profile_load: async () =>
    makeProfileDraft({
      game: { name: 'Test Game Alpha', executable_path: '/mock/game.exe' },
    }),
  collection_list: async () => [],
  collection_list_profiles: async () => [],
  validate_launch: async () => null,
  check_offline_readiness: async () => ({
    profile_name: 'Test Game Alpha',
    score: 100,
    readiness_state: 'ready' as const,
    trainer_type: 'dll' as const,
    checks: [],
    blocking_reasons: [],
    checked_at: '2026-04-23T12:00:00.000Z',
  }),
  check_game_running: async () => false,
  check_gamescope_session: async () => false,
  get_optimization_catalog: async () => null,
  get_trainer_type_catalog: async () => [],
  get_cached_health_snapshots: async () => [],
  get_cached_offline_readiness_snapshots: async () => [],
  batch_offline_readiness: async () => [],
  launch_platform_status: async () => ({ unshare_net_available: false }),
  get_capabilities: async () => [],
  check_generalized_readiness: async () => ({
    checks: [],
    all_passed: true,
    critical_failures: 0,
    warnings: 0,
    umu_install_guidance: null,
    steam_deck_caveats: null,
    tool_checks: [],
    detected_distro_family: null,
  }),
  get_cached_host_readiness_snapshot: async () => null,
  get_dependency_status: async () => [],
  install_prefix_dependency: async () => null,
  protondb_fetch_recommendations: async () => null,
  batch_validate_profiles: async () => ({
    profiles: [],
    healthy_count: 0,
    stale_count: 0,
    broken_count: 0,
    total_count: 0,
    validated_at: '2026-04-23T12:00:00.000Z',
  }),
};

describe('populated-fixture accessibility', () => {
  beforeEach(() => {
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

  it('LibraryPage with populated profiles has no axe violations', async () => {
    // Default mock handlers seed 8 demo profiles via seedDemoProfiles(), so
    // library cards, toolbar chips, and sort controls are all rendered.
    const { container } = renderWithMocks(
      <AllProviders>
        <LibraryPage onNavigate={noop} onOpenCommandPalette={noop} />
      </AllProviders>
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });

  it('LaunchPage with populated profiles has no axe violations', async () => {
    // Two profiles in the selector; launch panel content (readiness, env-var
    // tabs) is rendered.
    const { container } = renderWithMocks(
      <AllProviders>
        <LaunchPage />
      </AllProviders>,
      { handlerOverrides: POPULATED_LAUNCH_OVERRIDES }
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });

  it('HealthDashboardPage with populated health data has no axe violations', async () => {
    // Substitutes for ProfilesPage: a populated ProfilesPage renders
    // `<datalist id=":rN:-suggestions">` which is an invalid CSS selector
    // that crashes happy-dom's querySelectorAll during axe traversal.
    // Default mock handlers seed demo profiles and synthesise health rows so
    // the dashboard cards and status chips are rendered.
    const { container } = renderWithMocks(
      <AllProviders>
        <HealthDashboardPage />
      </AllProviders>
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});
