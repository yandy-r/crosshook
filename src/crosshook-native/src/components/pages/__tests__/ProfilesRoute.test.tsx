import { TooltipProvider } from '@radix-ui/react-tooltip';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { emitMockEvent } from '@/lib/events';
import { makeProfileDraft } from '@/test/fixtures';
import { renderWithMocks } from '@/test/render';
import { ProfilesPage } from '../ProfilesPage';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

function ProfilesRouteProviders({ children }: { children: ReactNode }) {
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

function renderProfilesRoute(options: Parameters<typeof renderWithMocks>[1] = {}) {
  return renderWithMocks(
    <ProfilesRouteProviders>
      <ProfilesPage />
    </ProfilesRouteProviders>,
    options
  );
}

/** Empty profile list — keeps datalist out of DOM, avoiding happy-dom CSS-selector crash
 * with Radix-generated IDs (`:r2m:` etc. are invalid CSS identifiers). */
const baseOverrides = {
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

describe('ProfilesRoute', () => {
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
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('(a) renders the profiles page shell class', async () => {
    const { container } = renderProfilesRoute({ handlerOverrides: baseOverrides });

    await waitFor(() => {
      expect(container.querySelector('.crosshook-page-scroll-shell--profiles')).toBeInTheDocument();
    });

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(a) DashboardPanelSection-wrapped groups render (profiles body and footer sections)', async () => {
    const { container } = renderProfilesRoute({ handlerOverrides: baseOverrides });

    await waitFor(() => {
      expect(container.querySelector('.crosshook-profiles-page__body')).toBeInTheDocument();
    });

    // Footer actions section is also rendered
    expect(container.querySelector('.crosshook-profiles-page__actions')).toBeInTheDocument();

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(b) auto-load-profile event populates profile name input and enables Save button', async () => {
    // Uses auto-load-profile event to load a profile without populating profiles[],
    // so the datalist is never rendered (avoids happy-dom CSS-selector crash with
    // Radix-generated `:r2m:` IDs like `datalist#:r2m:-suggestions`).
    //
    // Race-condition guard: profile_list is called by refreshProfiles() on mount.
    // When it resolves with [], refreshProfiles resets state. We must wait for
    // refreshProfiles to complete before emitting auto-load-profile, otherwise
    // the reset races with loadProfile and reverts profileName to ''.
    const PROFILE_NAME = 'My New Game';

    // Track when profile_list has been resolved so we know refreshProfiles completed
    let profileListResolveFn: (() => void) | null = null;
    const profileListCalled = new Promise<void>((resolve) => {
      profileListResolveFn = resolve;
    });

    renderProfilesRoute({
      handlerOverrides: {
        ...baseOverrides,
        profile_list: async () => {
          profileListResolveFn?.();
          return [];
        },
        profile_load: async () =>
          makeProfileDraft({
            game: { name: PROFILE_NAME, executable_path: '/mock/game.exe' },
          }),
      },
    });

    // Wait for the page shell to appear
    await waitFor(() => {
      expect(document.querySelector('.crosshook-page-scroll-shell--profiles')).toBeInTheDocument();
    });

    // Wait for profile_list to have been called (refreshProfiles has started).
    // Then yield via setTimeout(0) to ensure refreshProfiles has fully completed
    // (it processes [] synchronously after await resolves).
    await profileListCalled;
    await new Promise<void>((resolve) => setTimeout(resolve, 0));

    // Trigger profile load via event — loadProfile sets profileName and
    // profile.game.executable_path without adding to profiles[], so no datalist.
    emitMockEvent('auto-load-profile', PROFILE_NAME);

    // Wait until the profile name input reflects the loaded profile name.
    // This confirms loadProfile was called and state was set correctly.
    await waitFor(
      () => {
        const nameInput = screen.getByPlaceholderText('Enter or choose a profile name') as HTMLInputElement;
        expect(nameInput.value).toBe(PROFILE_NAME);
        expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();
      },
      { timeout: 3000 }
    );

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('(c) crosshook-error-banner shows when profile_list rejects during refreshProfiles', async () => {
    // When profile_list throws, refreshProfiles catches and calls setError,
    // which causes the error banner to appear in the DOM.
    // We let the initial mount complete without error, then re-render with a throwing list.
    // Simpler approach: let profile_list throw on first call so refreshProfiles sets error.
    renderProfilesRoute({
      handlerOverrides: {
        ...baseOverrides,
        profile_list: async () => {
          throw new Error('profile list fetch failed intentionally');
        },
      },
    });

    // Wait for the page shell to appear
    await waitFor(() => {
      expect(document.querySelector('.crosshook-page-scroll-shell--profiles')).toBeInTheDocument();
    });

    // A crosshook-error-banner should appear after refreshProfiles rejects
    await waitFor(
      () => {
        const errorBanner = document.querySelector('.crosshook-error-banner');
        expect(errorBanner).toBeInTheDocument();
      },
      { timeout: 3000 }
    );

    // consoleErrorSpy may be called by the component's error handling — allow it
  });

  it('(d) ProtonDB section renders without errors when protondb commands are seeded', async () => {
    let profileListResolveFn: (() => void) | null = null;
    const profileListCalled = new Promise<void>((resolve) => {
      profileListResolveFn = resolve;
    });
    const user = userEvent.setup();

    renderProfilesRoute({
      handlerOverrides: {
        ...baseOverrides,
        profile_list: async () => {
          profileListResolveFn?.();
          return [];
        },
        profile_load: async () =>
          makeProfileDraft({
            game: { name: 'ProtonDB Game', executable_path: '/mock/game.exe' },
            steam: {
              enabled: true,
              app_id: '1245620',
              compatdata_path: '/mock/compatdata/1245620',
              proton_path: '/mock/proton',
              launcher: {
                icon_path: '',
                display_name: 'Steam',
              },
            },
            launch: {
              method: 'proton_run',
              optimizations: { enabled_option_ids: [] },
              custom_env_vars: {},
            },
          }),
        protondb_fetch_recommendations: async () => null,
      },
    });

    await waitFor(() => {
      expect(document.querySelector('.crosshook-page-scroll-shell--profiles')).toBeInTheDocument();
    });
    await profileListCalled;
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    emitMockEvent('auto-load-profile', 'ProtonDB Game');
    await waitFor(() => {
      const nameInput = screen.getByPlaceholderText('Enter or choose a profile name') as HTMLInputElement;
      expect(nameInput.value).toBe('ProtonDB Game');
    });
    await user.click(screen.getByRole('tab', { name: 'Runtime' }));
    await waitFor(() => {
      expect(screen.getByRole('tab', { name: 'Runtime' })).toHaveAttribute('data-state', 'active');
    });

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});
