import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ProtonInstallOption } from '@/types/proton';
import type { ProtonUpSuggestion } from '@/types/protonup';
import { useProfilesPageProton } from '../profiles/useProfilesPageProton';

const callCommandMock = vi.fn();
const getSuggestionMock = vi.fn();
const installVersionMock = vi.fn();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

vi.mock('@/hooks/useProtonUp', () => ({
  resolveProtonUpProviderForVersion: vi.fn(async () => 'ge-proton'),
  useProtonUp: () => ({
    versions: [],
    cacheMeta: null,
    catalogLoading: false,
    catalogError: null,
    refreshCatalog: vi.fn(),
    installVersion: installVersionMock,
    installing: false,
    getSuggestion: getSuggestionMock,
  }),
}));

const unsortedInstalls: ProtonInstallOption[] = [
  { name: 'GE-Proton9-2', path: '/compat/ge-2', is_official: false },
  { name: 'Proton Experimental', path: '/compat/experimental', is_official: true },
  { name: 'GE-Proton9-1', path: '/compat/ge-1', is_official: false },
];

const suggestion: ProtonUpSuggestion = {
  status: 'matched',
  community_version: 'GE-Proton9-1',
  matched_install_name: 'GE-Proton9-1',
  recommended_version: 'GE-Proton9-1',
};

describe('useProfilesPageProton', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    getSuggestionMock.mockReset();
    installVersionMock.mockReset();
    callCommandMock.mockImplementation((name: string) => {
      if (name === 'list_proton_installs') {
        return Promise.resolve(unsortedInstalls);
      }
      if (name === 'community_list_indexed_profiles') {
        return Promise.resolve([
          { game_name: 'Other Game', proton_version: 'GE-Proton8-32' },
          { game_name: 'Synthetic Quest', proton_version: 'GE-Proton9-1' },
        ]);
      }
      throw new Error(`[test-mock] unhandled command ${name}`);
    });
    getSuggestionMock.mockResolvedValue(suggestion);
  });

  it('loads sorted Proton installs and surfaces the community suggestion row', async () => {
    const { result } = renderHook(() =>
      useProfilesPageProton({
        effectiveSteamClientInstallPath: '/home/yandy/.steam',
        gameName: 'Synthetic Quest',
        selectedProfile: 'Synthetic Quest',
      })
    );

    await waitFor(() => {
      expect(result.current.protonInstalls.map((install) => install.name)).toEqual([
        'Proton Experimental',
        'GE-Proton9-1',
        'GE-Proton9-2',
      ]);
    });

    await waitFor(() => {
      expect(result.current.suggestion).toEqual(suggestion);
    });
    expect(callCommandMock).toHaveBeenCalledWith('list_proton_installs', {
      steamClientInstallPath: '/home/yandy/.steam',
    });
    expect(callCommandMock).toHaveBeenCalledWith('community_list_indexed_profiles', undefined);
    expect(getSuggestionMock).toHaveBeenCalledWith('GE-Proton9-1');
  });
});
