import type { LibraryViewMode } from '../../types/library';

interface LibraryToolbarProps {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  viewMode: LibraryViewMode;
  onViewModeChange: (mode: LibraryViewMode) => void;
}

export function LibraryToolbar({
  searchQuery,
  onSearchChange,
  viewMode,
  onViewModeChange,
}: LibraryToolbarProps) {
  return (
    <div className="crosshook-library-toolbar">
      <input
        type="search"
        className="crosshook-library-toolbar__search"
        placeholder="Search games..."
        aria-label="Search games"
        value={searchQuery}
        onChange={(e) => onSearchChange(e.target.value)}
      />
      <div className="crosshook-library-toolbar__view-toggle">
        <button
          className="crosshook-library-toolbar__view-btn"
          aria-label="Grid view"
          aria-pressed={viewMode === 'grid'}
          onClick={() => onViewModeChange('grid')}
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
            <rect x="1" y="1" width="6" height="6" rx="1" />
            <rect x="9" y="1" width="6" height="6" rx="1" />
            <rect x="1" y="9" width="6" height="6" rx="1" />
            <rect x="9" y="9" width="6" height="6" rx="1" />
          </svg>
        </button>
        <button
          className="crosshook-library-toolbar__view-btn"
          aria-label="List view"
          aria-pressed={viewMode === 'list'}
          onClick={() => onViewModeChange('list')}
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
            <rect x="1" y="2" width="14" height="2" rx="1" />
            <rect x="1" y="7" width="14" height="2" rx="1" />
            <rect x="1" y="12" width="14" height="2" rx="1" />
          </svg>
        </button>
      </div>
    </div>
  );
}

export default LibraryToolbar;
