import { LAUNCH_OPTIMIZATION_APPLICABLE_METHODS } from '../../types/launch-optimizations';
import type { LaunchMethod } from '../../types';
import type { LaunchSubTabId } from './types';

interface UseTabVisibilityResult {
  tabs: LaunchSubTabId[];
  showsGamescopeTab: boolean;
  showsMangoHudTab: boolean;
  showsOptimizationsTab: boolean;
  showsSteamOptionsTab: boolean;
}

export function useTabVisibility(launchMethod: LaunchMethod): UseTabVisibilityResult {
  const isNative = launchMethod === 'native';

  const launchMethodSupportsOptimizations =
    !isNative &&
    LAUNCH_OPTIMIZATION_APPLICABLE_METHODS.includes(
      launchMethod as (typeof LAUNCH_OPTIMIZATION_APPLICABLE_METHODS)[number]
    );

  const showsGamescopeTab = launchMethod === 'proton_run' || launchMethod === 'steam_applaunch';
  const showsMangoHudTab = launchMethodSupportsOptimizations;
  const showsOptimizationsTab = launchMethodSupportsOptimizations;
  const showsSteamOptionsTab = launchMethod === 'steam_applaunch';

  const tabs: LaunchSubTabId[] = isNative
    ? ['environment', 'offline']
    : [
        ...(showsOptimizationsTab ? ['optimizations' as const] : []),
        'environment',
        ...(showsMangoHudTab ? ['mangohud' as const] : []),
        ...(showsGamescopeTab ? ['gamescope' as const] : []),
        ...(showsSteamOptionsTab ? ['steam-options' as const] : []),
        'offline',
      ];

  return { tabs, showsGamescopeTab, showsMangoHudTab, showsOptimizationsTab, showsSteamOptionsTab };
}
