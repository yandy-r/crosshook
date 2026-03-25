export const LAUNCH_OPTIMIZATION_CATEGORIES = [
  'input',
  'performance',
  'display',
  'graphics',
  'compatibility',
] as const;

export type LaunchOptimizationCategory = (typeof LAUNCH_OPTIMIZATION_CATEGORIES)[number];

export const LAUNCH_OPTIMIZATION_CATEGORY_LABELS: Record<LaunchOptimizationCategory, string> = {
  input: 'Input & Controller',
  performance: 'Performance & Overlay',
  display: 'Display & Windowing',
  graphics: 'Graphics & HDR',
  compatibility: 'Compatibility Workarounds',
};

// Launch methods where profile-scoped optimization toggles apply (direct Proton or Steam copy/paste line).
export const LAUNCH_OPTIMIZATION_APPLICABLE_METHODS = ['proton_run', 'steam_applaunch'] as const;

export type LaunchOptimizationLaunchMethod = (typeof LAUNCH_OPTIMIZATION_APPLICABLE_METHODS)[number];

export const LAUNCH_OPTIMIZATION_IDS = [
  'disable_steam_input',
  'prefer_sdl_input',
  'hide_window_decorations',
  'show_mangohud_overlay',
  'use_gamemode',
  'use_game_performance',
  'enable_hdr',
  'enable_wayland_driver',
  'use_ntsync',
  'enable_local_shader_cache',
  'enable_fsr4_upgrade',
  'enable_fsr4_rdna3_upgrade',
  'enable_xess_upgrade',
  'enable_dlss_upgrade',
  'show_dlss_indicator',
  'enable_nvidia_libs',
  'steamdeck_compat_mode',
] as const;

export type LaunchOptimizationId = (typeof LAUNCH_OPTIMIZATION_IDS)[number];

export interface LaunchOptimizations {
  enabled_option_ids: LaunchOptimizationId[];
}

export interface LaunchOptimizationOption {
  id: LaunchOptimizationId;
  label: string;
  description: string;
  helpText: string;
  category: LaunchOptimizationCategory;
  advanced: boolean;
  community: boolean;
  applicableMethods: readonly LaunchOptimizationLaunchMethod[];
  conflictsWith?: readonly LaunchOptimizationId[];
}

export interface LaunchOptimizationConflict {
  optionId: LaunchOptimizationId;
  conflictsWith: LaunchOptimizationId;
}

export const LAUNCH_OPTIMIZATION_OPTIONS: readonly LaunchOptimizationOption[] = [
  {
    id: 'disable_steam_input',
    label: 'Disable Steam Input',
    description: 'Let Proton handle controller input directly.',
    helpText:
      'Useful when Steam Input causes double mapping, bad glyphs, or gyro conflicts. Keep it off if the game depends on Steam remapping layers.',
    category: 'input',
    advanced: false,
    community: false,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
  },
  {
    id: 'prefer_sdl_input',
    label: 'Prefer SDL controller handling',
    description: 'Bias Proton toward SDL-style controller handling.',
    helpText:
      'Helps when controller detection is inconsistent and SDL mappings behave better than the default path. Leave it off if the game already plays nicely with the current input stack.',
    category: 'input',
    advanced: false,
    community: false,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
  },
  {
    id: 'hide_window_decorations',
    label: 'Hide window decorations',
    description: 'Request a cleaner, borderless-style Proton window.',
    helpText:
      'Helpful for games that misbehave with desktop title bars, resize handles, or other window chrome. It is a presentation tweak, not a performance boost.',
    category: 'display',
    advanced: false,
    community: false,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
  },
  {
    id: 'show_mangohud_overlay',
    label: 'Show MangoHud overlay',
    description: 'Overlay frame-time and performance stats during launch.',
    helpText:
      'Launches the game through MangoHud so performance and frame pacing data stays visible. Requires MangoHud to be installed and can interact with other preload-heavy launch paths.',
    category: 'performance',
    advanced: false,
    community: false,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
  },
  {
    id: 'use_gamemode',
    label: 'Use GameMode',
    description: 'Request GameMode performance adjustments for the session.',
    helpText:
      'Launches through gamemoderun when the GameMode service is available. Prefer this over distro-specific performance wrappers when you want a portable wrapper.',
    category: 'performance',
    advanced: false,
    community: false,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
    conflictsWith: ['use_game_performance'],
  },
  {
    id: 'use_game_performance',
    label: 'Use CachyOS performance wrapper',
    description: 'Launch through CachyOS game-performance when available.',
    helpText:
      'Uses the distro-specific game-performance wrapper to switch the system into a performance profile while the game runs. Only meaningful on systems that provide the wrapper.',
    category: 'performance',
    advanced: true,
    community: true,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
    conflictsWith: ['use_gamemode'],
  },
  {
    id: 'enable_hdr',
    label: 'Enable HDR',
    description: 'Turn on Proton HDR support for the launch.',
    helpText:
      'Only helps when the display path, compositor, and game can all present HDR correctly. Treat it as an advanced compatibility toggle, not a default boost.',
    category: 'graphics',
    advanced: true,
    community: true,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
  },
  {
    id: 'enable_wayland_driver',
    label: 'Use native Wayland support',
    description: "Prefer Proton's Wayland path when the stack supports it.",
    helpText:
      'Experimental on some setups and can affect overlays or input handling. Use it only when you know the game and runtime behave correctly under Wayland.',
    category: 'compatibility',
    advanced: true,
    community: true,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
  },
  {
    id: 'use_ntsync',
    label: 'Use NTSync',
    description: 'Enable NTSync support for Proton when the kernel allows it.',
    helpText:
      'Works only on kernels and Proton builds that support NTSync. Keep it in the advanced bucket because it is still environment-dependent.',
    category: 'compatibility',
    advanced: true,
    community: true,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
  },
  {
    id: 'enable_local_shader_cache',
    label: 'Isolate shader cache per game',
    description: 'Keep shader cache data separated for the current profile.',
    helpText:
      "Reduces cross-game shader cache interference at the cost of more storage per profile. Useful when one game's cache should not affect another's runtime behavior.",
    category: 'graphics',
    advanced: true,
    community: true,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
  },
  {
    id: 'enable_fsr4_upgrade',
    label: 'Auto-upgrade FSR4',
    description: 'Use the community FSR4 upgrade path when available.',
    helpText:
      'Community-documented and potentially useful on compatible titles, but not universal Proton behavior. Treat it as an advanced graphics tweak.',
    category: 'graphics',
    advanced: true,
    community: true,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
    conflictsWith: ['enable_fsr4_rdna3_upgrade'],
  },
  {
    id: 'enable_fsr4_rdna3_upgrade',
    label: 'Use RDNA3-optimized FSR4',
    description: 'Select the RDNA3-specific FSR4 upgrade path.',
    helpText:
      'Only relevant for RDNA3 hardware and should not be treated as a general graphics fix. Keep it grouped with the other advanced graphics options.',
    category: 'graphics',
    advanced: true,
    community: true,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
    conflictsWith: ['enable_fsr4_upgrade'],
  },
  {
    id: 'enable_xess_upgrade',
    label: 'Auto-upgrade XeSS',
    description: 'Use the community XeSS upgrade path when available.',
    helpText:
      "Vendor-specific and dependent on the game's upscaler path. It is useful for some Intel or mixed-vendor setups, but it is not a default optimization.",
    category: 'graphics',
    advanced: true,
    community: true,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
  },
  {
    id: 'enable_dlss_upgrade',
    label: 'Auto-upgrade DLSS',
    description: 'Use the community DLSS upgrade path when available.',
    helpText:
      'Best treated as an NVIDIA-specific compatibility tool, not a general performance boost. The game still has to support the upscaler path.',
    category: 'graphics',
    advanced: true,
    community: true,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
  },
  {
    id: 'show_dlss_indicator',
    label: 'Show DLSS indicator',
    description: 'Display the DLSS indicator used by some community Proton builds.',
    helpText:
      'Useful for confirming that the DLSS-related upgrade path is active in supported games. It is primarily diagnostic and cosmetic.',
    category: 'graphics',
    advanced: true,
    community: true,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
  },
  {
    id: 'enable_nvidia_libs',
    label: 'Enable NVIDIA game libraries',
    description: 'Make the NVIDIA game-library helpers available when needed.',
    helpText:
      'Only meaningful on NVIDIA systems and Proton variants that support the flag. Keep it in the advanced graphics group because it is hardware-specific.',
    category: 'graphics',
    advanced: true,
    community: true,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
  },
  {
    id: 'steamdeck_compat_mode',
    label: 'Use Steam Deck compatibility mode',
    description: 'Apply the community Steam Deck compatibility signal.',
    helpText:
      'A narrow compatibility workaround for titles that benefit from deck-style behavior. It can change how a game behaves, so keep it clearly labeled as advanced.',
    category: 'compatibility',
    advanced: true,
    community: true,
    applicableMethods: LAUNCH_OPTIMIZATION_APPLICABLE_METHODS,
  },
] as const;

export const LAUNCH_OPTIMIZATION_OPTIONS_BY_ID: Record<LaunchOptimizationId, LaunchOptimizationOption> =
  Object.fromEntries(LAUNCH_OPTIMIZATION_OPTIONS.map((option) => [option.id, option])) as Record<
    LaunchOptimizationId,
    LaunchOptimizationOption
  >;

export const LAUNCH_OPTIMIZATION_CONFLICT_MATRIX: Readonly<
  Record<LaunchOptimizationId, readonly LaunchOptimizationId[]>
> = Object.fromEntries(
  LAUNCH_OPTIMIZATION_IDS.map((optionId) => [
    optionId,
    LAUNCH_OPTIMIZATION_OPTIONS_BY_ID[optionId].conflictsWith ?? [],
  ])
) as Record<LaunchOptimizationId, readonly LaunchOptimizationId[]>;

export function getConflictingLaunchOptimizationIds(
  optionId: LaunchOptimizationId,
  enabledOptionIds: readonly LaunchOptimizationId[]
): LaunchOptimizationId[] {
  const conflictsWith = LAUNCH_OPTIMIZATION_CONFLICT_MATRIX[optionId];
  if (conflictsWith.length === 0) {
    return [];
  }

  return enabledOptionIds.filter((enabledOptionId) => conflictsWith.includes(enabledOptionId));
}

export function findLaunchOptimizationConflicts(
  enabledOptionIds: readonly LaunchOptimizationId[]
): LaunchOptimizationConflict[] {
  const conflicts: LaunchOptimizationConflict[] = [];

  for (const optionId of enabledOptionIds) {
    for (const conflictingId of getConflictingLaunchOptimizationIds(optionId, enabledOptionIds)) {
      if (optionId >= conflictingId) {
        continue;
      }

      conflicts.push({
        optionId,
        conflictsWith: conflictingId,
      });
    }
  }

  return conflicts;
}
