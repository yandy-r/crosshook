import type { LibraryCardData } from '../../types/library';
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
  onNavigate?: (route: AppRoute) => void;
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
  onNavigate,
  onContextMenu,
}: LibraryListProps) {
  if (profiles.length === 0) {
    return (
      <div className="crosshook-library-empty">
        <h2 className="crosshook-library-empty__heading">No game profiles yet</h2>
        <p>Create your first profile to see it here.</p>
        <button type="button" className="crosshook-library-empty__cta" onClick={() => onNavigate?.('profiles')}>
          Create a profile
        </button>
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
