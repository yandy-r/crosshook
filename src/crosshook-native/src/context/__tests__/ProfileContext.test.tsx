import { act, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ProfileProvider, useProfileContext } from '@/context/ProfileContext';
import { makeProfileDraft } from '@/test/fixtures';
import { renderWithMocks } from '@/test/render';
import { makeStartupReport } from '@/test/startupReportFixture';
import { DEFAULT_UI_SETTINGS } from '@/types/settings';
import type { StartupReport } from '@/types/startup';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

const baseSettings = {
  auto_load_last_profile: false,
  last_used_profile: null,
  auto_install_prefix_deps: false,
  umu_preference: 'auto',
  steamgriddb_api_key: null,
  community_taps: [],
  steam_client_install_path: null,
  ui: { ...DEFAULT_UI_SETTINGS },
};

const baseOverrides = {
  profile_list: async () => [],
  profile_list_summaries: async () => [],
  profile_list_favorites: async () => [],
  get_optimization_catalog: async () => null,
  settings_load: async () => ({ ...baseSettings }),
  settings_save: async () => null,
  recent_files_load: async () => ({ game_paths: [], trainer_paths: [], dll_paths: [] }),
  recent_files_save: async () => null,
  get_startup_report: async () => makeStartupReport(),
};

function ProfileNameProbe() {
  const { profileName, selectedProfile, selectProfile } = useProfileContext();
  return (
    <div>
      <span data-testid="profile-name">{profileName}</span>
      <span data-testid="selected-profile">{selectedProfile}</span>
      <button type="button" data-testid="select-other" onClick={() => void selectProfile('User Picked')}>
        pick
      </button>
      <button type="button" data-testid="select-none" onClick={() => void selectProfile('')}>
        clear
      </button>
    </div>
  );
}

/** Crosses a macrotask boundary so every microtask queued so far settles. */
async function flushAsync(): Promise<void> {
  const { promise, resolve } = Promise.withResolvers<void>();
  setTimeout(resolve, 0);
  await act(async () => {
    await promise;
  });
}

/**
 * `profile_load` handler whose responses are held until released per profile
 * name, so a test decides which in-flight load resolves first.
 */
function makeGatedProfileLoad() {
  const resolvers = new Map<string, () => void>();
  const spy = vi.fn(async (args: unknown) => {
    const { name } = args as { name: string };
    const gate = Promise.withResolvers<void>();
    resolvers.set(name, gate.resolve);
    await gate.promise;
    return makeProfileDraft({ game: { name, executable_path: '/mock/game.exe' } });
  });
  return {
    spy,
    release(name: string): void {
      resolvers.get(name)?.();
    },
  };
}

describe('ProfileContext', () => {
  beforeEach(() => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.spyOn(console, 'debug').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('auto-loads when the startup report resolves late', async () => {
    const PROFILE_NAME = 'My New Game';
    const reportGate = Promise.withResolvers<StartupReport>();
    const getReport = vi.fn(async () => reportGate.promise);
    const profileLoad = vi.fn(async () =>
      makeProfileDraft({ game: { name: PROFILE_NAME, executable_path: '/mock/game.exe' } })
    );

    const { getByTestId } = renderWithMocks(
      <ProfileProvider>
        <ProfileNameProbe />
      </ProfileProvider>,
      {
        handlerOverrides: {
          ...baseOverrides,
          get_startup_report: getReport,
          profile_load: profileLoad,
        },
      }
    );

    // The report fetch started but has not resolved — nothing is selected yet.
    await waitFor(() => expect(getReport).toHaveBeenCalledTimes(1));
    await flushAsync();
    expect(getByTestId('selected-profile').textContent).toBe('');
    expect(profileLoad).not.toHaveBeenCalled();

    // The report resolves after a slow mount.
    await act(async () => {
      reportGate.resolve(makeStartupReport({ auto_load_profile: PROFILE_NAME }));
    });

    await waitFor(() => {
      expect(getByTestId('profile-name').textContent).toBe(PROFILE_NAME);
      expect(getByTestId('selected-profile').textContent).toBe(PROFILE_NAME);
    });
    expect(profileLoad).toHaveBeenCalledTimes(1);
    expect(profileLoad).toHaveBeenCalledWith({ name: PROFILE_NAME, collectionId: undefined });
  });

  it('auto-loads when the report resolves before profile_list', async () => {
    const PROFILE_NAME = 'Early Report Game';
    const listGate = Promise.withResolvers<string[]>();
    const profileLoad = vi.fn(async () =>
      makeProfileDraft({ game: { name: PROFILE_NAME, executable_path: '/mock/game.exe' } })
    );

    const { getByTestId } = renderWithMocks(
      <ProfileProvider>
        <ProfileNameProbe />
      </ProfileProvider>,
      {
        handlerOverrides: {
          ...baseOverrides,
          get_startup_report: async () => makeStartupReport({ auto_load_profile: PROFILE_NAME }),
          profile_list: async () => listGate.promise,
          profile_load: profileLoad,
        },
      }
    );

    await waitFor(() => {
      expect(getByTestId('selected-profile').textContent).toBe(PROFILE_NAME);
    });
    expect(profileLoad).toHaveBeenCalledTimes(1);

    await act(async () => {
      listGate.resolve([]);
    });

    // The empty-list guard must not wipe the auto-loaded selection.
    await waitFor(() => {
      expect(getByTestId('selected-profile').textContent).toBe(PROFILE_NAME);
    });
    expect(profileLoad).toHaveBeenCalledTimes(1);
  });

  it('auto-loads when profile_list resolves before the report', async () => {
    const PROFILE_NAME = 'Late Report Game';
    const reportGate = Promise.withResolvers<StartupReport>();
    let listResolved = false;
    const profileLoad = vi.fn(async () =>
      makeProfileDraft({ game: { name: PROFILE_NAME, executable_path: '/mock/game.exe' } })
    );

    const { getByTestId } = renderWithMocks(
      <ProfileProvider>
        <ProfileNameProbe />
      </ProfileProvider>,
      {
        handlerOverrides: {
          ...baseOverrides,
          get_startup_report: async () => reportGate.promise,
          profile_list: async () => {
            listResolved = true;
            return [];
          },
          profile_load: profileLoad,
        },
      }
    );

    await waitFor(() => expect(listResolved).toBe(true));
    await act(async () => {
      reportGate.resolve(makeStartupReport({ auto_load_profile: PROFILE_NAME }));
    });

    await waitFor(() => {
      expect(getByTestId('selected-profile').textContent).toBe(PROFILE_NAME);
    });
    expect(profileLoad).toHaveBeenCalledTimes(1);
  });

  it('does not fire a second load when the report resolves while the user pick is in flight', async () => {
    const AUTO = 'Auto Game';
    const USER = 'User Picked';
    const reportGate = Promise.withResolvers<StartupReport>();
    const getReport = vi.fn(async () => reportGate.promise);
    const { spy: profileLoad, release } = makeGatedProfileLoad();

    const { getByTestId } = renderWithMocks(
      <ProfileProvider>
        <ProfileNameProbe />
      </ProfileProvider>,
      {
        handlerOverrides: {
          ...baseOverrides,
          get_startup_report: getReport,
          profile_load: profileLoad,
        },
      }
    );

    await waitFor(() => expect(getReport).toHaveBeenCalledTimes(1));

    // The user picks a profile; its load is held in flight.
    await act(async () => {
      getByTestId('select-other').click();
    });
    await waitFor(() => expect(profileLoad).toHaveBeenCalledTimes(1));
    expect(profileLoad).toHaveBeenCalledWith({ name: USER, collectionId: undefined });

    // The startup report resolves while the user's load is still pending.
    await act(async () => {
      reportGate.resolve(makeStartupReport({ auto_load_profile: AUTO }));
    });
    await flushAsync();

    // The auto-load must not race the in-flight user pick: no second load.
    expect(profileLoad).toHaveBeenCalledTimes(1);

    // The user's load lands — their pick wins.
    await act(async () => {
      release(USER);
    });
    await waitFor(() => expect(getByTestId('selected-profile').textContent).toBe(USER));
    expect(getByTestId('profile-name').textContent).toBe(USER);
    expect(profileLoad).toHaveBeenCalledTimes(1);
    expect(profileLoad).toHaveBeenCalledWith({ name: USER, collectionId: undefined });
  });

  it('keeps the user pick when a stale auto-load response lands after it', async () => {
    const AUTO = 'Auto Game';
    const USER = 'User Picked';
    const reportGate = Promise.withResolvers<StartupReport>();
    const { spy: profileLoad, release } = makeGatedProfileLoad();

    const { getByTestId } = renderWithMocks(
      <ProfileProvider>
        <ProfileNameProbe />
      </ProfileProvider>,
      {
        handlerOverrides: {
          ...baseOverrides,
          get_startup_report: async () => reportGate.promise,
          profile_load: profileLoad,
        },
      }
    );

    // The report resolves first: the auto-load starts and is held in flight.
    await act(async () => {
      reportGate.resolve(makeStartupReport({ auto_load_profile: AUTO }));
    });
    await waitFor(() => {
      expect(profileLoad).toHaveBeenCalledTimes(1);
      expect(profileLoad).toHaveBeenCalledWith({ name: AUTO, collectionId: undefined });
    });

    // The user picks another profile while the auto-load is still pending.
    await act(async () => {
      getByTestId('select-other').click();
    });
    await waitFor(() => expect(profileLoad).toHaveBeenCalledTimes(2));

    // The user's load lands first…
    await act(async () => {
      release(USER);
    });
    await waitFor(() => expect(getByTestId('selected-profile').textContent).toBe(USER));

    // …then the stale auto-load response lands. It must not steal the selection.
    await act(async () => {
      release(AUTO);
    });
    await flushAsync();
    expect(getByTestId('selected-profile').textContent).toBe(USER);
    expect(getByTestId('profile-name').textContent).toBe(USER);
    expect(profileLoad).toHaveBeenCalledTimes(2);
  });

  it('ignores an in-flight load after an explicit deselection', async () => {
    const USER = 'User Picked';
    const { spy: profileLoad, release } = makeGatedProfileLoad();

    const { getByTestId } = renderWithMocks(
      <ProfileProvider>
        <ProfileNameProbe />
      </ProfileProvider>,
      {
        handlerOverrides: {
          ...baseOverrides,
          profile_load: profileLoad,
        },
      }
    );

    // The user picks a profile; its load is held in flight.
    await act(async () => {
      getByTestId('select-other').click();
    });
    await waitFor(() => expect(profileLoad).toHaveBeenCalledTimes(1));

    // The user deselects (e.g. the onboarding wizard reset) while it is pending.
    await act(async () => {
      getByTestId('select-none').click();
    });
    await flushAsync();
    expect(getByTestId('selected-profile').textContent).toBe('');
    expect(getByTestId('profile-name').textContent).toBe('');

    // The stale load lands; it must not resurrect the selection.
    await act(async () => {
      release(USER);
    });
    await flushAsync();
    expect(getByTestId('selected-profile').textContent).toBe('');
    expect(getByTestId('profile-name').textContent).toBe('');
  });

  it('does not auto-load when the report has no auto_load_profile', async () => {
    const getReport = vi.fn(async () => makeStartupReport());
    const profileLoad = vi.fn(async () => makeProfileDraft());

    const { getByTestId } = renderWithMocks(
      <ProfileProvider>
        <ProfileNameProbe />
      </ProfileProvider>,
      {
        handlerOverrides: {
          ...baseOverrides,
          get_startup_report: getReport,
          profile_load: profileLoad,
        },
      }
    );

    // Wait for the report to actually resolve before asserting nothing happened.
    await waitFor(() => expect(getReport).toHaveBeenCalledTimes(1));
    await flushAsync();

    expect(profileLoad).not.toHaveBeenCalled();
    expect(getByTestId('selected-profile').textContent).toBe('');
  });

  it('does not override a completed user selection when the report resolves', async () => {
    const AUTO = 'Auto Game';
    const USER = 'User Picked';
    const reportGate = Promise.withResolvers<StartupReport>();
    const getReport = vi.fn(async () => reportGate.promise);
    const profileLoad = vi.fn(async (args: unknown) => {
      const { name } = args as { name: string };
      return makeProfileDraft({ game: { name, executable_path: '/mock/game.exe' } });
    });

    const { getByTestId } = renderWithMocks(
      <ProfileProvider>
        <ProfileNameProbe />
      </ProfileProvider>,
      {
        handlerOverrides: {
          ...baseOverrides,
          get_startup_report: getReport,
          profile_load: profileLoad,
        },
      }
    );

    await waitFor(() => expect(getReport).toHaveBeenCalledTimes(1));

    // The user picks and their load completes before the report resolves.
    await act(async () => {
      getByTestId('select-other').click();
    });
    await waitFor(() => expect(getByTestId('selected-profile').textContent).toBe(USER));

    await act(async () => {
      reportGate.resolve(makeStartupReport({ auto_load_profile: AUTO }));
    });
    await flushAsync();

    // The auto-load never fires; only the user's load was requested.
    expect(profileLoad).toHaveBeenCalledTimes(1);
    expect(profileLoad).toHaveBeenCalledWith({ name: USER, collectionId: undefined });
    expect(getByTestId('selected-profile').textContent).toBe(USER);
  });
});
