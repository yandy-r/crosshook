/**
 * Parity fixtures for the Rust port in `crates/crosshook-view-model` (launch
 * pipeline, pipeline detail, validation mapping, Hero Detail model).
 *
 * Each case runs the real TypeScript function and records input and output.
 * By default this test fails when the committed JSON under
 * `crates/crosshook-view-model/tests/fixtures/ts_parity/` differs from what
 * TypeScript produces now, so a behaviour change in either copy is caught.
 * `scripts/export-ts-parity-fixtures.sh` sets `CROSSHOOK_WRITE_PARITY_FIXTURES=1`
 * to rewrite the files instead.
 */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  displayPath,
  HERO_DETAIL_DEFAULT_TAB,
  HERO_DETAIL_TABS,
  isHeroDetailTabId,
  resolveGameDetailsHero,
} from '@/components/library/hero-detail-model';
import { makeLaunchPreview, makePreviewWrapperDetail, makeProfileDraft, makeUmuDecisionPreview } from '@/test/fixtures';
import {
  type EnvVarSource,
  LaunchPhase,
  type LaunchPreview,
  type LaunchValidationIssue,
  type PipelineNodeId,
  type UmuGameIdResolutionSource,
} from '@/types/launch';
import type { GameProfile } from '@/types/profile';
import {
  deriveEnvGroups,
  deriveWrapperChain,
  detailSummaryMeta,
  type EnvGroup,
  formatEnvGroupClipboard,
} from '../derivePipelineDetail';
import { derivePipelineNodes } from '../derivePipelineNodes';
import type { ResolvedLaunchMethod } from '../launch';
import { envSourceLabel, umuGameIdResolutionSourceLabel } from '../launchPreviewPresentation';
import { mapValidationToNode } from '../mapValidationToNode';

const FIXTURE_DIR = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../../crates/crosshook-view-model/tests/fixtures/ts_parity'
);
const WRITE = process.env.CROSSHOOK_WRITE_PARITY_FIXTURES === '1';
if (WRITE && process.env.CI) {
  throw new Error('CROSSHOOK_WRITE_PARITY_FIXTURES must not be set in CI: it would hide fixture drift.');
}

/** Writes (export mode) or compares (default) one fixture file. */
function checkFixture(name: string, data: unknown): void {
  const file = path.join(FIXTURE_DIR, `${name}.json`);
  const text = `${JSON.stringify(data, null, 2)}\n`;
  if (WRITE) {
    mkdirSync(FIXTURE_DIR, { recursive: true });
    writeFileSync(file, text);
    return;
  }
  expect(JSON.parse(readFileSync(file, 'utf8'))).toEqual(JSON.parse(text));
}

/** Profile fields the tier-1 pipeline reads (mirrors Rust `PipelineProfile`). */
interface PipelineProfileInput {
  game_executable_path: string;
  prefix_path: string;
  proton_path: string;
  steam_app_id: string;
  trainer_path: string;
  enabled_optimizations: number;
}

const FULL_PROFILE: PipelineProfileInput = {
  game_executable_path: '/games/synthetic-quest/game.exe',
  prefix_path: '/prefixes/synthetic-quest',
  proton_path: '/compat/proton',
  steam_app_id: '9999001',
  trainer_path: '/trainers/synthetic.exe',
  enabled_optimizations: 2,
};

function toGameProfile(input: PipelineProfileInput): GameProfile {
  const profile = makeProfileDraft({
    game: { name: 'Synthetic Quest', executable_path: input.game_executable_path } as GameProfile['game'],
    runtime: { prefix_path: input.prefix_path, proton_path: input.proton_path } as GameProfile['runtime'],
    steam: { app_id: input.steam_app_id } as GameProfile['steam'],
    trainer: { path: input.trainer_path } as GameProfile['trainer'],
  });
  profile.launch.optimizations = {
    ...profile.launch.optimizations,
    enabled_option_ids: Array.from({ length: input.enabled_optimizations }, (_, i) => `option_${i}`),
  };
  return profile;
}

function issue(code: string | undefined, severity: LaunchValidationIssue['severity'], message: string) {
  return { message, help: 'help', severity, code };
}

const TRAINER_PREVIEW = { trainer: { path: '/trainers/synthetic.exe' } } as Partial<LaunchPreview>;
const STEAM_PREVIEW: Partial<LaunchPreview> = {
  resolved_method: 'steam_applaunch',
  steam_launch_options: 'PROTON_LOG=1 %command%',
};
const NATIVE_PREVIEW: Partial<LaunchPreview> = {
  resolved_method: 'native',
  environment: null,
  wrappers: null,
  wrapper_details: null,
  proton_setup: null,
  cleared_variables: [],
};
const BLOCKED_PREVIEW: Partial<LaunchPreview> = {
  validation: {
    issues: [
      issue('runtime_prefix_missing', 'warning', 'Prefix warning'),
      issue('runtime_prefix_missing', 'fatal', 'Prefix missing'),
      issue('trainer_path_missing', 'fatal', 'Trainer missing'),
      issue(undefined, 'warning', 'Generic warning'),
    ],
  },
};

interface PipelineCase {
  name: string;
  method: ResolvedLaunchMethod;
  profile: PipelineProfileInput | null;
  preview: Partial<LaunchPreview> | null;
  phase: LaunchPhase;
  omit?: PipelineNodeId[];
}

const PIPELINE_CASES: PipelineCase[] = [
  // derivePipelineNodes.test.ts
  {
    name: 'omits nodes (tier 1)',
    method: 'proton_run',
    profile: null,
    preview: null,
    phase: LaunchPhase.Idle,
    omit: ['trainer', 'optimizations'],
  },
  {
    name: 'launch ignores omitted nodes',
    method: 'proton_run',
    profile: null,
    preview: {},
    phase: LaunchPhase.Idle,
    omit: ['trainer', 'optimizations'],
  },
  { name: 'unresolved trainer pins launch', method: 'proton_run', profile: null, preview: {}, phase: LaunchPhase.Idle },
  { name: 'null profile tier 1', method: 'proton_run', profile: null, preview: null, phase: LaunchPhase.Idle },
  {
    name: 'game launching with omitted nodes',
    method: 'proton_run',
    profile: null,
    preview: {},
    phase: LaunchPhase.GameLaunching,
    omit: ['trainer', 'optimizations'],
  },
  // State matrix: idle/validating (tier 1), blocked, launching, running, exited.
  {
    name: 'idle configured tier 1',
    method: 'proton_run',
    profile: FULL_PROFILE,
    preview: null,
    phase: LaunchPhase.Idle,
  },
  {
    name: 'idle partial tier 1',
    method: 'proton_run',
    profile: { ...FULL_PROFILE, proton_path: '  ', enabled_optimizations: 0 },
    preview: null,
    phase: LaunchPhase.Idle,
  },
  {
    name: 'validating steam tier 1',
    method: 'steam_applaunch',
    profile: FULL_PROFILE,
    preview: null,
    phase: LaunchPhase.Idle,
  },
  { name: 'validating native tier 1', method: 'native', profile: FULL_PROFILE, preview: null, phase: LaunchPhase.Idle },
  {
    name: 'blocked by fatal issues',
    method: 'proton_run',
    profile: FULL_PROFILE,
    preview: { ...TRAINER_PREVIEW, ...BLOCKED_PREVIEW },
    phase: LaunchPhase.Idle,
  },
  {
    name: 'blocked by launch issue',
    method: 'proton_run',
    profile: FULL_PROFILE,
    preview: { ...TRAINER_PREVIEW, validation: { issues: [issue('', 'fatal', 'Launch blocked')] } },
    phase: LaunchPhase.Idle,
  },
  {
    name: 'directives error',
    method: 'proton_run',
    profile: FULL_PROFILE,
    preview: {
      ...TRAINER_PREVIEW,
      directives_error: 'bad directive',
      environment: null,
      wrappers: null,
      effective_command: null,
    },
    phase: LaunchPhase.Idle,
  },
  {
    name: 'no effective command',
    method: 'proton_run',
    profile: FULL_PROFILE,
    preview: { ...TRAINER_PREVIEW, effective_command: '  ' },
    phase: LaunchPhase.Idle,
  },
  {
    name: 'trainer attached ready',
    method: 'proton_run',
    profile: FULL_PROFILE,
    preview: TRAINER_PREVIEW,
    phase: LaunchPhase.Idle,
  },
  {
    name: 'launching game',
    method: 'proton_run',
    profile: FULL_PROFILE,
    preview: TRAINER_PREVIEW,
    phase: LaunchPhase.GameLaunching,
  },
  {
    name: 'waiting for trainer',
    method: 'proton_run',
    profile: FULL_PROFILE,
    preview: TRAINER_PREVIEW,
    phase: LaunchPhase.WaitingForTrainer,
  },
  {
    name: 'trainer launching',
    method: 'proton_run',
    profile: FULL_PROFILE,
    preview: TRAINER_PREVIEW,
    phase: LaunchPhase.TrainerLaunching,
  },
  {
    name: 'running',
    method: 'proton_run',
    profile: FULL_PROFILE,
    preview: TRAINER_PREVIEW,
    phase: LaunchPhase.SessionActive,
  },
  { name: 'exited ok', method: 'proton_run', profile: FULL_PROFILE, preview: TRAINER_PREVIEW, phase: LaunchPhase.Idle },
  {
    name: 'exited with error',
    method: 'proton_run',
    profile: FULL_PROFILE,
    preview: { ...TRAINER_PREVIEW, ...BLOCKED_PREVIEW },
    phase: LaunchPhase.Idle,
  },
  {
    name: 'steam ready',
    method: 'steam_applaunch',
    profile: FULL_PROFILE,
    preview: { ...TRAINER_PREVIEW, ...STEAM_PREVIEW },
    phase: LaunchPhase.Idle,
  },
  {
    name: 'steam waiting for trainer',
    method: 'steam_applaunch',
    profile: FULL_PROFILE,
    preview: { ...TRAINER_PREVIEW, ...STEAM_PREVIEW },
    phase: LaunchPhase.WaitingForTrainer,
  },
  {
    name: 'steam method on proton preview',
    method: 'steam_applaunch',
    profile: FULL_PROFILE,
    preview: { ...TRAINER_PREVIEW, steam_launch_options: '' },
    phase: LaunchPhase.Idle,
  },
  { name: 'native ready', method: 'native', profile: FULL_PROFILE, preview: NATIVE_PREVIEW, phase: LaunchPhase.Idle },
  {
    name: 'native with trainer running',
    method: 'native',
    profile: FULL_PROFILE,
    preview: { ...NATIVE_PREVIEW, ...TRAINER_PREVIEW },
    phase: LaunchPhase.SessionActive,
  },
  {
    name: 'game falls back to executable path',
    method: 'proton_run',
    profile: FULL_PROFILE,
    preview: {
      game_executable_name: ' ',
      game_executable: 'C:\\games\\quest.exe',
      proton_setup: { wine_prefix_path: '  ', proton_executable: '' } as LaunchPreview['proton_setup'],
    },
    phase: LaunchPhase.Idle,
  },
  {
    name: 'empty environment',
    method: 'proton_run',
    profile: FULL_PROFILE,
    preview: { environment: [], wrappers: null },
    phase: LaunchPhase.Idle,
  },
];

function pipelineFixtures() {
  return PIPELINE_CASES.map((c) => {
    const preview = c.preview ? makeLaunchPreview(c.preview) : null;
    const profile = c.profile ? toGameProfile(c.profile) : null;
    return {
      name: c.name,
      method: c.method,
      profile: c.profile,
      preview,
      phase: c.phase,
      omit: c.omit ?? [],
      expected: derivePipelineNodes(c.method, profile, preview, c.phase, { omitNodes: c.omit }),
    };
  });
}

const DETAIL_CASES: { name: string; preview: Partial<LaunchPreview> }[] = [
  {
    name: 'group order',
    preview: {
      environment: [
        { key: 'DXVK_HUD', value: 'fps', source: 'profile_custom' },
        { key: 'PROTON_NO_ESYNC', value: '1', source: 'launch_optimization' },
        { key: 'STEAM_COMPAT_CLIENT_INSTALL_PATH', value: '/steam/root', source: 'steam_proton' },
        { key: 'WINEPREFIX', value: '/prefixes/synthetic-quest', source: 'proton_runtime' },
        { key: 'DISPLAY', value: ':0', source: 'host' },
        { key: 'XDG_RUNTIME_DIR', value: '/run/user/1000', source: 'host' },
      ],
      cleared_variables: [],
    },
  },
  { name: 'default preview', preview: {} },
  { name: 'null environment', preview: { environment: null } },
  {
    name: 'wrapper order and flags',
    preview: {
      wrapper_details: [
        makePreviewWrapperDetail({
          command: ['unshare', '--net'],
          source: 'network_isolation',
          active: false,
          reason: 'unshare --net is not available on the host',
        }),
        makePreviewWrapperDetail(),
        makePreviewWrapperDetail({
          command: ['gamescope', '-f', '--'],
          source: 'gamescope',
          active: false,
          reason: 'skipped',
        }),
      ],
    },
  },
  {
    name: 'folded wrappers',
    preview: {
      wrapper_details: [
        makePreviewWrapperDetail({ command: ['gamescope', '--'], source: 'gamescope', reason: 'active' }),
        makePreviewWrapperDetail({ command: ['mangohud'], folded_into: 'gamescope --mangoapp' }),
        makePreviewWrapperDetail({ folded_into: undefined }),
      ],
    },
  },
  { name: 'umu row', preview: { umu_decision: makeUmuDecisionPreview() } },
  {
    name: 'umu gameid',
    preview: {
      umu_decision: makeUmuDecisionPreview({
        csv_coverage: 'missing',
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
    },
  },
  {
    name: 'umu direct proton fallback',
    preview: {
      umu_decision: makeUmuDecisionPreview({
        will_use_umu: false,
        reason: 'preference = Proton — direct Proton always',
      }),
    },
  },
  { name: 'steam runtime row', preview: { resolved_method: 'steam_applaunch' } },
  { name: 'null wrapper details', preview: { wrapper_details: null } },
  { name: 'native no runtime rows', preview: { resolved_method: 'native', wrapper_details: [], proton_setup: null } },
  {
    name: 'single env var, inactive wrapper',
    preview: {
      environment: [{ key: 'DISPLAY', value: ':0', source: 'host' }],
      wrapper_details: [makePreviewWrapperDetail({ active: false })],
      proton_setup: null,
    },
  },
];

function detailFixtures() {
  const cases = DETAIL_CASES.map((c) => {
    const preview = makeLaunchPreview(c.preview);
    const groups = deriveEnvGroups(preview);
    const chain = deriveWrapperChain(preview);
    return {
      name: c.name,
      preview,
      expected: {
        env_groups: groups,
        clipboard: groups.map(formatEnvGroupClipboard),
        wrapper_chain: chain,
        summary_meta: detailSummaryMeta(groups, chain, preview.umu_decision),
      },
    };
  });
  const emptyGroups: EnvGroup[] = [];
  const envSources: EnvVarSource[] = [
    'proton_runtime',
    'launch_optimization',
    'host',
    'steam_proton',
    'profile_custom',
  ];
  const gameIdSources: UmuGameIdResolutionSource[] = [
    'explicit_override',
    'steam_app_id',
    'fresh_cache',
    'fresh_lookup',
    'stale_cache',
    'cached_not_found',
    'lookup_disabled',
    'missing_hints',
    'api_unavailable',
    'fallback',
  ];
  return {
    cases,
    empty_summary_meta: detailSummaryMeta(emptyGroups, null, undefined),
    env_source_labels: envSources.map((source) => ({ source, label: envSourceLabel(source) })),
    gameid_source_labels: gameIdSources.map((source) => ({ source, label: umuGameIdResolutionSourceLabel(source) })),
  };
}

const VALIDATION_CODES: (string | null)[] = [
  null,
  '',
  '   ',
  'game_path_missing',
  'native_windows_executable',
  'runtime_prefix_path_missing',
  'runtime_proton_path_missing',
  'steam_app_id_missing',
  'trainer_hash_mismatch',
  'native_trainer_unsupported',
  'unshare_net_unavailable',
  'mod_coexistence_injection_vector',
  'mod_coexistence_overlay_conflict',
  'mod_coexistence_file_replacement_notice',
  'mod_coexistence_script_extender_launch_order',
  'mod_coexistence_registered_path_missing',
  'unknown_launch_optimization',
  'duplicate_launch_optimization',
  'incompatible_launch_optimization',
  'launch_optimization_requires_tool',
  'gamescope_missing',
  'custom_env_var_reserved',
  '  trainer_padded  ',
  'hook_failed',
  'GAME_PATH_UPPER',
];

function validationFixtures() {
  return VALIDATION_CODES.map((code) => ({
    code,
    expected: mapValidationToNode({ message: 'm', help: 'h', severity: 'info', code: code ?? undefined }),
  }));
}

const HERO_IMAGE_CASES: Parameters<typeof resolveGameDetailsHero>[0][] = [
  {
    customBgPath: '/art/bg.png',
    bg: { url: 'bg', loading: false },
    hero: { url: 'hero', loading: false },
    headerImage: 'h',
    metaLoading: false,
  },
  {
    customBgPath: '/art/bg.png',
    bg: { url: null, loading: true },
    hero: { url: 'hero', loading: false },
    headerImage: 'h',
    metaLoading: false,
  },
  {
    customBgPath: '/art/bg.png',
    bg: { url: null, loading: false },
    hero: { url: 'hero', loading: false },
    headerImage: 'h',
    metaLoading: false,
  },
  {
    customBgPath: '  ',
    bg: { url: null, loading: true },
    hero: { url: 'hero', loading: false },
    headerImage: 'h',
    metaLoading: false,
  },
  { bg: { url: 'bg', loading: false }, hero: { url: 'hero', loading: false }, headerImage: 'h', metaLoading: false },
  { bg: { url: null, loading: false }, hero: { url: null, loading: true }, headerImage: 'h', metaLoading: false },
  { bg: { url: null, loading: false }, hero: { url: 'hero', loading: false }, headerImage: 'h', metaLoading: true },
  { bg: { url: null, loading: false }, hero: { url: null, loading: false }, headerImage: 'h', metaLoading: true },
  { bg: { url: null, loading: false }, hero: { url: null, loading: false }, headerImage: 'h', metaLoading: false },
  { bg: { url: '', loading: false }, hero: { url: '', loading: false }, headerImage: '', metaLoading: false },
  { bg: { url: null, loading: false }, hero: { url: null, loading: false }, headerImage: null, metaLoading: false },
];

function heroFixtures() {
  const ids = [
    'overview',
    'profiles',
    'launch-options',
    'trainer',
    'history',
    'compatibility',
    'Overview',
    ' trainer',
    'launch_options',
    '',
  ];
  return {
    tabs: HERO_DETAIL_TABS.map((tab) => ({ id: tab.id, label: tab.label })),
    default_tab: HERO_DETAIL_DEFAULT_TAB,
    tab_ids: ids.map((id) => ({ id, valid: isHeroDetailTabId(id) })),
    display_path: [null, '', '   ', ' /games/quest ', '/x'].map((value) => ({ value, expected: displayPath(value) })),
    hero_image: HERO_IMAGE_CASES.map((input) => ({ input, expected: resolveGameDetailsHero(input) })),
  };
}

describe('TS parity fixtures for crosshook-view-model', () => {
  it('pipeline', () => checkFixture('pipeline', pipelineFixtures()));
  it('pipeline detail', () => checkFixture('pipeline_detail', detailFixtures()));
  it('validation map', () => checkFixture('validation_map', validationFixtures()));
  it('hero model', () => checkFixture('hero', heroFixtures()));
});
