import type { LibraryCardData } from '../../types/library';
import type { AppNavigateOptions } from '../../types/navigation';
import type { AppRoute } from '../layout/Sidebar';
import { LibraryListRow } from './LibraryListRow';
import type { LibraryOpenDetailsHandler } from './library-card-interactions';

interface LibraryListProps {
  profiles: LibraryCardData[];
  selectedName?: string;
  /** When set, primary row hit selects for the inspector (details use the info control). */
  onSelect?: (name: string) => void;
  onOpenDetails: LibraryOpenDetailsHandler;
  onLaunch: (name: string) => void;
  onEdit: (name: string) => void;
  onToggleFavorite: (name: string, current: boolean) => void;
  launchingName?: string;
  onNavigate?: (route: AppRoute, options?: AppNavigateOptions) => void;
  onAddGame?: (restoreFocusTo?: HTMLElement | null) => void;
  /** True when the library has zero profiles (not merely zero after search/filter). */
  hasNoProfiles?: boolean;
  onContextMenu?: (position: { x: number; y: number }, profileName: string, restoreFocusTo: HTMLElement) => void;
}

export function LibraryList({
  profiles,
  selectedName,
  onSelect,
  onOpenDetails,
  onLaunch,
  onEdit,
  onToggleFavorite,
  launchingName,
  onNavigate: _onNavigate,
  onAddGame,
  hasNoProfiles = false,
  onContextMenu,
}: LibraryListProps) {
  if (profiles.length === 0) {
    if (hasNoProfiles) {
      return (
        <div className="crosshook-library-empty">
          <h2 className="crosshook-library-empty__heading">Add your first game</h2>
          <p className="crosshook-library-empty__body">
            CrossHook builds your library from saved game profiles. Add a game profile to choose its executable, runner,
            trainer, and artwork.
          </p>
          {onAddGame ? (
            <button
              type="button"
              className="crosshook-button crosshook-button--primary crosshook-library-empty__cta"
              onClick={(event) => onAddGame(event.currentTarget)}
            >
              Add game
            </button>
          ) : null}
        </div>
      );
    }
    return (
      <div className="crosshook-library-empty crosshook-library-empty--filtered">
        <p className="crosshook-library-empty__body">No games match your search or filters.</p>
      </div>
    );
  }

  return (
    <ul className="crosshook-library-list">
      {profiles.map((profile) => (
        <LibraryListRow
          key={profile.name}
          profile={profile}
          isSelected={selectedName === profile.name}
          onSelect={onSelect}
          onOpenDetails={onOpenDetails}
          onLaunch={onLaunch}
          onEdit={onEdit}
          onToggleFavorite={onToggleFavorite}
          isLaunching={launchingName === profile.name}
          onContextMenu={onContextMenu}
        />
      ))}
    </ul>
  );
}

export default LibraryList;
