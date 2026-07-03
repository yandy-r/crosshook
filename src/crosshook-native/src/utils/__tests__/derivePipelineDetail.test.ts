import { describe, expect, it } from 'vitest';
import { makeLaunchPreview, makePreviewWrapperDetail, makeUmuDecisionPreview } from '@/test/fixtures';
import {
  deriveEnvGroups,
  deriveWrapperChain,
  detailSummaryMeta,
  type EnvGroup,
  formatEnvGroupClipboard,
} from '../derivePipelineDetail';

describe('deriveEnvGroups', () => {
  it('orders groups host → steam_proton → proton_runtime → launch_optimization → profile_custom', () => {
    const preview = makeLaunchPreview({
      environment: [
        { key: 'DXVK_HUD', value: 'fps', source: 'profile_custom' },
        { key: 'PROTON_NO_ESYNC', value: '1', source: 'launch_optimization' },
        { key: 'STEAM_COMPAT_CLIENT_INSTALL_PATH', value: '/steam/root', source: 'steam_proton' },
        { key: 'WINEPREFIX', value: '/prefixes/synthetic-quest', source: 'proton_runtime' },
        { key: 'DISPLAY', value: ':0', source: 'host' },
      ],
      cleared_variables: [],
    });

    expect(deriveEnvGroups(preview).map((group) => group.id)).toEqual([
      'host',
      'steam_proton',
      'proton_runtime',
      'launch_optimization',
      'profile_custom',
    ]);
  });

  it('omits empty groups', () => {
    const preview = makeLaunchPreview({
      environment: [
        { key: 'DISPLAY', value: ':0', source: 'host' },
        { key: 'DXVK_HUD', value: 'fps', source: 'profile_custom' },
      ],
      cleared_variables: [],
    });

    expect(deriveEnvGroups(preview).map((group) => group.id)).toEqual(['host', 'profile_custom']);
  });

  it('appends a cleared group with null values from cleared_variables', () => {
    const groups = deriveEnvGroups(makeLaunchPreview());
    const cleared = groups[groups.length - 1];

    expect(cleared).toEqual({
      id: 'cleared',
      label: 'Cleared before launch',
      vars: [
        { key: 'WINEDLLOVERRIDES', value: null },
        { key: 'WINEESYNC', value: null },
      ],
      cleared: true,
    });
  });

  it('returns [] when environment is null', () => {
    expect(deriveEnvGroups(makeLaunchPreview({ environment: null }))).toEqual([]);
  });
});

describe('formatEnvGroupClipboard', () => {
  it('emits KEY=value lines for regular groups', () => {
    const group: EnvGroup = {
      id: 'proton_runtime',
      label: 'Proton runtime',
      vars: [
        { key: 'WINEPREFIX', value: '/prefixes/synthetic-quest' },
        { key: 'PROTON_VERB', value: 'waitforexitandrun' },
      ],
      cleared: false,
    };

    expect(formatEnvGroupClipboard(group)).toBe('WINEPREFIX=/prefixes/synthetic-quest\nPROTON_VERB=waitforexitandrun');
  });

  it('emits unset KEY lines for the cleared group', () => {
    const group: EnvGroup = {
      id: 'cleared',
      label: 'Cleared before launch',
      vars: [
        { key: 'WINEDLLOVERRIDES', value: null },
        { key: 'WINEESYNC', value: null },
      ],
      cleared: true,
    };

    expect(formatEnvGroupClipboard(group)).toBe('unset WINEDLLOVERRIDES\nunset WINEESYNC');
  });
});

describe('deriveWrapperChain', () => {
  it('maps wrapper_details order, active flags, and reasons verbatim', () => {
    const preview = makeLaunchPreview({
      wrapper_details: [
        makePreviewWrapperDetail({
          command: ['unshare', '--net'],
          source: 'network_isolation',
          active: false,
          reason: 'unshare --net is not available on the host',
          optimization_id: null,
          optimization_label: null,
        }),
        makePreviewWrapperDetail(),
        makePreviewWrapperDetail({
          command: ['gamescope', '-f', '--'],
          source: 'gamescope',
          active: false,
          reason: 'skipped: already inside a gamescope session and nested gamescope is not allowed',
          optimization_id: null,
          optimization_label: null,
        }),
      ],
      umu_decision: null,
    });

    const chain = deriveWrapperChain(preview);
    expect(chain?.slice(0, 3)).toEqual([
      {
        id: 'isolation-0',
        kind: 'isolation',
        token: 'unshare --net',
        active: false,
        reason: 'unshare --net is not available on the host',
      },
      {
        id: 'optimization-1',
        kind: 'optimization',
        token: 'gamemoderun',
        active: true,
        reason: "Enabled by launch optimization 'Use GameMode'",
      },
      {
        id: 'gamescope-2',
        kind: 'gamescope',
        token: 'gamescope -f --',
        active: false,
        reason: 'skipped: already inside a gamescope session and nested gamescope is not allowed',
      },
    ]);
  });

  it('passes folded_into through as foldedInto on chain items', () => {
    const preview = makeLaunchPreview({
      wrapper_details: [
        makePreviewWrapperDetail({
          command: ['mangohud'],
          reason: "Enabled by launch optimization 'Show MangoHud overlay'",
          folded_into: 'gamescope --mangoapp',
        }),
      ],
      umu_decision: null,
    });

    const row = deriveWrapperChain(preview)?.[0];
    expect(row?.reason).toBe("Enabled by launch optimization 'Show MangoHud overlay'");
    expect(row?.foldedInto).toBe('gamescope --mangoapp');
    expect(row?.active).toBe(true);
  });

  it('leaves foldedInto undefined when folded_into is null or absent', () => {
    const preview = makeLaunchPreview({
      wrapper_details: [
        makePreviewWrapperDetail({ folded_into: null }),
        makePreviewWrapperDetail({ folded_into: undefined }),
      ],
      umu_decision: null,
    });

    const chain = deriveWrapperChain(preview);
    expect(chain?.[0].foldedInto).toBeUndefined();
    expect(chain?.[1].foldedInto).toBeUndefined();
  });

  it('appends an active umu-run runtime row from umu_decision', () => {
    const preview = makeLaunchPreview({ umu_decision: makeUmuDecisionPreview() });

    const chain = deriveWrapperChain(preview);
    expect(chain?.at(-1)).toEqual({
      id: 'runtime-umu',
      kind: 'runtime',
      token: 'umu-run',
      active: true,
      reason: 'using umu-run at /usr/bin/umu-run',
      detail: 'csv coverage: found',
    });
  });

  it('includes the GAMEID resolution summary in the umu detail line', () => {
    const preview = makeLaunchPreview({
      umu_decision: makeUmuDecisionPreview({
        gameid_resolution: {
          game_id: 'umu-9999001',
          store: 'steam',
          source: 'steam_app_id',
          lookup_key: null,
          fetched_at: null,
          expires_at: null,
          error_category: null,
        },
      }),
    });

    const umuRow = deriveWrapperChain(preview)?.find((item) => item.id === 'runtime-umu');
    expect(umuRow?.detail).toBe('csv coverage: found · GAMEID umu-9999001 (Steam app id)');
  });

  it('appends a direct-Proton fallback row when will_use_umu is false', () => {
    const preview = makeLaunchPreview({
      umu_decision: makeUmuDecisionPreview({
        will_use_umu: false,
        umu_run_path_on_backend_path: null,
        reason: 'preference = Proton — direct Proton always',
      }),
    });

    const chain = deriveWrapperChain(preview);
    const umuRow = chain?.find((item) => item.id === 'runtime-umu');
    expect(umuRow?.active).toBe(false);
    expect(chain?.at(-1)).toEqual({
      id: 'runtime-proton',
      kind: 'runtime',
      token: 'proton',
      active: true,
      reason: 'Direct Proton launch',
    });
  });

  it('labels the proton runtime row as Steam-launched for steam_applaunch previews', () => {
    const preview = makeLaunchPreview({
      resolved_method: 'steam_applaunch',
      umu_decision: null,
    });

    expect(deriveWrapperChain(preview)?.at(-1)).toEqual({
      id: 'runtime-proton',
      kind: 'runtime',
      token: 'proton',
      active: true,
      reason: 'Launched through Steam',
    });
  });

  it('returns null when wrapper_details is absent', () => {
    expect(deriveWrapperChain(makeLaunchPreview({ wrapper_details: null }))).toBeNull();
    expect(deriveWrapperChain(makeLaunchPreview({ wrapper_details: undefined }))).toBeNull();
  });

  it('yields no runtime rows for native previews', () => {
    const preview = makeLaunchPreview({
      resolved_method: 'native',
      wrapper_details: [],
      proton_setup: null,
      umu_decision: null,
    });

    expect(deriveWrapperChain(preview)).toEqual([]);
  });
});

describe('detailSummaryMeta', () => {
  it('composes env count, active wrapper count, and umu summary', () => {
    const preview = makeLaunchPreview({
      umu_decision: makeUmuDecisionPreview({
        will_use_umu: false,
        umu_run_path_on_backend_path: null,
        reason: 'preference = Proton — direct Proton always',
      }),
    });
    const groups = deriveEnvGroups(preview);
    const chain = deriveWrapperChain(preview);

    expect(detailSummaryMeta(groups, chain, preview.umu_decision)).toBe('4 env vars · 1 wrapper · umu: direct Proton');
  });

  it('excludes cleared variables and inactive wrappers from counts', () => {
    const preview = makeLaunchPreview({
      environment: [{ key: 'DISPLAY', value: ':0', source: 'host' }],
      wrapper_details: [makePreviewWrapperDetail({ active: false })],
      umu_decision: null,
      proton_setup: null,
    });
    const groups = deriveEnvGroups(preview);
    const chain = deriveWrapperChain(preview);

    expect(detailSummaryMeta(groups, chain, null)).toBe('1 env var · 0 wrappers');
  });

  it('excludes wrappers folded into another invocation from the active count', () => {
    const preview = makeLaunchPreview({
      environment: [{ key: 'DISPLAY', value: ':0', source: 'host' }],
      wrapper_details: [
        makePreviewWrapperDetail({
          command: ['gamescope', '--'],
          source: 'gamescope',
          reason: 'active',
          optimization_id: null,
          optimization_label: null,
        }),
        makePreviewWrapperDetail({ command: ['mangohud'], folded_into: 'gamescope --mangoapp' }),
      ],
      umu_decision: null,
      proton_setup: null,
    });
    const groups = deriveEnvGroups(preview);
    const chain = deriveWrapperChain(preview);

    expect(detailSummaryMeta(groups, chain, null)).toBe('1 env var · 1 wrapper');
  });

  it('omits the wrapper count when the chain is null', () => {
    expect(detailSummaryMeta([], null, undefined)).toBe('0 env vars');
  });
});
