import type { OfflineReadinessReport } from '../../../types';
import type { EnrichedProfileHealthReport, HealthIssue } from '../../../types/health';
import type { VersionCorrelationStatus } from '../../../types/version';
import { CATEGORY_LABELS, type IssueCategory, type IssueCategoryCount } from './constants';

export function categorizeIssue(issue: HealthIssue): IssueCategory {
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

export function getVersionStatusColor(status: VersionCorrelationStatus | null | undefined): string {
  if (status === 'matched') return 'var(--crosshook-color-success)';
  if (status === 'game_updated' || status === 'trainer_changed' || status === 'both_changed') {
    return 'var(--crosshook-color-warning)';
  }
  return 'var(--crosshook-color-text-subtle)';
}

export function getVersionStatusLabel(status: VersionCorrelationStatus | null | undefined): string {
  switch (status) {
    case 'matched':
      return 'Matched';
    case 'game_updated':
      return 'Game Updated';
    case 'trainer_changed':
      return 'Trainer Changed';
    case 'both_changed':
      return 'Both Changed';
    case 'update_in_progress':
      return 'Updating';
    case 'untracked':
      return 'Untracked';
    case 'unknown':
      return 'Unknown';
    default:
      return 'Untracked';
  }
}

export function mergeOfflineReadinessForRow(
  report: EnrichedProfileHealthReport,
  hookReport: OfflineReadinessReport | undefined
): OfflineReadinessReport | undefined {
  const brief = report.offline_readiness;
  const offlineIssues = report.issues.filter((i) => i.field.startsWith('offline_readiness.'));
  const checks = offlineIssues.map((i) => ({
    ...i,
    field: i.field.replace(/^offline_readiness\./, ''),
  }));
  if (brief) {
    return {
      profile_name: brief.profile_name,
      score: brief.score,
      readiness_state: brief.readiness_state,
      trainer_type: brief.trainer_type,
      blocking_reasons: [...brief.blocking_reasons],
      checked_at: brief.checked_at,
      checks,
    };
  }
  if (hookReport) {
    return hookReport;
  }
  if (checks.length > 0) {
    return {
      profile_name: report.name,
      score: 0,
      readiness_state: 'unknown',
      trainer_type: 'unknown',
      blocking_reasons: [],
      checked_at: report.checked_at,
      checks,
    };
  }
  return undefined;
}

export function offlineSortScore(
  report: EnrichedProfileHealthReport,
  hookReport: OfflineReadinessReport | undefined
): number {
  const merged = mergeOfflineReadinessForRow(report, hookReport);
  if (merged && merged.score !== undefined && !Number.isNaN(merged.score)) {
    return merged.score;
  }
  return -1;
}

export function buildCategoryCounts(profiles: EnrichedProfileHealthReport[]): {
  categoryCounts: IssueCategoryCount[];
  totalRawIssues: number;
} {
  const counts = new Map<IssueCategory, number>();
  let totalRaw = 0;
  for (const report of profiles) {
    totalRaw += report.issues.length;
    for (const issue of report.issues) {
      if (issue.severity === 'info') {
        continue;
      }
      const cat = categorizeIssue(issue);
      counts.set(cat, (counts.get(cat) ?? 0) + 1);
    }
  }
  const result: IssueCategoryCount[] = [];
  for (const [category, count] of counts.entries()) {
    result.push({ category, label: CATEGORY_LABELS[category], count });
  }
  result.sort((a, b) => b.count - a.count);
  return { categoryCounts: result, totalRawIssues: totalRaw };
}
