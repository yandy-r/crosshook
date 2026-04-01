import type { GameProfile, LaunchMethod } from '../../types';
import { FieldRow } from '../ProfileFormSections';
import { chooseFile } from '../../utils/dialog';

export interface MediaSectionProps {
  profile: GameProfile;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  launchMethod: LaunchMethod;
}

export function MediaSection({ profile, onUpdateProfile, launchMethod }: MediaSectionProps) {
  return (
    <>
      <div className="crosshook-install-section-title">Media</div>
      <div className="crosshook-install-grid">
        <FieldRow
          label="Custom Cover Art"
          value={profile.game.custom_cover_art_path ?? ''}
          onChange={(value) =>
            onUpdateProfile((current) => ({
              ...current,
              game: { ...current.game, custom_cover_art_path: value },
            }))
          }
          placeholder="/path/to/cover.png"
          browseLabel="Browse"
          onBrowse={async () => {
            const path = await chooseFile('Select Custom Cover Art', [
              { name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] },
            ]);
            if (path) {
              onUpdateProfile((current) => ({
                ...current,
                game: { ...current.game, custom_cover_art_path: path },
              }));
            }
          }}
          helperText="Overrides Steam/SteamGridDB. Shown as a full-width backdrop behind profile tabs (image is cropped to fill). Steam's store header is 460×215 (~2.14:1); a larger landscape file (e.g. 920×430) with the subject in the upper area usually looks best."
        />

        {launchMethod !== 'native' ? (
          <FieldRow
            label="Launcher Icon"
            value={profile.steam.launcher.icon_path}
            onChange={(value) =>
              onUpdateProfile((current) => ({
                ...current,
                steam: {
                  ...current.steam,
                  launcher: { ...current.steam.launcher, icon_path: value },
                },
              }))
            }
            placeholder="/path/to/icon.png"
            browseLabel="Browse"
            onBrowse={async () => {
              const path = await chooseFile('Select Launcher Icon', [
                { name: 'Images', extensions: ['png', 'jpg', 'jpeg'] },
              ]);
              if (path) {
                onUpdateProfile((current) => ({
                  ...current,
                  steam: {
                    ...current.steam,
                    launcher: { ...current.steam.launcher, icon_path: path },
                  },
                }));
              }
            }}
          />
        ) : null}
      </div>
    </>
  );
}

export default MediaSection;
