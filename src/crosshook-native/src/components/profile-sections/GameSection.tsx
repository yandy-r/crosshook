import type { GameProfile, LaunchMethod } from '../../types';
import { FieldRow, updateGameExecutablePath } from '../ProfileFormSections';
import { chooseFile } from '../../utils/dialog';

export interface GameSectionProps {
  profile: GameProfile;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  reviewMode?: boolean;
  launchMethod: LaunchMethod;
}

/**
 * Renders the "Game" section: game path field with browse button.
 * Working Directory belongs in RuntimeSection, not here.
 * Steam App ID belongs in RuntimeSection as part of the runner-conditional fields.
 */
export function GameSection({ profile, onUpdateProfile, launchMethod }: GameSectionProps) {
  return (
    <>
      <div className="crosshook-install-section-title">Game</div>
      <div className="crosshook-install-grid">
        <FieldRow
          label="Game Path"
          value={profile.game.executable_path}
          onChange={(value) => onUpdateProfile((current) => updateGameExecutablePath(current, value))}
          placeholder="/path/to/game.exe"
          browseLabel="Browse"
          onBrowse={async () => {
            const path =
              launchMethod === 'native'
                ? await chooseFile('Select Linux Game Executable')
                : await chooseFile('Select Game Executable', [{ name: 'Windows Executable', extensions: ['exe'] }]);

            if (path) {
              onUpdateProfile((current) => updateGameExecutablePath(current, path));
            }
          }}
        />
      </div>
    </>
  );
}

export default GameSection;
