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
  return {
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
    local_override: {
      ...baseLocalOverride,
      ...(overrides.local_override ?? {}),
      game: {
        ...baseLocalOverride.game,
        ...(overrides.local_override?.game ?? {}),
      },
      trainer: {
        ...baseLocalOverride.trainer,
        ...(overrides.local_override?.trainer ?? {}),
        extra_protontricks: [...(overrides.local_override?.trainer?.extra_protontricks ?? [])],
      },
      steam: {
        ...baseLocalOverride.steam,
        ...(overrides.local_override?.steam ?? {}),
      },
      runtime: {
        ...baseLocalOverride.runtime,
        ...(overrides.local_override?.runtime ?? {}),
      },
    },
  };
}
