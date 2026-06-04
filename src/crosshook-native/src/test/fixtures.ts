import type { ProfileHealthReport } from '@/types/health';
import type { LaunchPreview, LaunchRequest } from '@/types/launch';
import type { LibraryCardData } from '@/types/library';
import type { Capability, HostToolCheckResult, HostToolInstallCommand, ReadinessCheckResult } from '@/types/onboarding';
import { createDefaultProfile, type GameProfile } from '@/types/profile';

export function makeInstallHint(overrides: Partial<HostToolInstallCommand> = {}): HostToolInstallCommand {
  return {
    distro_family: 'Arch',
    command: 'sudo pacman -S gamescope',
    alternatives: 'Use distro docs if unavailable.',
    ...overrides,
  };
}

export function makeHostToolCheck(overrides: Partial<HostToolCheckResult> = {}): HostToolCheckResult {
  return {
    tool_id: 'gamescope',
    display_name: 'Gamescope',
    is_available: false,
    is_required: true,
    category: 'performance',
    docs_url: 'https://example.invalid/gamescope',
    tool_version: null,
    resolved_path: null,
    install_guidance: makeInstallHint(),
    ...overrides,
  };
}

export function makeCapability(overrides: Partial<Capability> = {}): Capability {
  const missingRequired = overrides.missing_required ?? [makeHostToolCheck()];
  const missingOptional = overrides.missing_optional ?? [];
  return {
    id: 'gamescope',
    label: 'Gamescope',
    category: 'performance',
    state: 'degraded',
    rationale: 'Gamescope is missing.',
    required_tool_ids: missingRequired.map((tool) => tool.tool_id),
    optional_tool_ids: missingOptional.map((tool) => tool.tool_id),
    missing_required: missingRequired,
    missing_optional: missingOptional,
    install_hints:
      overrides.install_hints ??
      missingRequired.flatMap((tool) => (tool.install_guidance ? [tool.install_guidance] : [])),
    ...overrides,
  };
}

export function makeReadinessResult(overrides: Partial<ReadinessCheckResult> = {}): ReadinessCheckResult {
  return {
    checks: [],
    all_passed: false,
    critical_failures: 1,
    warnings: 0,
    umu_install_guidance: null,
    steam_deck_caveats: null,
    tool_checks: [makeHostToolCheck()],
    detected_distro_family: 'Arch',
    ...overrides,
  };
}

export function makeLibraryCardData(overrides: Partial<LibraryCardData> = {}): LibraryCardData {
  return {
    name: 'Synthetic Quest',
    gameName: 'Synthetic Quest',
    steamAppId: '9999001',
    customCoverArtPath: '',
    customPortraitArtPath: '',
    networkIsolation: false,
    isFavorite: false,
    ...overrides,
  };
}

/** Fixed timestamp for stable snapshots and deterministic health fixtures. */
export const FIXTURE_CHECKED_AT = '2020-01-01T00:00:00.000Z';

/**
 * Factory for `ProfileHealthReport` used in inspector / health UI tests.
 * Commonly overridden fields: `status`, `issues`.
 */
export function makeProfileHealthReport(overrides: Partial<ProfileHealthReport> = {}): ProfileHealthReport {
  return {
    name: 'Synthetic Quest',
    status: 'healthy',
    launch_method: 'steam',
    issues: [],
    checked_at: FIXTURE_CHECKED_AT,
    ...overrides,
  };
}

/**
 * Factory for `LaunchRequest` used in launch-tab and gate tests.
 * Commonly overridden fields: `method`, `game_path`, `optimizations`.
 */
export function makeLaunchRequest(overrides: Partial<LaunchRequest> = {}): LaunchRequest {
  return {
    method: 'proton_run',
    game_path: '/games/synthetic-quest/game.exe',
    trainer_path: '/trainers/synthetic-quest/trainer.exe',
    trainer_host_path: '/trainers/synthetic-quest/trainer.exe',
    trainer_loading_mode: 'source_directory',
    steam: {
      app_id: '9999001',
      compatdata_path: '/steam/compatdata/9999001',
      proton_path: '/compatibilitytools/proton-ge/proton',
      steam_client_install_path: '/steam/root',
    },
    runtime: {
      prefix_path: '/prefixes/synthetic-quest',
      proton_path: '/compatibilitytools/proton-ge/proton',
      working_directory: '/games/synthetic-quest',
      steam_app_id: '9999001',
    },
    optimizations: { enabled_option_ids: [] },
    launch_trainer_only: false,
    launch_game_only: false,
    profile_name: 'Synthetic Quest',
    custom_env_vars: { DXVK_HUD: 'fps' },
    network_isolation: false,
    gamescope: {
      enabled: false,
      fullscreen: false,
      borderless: false,
      grab_cursor: false,
      force_grab_cursor: false,
      hdr_enabled: false,
      allow_nested: false,
      extra_args: [],
    },
    trainer_gamescope: {
      enabled: false,
      fullscreen: false,
      borderless: false,
      grab_cursor: false,
      force_grab_cursor: false,
      hdr_enabled: false,
      allow_nested: false,
      extra_args: [],
    },
    mangohud: {
      enabled: false,
      gpu_stats: false,
      cpu_stats: false,
      ram: false,
      frametime: false,
      battery: false,
      watt: false,
    },
    ...overrides,
  };
}

/**
 * Factory for `LaunchPreview` used in launch-tab, gate, and command section tests.
 * Commonly overridden fields: `resolved_method`, `effective_command`, `validation`.
 */
export function makeLaunchPreview(overrides: Partial<LaunchPreview> = {}): LaunchPreview {
  return {
    resolved_method: 'proton_run',
    validation: { issues: [] },
    environment: [{ key: 'DXVK_HUD', value: 'fps', source: 'profile_custom' }],
    cleared_variables: [],
    wrappers: ['gamescope'],
    effective_command: 'gamescope -- /compat/proton run /games/synthetic-quest/game.exe',
    directives_error: null,
    steam_launch_options: null,
    proton_setup: {
      wine_prefix_path: '/prefixes/synthetic-quest',
      compat_data_path: '/steam/compatdata/9999001',
      steam_client_install_path: '/steam/root',
      proton_executable: '/compat/proton',
      umu_run_path: null,
    },
    working_directory: '/games/synthetic-quest',
    game_executable: '/games/synthetic-quest/game.exe',
    game_executable_name: 'game.exe',
    trainer: null,
    generated_at: '2026-04-23T12:00:00.000Z',
    display_text: '',
    umu_decision: null,
    ...overrides,
  };
}

export function makeProfileDraft(overrides: Partial<GameProfile> = {}): GameProfile {
  const base = createDefaultProfile();
  const baseLocalOverride = base.local_override ?? {
    game: {
      executable_path: '',
      custom_cover_art_path: '',
      custom_portrait_art_path: '',
      custom_background_art_path: '',
    },
    trainer: {
      path: '',
      extra_protontricks: [],
    },
    steam: {
      compatdata_path: '',
      proton_path: '',
    },
    runtime: {
      prefix_path: '',
      proton_path: '',
    },
  };
  const profile: GameProfile = {
    ...base,
    ...overrides,
    game: {
      ...base.game,
      ...(overrides.game ?? {}),
    },
    trainer: {
      ...base.trainer,
      ...(overrides.trainer ?? {}),
    },
    steam: {
      ...base.steam,
      ...(overrides.steam ?? {}),
    },
    runtime: {
      ...base.runtime,
      ...(overrides.runtime ?? {}),
    },
    launch: {
      ...base.launch,
      ...(overrides.launch ?? {}),
    },
  };

  if (overrides.local_override !== undefined) {
    profile.local_override = {
      ...baseLocalOverride,
      ...overrides.local_override,
      game: {
        ...baseLocalOverride.game,
        ...(overrides.local_override.game ?? {}),
      },
      trainer: {
        ...baseLocalOverride.trainer,
        ...(overrides.local_override.trainer ?? {}),
        extra_protontricks: [...(overrides.local_override.trainer?.extra_protontricks ?? [])],
      },
      steam: {
        ...baseLocalOverride.steam,
        ...(overrides.local_override.steam ?? {}),
      },
      runtime: {
        ...baseLocalOverride.runtime,
        ...(overrides.local_override.runtime ?? {}),
      },
    };
  }

  return profile;
}
