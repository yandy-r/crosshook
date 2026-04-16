import { InfoCircleIcon } from './icons/SidebarIcons';

interface PinnedProfilesStripProps {
  favoriteProfiles: string[];
  selectedProfile: string;
  onSelectProfile: (name: string) => Promise<void>;
  onToggleFavorite: (name: string, favorite: boolean) => Promise<void>;
  umuCoverageNoteByProfile?: Record<string, boolean>;
}

export function PinnedProfilesStrip({
  favoriteProfiles,
  selectedProfile,
  onSelectProfile,
  onToggleFavorite,
  umuCoverageNoteByProfile,
}: PinnedProfilesStripProps) {
  if (favoriteProfiles.length === 0) return null;

  return (
    <section className="crosshook-pinned-strip" aria-label="Pinned profiles">
      <span className="crosshook-heading-eyebrow">Pinned Profiles</span>
      <div className="crosshook-pinned-strip__scroll">
        {favoriteProfiles.map((name) => {
          const isActive = name === selectedProfile;
          return (
            <div key={name} className="crosshook-pinned-strip__chip-container">
              <button
                type="button"
                className={`crosshook-pinned-strip__chip${isActive ? ' crosshook-pinned-strip__chip--active' : ''}`}
                onClick={() => void onSelectProfile(name)}
                aria-current={isActive ? 'true' : undefined}
                title={name}
              >
                <span className="crosshook-pinned-strip__chip-name">{name}</span>
                {umuCoverageNoteByProfile?.[name] ? (
                  <span
                    role="img"
                    className="crosshook-pinned-strip__badge crosshook-pinned-strip__badge--info"
                    title="umu has no known entry for this app id in the current umu database. The database only tracks titles needing protonfixes — most titles work fine without an entry."
                    aria-label="no umu-database protonfix row for this profile"
                  >
                    <InfoCircleIcon width={14} height={14} />
                  </span>
                ) : null}
              </button>
              <button
                type="button"
                className="crosshook-pinned-strip__unpin"
                aria-label={`Unpin ${name}`}
                title="Remove from pinned"
                onClick={(e) => {
                  e.stopPropagation();
                  void onToggleFavorite(name, false);
                }}
              >
                &times;
              </button>
            </div>
          );
        })}
      </div>
    </section>
  );
}

export default PinnedProfilesStrip;
