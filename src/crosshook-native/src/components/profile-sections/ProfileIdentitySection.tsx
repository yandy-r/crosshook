import { type ChangeEvent, useId } from 'react';

import type { GameProfile } from '../../types';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';

export interface ProfileIdentitySectionProps {
  profileName: string;
  profile: GameProfile;
  onProfileNameChange: (value: string) => void;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  reviewMode?: boolean;
  dirty?: boolean;
  profileExists?: boolean;
  /** When provided, renders a datalist for profile name suggestions */
  profiles?: string[];
}

/**
 * Renders the "Profile Identity" section: profile name input and game name input.
 * The game path belongs in GameSection; runtime fields belong in RuntimeSection.
 */
export function ProfileIdentitySection({
  profileName,
  profile,
  onProfileNameChange,
  onUpdateProfile,
  profileExists = false,
  profiles,
}: ProfileIdentitySectionProps) {
  const profileNamesListId = useId();
  const gameNameId = useId();

  return (
    <DashboardPanelSection
      titleAs="h3"
      eyebrow="Profile"
      title="Identity"
      actions={
        profileExists ? (
          <span className="crosshook-editor-field-readonly" title="Profile name is locked">
            {profileName}
          </span>
        ) : null
      }
    >
      <div
        style={{
          display: 'grid',
          gap: 12,
          gridTemplateColumns: 'minmax(0, 1fr)',
        }}
      >
        <div className="crosshook-field">
          <label className="crosshook-label" htmlFor={profileNamesListId}>
            Profile Name
          </label>
          <input
            id={profileNamesListId}
            className="crosshook-input"
            list={profiles && profiles.length > 0 ? `${profileNamesListId}-suggestions` : undefined}
            value={profileName}
            placeholder="Enter or choose a profile name"
            readOnly={profileExists}
            onChange={(event: ChangeEvent<HTMLInputElement>) => onProfileNameChange(event.target.value)}
          />
          {profiles && profiles.length > 0 ? (
            <datalist id={`${profileNamesListId}-suggestions`}>
              {profiles.map((name) => (
                <option key={name} value={name} />
              ))}
            </datalist>
          ) : null}
        </div>

        <div className="crosshook-field">
          <label className="crosshook-label" htmlFor={gameNameId}>
            Game Name
          </label>
          <input
            id={gameNameId}
            className="crosshook-input"
            value={profile.game.name}
            placeholder="God of War Ragnarok"
            onChange={(event: ChangeEvent<HTMLInputElement>) =>
              onUpdateProfile((current) => ({
                ...current,
                game: { ...current.game, name: event.target.value },
              }))
            }
          />
        </div>
      </div>
    </DashboardPanelSection>
  );
}

export default ProfileIdentitySection;
