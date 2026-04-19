import { formatRelativeTime } from '../../../utils/format';
import type { SortDirection, SortField, StatusFilter } from './constants';

const STATUS_OPTIONS: { value: StatusFilter; label: string }[] = [
  { value: 'all', label: 'All' },
  { value: 'healthy', label: 'Healthy' },
  { value: 'stale', label: 'Stale' },
  { value: 'broken', label: 'Broken' },
];

export function TableToolbar({
  statusFilter,
  onStatusFilter,
  searchQuery,
  onSearchQuery,
  shownCount,
  totalCount,
  loading,
  onRecheck,
  lastValidated,
  missingProtonCount,
  onFixProtonPaths,
  isScanning,
  onCheckAllVersions,
  isVersionScanning,
  versionScanProgress,
}: {
  statusFilter: StatusFilter;
  onStatusFilter: (f: StatusFilter) => void;
  searchQuery: string;
  onSearchQuery: (q: string) => void;
  shownCount: number;
  totalCount: number;
  loading: boolean;
  onRecheck: () => void;
  lastValidated: string | null;
  missingProtonCount?: number;
  onFixProtonPaths?: () => void;
  isScanning?: boolean;
  onCheckAllVersions?: () => void;
  isVersionScanning?: boolean;
  versionScanProgress?: { done: number; total: number } | null;
}) {
  return (
    <div className="crosshook-health-dashboard-toolbar">
      <fieldset className="crosshook-health-dashboard-toolbar__filters crosshook-fieldset-reset">
        <legend className="crosshook-visually-hidden">Filter by status</legend>
        {STATUS_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            type="button"
            className={`crosshook-status-chip crosshook-health-dashboard-toolbar__pill${statusFilter === opt.value ? ' crosshook-health-dashboard-toolbar__pill--active' : ''}`}
            onClick={() => onStatusFilter(opt.value)}
            aria-pressed={statusFilter === opt.value}
          >
            {opt.label}
          </button>
        ))}
      </fieldset>
      <input
        type="search"
        className="crosshook-input crosshook-health-dashboard-toolbar__search"
        placeholder="Filter profiles..."
        value={searchQuery}
        maxLength={200}
        onChange={(e) => onSearchQuery(e.target.value)}
        aria-label="Filter profiles by name"
      />
      <span className="crosshook-muted crosshook-health-dashboard-toolbar__count">
        Showing {shownCount} of {totalCount}
      </span>
      <div className="crosshook-health-dashboard-toolbar__recheck">
        {lastValidated && (
          <span className="crosshook-muted crosshook-health-dashboard-toolbar__validated">
            {formatRelativeTime(lastValidated)}
          </span>
        )}
        <button
          type="button"
          className="crosshook-button crosshook-button--ghost"
          disabled={loading}
          onClick={onRecheck}
          aria-label="Re-check all profiles"
        >
          {loading ? '↻ Checking...' : '↻ Re-check All'}
        </button>
        {onCheckAllVersions !== undefined && (
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost crosshook-focus-ring crosshook-nav-target"
            style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
            disabled={isVersionScanning}
            onClick={onCheckAllVersions}
            aria-label="Check version status for all displayed profiles"
            aria-disabled={isVersionScanning}
          >
            {isVersionScanning
              ? versionScanProgress
                ? `Checking ${versionScanProgress.done}/${versionScanProgress.total}\u2026`
                : 'Checking\u2026'
              : 'Check All Versions'}
          </button>
        )}
        {missingProtonCount !== undefined && missingProtonCount >= 2 && onFixProtonPaths !== undefined && (
          <button
            type="button"
            className="crosshook-button crosshook-focus-ring crosshook-nav-target"
            style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
            disabled={isScanning}
            onClick={onFixProtonPaths}
            aria-label={`Fix ${missingProtonCount} profiles with stale Proton paths`}
            aria-disabled={isScanning}
          >
            {isScanning ? 'Scanning\u2026' : `Fix Proton Paths (${missingProtonCount})`}
          </button>
        )}
      </div>
    </div>
  );
}

export function SortArrow({
  field,
  sortField,
  sortDirection,
}: {
  field: SortField;
  sortField: SortField;
  sortDirection: SortDirection;
}) {
  if (field !== sortField)
    return (
      <span
        className="crosshook-health-dashboard-sort-arrow crosshook-health-dashboard-sort-arrow--inactive"
        aria-hidden="true"
      >
        ↕
      </span>
    );
  return (
    <span
      className="crosshook-health-dashboard-sort-arrow crosshook-health-dashboard-sort-arrow--active"
      aria-hidden="true"
    >
      {sortDirection === 'asc' ? '↑' : '↓'}
    </span>
  );
}
