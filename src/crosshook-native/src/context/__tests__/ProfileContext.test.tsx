import { waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ProfileProvider, useProfileContext } from '@/context/ProfileContext';
import { emitMockEvent } from '@/lib/events';
import { makeProfileDraft } from '@/test/fixtures';
import { renderWithMocks } from '@/test/render';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

const baseOverrides = {
  profile_list: async () => [],
  profile_list_summaries: async () => [],
  profile_list_favorites: async () => [],
  get_optimization_catalog: async () => null,
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

function ProfileNameProbe() {
  const { profileName, selectedProfile } = useProfileContext();
  return (
    <div>
      <span data-testid="profile-name">{profileName}</span>
      <span data-testid="selected-profile">{selectedProfile}</span>
    </div>
  );
}

describe('ProfileContext', () => {
  beforeEach(() => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.spyOn(console, 'debug').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('auto-load-profile populates profile name after initial refreshProfiles completes', async () => {
    const PROFILE_NAME = 'My New Game';

    let profileListResolveFn: (() => void) | null = null;
    const profileListCalled = new Promise<void>((resolve) => {
      profileListResolveFn = resolve;
    });

    const { getByTestId } = renderWithMocks(
      <ProfileProvider>
        <ProfileNameProbe />
      </ProfileProvider>,
      {
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
      }
    );

    await profileListCalled;
    await new Promise<void>((resolve) => setTimeout(resolve, 0));

    emitMockEvent('auto-load-profile', PROFILE_NAME);

    await waitFor(
      () => {
        expect(getByTestId('profile-name').textContent).toBe(PROFILE_NAME);
        expect(getByTestId('selected-profile').textContent).toBe(PROFILE_NAME);
      },
      { timeout: 3000 }
    );
  });
});
