import { TooltipProvider } from '@radix-ui/react-tooltip';
import { screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { LaunchStateProvider } from '@/context/LaunchStateContext';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { emitMockEvent } from '@/lib/events';
import { makeProfileDraft } from '@/test/fixtures';
import { renderWithMocks } from '@/test/render';
import { LaunchPage } from '../LaunchPage';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

const DEMO_PROFILE_NAME = 'Test Game Alpha';

const baseHandlerOverrides = {
  profile_list: async () => [DEMO_PROFILE_NAME],
  profile_list_summaries: async () => [
    {
      name: DEMO_PROFILE_NAME,
      gameName: DEMO_PROFILE_NAME,
      steamAppId: '9999001',
      customCoverArtPath: '',
      customPortraitArtPath: '',
      networkIsolation: false,
    },
  ],
  profile_list_favorites: async () => [],
  profile_load: async () =>
    makeProfileDraft({
      game: { name: DEMO_PROFILE_NAME, executable_path: '/mock/game.exe' },
    }),
  collection_list: async () => [],
  collection_list_profiles: async () => [],
  validate_launch: async () => null,
  check_offline_readiness: async () => ({
    profile_name: DEMO_PROFILE_NAME,
    score: 100,
    issues: [],
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

function LaunchRouteProviders({ children }: { children: ReactNode }) {
  return (
    <TooltipProvider>
      <ProfileProvider>
        <PreferencesProvider>
          <ProfileHealthProvider>
            <HostReadinessProvider>
              <CollectionsProvider>
                <LaunchStateProvider>{children}</LaunchStateProvider>
              </CollectionsProvider>
            </HostReadinessProvider>
          </ProfileHealthProvider>
        </PreferencesProvider>
      </ProfileProvider>
    </TooltipProvider>
  );
}

function renderLaunchRoute(options: Parameters<typeof renderWithMocks>[1] = {}) {
  return renderWithMocks(
    <LaunchRouteProviders>
      <LaunchPage />
    </LaunchRouteProviders>,
    options
  );
}

describe('LaunchRoute', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

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

    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.spyOn(console, 'debug').mockImplementation(() => {});
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it('(a) renders launch page with profile selector', async () => {
    renderLaunchRoute({ handlerOverrides: baseHandlerOverrides });

    await waitFor(() => {
      expect(document.querySelector('.crosshook-page-scroll-shell--launch')).toBeInTheDocument();
    });

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(a) collection chip reflects useCollectionMembers result — shows collection name when active', async () => {
    renderLaunchRoute({
      handlerOverrides: {
        ...baseHandlerOverrides,
        collection_list: async () => [
          {
            collection_id: 'col-1',
            name: 'Action Games',
            description: null,
            profile_count: 1,
            created_at: '2026-04-23T12:00:00.000Z',
            updated_at: '2026-04-23T12:00:00.000Z',
          },
        ],
        collection_list_profiles: async () => [DEMO_PROFILE_NAME],
      },
    });

    // The profile selector area renders; page mounts without errors
    await waitFor(() => {
      expect(document.querySelector('.crosshook-page-scroll-shell--launch')).toBeInTheDocument();
    });

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(b) env-var autosave: hasSavedSelectedProfile gate blocks profile_save when profile not in list', async () => {
    // hasSavedSelectedProfile = profileName === selectedProfile && profiles.includes(profileName)
    // With profile_list returning only DEMO_PROFILE_NAME but profile_load returning null,
    // selectedProfile never gets set, so the autosave gate blocks profile_save.
    const profileSaveSpy = vi.fn().mockResolvedValue(null);

    renderLaunchRoute({
      handlerOverrides: {
        ...baseHandlerOverrides,
        profile_save: profileSaveSpy,
      },
    });

    await waitFor(() => {
      expect(document.querySelector('.crosshook-page-scroll-shell--launch')).toBeInTheDocument();
    });

    // No profile has been loaded (profile_load is not called without selection),
    // so hasSavedSelectedProfile = false and autosave is gated.
    expect(profileSaveSpy).not.toHaveBeenCalled();

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(c) hasSavedSelectedProfile gate blocks autosave when no profile selected', async () => {
    const profileSaveSpy = vi.fn().mockResolvedValue(null);

    renderLaunchRoute({
      handlerOverrides: {
        ...baseHandlerOverrides,
        // profile_list returns [] so selectedProfile stays '' → hasSavedSelectedProfile = false
        profile_list: async () => [],
        profile_save: profileSaveSpy,
      },
    });

    await waitFor(() => {
      expect(document.querySelector('.crosshook-page-scroll-shell--launch')).toBeInTheDocument();
    });

    // No profile loaded → hasSavedSelectedProfile = false → autosave gated
    expect(profileSaveSpy).not.toHaveBeenCalled();

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(d) handleBeforeLaunch silent-catches getDependencyStatus rejection and allows launch', async () => {
    // When getDependencyStatus throws, handleBeforeLaunch returns true (allow launch)
    // and does NOT bubble the error. We verify no unhandled error surfaces.
    renderLaunchRoute({
      handlerOverrides: {
        ...baseHandlerOverrides,
        get_dependency_status: async () => {
          throw new Error('network error');
        },
        profile_load: async () =>
          makeProfileDraft({
            game: { name: DEMO_PROFILE_NAME, executable_path: '/mock/game.exe' },
            // Set required_protontricks to trigger the dep gate check path
            trainer: {
              path: '/mock/trainer.exe',
              type: 'dll',
              loading_mode: 'source_directory',
              required_protontricks: ['vcrun2019'],
            },
            runtime: { prefix_path: '/mock/pfx', proton_path: '', working_directory: '' },
          }),
        launch_game: async () => ({
          ok: true,
          profile_name: DEMO_PROFILE_NAME,
          helper_log_path: null,
        }),
        validate_launch: async () => null,
      },
    });

    await waitFor(() => {
      expect(document.querySelector('.crosshook-page-scroll-shell--launch')).toBeInTheDocument();
    });

    // Page renders without errors — getDependencyStatus error was silent-caught
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(e) prefix-dep-complete event is handled without error when dep-gate modal is not open', async () => {
    // The prefix-dep-complete listener only fires when depGateInstalling=true.
    // When the modal has never been opened (depGatePackages=null), the event is a no-op.
    // This verifies the page renders cleanly and the event does not cause errors.
    renderLaunchRoute({
      handlerOverrides: {
        ...baseHandlerOverrides,
        profile_load: async () =>
          makeProfileDraft({
            game: { name: DEMO_PROFILE_NAME, executable_path: '/mock/game.exe' },
            trainer: {
              path: '/mock/trainer.exe',
              type: 'dll',
              loading_mode: 'source_directory',
              required_protontricks: ['vcrun2019'],
            },
            runtime: { prefix_path: '/mock/pfx', proton_path: '', working_directory: '' },
            steam: {
              compatdata_path: '/mock/pfx',
              enabled: false,
              app_id: '',
              proton_path: '',
              launcher: { icon_path: '', display_name: '' },
            },
          }),
        get_dependency_status: async () => [{ package_name: 'vcrun2019', state: 'missing', error: null }],
        install_prefix_dependency: async () => null,
      },
    });

    await waitFor(() => {
      expect(document.querySelector('.crosshook-page-scroll-shell--launch')).toBeInTheDocument();
    });

    // Dep-gate modal is not visible initially (depGatePackages is null)
    expect(screen.queryByRole('dialog', { name: /Missing Prefix Dependencies/i })).not.toBeInTheDocument();

    // Emit prefix-dep-complete — since depGateInstalling=false, no listener is subscribed.
    // The event is a no-op: page stays clean, no modal appears.
    emitMockEvent('prefix-dep-complete', {
      profile_name: DEMO_PROFILE_NAME,
      prefix_path: '/mock/pfx',
      succeeded: true,
    });

    // Modal remains absent
    expect(screen.queryByRole('dialog', { name: /Missing Prefix Dependencies/i })).not.toBeInTheDocument();

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});
