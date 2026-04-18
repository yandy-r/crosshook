import { Fragment, useDeferredValue, useEffect, useMemo, useRef, useState } from 'react';
import { useProfileContext } from '../../context/ProfileContext';
import { useProfileHealthContext } from '../../context/ProfileHealthContext';
import { useOfflineReadiness } from '../../hooks/useOfflineReadiness';
import type { TrendDirection } from '../../hooks/useProfileHealth';
import { useProtonMigration } from '../../hooks/useProtonMigration';
import { useVersionCheck } from '../../hooks/useVersionCheck';
import { formatRelativeTime } from '../../utils/format';
import { HealthBadge } from '../HealthBadge';
import { RouteBanner } from '../layout/RouteBanner';
import type { AppRoute } from '../layout/Sidebar';
import { MigrationReviewModal } from '../MigrationReviewModal';
import { OfflineStatusBadge } from '../OfflineStatusBadge';
import { CommunityImportHealthPanel } from './health-dashboard/CommunityImportHealthPanel';
import type { CardTrend, SortDirection, SortField, StatusFilter } from './health-dashboard/constants';
import { STATUS_RANK, VERSION_STATUS_RANK } from './health-dashboard/constants';
import { IssueBreakdownPanel, IssueDetailRow } from './health-dashboard/IssueBreakdownPanel';
import { LauncherDriftPanel } from './health-dashboard/LauncherDriftPanel';
import { RecentFailuresPanel } from './health-dashboard/RecentFailuresPanel';
import { SkeletonCards, SummaryCard } from './health-dashboard/SummaryCards';
import { SortArrow, TableToolbar } from './health-dashboard/TableControls';
import {
  categorizeIssue,
  getVersionStatusColor,
  getVersionStatusLabel,
  mergeOfflineReadinessForRow,
  offlineSortScore,
} from './health-dashboard/utils';

export function HealthDashboardPage({ onNavigate }: { onNavigate?: (route: AppRoute) => void }) {
  const { summary, loading, error, batchValidate, cachedSnapshots, revalidateSingle, trendByName, staleInfoByName } =
    useProfileHealthContext();
  const { selectProfile } = useProfileContext();
  const {
    scanResult: batchScanResult,
    isScanning: isBatchScanning,
    applyBatchMigration,
    isBatchApplying,
    batchResult,
    batchError,
    scanMigrations,
  } = useProtonMigration();

  const offlineReadiness = useOfflineReadiness();
  const { checkVersionStatus } = useVersionCheck();

  const [sortField, setSortField] = useState<SortField>('status');
  const [sortDirection, setSortDirection] = useState<SortDirection>('desc');
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [expandedProfile, setExpandedProfile] = useState<string | null>(null);
  const [isMigrationModalOpen, setIsMigrationModalOpen] = useState(false);
  const [isVersionScanning, setIsVersionScanning] = useState(false);
  const [versionScanProgress, setVersionScanProgress] = useState<{ done: number; total: number } | null>(null);

  const deferredSearch = useDeferredValue(searchQuery);

  async function handleFixNavigation(profileName: string) {
    await selectProfile(profileName);
    onNavigate?.('profiles');
  }

  async function handleFixProtonPaths() {
    const result = await scanMigrations();
    if (result !== null) {
      setIsMigrationModalOpen(true);
    }
  }

  function handleMigrationModalClose() {
    setIsMigrationModalOpen(false);
  }

  async function handleCheckAllVersions() {
    if (isVersionScanning || filteredProfiles.length === 0) return;
    setIsVersionScanning(true);
    setVersionScanProgress({ done: 0, total: filteredProfiles.length });
    try {
      for (const report of filteredProfiles) {
        await checkVersionStatus(report.name);
        setVersionScanProgress((prev) => (prev ? { ...prev, done: prev.done + 1 } : null));
      }
    } finally {
      setIsVersionScanning(false);
      setVersionScanProgress(null);
    }
    void batchValidate();
  }

  function handleSortClick(field: SortField) {
    if (field === sortField) {
      setSortDirection((d) => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortField(field);
      setSortDirection('asc');
    }
  }

  function handleRowClick(report: { name: string }) {
    setExpandedProfile((prev) => (prev === report.name ? null : report.name));
  }

  const allProfiles = useMemo(() => {
    return (summary?.profiles ?? []).filter((r) => r.name !== '<unknown>');
  }, [summary?.profiles]);

  const missingProtonCount = useMemo(() => {
    return allProfiles.filter((r) => r.issues.some((issue) => categorizeIssue(issue) === 'missing_proton')).length;
  }, [allProfiles]);

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
        case 'version_status': {
          const aRank = VERSION_STATUS_RANK[a.metadata?.version_status ?? 'unknown'] ?? -1;
          const bRank = VERSION_STATUS_RANK[b.metadata?.version_status ?? 'unknown'] ?? -1;
          cmp = aRank - bRank;
          break;
        }
        case 'offline_score': {
          cmp =
            offlineSortScore(a, offlineReadiness.reportForProfile(a.name)) -
            offlineSortScore(b, offlineReadiness.reportForProfile(b.name));
          if (cmp === 0) {
            cmp = a.name.localeCompare(b.name);
          }
          break;
        }
        default:
          cmp = 0;
      }

      return sortDirection === 'asc' ? cmp : -cmp;
    });

    return result;
  }, [allProfiles, sortField, sortDirection, statusFilter, deferredSearch, offlineReadiness]);

  const hasUnknownSentinel = (summary?.profiles ?? []).some((r) => r.name === '<unknown>');

  const cachedSnapshotList = useMemo(() => {
    return Object.values(cachedSnapshots)
      .slice()
      .sort((a, b) => {
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
    summary !== null && summary.broken_count === 0 && summary.stale_count === 0 && summary.total_count > 0;

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
        `Validation complete. ${summary.broken_count} broken, ${summary.stale_count} stale, ${summary.healthy_count} healthy.`
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
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--health">
      <div className="crosshook-route-stack" data-crosshook-focus-zone="content">
        <RouteBanner route="health" />
        {error && (
          <div role="alert" className="crosshook-health-dashboard-error crosshook-panel">
            <p>Health scan failed. Check app logs for details.</p>
            <button type="button" className="crosshook-button" onClick={handleRetry}>
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
            <button type="button" className="crosshook-button" onClick={() => onNavigate?.('profiles')}>
              Go to Profiles
            </button>
          </div>
        )}

        {allHealthy && (
          <p className="crosshook-health-dashboard-all-healthy crosshook-muted">All profiles are healthy.</p>
        )}

        {hasUnknownSentinel && (
          <div role="alert" className="crosshook-health-dashboard-error crosshook-panel">
            <p>One or more profiles could not be identified. Check app logs for details.</p>
          </div>
        )}

        {(summary || showLoadingCards || cachedSnapshotList.length > 0) && (
          <div className="crosshook-health-dashboard-section">
            <h2 className="crosshook-heading-section crosshook-health-dashboard-section__heading">Profile Status</h2>
          </div>
        )}

        {summary === null && cachedSnapshotList.length > 0 && (
          <div className="crosshook-panel">
            <p className="crosshook-muted">Cached — checking...</p>
            <table
              aria-label="Profile health status (cached)"
              aria-rowcount={cachedSnapshotList.length}
              className="crosshook-health-dashboard-table"
            >
              <thead>
                <tr>
                  <th scope="col">Status</th>
                  <th scope="col">Name</th>
                  <th scope="col">Issues</th>
                  <th scope="col">Last Success</th>
                  <th scope="col">Method</th>
                  <th scope="col">Failures</th>
                  <th scope="col">&#9733;</th>
                  <th scope="col">Version</th>
                  <th scope="col">Offline</th>
                  <th scope="col">Source</th>
                  <th scope="col">Actions</th>
                </tr>
              </thead>
              <tbody>
                {cachedSnapshotList.map((snap) => {
                  const offCached = offlineReadiness.reportForProfile(snap.profile_name);
                  return (
                    <tr
                      key={snap.profile_name}
                      tabIndex={0}
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
                      <td className="crosshook-health-dashboard-td--status">
                        <HealthBadge status={snap.status} />
                      </td>
                      <td>{snap.profile_name}</td>
                      <td>{snap.issue_count}</td>
                      <td>&#8212;</td>
                      <td>&#8212;</td>
                      <td>&#8212;</td>
                      <td></td>
                      <td></td>
                      <td className="crosshook-health-dashboard-td--offline">
                        {offCached ? (
                          <OfflineStatusBadge report={offCached} compact />
                        ) : (
                          <span className="crosshook-muted">—</span>
                        )}
                      </td>
                      <td></td>
                      <td></td>
                    </tr>
                  );
                })}
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
              missingProtonCount={missingProtonCount}
              onFixProtonPaths={() => void handleFixProtonPaths()}
              isScanning={isBatchScanning}
              onCheckAllVersions={() => void handleCheckAllVersions()}
              isVersionScanning={isVersionScanning}
              versionScanProgress={versionScanProgress}
            />

            <table
              aria-label="Profile health status"
              aria-rowcount={filteredProfiles.length}
              className="crosshook-health-dashboard-table"
            >
              <thead>
                <tr>
                  {renderSortHeader('status', 'Status')}
                  {renderSortHeader('name', 'Name')}
                  {renderSortHeader('issues', 'Issues')}
                  {renderSortHeader('last_success', 'Last Success')}
                  {renderSortHeader('launch_method', 'Method')}
                  {renderSortHeader('failures', 'Failures')}
                  {renderSortHeader('favorite', '★')}
                  {renderSortHeader('version_status', 'Version')}
                  {renderSortHeader('offline_score', 'Offline')}
                  <th scope="col" className="crosshook-health-dashboard-th">
                    Source
                  </th>
                  <th scope="col" className="crosshook-health-dashboard-th crosshook-health-dashboard-th--actions">
                    Actions
                  </th>
                </tr>
              </thead>
              <tbody>
                {filteredProfiles.map((report) => {
                  const isExpanded = expandedProfile === report.name;
                  const rowTrend = trendByName[report.name];
                  const rowStaleInfo = staleInfoByName[report.name];
                  const activeTrend: TrendDirection =
                    rowTrend === 'unchanged' || rowTrend === undefined ? null : (rowTrend ?? null);
                  const offlineReport = mergeOfflineReadinessForRow(
                    report,
                    offlineReadiness.reportForProfile(report.name)
                  );
                  return (
                    <Fragment key={report.name}>
                      <tr
                        tabIndex={0}
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
                        <td className="crosshook-health-dashboard-td--version">
                          <span
                            className="crosshook-status-chip crosshook-health-dashboard-version-badge"
                            style={{ color: getVersionStatusColor(report.metadata?.version_status) }}
                          >
                            {getVersionStatusLabel(report.metadata?.version_status)}
                          </span>
                        </td>
                        <td className="crosshook-health-dashboard-td--offline">
                          {offlineReport ? (
                            <OfflineStatusBadge report={offlineReport} compact />
                          ) : (
                            <span className="crosshook-muted">—</span>
                          )}
                        </td>
                        <td className="crosshook-health-dashboard-td--source">
                          {report.metadata?.is_community_import ? (
                            <span className="crosshook-status-chip crosshook-health-dashboard-source-chip">
                              Community
                            </span>
                          ) : null}
                        </td>
                        {/* biome-ignore lint/a11y/useKeyWithClickEvents: td stopPropagation, not an action */}
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
                          offlineReadinessReport={offlineReport}
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
                <p className="crosshook-health-dashboard-validation-strip__text">{ariaAnnouncement}</p>
              )}
            </div>

            <IssueBreakdownPanel profiles={allProfiles} />

            <RecentFailuresPanel profiles={recentFailures} />

            <LauncherDriftPanel profiles={allProfiles} />

            <CommunityImportHealthPanel profiles={allProfiles} />
          </>
        )}

        {isMigrationModalOpen && batchScanResult !== null && (
          <MigrationReviewModal
            scanResult={batchScanResult}
            onClose={handleMigrationModalClose}
            onApply={(requests) => void applyBatchMigration(requests)}
            isBatchApplying={isBatchApplying}
            batchResult={batchResult}
            batchError={batchError}
          />
        )}
      </div>
    </div>
  );
}

export default HealthDashboardPage;
