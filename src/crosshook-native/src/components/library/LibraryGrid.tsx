import type { LibraryCardData } from '../../types/library';
import type { AppNavigateOptions } from '../../types/navigation';
import type { AppRoute } from '../layout/Sidebar';
import { LibraryCard } from './LibraryCard';
import type { LibraryOpenDetailsHandler } from './library-card-interactions';

interface LibraryGridProps {
  profiles: LibraryCardData[];
  selectedName?: string;
  onSelect?: (name: string) => void;
  onOpenDetails: LibraryOpenDetailsHandler;
  onLaunch: (name: string) => void;
  onEdit: (name: string) => void;
  onToggleFavorite: (name: string, current: boolean) => void;
  launchingName?: string;
  onNavigate?: (route: AppRoute, options?: AppNavigateOptions) => void;
  onContextMenu?: (position: { x: number; y: number }, profileName: string, restoreFocusTo: HTMLElement) => void;
}

export function LibraryGrid({
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
}: LibraryGridProps) {
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
    <ul className="crosshook-library-grid">
      {profiles.map((profile) => (
        <LibraryCard
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

export default LibraryGrid;
