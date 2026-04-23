import { useMemo } from 'react';
import { ThemedSelect } from '../ui/ThemedSelect';

export type ProfileFormSectionsProfileSelector = {
  profiles: string[];
  favoriteProfiles: string[];
  selectedProfile: string;
  onSelectProfile: (name: string) => Promise<void>;
  onToggleFavorite: (name: string, favorite: boolean) => Promise<void>;
};

export function ProfileSelectorField({
  profileNamesListId,
  profileSelector,
  selectedProfile,
}: {
  profileNamesListId: string;
  profileSelector: ProfileFormSectionsProfileSelector;
  selectedProfile: string;
}) {
  const isPinned = selectedProfile !== '' && profileSelector.favoriteProfiles.includes(selectedProfile);
  const pinnedSet = useMemo(() => new Set(profileSelector.favoriteProfiles), [profileSelector.favoriteProfiles]);
  const handleTogglePin = useMemo(
    () => (value: string) => {
      void profileSelector.onToggleFavorite(value, !pinnedSet.has(value));
    },
    [pinnedSet, profileSelector]
  );

  return (
    <div className="crosshook-field">
      <label className="crosshook-label" htmlFor={`${profileNamesListId}-selector`}>
        Load Profile
      </label>
      <div style={{ display: 'flex', gap: 8, alignItems: 'stretch' }}>
        <div style={{ flex: '1 1 0', minWidth: 0 }}>
          <ThemedSelect
            id={`${profileNamesListId}-selector`}
            value={selectedProfile}
            onValueChange={(val) => void profileSelector.onSelectProfile(val)}
            placeholder="Create New"
            pinnedValues={pinnedSet}
            onTogglePin={handleTogglePin}
            options={[
              { value: '', label: 'Create New' },
              ...profileSelector.profiles.map((name) => ({ value: name, label: name })),
            ]}
          />
        </div>
        {selectedProfile !== '' ? (
          <button
            type="button"
            className={`crosshook-profile-pin-btn${isPinned ? ' crosshook-profile-pin-btn--active' : ''}`}
            onClick={() => void profileSelector.onToggleFavorite(selectedProfile, !isPinned)}
            aria-label={isPinned ? `Unpin ${selectedProfile}` : `Pin ${selectedProfile}`}
            title={isPinned ? 'Remove from pinned' : 'Pin to top'}
          >
            {isPinned ? '★' : '☆'}
          </button>
        ) : null}
      </div>
    </div>
  );
}
