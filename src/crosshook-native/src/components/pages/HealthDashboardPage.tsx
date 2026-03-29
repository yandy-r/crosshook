import { Fragment, useEffect, useMemo, useRef, useState, useDeferredValue } from 'react';
import type { AppRoute } from '../layout/Sidebar';
import { HealthDashboardArt, PageBanner } from '../layout/PageBanner';
import { useProfileHealthContext } from '../../context/ProfileHealthContext';
import type { TrendDirection } from '../../hooks/useProfileHealth';
import { HealthBadge } from '../HealthBadge';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { formatRelativeTime } from '../../utils/format';
import { useProfileContext } from '../../context/ProfileContext';
import type { EnrichedProfileHealthReport, HealthIssue, HealthStatus } from '../../types/health';

type IssueCategory =
  | 'missing_executable'
  | 'missing_trainer'
  | 'missing_dll'
  | 'missing_proton'
  | 'missing_compatdata'
  | 'missing_prefix'
  | 'inaccessible_path'
  | 'other';

interface IssueCategoryCount {
  category: IssueCategory;
  label: string;
  count: number;
}

const CATEGORY_LABELS: Record<IssueCategory, string> = {
  missing_executable: 'Missing executable',
  missing_trainer: 'Missing trainer',
  missing_dll: 'Missing DLL',
  missing_proton: 'Missing/invalid Proton path',
  missing_compatdata: 'Inaccessible compatdata',
  missing_prefix: 'Missing prefix path',
  inaccessible_path: 'Inaccessible path',
  other: 'Other',
};

function categorizeIssue(issue: HealthIssue): IssueCategory {
  const { field, severity } = issue;
  if (field === 'game.executable_path') return 'missing_executable';
  if (field === 'trainer.path') return 'missing_trainer';
  if (field.startsWith('injection.dll_paths')) return 'missing_dll';
  if (field === 'steam.proton_path' || field === 'runtime.proton_path') return 'missing_proton';
  if (field === 'steam.compatdata_path') return 'missing_compatdata';
  if (field === 'runtime.prefix_path') return 'missing_prefix';
  if (severity === 'warning') return 'inaccessible_path';
  return 'other';
}

type SortField = 'name' | 'status' | 'issues' | 'last_success' | 'launch_method' | 'failures' | 'favorite';
type SortDirection = 'asc' | 'desc';
type StatusFilter = 'all' | HealthStatus;

const STATUS_RANK: Record<string, number> = { broken: 2, stale: 1, healthy: 0 };

type CardTrend = 'up' | 'down' | null;

function TrendArrow({ trend, improving }: { trend: CardTrend; improving: boolean }) {
  if (trend === null) return null;
  const isPositive = (trend === 'up' && improving) || (trend === 'down' && !improving);
  const color = isPositive ? 'var(--crosshook-color-success)' : 'var(--crosshook-color-danger)';
  const label = trend === 'up' ? 'trending up' : 'trending down';
  return (
    <span
      role="img"
      aria-label={label}
      style={{ fontSize: '0.8em', fontWeight: 700, color, marginLeft: '4px', lineHeight: 1 }}
    >
      {trend === 'up' ? '\u2191' : '\u2193'}
    </span>
  );
}

function SummaryCard({
  count,
  label,
  accentColor,
  disabled,
  trend,
  improving,
}: {
  count: number | null;
  label: string;
  accentColor: string;
  disabled?: boolean;
  trend?: CardTrend;
  improving?: boolean;
}) {
  const displayCount = disabled || count === null ? '—' : String(count);
  return (
    <div
      className="crosshook-card crosshook-health-dashboard-card"
      style={{ borderLeftColor: accentColor }}
      aria-disabled={disabled}
    >
      <div
        className="crosshook-health-dashboard-card__count"
        style={{ color: disabled ? undefined : accentColor }}
        aria-label={disabled ? undefined : `${count} ${label}`}
      >
        {displayCount}
        {!disabled && trend != null && <TrendArrow trend={trend} improving={improving ?? false} />}
      </div>
      <div className="crosshook-health-dashboard-card__label crosshook-muted">{label}</div>
    </div>
  );
}

function TableToolbar({
  statusFilter,
  onStatusFilter,
  searchQuery,
  onSearchQuery,
  shownCount,
  totalCount,
  loading,
  onRecheck,
  lastValidated,
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
}) {
  const statusOptions: { value: StatusFilter; label: string }[] = [
    { value: 'all', label: 'All' },
    { value: 'healthy', label: 'Healthy' },
    { value: 'stale', label: 'Stale' },
    { value: 'broken', label: 'Broken' },
  ];

  return (
    <div className="crosshook-health-dashboard-toolbar">
      <div className="crosshook-health-dashboard-toolbar__filters" role="group" aria-label="Filter by status">
        {statusOptions.map((opt) => (
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
      </div>
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
      </div>
    </div>
  );
}

function SortArrow({ field, sortField, sortDirection }: { field: SortField; sortField: SortField; sortDirection: SortDirection }) {
  if (field !== sortField) return <span className="crosshook-health-dashboard-sort-arrow crosshook-health-dashboard-sort-arrow--inactive" aria-hidden="true">↕</span>;
  return (
    <span className="crosshook-health-dashboard-sort-arrow crosshook-health-dashboard-sort-arrow--active" aria-hidden="true">
      {sortDirection === 'asc' ? '↑' : '↓'}
    </span>
  );
}

function SkeletonCards() {
  return (
    <>
      {[0, 1, 2, 3].map((i) => (
        <div
          key={i}
          className="crosshook-card crosshook-health-dashboard-skeleton-card"
          aria-hidden="true"
        >
          <div className="crosshook-health-dashboard-skeleton crosshook-health-dashboard-skeleton-card__count" />
          <div className="crosshook-health-dashboard-skeleton crosshook-health-dashboard-skeleton-card__label" />
        </div>
      ))}
    </>
  );
}

function SkeletonRows() {
  const widths = ['60%', '80%', '70%', '55%', '75%', '65%'];
  return (
    <>
      {[0, 1, 2, 3, 4, 5].map((i) => (
        <tr key={i} className="crosshook-health-dashboard-skeleton-row" aria-hidden="true">
          <td>
            <span
              className="crosshook-health-dashboard-skeleton crosshook-health-dashboard-skeleton-cell"
              style={{ width: '56px' }}
            />
          </td>
          <td>
            <span
              className="crosshook-health-dashboard-skeleton crosshook-health-dashboard-skeleton-cell"
              style={{ width: widths[i] }}
            />
          </td>
          <td>
            <span
              className="crosshook-health-dashboard-skeleton crosshook-health-dashboard-skeleton-cell"
              style={{ width: '24px' }}
            />
          </td>
          <td>
            <span
              className="crosshook-health-dashboard-skeleton crosshook-health-dashboard-skeleton-cell"
              style={{ width: '36px' }}
            />
          </td>
        </tr>
      ))}
    </>
  );
}

function RecentFailuresPanel({ profiles }: { profiles: EnrichedProfileHealthReport[] }) {
  if (profiles.length === 0) {
    return (
      <CollapsibleSection title="Recent Failures" defaultOpen={false}>
        <p className="crosshook-muted">No profiles with recent launch failures.</p>
      </CollapsibleSection>
    );
  }
  return (
    <CollapsibleSection title="Recent Failures" defaultOpen={false}>
      <ul className="crosshook-health-dashboard-failures-list">
        {profiles.map((report) => (
          <li key={report.name} className="crosshook-health-dashboard-failures-item">
            <span className="crosshook-health-dashboard-failures-item__name">{report.name}</span>
            <span className="crosshook-status-chip crosshook-health-dashboard-failures-item__count">
              {report.metadata!.failure_count_30d} failure{report.metadata!.failure_count_30d !== 1 ? 's' : ''} (30d)
            </span>
            <span className="crosshook-muted crosshook-health-dashboard-failures-item__last-success">
              {report.metadata?.last_success
                ? `Last success ${formatRelativeTime(report.metadata.last_success)}`
                : 'No successful launches recorded'}
            </span>
          </li>
        ))}
      </ul>
    </CollapsibleSection>
  );
}

const DRIFT_STATE_MESSAGES: Record<string, string> = {
  missing: 'Exported launcher not found',
  moved: 'Launcher has moved',
  stale: 'Launcher may be outdated',
};

function LauncherDriftPanel({ profiles }: { profiles: EnrichedProfileHealthReport[] }) {
  const driftProfiles = useMemo(() => {
    return (profiles ?? []).filter((r) => {
      const state = r.metadata?.launcher_drift_state;
      return state != null && state !== 'aligned' && state !== 'unknown';
    });
  }, [profiles]);

  return (
    <CollapsibleSection title="Launcher Drift" defaultOpen={false}>
      {driftProfiles.length === 0 ? (
        <p className="crosshook-muted">All exported launchers are current.</p>
      ) : (
        <ul className="crosshook-health-dashboard-issues-list">
          {driftProfiles.map((r) => {
            const state = r.metadata!.launcher_drift_state!;
            const message = DRIFT_STATE_MESSAGES[state] ?? state;
            return (
              <li key={r.name} className="crosshook-health-dashboard-issue">
                <span className="crosshook-health-dashboard-issue__field">{r.name}</span>
                <span className="crosshook-health-dashboard-issue__message crosshook-muted">{message}</span>
              </li>
            );
          })}
        </ul>
      )}
    </CollapsibleSection>
  );
}

function IssueBreakdownPanel({ profiles }: { profiles: EnrichedProfileHealthReport[] }) {
  const categoryCounts = useMemo(() => {
    const counts = new Map<IssueCategory, number>();
    for (const report of profiles) {
      for (const issue of report.issues) {
        const cat = categorizeIssue(issue);
        counts.set(cat, (counts.get(cat) ?? 0) + 1);
      }
    }
    const result: IssueCategoryCount[] = [];
    for (const [category, count] of counts.entries()) {
      result.push({ category, label: CATEGORY_LABELS[category], count });
    }
    result.sort((a, b) => b.count - a.count);
    return result;
  }, [profiles]);

  const maxCount = categoryCounts.length > 0 ? categoryCounts[0].count : 1;

  return (
    <CollapsibleSection title="Issue Breakdown" defaultOpen>
      {categoryCounts.length === 0 ? (
        <p className="crosshook-muted">No issues found across all profiles.</p>
      ) : (
        <ul className="crosshook-health-dashboard-breakdown-list">
          {categoryCounts.map(({ category, label, count }) => (
            <li key={category} className="crosshook-health-dashboard-breakdown-row">
              <span className="crosshook-health-dashboard-breakdown-label">{label}</span>
              <span className="crosshook-status-chip crosshook-health-dashboard-breakdown-badge">
                {count}
              </span>
              <div className="crosshook-health-dashboard-breakdown-bar-track" aria-hidden="true">
                <div
                  className="crosshook-health-dashboard-breakdown-bar"
                  style={{ width: `${Math.round((count / maxCount) * 100)}%` }}
                />
              </div>
            </li>
          ))}
        </ul>
      )}
    </CollapsibleSection>
  );
}

function IssueDetailRow({
  report,
  onRevalidate,
  onFixNavigate,
}: {
  report: EnrichedProfileHealthReport;
  onRevalidate: (name: string) => void;
  onFixNavigate: (name: string) => void | Promise<void>;
}) {
  const meta = report.metadata;
  return (
    <tr className="crosshook-health-dashboard-expanded-row">
      <td colSpan={9}>
        <div className="crosshook-health-dashboard-expanded-content">
          <div className="crosshook-health-dashboard-expanded-meta">
            <span><strong>Launch Method:</strong> {report.launch_method}</span>
            {meta?.last_success && (
              <span><strong>Last Success:</strong> {formatRelativeTime(meta.last_success)}</span>
            )}
            {meta != null && meta.failure_count_30d > 0 && (
              <span><strong>Failures (30d):</strong> {meta.failure_count_30d}</span>
            )}
            {meta?.is_favorite && <span>★ Favorite</span>}
            {meta?.is_community_import && <span className="crosshook-status-chip">Community</span>}
          </div>
          {report.issues.length === 0 ? (
            <p className="crosshook-muted">No issues found.</p>
          ) : (
            <ul className="crosshook-health-dashboard-issues-list">
              {report.issues.map((issue, i) => (
                <li key={i} className="crosshook-health-dashboard-issue">
                  <span className="crosshook-health-dashboard-issue__field">{issue.field}</span>
                  {issue.path && (
                    <code className="crosshook-health-dashboard-issue__path">{issue.path}</code>
                  )}
                  <span className="crosshook-health-dashboard-issue__message">{issue.message}</span>
                  {issue.remediation && (
                    <span className="crosshook-health-dashboard-issue__remediation crosshook-muted">{issue.remediation}</span>
                  )}
                </li>
              ))}
            </ul>
          )}
          <div className="crosshook-health-dashboard-expanded-actions">
            <button
              type="button"
              className="crosshook-button crosshook-button--small"
              onClick={() => onRevalidate(report.name)}
              aria-label={`Re-check ${report.name}`}
            >
              Re-check
            </button>
            {report.status !== 'healthy' && (
              <button
                type="button"
                className="crosshook-button crosshook-button--small"
                onClick={() => void onFixNavigate(report.name)}
                aria-label={`Fix ${report.name}`}
              >
                Fix
              </button>
            )}
          </div>
        </div>
      </td>
    </tr>
  );
}

function CommunityImportHealthPanel({ profiles }: { profiles: EnrichedProfileHealthReport[] }) {
  const unhealthyImports = useMemo(
    () => profiles.filter((r) => r.metadata?.is_community_import === true && r.status !== 'healthy'),
    [profiles],
  );

  return (
    <CollapsibleSection title="Community Import Health" defaultOpen={false}>
      {unhealthyImports.length === 0 ? (
        <p className="crosshook-muted">All community-imported profiles are healthy.</p>
      ) : (
        <>
          <p className="crosshook-help-text crosshook-muted">
            Imported profiles often need path adjustments for your system.
          </p>
          <ul className="crosshook-health-dashboard-panel-list">
            {unhealthyImports.map((report) => (
              <li key={report.name} className="crosshook-health-dashboard-panel-row">
                <span className="crosshook-health-dashboard-panel-row__name">{report.name}</span>
                <HealthBadge report={report} />
                <span className="crosshook-muted">{report.issues.length} issue{report.issues.length !== 1 ? 's' : ''}</span>
              </li>
            ))}
          </ul>
        </>
      )}
    </CollapsibleSection>
  );
}

export function HealthDashboardPage({ onNavigate }: { onNavigate?: (route: AppRoute) => void }) {
  const { summary, loading, error, batchValidate, cachedSnapshots, revalidateSingle, trendByName, staleInfoByName } = useProfileHealthContext();
  const { selectProfile } = useProfileContext();

  const [sortField, setSortField] = useState<SortField>('status');
  const [sortDirection, setSortDirection] = useState<SortDirection>('desc');
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [expandedProfile, setExpandedProfile] = useState<string | null>(null);

  const deferredSearch = useDeferredValue(searchQuery);

  async function handleFixNavigation(profileName: string) {
    await selectProfile(profileName);
    onNavigate?.('profiles');
  }

  function handleSortClick(field: SortField) {
    if (field === sortField) {
      setSortDirection((d) => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortField(field);
      setSortDirection('asc');
    }
  }

  function handleRowClick(report: EnrichedProfileHealthReport) {
    setExpandedProfile((prev) => (prev === report.name ? null : report.name));
  }

  const allProfiles = useMemo(() => {
    return (summary?.profiles ?? []).filter((r) => r.name !== '<unknown>');
  }, [summary?.profiles]);

  const filteredProfiles = useMemo(() => {
    const term = deferredSearch.toLowerCase().trim();

    let result = allProfiles;

    if (statusFilter !== 'all') {
      result = result.filter((r) => r.status === statusFilter);
    }

    if (term.length > 0) {
      result = result.filter((r) => r.name.toLowerCase().includes(term));
    }

    result = result.slice().sort((a, b) => {
      // Favorites always pin to top regardless of sort
      const aFav = a.metadata?.is_favorite ?? false;
      const bFav = b.metadata?.is_favorite ?? false;
      if (aFav !== bFav) return aFav ? -1 : 1;

      let cmp = 0;
      switch (sortField) {
        case 'name':
          cmp = a.name.localeCompare(b.name);
          break;
        case 'status':
          cmp = (STATUS_RANK[a.status] ?? 0) - (STATUS_RANK[b.status] ?? 0);
          if (cmp === 0) cmp = a.name.localeCompare(b.name);
          break;
        case 'issues':
          cmp = a.issues.length - b.issues.length;
          break;
        case 'last_success': {
          const aSuccess = a.metadata?.last_success ?? '';
          const bSuccess = b.metadata?.last_success ?? '';
          cmp = aSuccess.localeCompare(bSuccess);
          break;
        }
        case 'launch_method':
          cmp = a.launch_method.localeCompare(b.launch_method);
          break;
        case 'failures':
          cmp = (a.metadata?.failure_count_30d ?? 0) - (b.metadata?.failure_count_30d ?? 0);
          break;
        case 'favorite': {
          const aFavSort = a.metadata?.is_favorite ? 1 : 0;
          const bFavSort = b.metadata?.is_favorite ? 1 : 0;
          cmp = aFavSort - bFavSort;
          break;
        }
        default:
          cmp = 0;
      }

      return sortDirection === 'asc' ? cmp : -cmp;
    });

    return result;
  }, [allProfiles, sortField, sortDirection, statusFilter, deferredSearch]);

  const hasUnknownSentinel = (summary?.profiles ?? []).some((r) => r.name === '<unknown>');

  const cachedSnapshotList = useMemo(() => {
    return Object.values(cachedSnapshots).slice().sort((a, b) => {
      const rankDiff = (STATUS_RANK[b.status] ?? 0) - (STATUS_RANK[a.status] ?? 0);
      if (rankDiff !== 0) return rankDiff;
      return a.profile_name.localeCompare(b.profile_name);
    });
  }, [cachedSnapshots]);

  const recentFailures = useMemo(() => {
    return (summary?.profiles ?? [])
      .filter((r) => (r.metadata?.failure_count_30d ?? 0) > 0)
      .slice()
      .sort((a, b) => (b.metadata?.failure_count_30d ?? 0) - (a.metadata?.failure_count_30d ?? 0));
  }, [summary?.profiles]);

  const showLoadingCards = loading && !summary;
  const isEmpty = !loading && summary?.total_count === 0;
  const allHealthy =
    summary !== null &&
    summary.broken_count === 0 &&
    summary.stale_count === 0 &&
    summary.total_count > 0;

  const cardTrends = useMemo<{ healthy: CardTrend; stale: CardTrend; broken: CardTrend }>(() => {
    const snaps = Object.values(cachedSnapshots);
    if (snaps.length === 0 || !summary) {
      return { healthy: null, stale: null, broken: null };
    }
    const cachedHealthy = snaps.filter((s) => s.status === 'healthy').length;
    const cachedStale = snaps.filter((s) => s.status === 'stale').length;
    const cachedBroken = snaps.filter((s) => s.status === 'broken').length;
    const healthyDiff = summary.healthy_count - cachedHealthy;
    const staleDiff = summary.stale_count - cachedStale;
    const brokenDiff = summary.broken_count - cachedBroken;
    return {
      healthy: healthyDiff > 0 ? 'up' : healthyDiff < 0 ? 'down' : null,
      stale: staleDiff > 0 ? 'up' : staleDiff < 0 ? 'down' : null,
      broken: brokenDiff > 0 ? 'up' : brokenDiff < 0 ? 'down' : null,
    };
  }, [cachedSnapshots, summary]);

  const [ariaAnnouncement, setAriaAnnouncement] = useState('');
  const recheckPendingRef = useRef(false);
  const yButtonPrevRef = useRef(false);

  useEffect(() => {
    if (typeof window === 'undefined') return;

    let rafId = 0;

    const poll = () => {
      const gamepad = navigator.getGamepads?.()[0];
      if (gamepad) {
        const yPressed = Boolean(gamepad.buttons[3]?.pressed);
        const wasPressed = yButtonPrevRef.current;
        if (yPressed && !wasPressed && !loading) {
          recheckPendingRef.current = true;
          void batchValidate();
        }
        yButtonPrevRef.current = yPressed;
      }
      rafId = window.requestAnimationFrame(poll);
    };

    rafId = window.requestAnimationFrame(poll);

    return () => {
      window.cancelAnimationFrame(rafId);
    };
  }, [loading, batchValidate]);

  useEffect(() => {
    if (loading) {
      if (recheckPendingRef.current) {
        setAriaAnnouncement('Checking all profiles...');
      }
      return;
    }
    if (!recheckPendingRef.current) {
      return;
    }
    recheckPendingRef.current = false;
    if (error) {
      setAriaAnnouncement('');
    } else if (summary) {
      setAriaAnnouncement(
        `Validation complete. ${summary.broken_count} broken, ${summary.stale_count} stale, ${summary.healthy_count} healthy.`,
      );
    } else {
      setAriaAnnouncement('Validation complete.');
    }
  }, [loading, error, summary]);

  function handleRetry() {
    if (error) {
      console.error('Health scan error (retrying):', error);
    }
    void batchValidate();
  }

  function handleRecheck() {
    recheckPendingRef.current = true;
    void batchValidate();
  }

  function getAriaSortAttr(field: SortField): 'ascending' | 'descending' | 'none' {
    if (sortField !== field) return 'none';
    return sortDirection === 'asc' ? 'ascending' : 'descending';
  }

  function renderSortHeader(field: SortField, label: string) {
    return (
      <th
        role="columnheader"
        scope="col"
        aria-sort={getAriaSortAttr(field)}
        onClick={() => handleSortClick(field)}
        className="crosshook-health-dashboard-th"
      >
        <span className="crosshook-health-dashboard-th__inner">
          {label}
          <SortArrow field={field} sortField={sortField} sortDirection={sortDirection} />
        </span>
      </th>
    );
  }

  return (
    <>
      <PageBanner
        eyebrow="Dashboards"
        title="Profile Health"
        copy="Aggregate status across all profiles."
        illustration={<HealthDashboardArt />}
      />

      <div className="crosshook-page" data-crosshook-focus-zone="content">
        {error && (
          <div role="alert" className="crosshook-health-dashboard-error crosshook-panel">
            <p>Health scan failed. Check app logs for details.</p>
            <button
              type="button"
              className="crosshook-button"
              onClick={handleRetry}
            >
              Retry
            </button>
          </div>
        )}

        <div className="crosshook-health-dashboard-cards" aria-busy={showLoadingCards}>
          {showLoadingCards ? (
            <SkeletonCards />
          ) : (
            <>
              <SummaryCard
                count={summary?.total_count ?? null}
                label="Total"
                accentColor="var(--crosshook-color-accent)"
                disabled={false}
              />
              <SummaryCard
                count={summary?.healthy_count ?? null}
                label="Healthy"
                accentColor="var(--crosshook-color-success)"
                disabled={false}
                trend={cardTrends.healthy}
                improving
              />
              <SummaryCard
                count={summary?.stale_count ?? null}
                label="Stale"
                accentColor="var(--crosshook-color-warning)"
                disabled={false}
                trend={cardTrends.stale}
                improving={false}
              />
              <SummaryCard
                count={summary?.broken_count ?? null}
                label="Broken"
                accentColor="var(--crosshook-color-danger)"
                disabled={false}
                trend={cardTrends.broken}
                improving={false}
              />
            </>
          )}
        </div>

        {isEmpty && (
          <div className="crosshook-health-dashboard-empty crosshook-panel">
            <p>No profiles configured yet.</p>
            <button
              type="button"
              className="crosshook-button"
              onClick={() => onNavigate?.('profiles')}
            >
              Go to Profiles
            </button>
          </div>
        )}

        {allHealthy && (
          <p className="crosshook-health-dashboard-all-healthy crosshook-muted">
            All profiles are healthy.
          </p>
        )}

        {hasUnknownSentinel && (
          <div role="alert" className="crosshook-health-dashboard-error crosshook-panel">
            <p>One or more profiles could not be identified. Check app logs for details.</p>
          </div>
        )}

        {(summary || showLoadingCards || cachedSnapshotList.length > 0) && (
        <div className="crosshook-health-dashboard-section">
          <h2 className="crosshook-heading-section crosshook-health-dashboard-section__heading">
            Profile Status
          </h2>
        </div>
        )}

        {summary === null && cachedSnapshotList.length > 0 && (
          <div className="crosshook-panel">
            <p className="crosshook-muted">Cached — checking...</p>
            <table
              role="grid"
              aria-label="Profile health status (cached)"
              aria-rowcount={cachedSnapshotList.length}
              className="crosshook-health-dashboard-table"
            >
              <thead>
                <tr role="row">
                  <th role="columnheader" scope="col">Status</th>
                  <th role="columnheader" scope="col">Name</th>
                  <th role="columnheader" scope="col">Issues</th>
                  <th role="columnheader" scope="col">Last Success</th>
                  <th role="columnheader" scope="col">Method</th>
                  <th role="columnheader" scope="col">Failures</th>
                  <th role="columnheader" scope="col">&#9733;</th>
                  <th role="columnheader" scope="col">Source</th>
                  <th role="columnheader" scope="col">Actions</th>
                </tr>
              </thead>
              <tbody>
                {cachedSnapshotList.map((snap) => (
                  <tr
                    key={snap.profile_name}
                    tabIndex={0}
                    role="row"
                    aria-label={`${snap.profile_name} — ${snap.status}, ${snap.issue_count} issues`}
                    onClick={() => {
                      if (snap.status !== 'healthy') {
                        void handleFixNavigation(snap.profile_name);
                      }
                    }}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && snap.status !== 'healthy') {
                        void handleFixNavigation(snap.profile_name);
                      }
                    }}
                    style={snap.status !== 'healthy' ? { cursor: 'pointer' } : undefined}
                  >
                    <td>
                      <HealthBadge status={snap.status} />
                    </td>
                    <td>{snap.profile_name}</td>
                    <td>{snap.issue_count}</td>
                    <td>&#8212;</td>
                    <td>&#8212;</td>
                    <td>&#8212;</td>
                    <td></td>
                    <td></td>
                    <td></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {summary && (
          <>
            <TableToolbar
              statusFilter={statusFilter}
              onStatusFilter={setStatusFilter}
              searchQuery={searchQuery}
              onSearchQuery={setSearchQuery}
              shownCount={filteredProfiles.length}
              totalCount={allProfiles.length}
              loading={loading}
              onRecheck={handleRecheck}
              lastValidated={summary.validated_at ?? null}
            />

            <table
              role="grid"
              aria-label="Profile health status"
              aria-rowcount={filteredProfiles.length}
              className="crosshook-health-dashboard-table"
            >
              <thead>
                <tr role="row">
                  {renderSortHeader('status', 'Status')}
                  {renderSortHeader('name', 'Name')}
                  {renderSortHeader('issues', 'Issues')}
                  {renderSortHeader('last_success', 'Last Success')}
                  {renderSortHeader('launch_method', 'Method')}
                  {renderSortHeader('failures', 'Failures')}
                  {renderSortHeader('favorite', '★')}
                  <th role="columnheader" scope="col" className="crosshook-health-dashboard-th">Source</th>
                  <th role="columnheader" scope="col" className="crosshook-health-dashboard-th crosshook-health-dashboard-th--actions">Actions</th>
                </tr>
              </thead>
              <tbody>
                {filteredProfiles.map((report) => {
                  const isExpanded = expandedProfile === report.name;
                  const rowTrend = trendByName[report.name];
                  const rowStaleInfo = staleInfoByName[report.name];
                  const activeTrend: TrendDirection = (rowTrend === 'unchanged' || rowTrend === undefined) ? null : rowTrend ?? null;
                  return (
                    <Fragment key={report.name}>
                      <tr
                        tabIndex={0}
                        role="row"
                        aria-label={`${report.name} — ${report.status}, ${report.issues.length} issues`}
                        aria-expanded={isExpanded}
                        onClick={() => handleRowClick(report)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') {
                            handleRowClick(report);
                          }
                        }}
                        className={`crosshook-health-dashboard-row${isExpanded ? ' crosshook-health-dashboard-row--expanded' : ''}`}
                      >
                        <td className="crosshook-health-dashboard-td--status">
                          <HealthBadge report={report} trend={activeTrend} />
                          {rowStaleInfo?.isStale && (
                            <span className="crosshook-muted crosshook-health-dashboard-stale-hint">
                              (cached {rowStaleInfo.daysAgo}d ago)
                            </span>
                          )}
                        </td>
                        <td>{report.name}</td>
                        <td className="crosshook-health-dashboard-td--issues">{report.issues.length}</td>
                        <td className="crosshook-health-dashboard-td--last-success">
                          {report.metadata?.last_success ? formatRelativeTime(report.metadata.last_success) : 'N/A'}
                        </td>
                        <td className="crosshook-health-dashboard-td--method">{report.launch_method}</td>
                        <td className="crosshook-health-dashboard-td--failures">
                          {report.metadata != null ? report.metadata.failure_count_30d : 'N/A'}
                        </td>
                        <td className="crosshook-health-dashboard-td--favorite">
                          {report.metadata?.is_favorite ? '★' : ''}
                        </td>
                        <td className="crosshook-health-dashboard-td--source">
                          {report.metadata?.is_community_import ? (
                            <span className="crosshook-status-chip">Community</span>
                          ) : null}
                        </td>
                        <td className="crosshook-health-dashboard-td--actions" onClick={(e) => e.stopPropagation()}>
                          {report.status !== 'healthy' && (
                            <button
                              type="button"
                              className="crosshook-button crosshook-button--small"
                              onClick={() => void handleFixNavigation(report.name)}
                              aria-label={`Fix ${report.name}`}
                            >
                              Fix
                            </button>
                          )}
                        </td>
                      </tr>
                      {isExpanded && (
                        <IssueDetailRow
                          report={report}
                          onRevalidate={(name) => void revalidateSingle(name)}
                          onFixNavigate={handleFixNavigation}
                        />
                      )}
                    </Fragment>
                  );
                })}
              </tbody>
            </table>

            <div className="crosshook-health-dashboard-validation-strip" aria-live="polite" aria-atomic="true">
              {ariaAnnouncement && (
                <p className="crosshook-health-dashboard-validation-strip__text">
                  {ariaAnnouncement}
                </p>
              )}
            </div>

            <IssueBreakdownPanel profiles={allProfiles} />

            <RecentFailuresPanel profiles={recentFailures} />

            <LauncherDriftPanel profiles={allProfiles} />

            <CommunityImportHealthPanel profiles={allProfiles} />
          </>
        )}
      </div>
    </>
  );
}

export default HealthDashboardPage;
