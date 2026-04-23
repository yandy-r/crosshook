import { Fragment, useState } from 'react';
import { useProfileContext } from '../../context/ProfileContext';
import { useProfileHealthContext } from '../../context/ProfileHealthContext';
import { useOfflineReadiness } from '../../hooks/useOfflineReadiness';
import type { TrendDirection } from '../../hooks/useProfileHealth';
import { useProtonMigration } from '../../hooks/useProtonMigration';
import { useVersionCheck } from '../../hooks/useVersionCheck';
import { formatRelativeTime } from '../../utils/format';
import { HealthBadge } from '../HealthBadge';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import { RouteBanner } from '../layout/RouteBanner';
import type { AppRoute } from '../layout/Sidebar';
import { MigrationReviewModal } from '../MigrationReviewModal';
import { OfflineStatusBadge } from '../OfflineStatusBadge';
import { CommunityImportHealthPanel } from './health-dashboard/CommunityImportHealthPanel';
import type { SortField } from './health-dashboard/constants';
import { IssueBreakdownPanel, IssueDetailRow } from './health-dashboard/IssueBreakdownPanel';
import { LauncherDriftPanel } from './health-dashboard/LauncherDriftPanel';
import { RecentFailuresPanel } from './health-dashboard/RecentFailuresPanel';
import { SkeletonCards, SummaryCard } from './health-dashboard/SummaryCards';
import { SortArrow, TableToolbar } from './health-dashboard/TableControls';
import { useHealthDashboardState } from './health-dashboard/useHealthDashboardState';
import { getVersionStatusColor, getVersionStatusLabel, mergeOfflineReadinessForRow } from './health-dashboard/utils';

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

  const [expandedProfile, setExpandedProfile] = useState<string | null>(null);
  const [isMigrationModalOpen, setIsMigrationModalOpen] = useState(false);
  const [isVersionScanning, setIsVersionScanning] = useState(false);
  const [versionScanProgress, setVersionScanProgress] = useState<{ done: number; total: number } | null>(null);

  const {
    sortField,
    sortDirection,
    statusFilter,
    searchQuery,
    setStatusFilter,
    setSearchQuery,
    allProfiles,
    missingProtonCount,
    filteredProfiles,
    hasUnknownSentinel,
    cachedSnapshotList,
    recentFailures,
    cardTrends,
    ariaAnnouncement,
    recheckPendingRef,
    handleSortClick,
    getAriaSortAttr,
  } = useHealthDashboardState({ summary, loading, error, batchValidate, cachedSnapshots });

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

  function handleRowClick(report: { name: string }) {
    setExpandedProfile((prev) => (prev === report.name ? null : report.name));
  }

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

  const showLoadingCards = loading && !summary;
  const isEmpty = !loading && summary?.total_count === 0;
  const allHealthy =
    summary !== null && summary.broken_count === 0 && summary.stale_count === 0 && summary.total_count > 0;
  const showPrimarySection = summary !== null || cachedSnapshotList.length > 0;

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--health">
      <div className="crosshook-route-stack" data-crosshook-focus-zone="content">
        <RouteBanner route="health" />
        <div className="crosshook-dashboard-route-body crosshook-dashboard-route-section-stack">
          {error && (
            <div role="alert" className="crosshook-health-dashboard-error crosshook-panel">
              <p>Health scan failed. Check app logs for details.</p>
              <button type="button" className="crosshook-button" onClick={handleRetry}>
                Retry
              </button>
            </div>
          )}

          {hasUnknownSentinel && (
            <div role="alert" className="crosshook-health-dashboard-error crosshook-panel">
              <p>One or more profiles could not be identified. Check app logs for details.</p>
            </div>
          )}

          <DashboardPanelSection
            eyebrow="Health overview"
            title="Monitor profile readiness across launch, version, and offline checks"
            description="Review the current health snapshot, track recheck progress, and spot stale or broken profiles before they interrupt a launch."
          >
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

            <div className="crosshook-dashboard-pill-row" aria-live="polite">
              {loading && <span className="crosshook-dashboard-pill">Checking profile health…</span>}
              {summary?.validated_at && (
                <span className="crosshook-dashboard-pill">
                  Last checked {formatRelativeTime(summary.validated_at)}
                </span>
              )}
              {summary === null && cachedSnapshotList.length > 0 && (
                <span className="crosshook-dashboard-pill">Showing cached results while refreshing</span>
              )}
              {allHealthy && <span className="crosshook-dashboard-pill">All profiles are currently healthy</span>}
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
          </DashboardPanelSection>

          {showPrimarySection && (
            <DashboardPanelSection
              eyebrow="Primary status"
              title={summary ? 'Profile status' : 'Cached profile status'}
              description={
                summary
                  ? 'Filter, recheck, scan version status, and expand rows for detailed remediation guidance.'
                  : 'The last cached snapshot is shown while CrossHook refreshes the latest health results.'
              }
            >
              {summary === null && cachedSnapshotList.length > 0 && (
                <>
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
                </>
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
                        <th
                          scope="col"
                          className="crosshook-health-dashboard-th crosshook-health-dashboard-th--actions"
                        >
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
                                {report.metadata?.last_success
                                  ? formatRelativeTime(report.metadata.last_success)
                                  : 'N/A'}
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
                              <td
                                className="crosshook-health-dashboard-td--actions"
                                onClick={(e) => e.stopPropagation()}
                              >
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
                </>
              )}
            </DashboardPanelSection>
          )}

          {summary && (
            <>
              <DashboardPanelSection
                eyebrow="Supporting signals"
                title="Issue categories and recent failures"
                description="Use these rollups to prioritize broad profile issues before drilling into individual rows."
              >
                <div className="crosshook-dashboard-route-section-grid">
                  <IssueBreakdownPanel profiles={allProfiles} />
                  <RecentFailuresPanel profiles={recentFailures} />
                </div>
              </DashboardPanelSection>

              <DashboardPanelSection
                eyebrow="Supporting signals"
                title="Launcher drift and imported profile follow-up"
                description="Track exported launcher drift and community-imported profiles that still need system-specific cleanup."
              >
                <div className="crosshook-dashboard-route-section-grid">
                  <LauncherDriftPanel profiles={allProfiles} />
                  <CommunityImportHealthPanel profiles={allProfiles} />
                </div>
              </DashboardPanelSection>
            </>
          )}
        </div>

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
