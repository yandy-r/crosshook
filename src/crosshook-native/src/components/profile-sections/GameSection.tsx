import type { GameProfile, LaunchMethod } from '../../types';
import { resolveArtAppId } from '../../utils/art';
import { chooseFile } from '../../utils/dialog';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import { FieldRow, updateGameExecutablePath } from '../ProfileFormSections';

export interface GameSectionProps {
  profile: GameProfile;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  reviewMode?: boolean;
  launchMethod: LaunchMethod;
}

/**
 * Renders the "Game" section: game path field with browse button, plus readonly metadata rows.
 * Working Directory belongs in RuntimeSection, not here.
 * Steam App ID belongs in RuntimeSection as part of the runner-conditional fields.
 */
export function GameSection({ profile, onUpdateProfile, launchMethod }: GameSectionProps) {
  const resolvedAppId = resolveArtAppId(profile);
  const coverArtSource = profile.game.custom_cover_art_path ? 'Custom' : resolvedAppId ? 'Steam' : 'None';

  return (
    <DashboardPanelSection titleAs="h3" eyebrow="Profile" title="Game">
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

      <dl className="crosshook-dashboard-kv-list">
        <div className="crosshook-dashboard-kv-row">
          <dt className="crosshook-dashboard-kv-row__label">Steam App ID</dt>
          <dd className="crosshook-dashboard-kv-row__value">
            {resolvedAppId || <span className="crosshook-editor-field-readonly">Not set</span>}
          </dd>
        </div>
        <div className="crosshook-dashboard-kv-row">
          <dt className="crosshook-dashboard-kv-row__label">Cover art source</dt>
          <dd className="crosshook-dashboard-kv-row__value">{coverArtSource}</dd>
        </div>
      </dl>
    </DashboardPanelSection>
  );
}

export default GameSection;
