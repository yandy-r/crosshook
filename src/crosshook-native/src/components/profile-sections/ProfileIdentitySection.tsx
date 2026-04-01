import { useId, type ChangeEvent } from 'react';

import type { GameProfile } from '../../types';

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

  return (
    <>
      <div className="crosshook-install-section-title">Profile Identity</div>
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
          <label className="crosshook-label">Game Name</label>
          <input
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
    </>
  );
}

export default ProfileIdentitySection;
