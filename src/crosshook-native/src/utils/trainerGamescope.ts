import type { GameProfile, GamescopeConfig } from '@/types/profile';
import { DEFAULT_GAMESCOPE_CONFIG } from '@/types/profile';

// Must mirror LaunchRequest::resolved_trainer_gamescope / LaunchSection::resolved_trainer_gamescope in crosshook-core.
export function resolveTrainerGamescopeForDisplay(profile: GameProfile): {
  config: GamescopeConfig;
  isGeneratedFromGame: boolean;
} {
  const trainerGamescope = profile.launch.trainer_gamescope;

  if (trainerGamescope?.enabled) {
    return {
      config: trainerGamescope,
      isGeneratedFromGame: false,
    };
  }

  const gameGamescope = profile.launch.gamescope;
  if (gameGamescope?.enabled) {
    return {
      config: {
        ...DEFAULT_GAMESCOPE_CONFIG,
        ...gameGamescope,
        enabled: true,
        fullscreen: false,
        borderless: false,
        extra_args: gameGamescope.extra_args ?? [],
      },
      isGeneratedFromGame: true,
    };
  }

  return {
    config: DEFAULT_GAMESCOPE_CONFIG,
    isGeneratedFromGame: false,
  };
}
