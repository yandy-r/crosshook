import { TooltipProvider } from '@radix-ui/react-tooltip';
import { screen } from '@testing-library/react';
import type { ReactElement, ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { renderWithMocks } from '@/test/render';
import { CompatibilityPage } from '../CompatibilityPage';
import { HealthDashboardPage } from '../HealthDashboardPage';
import { HostToolsPage } from '../HostToolsPage';
import { ProtonManagerPage } from '../ProtonManagerPage';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

function setInnerWidth(width: number): void {
  Object.defineProperty(window, 'innerWidth', { value: width, configurable: true, writable: true });
}

function setInnerHeight(height: number): void {
  Object.defineProperty(window, 'innerHeight', { value: height, configurable: true, writable: true });
}

function DashboardRouteProviders({ children }: { children: ReactNode }) {
  return (
    <TooltipProvider>
      <ProfileProvider>
        <PreferencesProvider>
          <ProfileHealthProvider>
            <HostReadinessProvider>
              <CollectionsProvider>{children}</CollectionsProvider>
            </HostReadinessProvider>
          </ProfileHealthProvider>
        </PreferencesProvider>
      </ProfileProvider>
    </TooltipProvider>
  );
}

function renderDashboardRoute(ui: ReactElement, options: Parameters<typeof renderWithMocks>[1] = {}) {
  return renderWithMocks(<DashboardRouteProviders>{ui}</DashboardRouteProviders>, options);
}

describe('dashboard routes', () => {
  let previousWidth: number;
  let previousHeight: number;
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    previousWidth = window.innerWidth;
    previousHeight = window.innerHeight;
    setInnerWidth(1920);
    setInnerHeight(1080);

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
  });

  afterEach(() => {
    setInnerWidth(previousWidth);
    setInnerHeight(previousHeight);
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('keeps the Health dashboard banner and shows the empty-library fallback', async () => {
    const { container } = renderDashboardRoute(<HealthDashboardPage />, {
      handlerOverrides: {
        profile_list: async () => [],
        profile_list_favorites: async () => [],
        get_cached_health_snapshots: async () => [],
        get_cached_offline_readiness_snapshots: async () => [],
        batch_validate_profiles: async () => ({
          profiles: [],
          healthy_count: 0,
          stale_count: 0,
          broken_count: 0,
          total_count: 0,
          validated_at: '2026-04-23T12:00:00.000Z',
        }),
      },
    });

    expect(await screen.findByRole('heading', { level: 1, name: 'Health' })).toBeInTheDocument();
    expect(
      await screen.findByRole('heading', {
        level: 2,
        name: 'Monitor profile readiness across launch, version, and offline checks',
      })
    ).toBeInTheDocument();
    expect(await screen.findByText('No profiles configured yet.')).toBeInTheDocument();
    expect(container.querySelector('.crosshook-page-scroll-shell--health')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('keeps the Host Tools dashboard banner and surfaces the initial-load error state', async () => {
    const { container } = renderDashboardRoute(<HostToolsPage />, {
      handlerOverrides: {
        get_cached_host_readiness_snapshot: async () => null,
        get_capabilities: async () => [],
        check_generalized_readiness: async () => {
          throw new Error('mock readiness failure');
        },
      },
    });

    expect(await screen.findByRole('heading', { level: 1, name: 'Host Tools' })).toBeInTheDocument();
    expect(
      await screen.findByRole('heading', {
        level: 2,
        name: 'Required and optional host tools',
      })
    ).toBeInTheDocument();
    expect(await screen.findByRole('heading', { level: 2, name: 'Unable to load host readiness' })).toBeInTheDocument();
    expect(container.querySelector('.crosshook-page-scroll-shell--host-tools')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('keeps the Proton Manager dashboard banner and preserves the runtime-discovery fallback copy', async () => {
    const { container } = renderDashboardRoute(<ProtonManagerPage />, {
      handlerOverrides: {
        profile_list: async () => [],
        default_steam_client_install_path: async () => '',
      },
    });

    expect(await screen.findByRole('heading', { level: 1, name: 'Proton Manager' })).toBeInTheDocument();
    expect(
      await screen.findByRole('heading', {
        level: 2,
        name: 'Manage installed Proton builds with clearer route context',
      })
    ).toBeInTheDocument();
    expect(await screen.findByText('No explicit path provided; runtime detection applies.')).toBeInTheDocument();
    expect(await screen.findByRole('heading', { level: 2, name: 'Installed' })).toBeInTheDocument();
    expect(await screen.findByRole('heading', { level: 2, name: 'Available' })).toBeInTheDocument();
    expect(container.querySelector('.crosshook-page-scroll-shell--proton-manager')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('keeps the Compatibility dashboard banner and shows the empty trainer-results fallback', async () => {
    const { container } = renderDashboardRoute(<CompatibilityPage />, {
      handlerOverrides: {
        community_list_profiles: async () => ({
          entries: [],
          diagnostics: [],
        }),
      },
    });

    expect(await screen.findByRole('heading', { level: 1, name: 'Compatibility' })).toBeInTheDocument();
    expect(
      await screen.findByRole('heading', {
        level: 2,
        name: 'Keep trainer reports and Proton runtimes in the same workflow',
      })
    ).toBeInTheDocument();
    expect(
      await screen.findByRole('heading', { level: 3, name: 'Trainer compatibility database' })
    ).toBeInTheDocument();
    expect(
      await screen.findByText('No indexed community compatibility entries are available yet.')
    ).toBeInTheDocument();
    expect(container.querySelector('.crosshook-page-scroll-shell--compatibility')).toBeInTheDocument();
    expect(container.querySelector('.crosshook-route-card-scroll')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});
