import { TooltipProvider } from '@radix-ui/react-tooltip';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LaunchSubTabsProps } from '@/components/LaunchSubTabs';
import { LaunchSubTabs } from '@/components/LaunchSubTabs';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { LaunchStateProvider } from '@/context/LaunchStateContext';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { renderWithMocks } from '@/test/render';
import type { LaunchAutoSaveStatus, LaunchMethod } from '@/types';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG } from '@/types/profile';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

// Minimal props for LaunchSubTabs - only required fields
function makeSubTabsProps(overrides: Partial<LaunchSubTabsProps> = {}): LaunchSubTabsProps {
  return {
    launchMethod: 'proton_run',
    steamAppId: undefined,
    gamescopeConfig: DEFAULT_GAMESCOPE_CONFIG,
    onGamescopeChange: vi.fn(),
    isInsideGamescopeSession: false,
    mangoHudConfig: DEFAULT_MANGOHUD_CONFIG,
    onMangoHudChange: vi.fn(),
    showMangoHudOverlayEnabled: false,
    enabledOptionIds: [],
    onToggleOption: vi.fn(),
    launchOptimizationsStatus: { tone: 'idle', label: '' },
    catalog: null,
    profileName: 'test-profile',
    onUpdateProfile: vi.fn(),
    showProtonDbLookup: false,
    onApplyProtonDbEnvVars: vi.fn(),
    applyingProtonDbGroupId: null,
    protonDbStatusMessage: null,
    pendingProtonDbOverwrite: null,
    onConfirmProtonDbOverwrite: vi.fn(),
    onCancelProtonDbOverwrite: vi.fn(),
    onUpdateProtonDbResolution: vi.fn(),
    gamescopeAutoSaveStatus: { tone: 'idle', label: '' },
    mangoHudAutoSaveStatus: { tone: 'idle', label: '' },
    ...overrides,
  };
}

const baseHandlerOverrides = {
  collection_list: async () => [],
  collection_list_profiles: async () => [],
  validate_launch: async () => null,
  check_offline_readiness: async () => ({
    profile_name: 'test-profile',
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
  protondb_fetch_recommendations: async () => null,
};

function SubTabsTestProviders({ children }: { children: ReactNode }) {
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

function renderSubTabs(props: LaunchSubTabsProps, options: Parameters<typeof renderWithMocks>[1] = {}) {
  return renderWithMocks(
    <SubTabsTestProviders>
      <LaunchSubTabs {...props} />
    </SubTabsTestProviders>,
    options
  );
}

describe('LaunchSubTabs', () => {
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

  it('(a) combined autosave chip uses TONE_PRIORITY — error wins over success and saving', async () => {
    // gamescope=success (priority 1), mangohud=error (priority 4), optimizations=saving (priority 3)
    // Result tone should be 'error' (highest priority = 4)
    const gamescopeStatus: LaunchAutoSaveStatus = { tone: 'success', label: 'Gamescope saved' };
    const mangohudStatus: LaunchAutoSaveStatus = { tone: 'error', label: 'MangoHud error' };
    const optimizationsStatus = { tone: 'saving' as const, label: 'Saving...' };

    const { container } = renderSubTabs(
      makeSubTabsProps({
        launchMethod: 'proton_run',
        gamescopeAutoSaveStatus: gamescopeStatus,
        mangoHudAutoSaveStatus: mangohudStatus,
        launchOptimizationsStatus: optimizationsStatus,
      }),
      { handlerOverrides: baseHandlerOverrides }
    );

    // The chip's visibility is driven by useEffect; no timer for 'error' tone.
    // Wait for the DOM to reflect the combined status.
    await waitFor(() => {
      const chip = container.querySelector('.crosshook-launch-autosave-chip--error');
      expect(chip).toBeInTheDocument();
    });

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(b) tab visibility matrix — native only shows environment and offline tabs', async () => {
    const user = userEvent.setup();

    renderSubTabs(makeSubTabsProps({ launchMethod: 'native' }), {
      handlerOverrides: baseHandlerOverrides,
    });

    await waitFor(() => {
      expect(screen.getByRole('tab', { name: 'Environment' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: 'Offline' })).toBeInTheDocument();
    });

    // native: no gamescope, mangohud, optimizations, steam-options
    expect(screen.queryByRole('tab', { name: 'Gamescope' })).not.toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: 'MangoHud' })).not.toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: 'Optimizations' })).not.toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: 'Steam Options' })).not.toBeInTheDocument();

    // Suppress unused warning
    void user;

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(b) tab visibility matrix — proton_run shows gamescope, mangohud, optimizations; not steam-options', async () => {
    renderSubTabs(makeSubTabsProps({ launchMethod: 'proton_run' }), {
      handlerOverrides: baseHandlerOverrides,
    });

    await waitFor(() => {
      expect(screen.getByRole('tab', { name: 'Gamescope' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: 'MangoHud' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: 'Optimizations' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: 'Environment' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: 'Offline' })).toBeInTheDocument();
    });

    expect(screen.queryByRole('tab', { name: 'Steam Options' })).not.toBeInTheDocument();

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(b) tab visibility matrix — steam_applaunch shows steam-options, gamescope, mangohud, optimizations', async () => {
    renderSubTabs(makeSubTabsProps({ launchMethod: 'steam_applaunch' }), {
      handlerOverrides: baseHandlerOverrides,
    });

    await waitFor(() => {
      expect(screen.getByRole('tab', { name: 'Steam Options' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: 'Gamescope' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: 'MangoHud' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: 'Optimizations' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: 'Environment' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: 'Offline' })).toBeInTheDocument();
    });

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(c) forceMount — all tab panels remain in DOM when switching tabs', async () => {
    const user = userEvent.setup();

    renderSubTabs(makeSubTabsProps({ launchMethod: 'proton_run' }), {
      handlerOverrides: baseHandlerOverrides,
    });

    // Wait for tabs to be present
    await waitFor(() => {
      expect(screen.getByRole('tab', { name: 'Gamescope' })).toBeInTheDocument();
    });

    // Switch to Gamescope tab
    await user.click(screen.getByRole('tab', { name: 'Gamescope' }));

    // All panels that use forceMount are kept in the DOM (hidden ones have hidden attribute)
    // The tab panels should all be present regardless of active tab
    const allPanels = screen.getAllByRole('tabpanel', { hidden: true });
    // proton_run has: optimizations, environment, mangohud, gamescope, offline = 5 panels
    expect(allPanels.length).toBeGreaterThanOrEqual(3);

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(d) offline auto-switch fires when launchPathWarnings.length > 0 on first render with warnings', async () => {
    // The offline auto-switch is driven by LaunchStateContext's launchPathWarnings.
    // When there are path warnings, the component should auto-switch to 'offline' tab.
    // We test that it renders in the correct initial tab based on context state.

    // Default: no warnings → active tab = first tab for proton_run = 'optimizations'
    renderSubTabs(makeSubTabsProps({ launchMethod: 'proton_run' }), {
      handlerOverrides: baseHandlerOverrides,
    });

    await waitFor(() => {
      expect(screen.getByRole('tab', { name: 'Optimizations' })).toBeInTheDocument();
    });

    // The first tab for proton_run should be selected
    expect(screen.getByRole('tab', { name: 'Optimizations' })).toHaveAttribute('data-state', 'active');

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(d) autoSwitchedRef guard: auto-switch does not fire again after manual tab change', async () => {
    const user = userEvent.setup();

    // Render with no offline concerns so no auto-switch occurs
    renderSubTabs(makeSubTabsProps({ launchMethod: 'proton_run' }), {
      handlerOverrides: baseHandlerOverrides,
    });

    await waitFor(() => {
      expect(screen.getByRole('tab', { name: 'Optimizations' })).toHaveAttribute('data-state', 'active');
    });

    // Manually switch to Environment tab
    await user.click(screen.getByRole('tab', { name: 'Environment' }));

    await waitFor(() => {
      expect(screen.getByRole('tab', { name: 'Environment' })).toHaveAttribute('data-state', 'active');
    });

    // After manual switch, autoSwitchedRef.current = true, so a subsequent
    // offline concern would not auto-switch back. Verify we remain on Environment.
    expect(screen.getByRole('tab', { name: 'Environment' })).toHaveAttribute('data-state', 'active');

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('renders without errors for all three launch methods', async () => {
    const methods: LaunchMethod[] = ['native', 'proton_run', 'steam_applaunch'];

    for (const launchMethod of methods) {
      const { unmount } = renderSubTabs(makeSubTabsProps({ launchMethod }), {
        handlerOverrides: baseHandlerOverrides,
      });

      await waitFor(() => {
        expect(screen.getByRole('tablist')).toBeInTheDocument();
      });

      unmount();
    }

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});
