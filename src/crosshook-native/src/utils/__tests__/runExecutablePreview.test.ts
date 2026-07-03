import { describe, expect, it } from 'vitest';
import type { RunExecutableRequest } from '@/types/run-executable';
import { buildRunExecutablePreviewRequest } from '../runExecutablePreview';

function makeRunExecutableRequest(overrides: Partial<RunExecutableRequest> = {}): RunExecutableRequest {
  return {
    executable_path: '/mnt/media/setup.exe',
    proton_path: '/compatibilitytools/proton-ge/proton',
    prefix_path: '/prefixes/adhoc',
    working_directory: '/mnt/media',
    steam_client_install_path: '/steam/root',
    ...overrides,
  };
}

describe('buildRunExecutablePreviewRequest', () => {
  it('returns null until executable_path is non-empty', () => {
    expect(buildRunExecutablePreviewRequest(makeRunExecutableRequest({ executable_path: '' }), 'auto')).toBeNull();
    expect(buildRunExecutablePreviewRequest(makeRunExecutableRequest({ executable_path: '   ' }), 'auto')).toBeNull();
  });

  it('returns null until proton_path is non-empty', () => {
    expect(buildRunExecutablePreviewRequest(makeRunExecutableRequest({ proton_path: '' }), 'auto')).toBeNull();
    expect(buildRunExecutablePreviewRequest(makeRunExecutableRequest({ proton_path: '   ' }), 'auto')).toBeNull();
  });

  it('maps executable, proton, prefix, working directory, and steam client install path', () => {
    const request = buildRunExecutablePreviewRequest(
      makeRunExecutableRequest({
        executable_path: '  /mnt/media/setup.exe  ',
        prefix_path: ' /prefixes/adhoc ',
        working_directory: ' /mnt/media ',
        steam_client_install_path: ' /steam/root ',
      }),
      'auto'
    );

    expect(request).not.toBeNull();
    expect(request?.method).toBe('proton_run');
    expect(request?.game_path).toBe('/mnt/media/setup.exe');
    expect(request?.runtime.proton_path).toBe('/compatibilitytools/proton-ge/proton');
    expect(request?.runtime.prefix_path).toBe('/prefixes/adhoc');
    expect(request?.runtime.working_directory).toBe('/mnt/media');
    expect(request?.steam.steam_client_install_path).toBe('/steam/root');
  });

  it('sets launch_game_only true and network_isolation false with empty trainer and optimizations', () => {
    const request = buildRunExecutablePreviewRequest(makeRunExecutableRequest(), 'auto');

    expect(request?.launch_game_only).toBe(true);
    expect(request?.launch_trainer_only).toBe(false);
    expect(request?.network_isolation).toBe(false);
    expect(request?.trainer_path).toBe('');
    expect(request?.trainer_host_path).toBe('');
    expect(request?.optimizations).toEqual({ enabled_option_ids: [] });
    expect(request?.custom_env_vars).toEqual({});
    expect(request?.gamescope?.enabled).toBe(false);
    expect(request?.mangohud?.enabled).toBe(false);
  });

  it('passes the umu preference through', () => {
    expect(buildRunExecutablePreviewRequest(makeRunExecutableRequest(), 'umu')?.umu_preference).toBe('umu');
    expect(buildRunExecutablePreviewRequest(makeRunExecutableRequest(), 'proton')?.umu_preference).toBe('proton');
  });
});
