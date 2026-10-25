import type { GameProfile } from '../../types';

/** Merges launch subsection fields that have dedicated autosave writers from live editor state. */
export function mergeLiveLaunchSections(draftProfile: GameProfile, liveProfile: GameProfile): GameProfile {
  return {
    ...draftProfile,
    launch: {
      ...draftProfile.launch,
      command_arguments: liveProfile.launch.command_arguments,
      optimizations: liveProfile.launch.optimizations,
      gamescope: liveProfile.launch.gamescope,
      trainer_gamescope: liveProfile.launch.trainer_gamescope,
      mangohud: liveProfile.launch.mangohud,
    },
  };
}
