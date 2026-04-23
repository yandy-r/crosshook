import type { LibraryFilterKey, LibrarySortKey, LibraryViewMode } from '../../types/library';

/** Sort/filter keys exposed in the toolbar (others stay in the type union for persisted state). */
const SORT_OPTIONS: readonly { key: LibrarySortKey; label: string }[] = [
  { key: 'recent', label: 'Recent' },
  { key: 'name', label: 'Name' },
] as const;

const FILTER_OPTIONS: readonly { key: LibraryFilterKey; label: string }[] = [
  { key: 'all', label: 'All' },
  { key: 'favorites', label: 'Favorites' },
  { key: 'installed', label: 'Installed' },
] as const;

interface LibraryToolbarProps {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  viewMode: LibraryViewMode;
  onViewModeChange: (mode: LibraryViewMode) => void;
  sortBy: LibrarySortKey;
  onSortChange: (key: LibrarySortKey) => void;
  filter: LibraryFilterKey;
  onFilterChange: (key: LibraryFilterKey) => void;
  onOpenCommandPalette?: (restoreFocusTo?: HTMLElement | null) => void;
}

export function LibraryToolbar({
  searchQuery,
  onSearchChange,
  viewMode,
  onViewModeChange,
  sortBy,
  onSortChange,
  filter,
  onFilterChange,
  onOpenCommandPalette,
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
      <div
        role="group"
        aria-labelledby="crosshook-library-toolbar-sort-label"
        className="crosshook-library-toolbar__chip-group"
      >
        <span id="crosshook-library-toolbar-sort-label" className="crosshook-visually-hidden">
          Sort games
        </span>
        {SORT_OPTIONS.map((opt) => (
          <button
            key={opt.key}
            type="button"
            className="crosshook-library-toolbar__chip"
            aria-pressed={sortBy === opt.key}
            onClick={() => onSortChange(opt.key)}
          >
            {opt.label}
          </button>
        ))}
      </div>
      <div
        role="group"
        aria-labelledby="crosshook-library-toolbar-filter-label"
        className="crosshook-library-toolbar__chip-group"
      >
        <span id="crosshook-library-toolbar-filter-label" className="crosshook-visually-hidden">
          Filter games
        </span>
        {FILTER_OPTIONS.map((opt) => (
          <button
            key={opt.key}
            type="button"
            className="crosshook-library-toolbar__chip"
            aria-pressed={filter === opt.key}
            onClick={() => onFilterChange(opt.key)}
          >
            {opt.label}
          </button>
        ))}
      </div>
      <div className="crosshook-library-toolbar__view-toggle">
        <button
          type="button"
          className="crosshook-library-toolbar__view-btn"
          aria-label="Grid view"
          aria-pressed={viewMode === 'grid'}
          onClick={() => onViewModeChange('grid')}
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
            <rect x="1" y="1" width="6" height="6" rx="1" />
            <rect x="9" y="1" width="6" height="6" rx="1" />
            <rect x="1" y="9" width="6" height="6" rx="1" />
            <rect x="9" y="9" width="6" height="6" rx="1" />
          </svg>
        </button>
        <button
          type="button"
          className="crosshook-library-toolbar__view-btn"
          aria-label="List view"
          aria-pressed={viewMode === 'list'}
          onClick={() => onViewModeChange('list')}
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
            <rect x="1" y="2" width="14" height="2" rx="1" />
            <rect x="1" y="7" width="14" height="2" rx="1" />
            <rect x="1" y="12" width="14" height="2" rx="1" />
          </svg>
        </button>
      </div>
      {onOpenCommandPalette ? (
        <button
          type="button"
          className="crosshook-library-toolbar__palette-trigger"
          aria-label="Open command palette"
          onClick={(event) => onOpenCommandPalette?.(event.currentTarget)}
        >
          ⌘K
        </button>
      ) : null}
    </div>
  );
}

export default LibraryToolbar;
