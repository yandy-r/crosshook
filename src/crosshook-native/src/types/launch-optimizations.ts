export const LAUNCH_OPTIMIZATION_CATEGORIES = ['input', 'performance', 'display', 'graphics', 'compatibility'] as const;

export type LaunchOptimizationCategory = (typeof LAUNCH_OPTIMIZATION_CATEGORIES)[number];
export type LaunchOptimizationGpuVendor = 'nvidia' | 'amd';

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

export const BUILTIN_LAUNCH_OPTIMIZATION_IDS = [
  'disable_steam_input',
  'prefer_sdl_input',
  'hide_window_decorations',
  'show_mangohud_overlay',
  'use_gamemode',
  'use_game_performance',
  'enable_hdr',
  'enable_wayland_driver',
  'use_ntsync',
  'disable_esync',
  'disable_fsync',
  'enable_nvapi',
  'force_large_address_aware',
  'enable_proton_log',
  'enable_local_shader_cache',
  'enable_dxvk_async',
  'cap_dxvk_frame_rate',
  'enable_vkd3d_dxr',
  'enable_fsr4_upgrade',
  'enable_fsr4_rdna3_upgrade',
  'enable_xess_upgrade',
  'enable_dlss_upgrade',
  'show_dlss_indicator',
  'enable_nvidia_libs',
  'steamdeck_compat_mode',
] as const;

// Backward-compat alias
export const LAUNCH_OPTIMIZATION_IDS = BUILTIN_LAUNCH_OPTIMIZATION_IDS;

export type LaunchOptimizationId = string;

export interface LaunchOptimizations {
  enabled_option_ids: string[];
}

export interface LaunchOptimizationOption {
  id: string;
  label: string;
  description: string;
  helpText: string;
  category: LaunchOptimizationCategory;
  targetGpuVendor?: LaunchOptimizationGpuVendor;
  advanced: boolean;
  community: boolean;
  applicableMethods: readonly LaunchOptimizationLaunchMethod[];
  conflictsWith?: readonly string[];
}

export interface LaunchOptimizationConflict {
  optionId: string;
  conflictsWith: string;
}

export function getConflictingLaunchOptimizationIds(
  optionId: string,
  enabledOptionIds: readonly string[],
  conflictMatrix: Readonly<Record<string, readonly string[]>>
): string[] {
  const conflictsWith = conflictMatrix[optionId] ?? [];
  if (conflictsWith.length === 0) {
    return [];
  }
  return enabledOptionIds.filter((enabledOptionId) => conflictsWith.includes(enabledOptionId));
}

export function findLaunchOptimizationConflicts(
  enabledOptionIds: readonly string[],
  conflictMatrix: Readonly<Record<string, readonly string[]>>
): LaunchOptimizationConflict[] {
  const conflicts: LaunchOptimizationConflict[] = [];
  for (const optionId of enabledOptionIds) {
    for (const conflictingId of getConflictingLaunchOptimizationIds(optionId, enabledOptionIds, conflictMatrix)) {
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
