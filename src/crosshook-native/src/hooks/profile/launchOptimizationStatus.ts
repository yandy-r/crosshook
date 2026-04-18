import type { LaunchAutoSaveStatus } from '../../types';
import type { ResolvedLaunchMethod } from '../../utils/launch';

export type LaunchOptimizationsStatus = LaunchAutoSaveStatus;

export function buildLaunchOptimizationsStatus(
  method: ResolvedLaunchMethod,
  hasExistingSavedProfile: boolean
): LaunchOptimizationsStatus {
  if (method !== 'proton_run' && method !== 'steam_applaunch') {
    return {
      tone: 'warning',
      label: 'Unavailable for current method',
      detail: 'Launch optimizations are only editable when the profile method is proton_run or steam_applaunch.',
    };
  }

  if (!hasExistingSavedProfile) {
    return {
      tone: 'warning',
      label: 'Save profile first to enable autosave',
      detail: 'Optimization changes stay local until the profile has been saved once.',
    };
  }

  return {
    tone: 'idle',
    label: 'Ready to autosave',
    detail:
      method === 'steam_applaunch'
        ? 'Only launch.optimizations will be written automatically; paste the generated line into Steam yourself.'
        : 'Only launch.optimizations will be written automatically for this saved profile.',
  };
}
