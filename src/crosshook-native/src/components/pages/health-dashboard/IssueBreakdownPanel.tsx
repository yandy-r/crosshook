import { useEffect, useMemo, useState } from 'react';
import { useProtonMigration } from '../../../hooks/useProtonMigration';
import type { MigrationSuggestion, OfflineReadinessReport, ProtonPathField } from '../../../types';
import type { EnrichedProfileHealthReport } from '../../../types/health';
import { formatRelativeTime } from '../../../utils/format';
import { OfflineReadinessPanel } from '../../OfflineReadinessPanel';
import { CollapsibleSection } from '../../ui/CollapsibleSection';
import { buildCategoryCounts, categorizeIssue } from './utils';

export function IssueBreakdownPanel({ profiles }: { profiles: EnrichedProfileHealthReport[] }) {
  const { categoryCounts, totalRawIssues } = useMemo(() => buildCategoryCounts(profiles), [profiles]);

  const maxCount = categoryCounts.length > 0 ? categoryCounts[0].count : 1;

  return (
    <CollapsibleSection title="Issue Breakdown" defaultOpen>
      {categoryCounts.length === 0 && totalRawIssues === 0 ? (
        <p className="crosshook-muted">No issues found across all profiles.</p>
      ) : categoryCounts.length === 0 && totalRawIssues > 0 ? (
        <p className="crosshook-muted">
          No actionable issues in this summary. Informational checks (including offline readiness notes) are excluded
          here — expand a profile row to see full details.
        </p>
      ) : (
        <ul className="crosshook-health-dashboard-breakdown-list">
          {categoryCounts.map(({ category, label, count }) => (
            <li key={category} className="crosshook-health-dashboard-breakdown-row">
              <span className="crosshook-health-dashboard-breakdown-label">{label}</span>
              <span className="crosshook-status-chip crosshook-health-dashboard-breakdown-badge">{count}</span>
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

export function IssueDetailRow({
  report,
  offlineReadinessReport,
  onRevalidate,
  onFixNavigate,
}: {
  report: EnrichedProfileHealthReport;
  offlineReadinessReport: OfflineReadinessReport | undefined;
  onRevalidate: (name: string) => void;
  onFixNavigate: (name: string) => void | Promise<void>;
}) {
  const {
    scanResult,
    isScanning,
    applyResult,
    isApplying,
    error: migrationError,
    scanMigrations,
    applySingleMigration,
  } = useProtonMigration();
  const [activeMigrationField, setActiveMigrationField] = useState<string | null>(null);
  const [successNotice, setSuccessNotice] = useState<string | null>(null);

  useEffect(() => {
    if (!applyResult || applyResult.outcome !== 'applied') return;
    setSuccessNotice('Proton path updated.');
    setActiveMigrationField(null);
    const timer = setTimeout(() => setSuccessNotice(null), 5000);
    return () => clearTimeout(timer);
  }, [applyResult]);

  function getProtonPathField(issueField: string): ProtonPathField | null {
    if (issueField === 'steam.proton_path') return 'steam_proton_path';
    if (issueField === 'runtime.proton_path') return 'runtime_proton_path';
    return null;
  }

  async function handleUpdateProton(issueField: string) {
    setActiveMigrationField(issueField);
    setSuccessNotice(null);
    await scanMigrations();
  }

  async function handleApply(suggestion: MigrationSuggestion) {
    await applySingleMigration({
      profile_name: suggestion.profile_name,
      field: suggestion.field,
      new_path: suggestion.new_path,
    });
  }

  const meta = report.metadata;
  return (
    <tr className="crosshook-health-dashboard-expanded-row">
      <td colSpan={11}>
        <div className="crosshook-health-dashboard-expanded-content">
          {offlineReadinessReport ? (
            <div className="crosshook-panel" style={{ marginBottom: 12 }}>
              <h3 className="crosshook-heading-section" style={{ margin: '0 0 8px', fontSize: '1rem' }}>
                Offline readiness
              </h3>
              <OfflineReadinessPanel report={offlineReadinessReport} />
            </div>
          ) : null}
          <div className="crosshook-health-dashboard-expanded-meta">
            <span>
              <strong>Launch Method:</strong> {report.launch_method}
            </span>
            {meta?.last_success && (
              <span>
                <strong>Last Success:</strong> {formatRelativeTime(meta.last_success)}
              </span>
            )}
            {meta != null && meta.failure_count_30d > 0 && (
              <span>
                <strong>Failures (30d):</strong> {meta.failure_count_30d}
              </span>
            )}
            {meta?.is_favorite && <span>★ Favorite</span>}
            {meta?.is_community_import && <span className="crosshook-status-chip">Community</span>}
          </div>
          {report.issues.length === 0 ? (
            <p className="crosshook-muted">No issues found.</p>
          ) : (
            <ul className="crosshook-health-dashboard-issues-list">
              {report.issues.map((issue) => {
                const issueCategory = categorizeIssue(issue);
                const protonField = issueCategory === 'missing_proton' ? getProtonPathField(issue.field) : null;
                const isMigrationActive = activeMigrationField === issue.field;
                const suggestion: MigrationSuggestion | null =
                  protonField != null && scanResult != null
                    ? (scanResult.suggestions.find((s) => s.profile_name === report.name && s.field === protonField) ??
                      null)
                    : null;
                const hasNoMatch = protonField != null && scanResult != null && suggestion === null;

                return (
                  <li
                    key={`${issue.field}-${issue.path}-${issue.message}-${issue.severity}`}
                    className="crosshook-health-dashboard-issue"
                  >
                    <span className="crosshook-health-dashboard-issue__field">{issue.field}</span>
                    {issue.path && <code className="crosshook-health-dashboard-issue__path">{issue.path}</code>}
                    <span className="crosshook-health-dashboard-issue__message">{issue.message}</span>
                    {issue.remediation && (
                      <span className="crosshook-health-dashboard-issue__remediation crosshook-muted">
                        {issue.remediation}
                      </span>
                    )}
                    {issueCategory === 'missing_proton' && (
                      <div className="crosshook-health-dashboard-migration-panel">
                        {!isMigrationActive && (
                          <button
                            type="button"
                            className="crosshook-button crosshook-button--small crosshook-focus-ring crosshook-nav-target"
                            style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                            onClick={() => void handleUpdateProton(issue.field)}
                            disabled={isScanning}
                            aria-label={`Update Proton for ${report.name}`}
                          >
                            Update Proton
                          </button>
                        )}
                        {isMigrationActive && (
                          <>
                            {isScanning && <span className="crosshook-muted">Scanning Proton installations…</span>}
                            {!isScanning && suggestion && (
                              <div className="crosshook-health-dashboard-migration-inline">
                                <div className="crosshook-health-dashboard-migration-path-comparison">
                                  <code style={{ color: 'var(--crosshook-color-danger)' }}>{suggestion.old_path}</code>
                                  {' \u2192 '}
                                  <code style={{ color: 'var(--crosshook-color-success)' }}>{suggestion.new_path}</code>
                                </div>
                                <div className="crosshook-health-dashboard-migration-versions crosshook-muted">
                                  {suggestion.old_proton_name} {'\u2192'} {suggestion.new_proton_name}
                                </div>
                                {suggestion.crosses_major_version && (
                                  <div
                                    role="alert"
                                    className="crosshook-health-dashboard-migration-warning"
                                    style={{ color: 'var(--crosshook-color-warning)' }}
                                  >
                                    &#9888; Major version change — WINE prefix may need recreation
                                  </div>
                                )}
                                {!suggestion.crosses_major_version && suggestion.confidence < 0.75 && (
                                  <div
                                    role="alert"
                                    className="crosshook-health-dashboard-migration-warning"
                                    style={{ color: 'var(--crosshook-color-warning)' }}
                                  >
                                    &#9888; Different Proton family — verify compatibility
                                  </div>
                                )}
                                <div className="crosshook-health-dashboard-migration-actions">
                                  <button
                                    type="button"
                                    className="crosshook-button crosshook-button--small crosshook-focus-ring crosshook-nav-target crosshook-focus-target"
                                    style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                                    onClick={() => void handleApply(suggestion)}
                                    disabled={isApplying}
                                    aria-label={`Use ${suggestion.new_proton_name} for ${report.name}`}
                                  >
                                    {isApplying ? 'Updating\u2026' : `Use ${suggestion.new_proton_name}`}
                                  </button>
                                  <button
                                    type="button"
                                    className="crosshook-button crosshook-button--small crosshook-button--ghost crosshook-focus-ring crosshook-nav-target"
                                    style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                                    onClick={() => setActiveMigrationField(null)}
                                    aria-label="Cancel Proton update"
                                  >
                                    Cancel
                                  </button>
                                </div>
                              </div>
                            )}
                            {!isScanning && hasNoMatch && (
                              <div className="crosshook-health-dashboard-migration-no-match">
                                <span className="crosshook-muted">No matching Proton installation found.</span>{' '}
                                <button
                                  type="button"
                                  className="crosshook-button crosshook-button--small crosshook-button--ghost crosshook-focus-ring crosshook-nav-target"
                                  style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                                  onClick={() => void onFixNavigate(report.name)}
                                  aria-label={`Browse Proton path for ${report.name}`}
                                >
                                  Browse\u2026
                                </button>
                              </div>
                            )}
                            {migrationError && (
                              <div
                                role="alert"
                                className="crosshook-health-dashboard-migration-error"
                                style={{ color: 'var(--crosshook-color-danger)' }}
                              >
                                {migrationError}
                              </div>
                            )}
                            {applyResult?.outcome === 'failed' && applyResult.profile_name === report.name && (
                              <div
                                role="alert"
                                className="crosshook-health-dashboard-migration-error"
                                style={{ color: 'var(--crosshook-color-danger)' }}
                              >
                                {applyResult.error ?? 'Failed to update Proton path.'}
                              </div>
                            )}
                          </>
                        )}
                      </div>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
          {successNotice && (
            <div
              role="status"
              className="crosshook-health-dashboard-migration-success"
              style={{
                color: 'var(--crosshook-color-success)',
                display: 'flex',
                alignItems: 'center',
                gap: '8px',
                marginTop: '8px',
              }}
            >
              <span>&#10003; {successNotice}</span>
              <button
                type="button"
                className="crosshook-button crosshook-button--small crosshook-button--ghost crosshook-focus-ring crosshook-nav-target"
                style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                onClick={() => setSuccessNotice(null)}
                aria-label="Dismiss notification"
              >
                &#x2715;
              </button>
            </div>
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
