/**
 * Golden parity: the TypeScript launch-request builders must produce the same request as
 * `crosshook_core::launch::LaunchRequest::from_profile` for every shared fixture in
 * crates/crosshook-core/tests/fixtures/launch_request (Rust is the source of truth).
 *
 * TS builders always emit keys that Rust's serializer skips when defaulted, so both sides
 * are normalized by applying the Rust deserialization defaults before comparing.
 */
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  buildLaunchRequest as buildCommunityLaunchRequest,
  normalizeProfile,
} from '@/components/community-import/utils';
import { applyLaunchPhase } from '@/hooks/useLaunchState';
import { LaunchPhase, type LaunchRequest } from '@/types';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG, type GameProfile } from '@/types/profile';
import type { UmuPreference } from '@/types/settings';
import { buildProfileLaunchRequest, resolveLaunchMethod } from '../launch';

const FIXTURE_DIR = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../../crates/crosshook-core/tests/fixtures/launch_request'
);

interface GoldenCase {
  description: string;
  input: {
    profile: GameProfile;
    steam_client_path: string;
    profile_name: string;
    umu_default: UmuPreference;
    is_flatpak: boolean;
  };
  expected: {
    method: string;
    trainer_method: string;
    request?: Record<string, unknown>;
    error?: string;
  };
}

type JsonRecord = Record<string, unknown>;

function loadCases(): Array<[string, GoldenCase]> {
  if (!existsSync(FIXTURE_DIR)) {
    return [];
  }
  return readdirSync(FIXTURE_DIR)
    .filter((name) => name.endsWith('.json'))
    .sort()
    .map((name) => [name, JSON.parse(readFileSync(path.join(FIXTURE_DIR, name), 'utf8')) as GoldenCase]);
}

function asRecord(value: unknown): JsonRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? (value as JsonRecord) : {};
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

/**
 * Fill Rust serde defaults while keeping every present key, so unknown fields still compare.
 * `undefined`/`null` keys count as absent (Rust `None` ⇔ TS omission).
 */
function withDefaults(defaults: object, value: unknown): JsonRecord {
  const present = Object.entries(asRecord(value)).filter(([, entry]) => entry !== undefined && entry !== null);
  return { ...defaults, ...Object.fromEntries(present) };
}

const HOOK_DEFAULTS = { id: '', name: '', path: '', stage: 'pre-launch', enabled: false };

/**
 * Canonical form of a serialized `LaunchRequest` after Rust `serde` deserialization defaults.
 * Every present key is kept (top-level and nested) so a field added on only one side fails;
 * known defaulted fields are then overlaid with their Rust default when absent.
 */
function normalizeRequest(value: unknown): JsonRecord {
  const request = asRecord(value);

  return {
    ...withDefaults({}, request),
    method: request.method ?? '',
    game_path: request.game_path ?? '',
    trainer_path: request.trainer_path ?? '',
    trainer_host_path: request.trainer_host_path ?? '',
    trainer_loading_mode: request.trainer_loading_mode ?? 'source_directory',
    steam: withDefaults(
      { app_id: '', compatdata_path: '', proton_path: '', steam_client_install_path: '' },
      request.steam
    ),
    runtime: withDefaults(
      {
        prefix_path: '',
        proton_path: '',
        working_directory: '',
        steam_app_id: '',
        umu_game_id: '',
        umu_store: '',
        umu_codename: '',
      },
      request.runtime
    ),
    optimizations: withDefaults({ enabled_option_ids: [] }, request.optimizations),
    command_arguments: withDefaults({ enabled_argument_ids: [], custom_args: [] }, request.command_arguments),
    launch_trainer_only: request.launch_trainer_only ?? false,
    launch_game_only: request.launch_game_only ?? false,
    profile_name: request.profile_name ?? null,
    custom_env_vars: asRecord(request.custom_env_vars),
    umu_preference: request.umu_preference ?? 'auto',
    network_isolation: request.network_isolation ?? true,
    gamescope: withDefaults(DEFAULT_GAMESCOPE_CONFIG, request.gamescope),
    // Rust omits a default trainer override (None); TS sends the default. Both resolve identically.
    trainer_gamescope: withDefaults(DEFAULT_GAMESCOPE_CONFIG, request.trainer_gamescope),
    mangohud: withDefaults(DEFAULT_MANGOHUD_CONFIG, request.mangohud),
    pre_launch_hooks: asArray(request.pre_launch_hooks).map((hook) => withDefaults(HOOK_DEFAULTS, hook)),
    post_exit_hooks: asArray(request.post_exit_hooks).map((hook) => withDefaults(HOOK_DEFAULTS, hook)),
  };
}

/**
 * Fields that are never part of a `from_profile` request: `resolved_umu_game_id` is
 * backend-computed (skip_deserializing) and `preview_target` is set only by preview callers.
 */
const NON_PROFILE_FIELDS = ['resolved_umu_game_id', 'preview_target'] as const;

function expectNoNonProfileFields(request: unknown): void {
  for (const field of NON_PROFILE_FIELDS) {
    expect(asRecord(request), field).not.toHaveProperty(field);
  }
}

const cases = loadCases();

describe('launch request golden fixtures', () => {
  it('discovers shared fixtures', () => {
    expect(cases.length, `no fixtures found in ${FIXTURE_DIR}`).toBeGreaterThan(0);
  });

  describe.each(cases)('%s', (_name, golden) => {
    const { input, expected } = golden;
    const { profile } = input;

    it('resolveLaunchMethod matches Rust resolve_launch_method', () => {
      expect(resolveLaunchMethod(profile)).toBe(expected.method);
    });

    it('buildProfileLaunchRequest matches LaunchRequest::from_profile', () => {
      const request = buildProfileLaunchRequest(
        profile,
        resolveLaunchMethod(profile),
        input.steam_client_path,
        input.profile_name,
        input.umu_default
      );

      if (expected.error) {
        expect(request).toBeNull();
        return;
      }
      expect(request).not.toBeNull();
      expectNoNonProfileFields(request);
      expectNoNonProfileFields(expected.request);
      expect(normalizeRequest(request)).toEqual(normalizeRequest(expected.request));
    });

    it('community-import buildLaunchRequest on the normalized draft matches with no profile name and auto umu default', () => {
      // The wizard always normalizes the draft first; building from it proves normalizeProfile keeps the runtime umu fields.
      const request = buildCommunityLaunchRequest(normalizeProfile(profile), input.steam_client_path);

      if (expected.error) {
        expect(request).toBeNull();
        return;
      }
      expectNoNonProfileFields(request);
      expect(normalizeRequest(request)).toEqual(
        normalizeRequest({
          ...expected.request,
          profile_name: null,
          umu_preference: profile.runtime.umu_preference ?? 'auto',
        })
      );
    });
  });

  it('applyLaunchPhase flips only the phase flags of a from_profile request', () => {
    const golden = cases.find(([, c]) => !c.expected.error)?.[1];
    expect(golden).toBeDefined();
    const request = normalizeRequest(golden?.expected.request) as unknown as LaunchRequest;

    expect(applyLaunchPhase(request, LaunchPhase.GameLaunching)).toEqual({
      ...request,
      launch_game_only: true,
      launch_trainer_only: false,
    });
    expect(applyLaunchPhase(request, LaunchPhase.TrainerLaunching)).toEqual({
      ...request,
      launch_game_only: false,
      launch_trainer_only: true,
    });
  });
});
