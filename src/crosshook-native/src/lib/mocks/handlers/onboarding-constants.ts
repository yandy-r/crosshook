import type { Capability, HostToolCheckResult, HostToolDetails } from '../../../types/onboarding';

export const ONBOARDING_EMIT_INITIAL_MS = 500;
export const ONBOARDING_EMIT_RETRY_MS = 200;
export const ONBOARDING_EMIT_MAX_ATTEMPTS = 25;
export const MOCK_DETECTED_DISTRO_FAMILY = 'Arch';

export interface CachedHostReadinessSnapshot {
  checked_at: string;
  detected_distro_family: string;
  tool_checks: HostToolCheckResult[];
  all_passed: boolean;
  critical_failures: number;
  warnings: number;
}

export interface DismissReadinessNagArgs {
  toolId: string;
}

export interface ProbeHostToolDetailsArgs {
  toolId?: string;
  tool_id?: string;
}

export interface MockCapabilityDefinition {
  id: string;
  label: string;
  category: Capability['category'];
  requiredToolIds: string[];
  optionalToolIds: string[];
}

export interface MockHostToolDefinition {
  check: HostToolCheckResult;
  details: HostToolDetails;
}

export const MOCK_CAPABILITY_DEFINITIONS: readonly MockCapabilityDefinition[] = [
  {
    id: 'gamescope',
    label: 'Gamescope',
    category: 'performance',
    requiredToolIds: ['gamescope'],
    optionalToolIds: [],
  },
  {
    id: 'mangohud',
    label: 'MangoHud',
    category: 'overlay',
    requiredToolIds: ['mangohud'],
    optionalToolIds: [],
  },
  {
    id: 'gamemode',
    label: 'GameMode',
    category: 'performance',
    requiredToolIds: ['gamemode'],
    optionalToolIds: [],
  },
  {
    id: 'prefix_tools',
    label: 'Prefix tools',
    category: 'prefix_tools',
    requiredToolIds: [],
    optionalToolIds: ['winetricks', 'protontricks'],
  },
  {
    id: 'non_steam_launch',
    label: 'Non-Steam launch',
    category: 'runtime',
    requiredToolIds: ['umu_run'],
    optionalToolIds: [],
  },
];

export const MOCK_HOST_TOOL_DEFINITIONS: readonly MockHostToolDefinition[] = [
  {
    check: {
      tool_id: 'umu_run',
      display_name: 'umu-launcher',
      is_available: false,
      is_required: false,
      category: 'runtime',
      docs_url: 'https://github.com/Open-Wine-Components/umu-launcher',
      tool_version: null,
      resolved_path: null,
      install_guidance: {
        distro_family: MOCK_DETECTED_DISTRO_FAMILY,
        command: 'sudo pacman -S umu-launcher',
        alternatives: 'If unavailable in mirrors, use upstream docs for source or user-level install.',
      },
    },
    details: {
      tool_id: 'umu_run',
      tool_version: null,
      resolved_path: null,
    },
  },
  {
    check: {
      tool_id: 'gamescope',
      display_name: 'Gamescope',
      is_available: false,
      is_required: false,
      category: 'performance',
      docs_url: 'https://github.com/ValveSoftware/gamescope',
      tool_version: null,
      resolved_path: null,
      install_guidance: {
        distro_family: MOCK_DETECTED_DISTRO_FAMILY,
        command: 'sudo pacman -S gamescope',
        alternatives: 'Use mesa-git or distro gaming repos if the stock package is old.',
      },
    },
    details: {
      tool_id: 'gamescope',
      tool_version: null,
      resolved_path: null,
    },
  },
  {
    check: {
      tool_id: 'mangohud',
      display_name: 'MangoHud',
      is_available: true,
      is_required: false,
      category: 'overlay',
      docs_url: 'https://github.com/flightlessmango/MangoHud',
      tool_version: '0.7.2',
      resolved_path: '/usr/bin/mangohud',
      install_guidance: null,
    },
    details: {
      tool_id: 'mangohud',
      tool_version: '0.7.2',
      resolved_path: '/usr/bin/mangohud',
    },
  },
  {
    check: {
      tool_id: 'game_performance',
      display_name: 'game-performance',
      is_available: false,
      is_required: false,
      category: 'performance',
      docs_url: 'https://wiki.cachyos.org/',
      tool_version: null,
      resolved_path: null,
      install_guidance: {
        distro_family: MOCK_DETECTED_DISTRO_FAMILY,
        command: 'sudo pacman -S game-performance',
        alternatives:
          'Package is provided on CachyOS. On vanilla Arch, use CachyOS repos or an AUR helper if available.',
      },
    },
    details: {
      tool_id: 'game_performance',
      tool_version: null,
      resolved_path: null,
    },
  },
  {
    check: {
      tool_id: 'gamemode',
      display_name: 'GameMode',
      is_available: true,
      is_required: false,
      category: 'performance',
      docs_url: 'https://github.com/FeralInteractive/gamemode',
      tool_version: '1.8.1',
      resolved_path: '/usr/bin/gamemoderun',
      install_guidance: null,
    },
    details: {
      tool_id: 'gamemode',
      tool_version: '1.8.1',
      resolved_path: '/usr/bin/gamemoderun',
    },
  },
  {
    check: {
      tool_id: 'winetricks',
      display_name: 'Winetricks',
      is_available: true,
      is_required: false,
      category: 'prefix_tools',
      docs_url: 'https://github.com/Winetricks/winetricks',
      tool_version: '20250102-next',
      resolved_path: '/usr/bin/winetricks',
      install_guidance: null,
    },
    details: {
      tool_id: 'winetricks',
      tool_version: '20250102-next',
      resolved_path: '/usr/bin/winetricks',
    },
  },
  {
    check: {
      tool_id: 'protontricks',
      display_name: 'Protontricks',
      is_available: false,
      is_required: false,
      category: 'prefix_tools',
      docs_url: 'https://github.com/Matoking/protontricks',
      tool_version: null,
      resolved_path: null,
      install_guidance: {
        distro_family: MOCK_DETECTED_DISTRO_FAMILY,
        command: 'sudo pacman -S protontricks',
        alternatives: 'Or pip: pip install --user protontricks',
      },
    },
    details: {
      tool_id: 'protontricks',
      tool_version: null,
      resolved_path: null,
    },
  },
];
