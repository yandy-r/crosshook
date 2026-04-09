import type { LibraryCardData } from '../../types/library';
import type { AppRoute } from '../layout/Sidebar';
import type { LibraryOpenDetailsHandler } from './library-card-interactions';
import { LibraryCard } from './LibraryCard';

interface LibraryGridProps {
  profiles: LibraryCardData[];
  selectedName?: string;
  onOpenDetails: LibraryOpenDetailsHandler;
  onLaunch: (name: string) => void;
  onEdit: (name: string) => void;
  onToggleFavorite: (name: string, current: boolean) => void;
  launchingName?: string;
  onNavigate?: (route: AppRoute) => void;
  onContextMenu?: (position: { x: number; y: number }, profileName: string) => void;
}

export function LibraryGrid({
  profiles,
  selectedName,
  onOpenDetails,
  onLaunch,
  onEdit,
  onToggleFavorite,
  launchingName,
  onNavigate,
  onContextMenu,
}: LibraryGridProps) {
  if (profiles.length === 0) {
    return (
      <div className="crosshook-library-empty">
        <h2 className="crosshook-library-empty__heading">No game profiles yet</h2>
        <p>Create your first profile to see it here.</p>
        <button
          className="crosshook-library-empty__cta"
          onClick={() => onNavigate?.('profiles')}
        >
          Create a profile
        </button>
      </div>
    );
  }

  return (
    <div className="crosshook-library-grid" role="list">
      {profiles.map((profile) => (
        <LibraryCard
          key={profile.name}
          profile={profile}
          isSelected={selectedName === profile.name}
          onOpenDetails={onOpenDetails}
          onLaunch={onLaunch}
          onEdit={onEdit}
          onToggleFavorite={onToggleFavorite}
          isLaunching={launchingName === profile.name}
          onContextMenu={onContextMenu}
        />
      ))}
    </div>
  );
}

export default LibraryGrid;
