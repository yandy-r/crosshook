import type { GameProfile } from '../../types/profile';
import { FieldRow } from './FormFieldRow';

const launcherNameHelperText =
  'CrossHook appends " - Trainer" to the exported launcher title. Enter only the base launcher name here.';

export function LauncherMetadataFields(props: {
  profile: GameProfile;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}) {
  return (
    <FieldRow
      label="Launcher Name"
      value={props.profile.steam.launcher.display_name}
      onChange={(value) =>
        props.onUpdateProfile((current) => ({
          ...current,
          steam: {
            ...current.steam,
            launcher: { ...current.steam.launcher, display_name: value },
          },
        }))
      }
      placeholder="God of War Ragnarok"
      helperText={launcherNameHelperText}
    />
  );
}
